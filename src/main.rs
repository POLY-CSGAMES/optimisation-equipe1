use anyhow::Result;
use chrono::Datelike;
use chrono::NaiveDateTime;
use futures::future::join_all;
use serde::{Deserialize, Serialize};
use std::time::Instant;
use time::macros::datetime;
use yahoo_finance_api::{Quote, YahooConnector};
#[derive(Debug, Clone, Copy)]
struct RawTransaction {
    stock: usize,
    date: NaiveDateTime,
}

#[derive(Debug, Deserialize, Serialize, Default)]
struct Transaction {
    date: String,
    action: String,
    ticker: String,
}

impl Transaction {
    fn new<T: Into<String>, U: Into<String>>(date: NaiveDateTime, action: U, ticker: T) -> Self {
        let date = format!("{}-{:02}-{:02}", date.year(), date.month(), date.day());
        Self {
            date,
            action: action.into(),
            ticker: ticker.into(),
        }
    }
}

struct DayPrice {
    day: NaiveDateTime,
    value: f64,
}

impl From<&Quote> for DayPrice {
    fn from(quote: &Quote) -> Self {
        let date = NaiveDateTime::from_timestamp_opt(quote.timestamp as i64, 0).unwrap();
        Self {
            day: date,
            value: quote.open,
        }
    }
}

fn find_max_n_day(
    quotes_entreprise: &Vec<Vec<DayPrice>>,
    tickers: &[&str],
) -> (f64, Vec<Transaction>) {
    let number_of_day = quotes_entreprise[0].len();
    let mut max = vec![(1.0f64, vec![]); number_of_day];
    for sell_day in 1..number_of_day {
        for buy_day in 0..sell_day {
            for (stock, quotes) in quotes_entreprise.iter().enumerate() {
                let default = (1.0f64, vec![]);
                let (previous_max, previous_transaction) = max.get(buy_day - 1).unwrap_or(&default);
                let new_ratio = previous_max * quotes[sell_day].value / quotes[buy_day].value;
                if new_ratio > max[sell_day].0 {
                    let mut transaction = previous_transaction.clone();
                    transaction.push(RawTransaction {
                        stock,
                        date: quotes[buy_day].day,
                    });
                    transaction.push(RawTransaction {
                        stock,
                        date: quotes[sell_day].day,
                    });
                    for day in sell_day..number_of_day {
                        max[day] = (new_ratio, transaction.clone());
                    }
                }
            }
        }
    }
    let (max_value, max_raw_transactions) = &max[number_of_day - 1];
    let mut transactions = vec![];
    let mut iter = max_raw_transactions.iter();
    while let (Some(buy), Some(sell)) = (iter.next(), iter.next()) {
        transactions.push(Transaction::new(buy.date, "BUY", tickers[buy.stock]));
        transactions.push(Transaction::new(sell.date, "SELL", tickers[sell.stock]));
    }
    (*max_value, transactions)
}

#[tokio::main]
async fn main() -> Result<()> {
    let api_start = Instant::now();
    let provider = YahooConnector::new();

    //let tickers = ["GOOG", "AMZN", "META", "MSFT", "AAPL"];
    let tickers = ["AAL", "DAL", "UAL", "LUV", "HA"];
    let start = datetime!(2023-1-1 0:00:00.00 UTC);
    let end = datetime!(2023-2-1 23:59:59.99 UTC);

    // returns historic quotes with daily interval
    let quotes_enterprise: Vec<Vec<DayPrice>> = join_all(
        tickers
            .iter()
            .map(|ticker| provider.get_quote_history(ticker, start, end)),
    )
    .await
    .into_iter()
    .filter_map(|x| x.ok()?.quotes().ok())
    .map(|quotes| {
        quotes
            .iter()
            .map(|quote| quote.into())
            .collect::<Vec<DayPrice>>()
    })
    .collect();

    println!(
        "Données obtenues de Yahoo finance en {} ms",
        api_start.elapsed().as_millis()
    );

    let computation_start = Instant::now();
    let (value, transactions) = find_max_n_day(&quotes_enterprise, &tickers);
    println!(
        "La valeur maximale est {:.2} calculée en {} µs.",
        1000000.0 * value,
        computation_start.elapsed().as_micros()
    );

    println!("{}", serde_json::to_string_pretty(&transactions)?);
    Ok(())
}

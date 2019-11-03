extern crate env_logger;
extern crate futures;
extern crate hyper;
#[macro_use]
extern crate lazy_static;
extern crate log;

use std::str;
use std::sync::{Arc, RwLock};
use std::vec::Vec;
use log::{info
};

use futures::{future, Stream};
use futures::future::Future;
use hyper::{Body, Client, Method, Request, Response};
use hyper::client::HttpConnector;
use hyper::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::Result;

#[derive(Serialize, Deserialize, Debug, Copy, Clone)] // Deriving traits we need
pub struct Trade {
    amount: i32,
}

type GenericError = Box<dyn std::error::Error + Send + Sync>;
type ResponseFuture = Box<dyn Future<Item = Response<Body>, Error = GenericError> + Send>;
type Trades = Arc<RwLock<Vec<Trade>>>;

// Back this "exchange" with an in-memory store
lazy_static! {
    pub static ref TRADES: Trades = Arc::new(RwLock::new(Vec::new()));
}

pub fn add_trade(t: Trade) {
    info!("adding trade {:?}", t);
    let trades = Arc::clone(&TRADES);
    let mut lock = trades.write().unwrap(); // take the write lock
    lock.push(t);
}

pub fn get_trades() -> Vec<Trade> {
    let trades = Arc::clone(&TRADES);
    let lock = trades.read().unwrap(); // take a read lock
    (*lock).clone()
}

#[cfg(test)]
mod tests {
    use crate::{add_trade, Trade, get_trades, Trades};

    #[test]
    fn test_add_trade() {
        add_trade(Trade{amount: 10});
        let trades = get_trades();
        assert_eq!(1, trades.len());
    }
}

pub fn router(req: Request<Body>, _client: &Client<HttpConnector>) -> ResponseFuture {
    match (req.method(), req.uri().path()) {
        (&Method::POST, "/") => {
            Box::new(
                req.into_body()
                    .concat2()
                    .from_err()
                    .and_then(|whole_body| {
                        let str_body = String::from_utf8(whole_body.to_vec()).unwrap();
                        let res: Result<Trade> = serde_json::from_str(&str_body);
                        match res {
                            Ok(t) => {
                                add_trade(t);
                                Box::new(future::ok(
                                    Response::builder()
                                        .status(StatusCode::OK)
                                        .body(Body::empty())
                                        .unwrap(),
                                ))
                            },
                            Err(_) => {
                                Box::new(future::ok(
                                    Response::builder()
                                        .status(StatusCode::BAD_REQUEST)
                                        .body(Body::empty())
                                        .unwrap(),
                                ))
                            }
                        }

                    }),
            )
        },
        (&Method::GET, "/") => {
            //let trades = Arc::clone(&TRADES);
           // let lock = trades.read().unwrap();
            let payload = serde_json::to_string(&get_trades()).unwrap();
            Box::new(future::ok(
                Response::builder()
                    .status(StatusCode::OK)
                    .body(Body::from(payload))
                    .unwrap(),
            ))
        },
        _ => {
            Box::new(
                future::ok(
                    Response::builder()
                        .status(StatusCode::METHOD_NOT_ALLOWED)
                        .body(Body::empty())
                        .unwrap(),
                ))
        }
    }
}
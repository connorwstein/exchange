extern crate env_logger;
extern crate futures;
extern crate hyper;
#[macro_use]
extern crate lazy_static;
extern crate log;

use std::str;
use std::sync::{Arc, RwLock};
use std::string::String;
use std::vec::Vec;
use log::{info, warn};

use futures::{future, Stream};
use futures::future::Future;
use hyper::{Body, Client, Method, Request, Response};
use hyper::client::HttpConnector;
use hyper::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::Result;


#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
enum Side {
    Buy,
    Sell
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
enum Symbol {
    IBM,
    AAPL,
    AMZN
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
enum OrderType {
    MKT,
    // TODO: limit, fok, etc.
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone)] // Deriving traits we need
pub struct Order {
    amount: u32,
    symbol: Symbol,
    side: Side,
    order_type: OrderType,
}


type GenericError = Box<dyn std::error::Error + Send + Sync>;
type ResponseFuture = Box<dyn Future<Item = Response<Body>, Error = GenericError> + Send>;
type OrderBook = Arc<RwLock<Vec<Order>>>;

// Back this "exchange" with an in-memory store
lazy_static! {
    pub static ref ORDER_BOOK: OrderBook = Arc::new(RwLock::new(Vec::new()));
}

pub fn add_order(t: Order) {
    info!("adding Order {:?}", t);
    let ob = Arc::clone(&ORDER_BOOK);
    let mut lock = ob.write().unwrap(); // take the write lock
    lock.push(t);
}

pub fn get_order_book() -> Vec<Order> {
    let ob = Arc::clone(&ORDER_BOOK);
    let lock = ob.read().unwrap(); // take a read lock
    (*lock).clone()
}


// Walk through all the available OrderBook and see if we
//pub fn fill_market_order(t: Order) -> Result<> {
//
//}

#[cfg(test)]
mod tests {
    use crate::{Order, add_order, get_order_book};
    use crate::Side::Buy;
    use crate::Symbol::AAPL;
    use crate::OrderType::MKT;

    #[test]
    fn test_Order() {
        add_order(Order{amount: 10, symbol: AAPL, side: Buy, order_type: MKT});
        let ob = get_order_book();
        assert_eq!(1, ob.len());
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
                        let res: Result<Order> = serde_json::from_str(&str_body);
                        match res {
                            Ok(v) => {
                                add_order(v);
                                Box::new(future::ok(
                                    Response::builder()
                                        .status(StatusCode::OK)
                                        .body(Body::empty())
                                        .unwrap(),
                                ))
                            },
                            Err(e) => {
                                warn!("bad request {}", e);
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
            let payload = serde_json::to_string(&get_order_book()).unwrap();
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
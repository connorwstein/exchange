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
use serde_json::{Result, Error};
use std::ptr::null;

use std::error::Error;

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

#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
pub struct OpenMarketOrder {
    id: u32,
    amount: u32,
    symbol: Symbol,
    side: Side,
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
pub struct FilledMarketOrder {
    open_order: OpenMarketOrder,
    executed_price: u32,
}


type GenericError = Box<dyn std::error::Error + Send + Sync>;
type ResponseFuture = Box<dyn Future<Item = Response<Body>, Error = GenericError> + Send>;
type OrderBook = Arc<RwLock<Vec<OpenMarketOrder>>>;

// Back this "exchange" with an in-memory store
lazy_static! {
    pub static ref ORDER_BOOK: OrderBook = Arc::new(RwLock::new(Vec::new()));
    pub static ref FILLED_ORDERS: FilledOrders = Arc::new(RwLock::new(Vec::new()));
}

pub fn add_order(t: OpenMarketOrder) {
    info!("adding Order {:?}", t);
    let ob = Arc::clone(&ORDER_BOOK);
    let mut lock = ob.write().unwrap(); // take the write lock
    lock.push(t);
}

pub fn get_order_book() -> Vec<OpenMarketOrder> {
    let ob = Arc::clone(&ORDER_BOOK);
    let lock = ob.read().unwrap(); // take a read lock
    (*lock).clone()
}

pub fn remove_order(t: OpenMarketOrder) {
    let ob = Arc::clone(&ORDER_BOOK);
    let mut lock = ob.write().unwrap(); // take a read lock
    let mut index: Option<usize> = None;
    for (i, x) in lock.iter().enumerate() {
        println!("{:?}", x);
        if x.id == t.id {
            index = Some(i as usize);
            println!("found match");
            break;
        }
    }
    match index {
        Some(index) => {
            println!("removing item at {}, len {}", index, lock.len());
            lock.remove(index);
        },
        None => {
            println!("no such order");
        }
    }
}

// Walk through all the available OrderBook and see if we can fill this
//pub fn fill_market_order(t: OpenMarketOrder) -> Result<FilledMarketOrder> {
//    let ob = Arc::clone(&ORDER_BOOK);
//    let mut lock = ob.write().unwrap(); // take a write lock so we can remove it if it fills
//
//    let mut price: Option<u32> = None;
//    for (i, open_order) in lock.iter().enumerate() {
//        match open_order.side {
//            Side::Buy => {
//                if t.side == Side::Sell {
//
//                }
//            }
//            Side::Sell => {
//                if t.side == Side::Buy {
//
//                }
//            }
//        }
//    }
//    match price {
//        Some(price) => {
//            Ok(FilledMarketOrder{
//                executed_price: price,
//                open_order: t,
//            })
//        },
//        None => {
//            error!("cant fill order");
//        }
//    }
//}


pub fn save_filled_market_order(t: FilledMarketOrder) {
    let ob = Arc::clone(&FILLED_ORDERS);
    let mut lock = ob.write().unwrap();
    lock.push(t);
}

#[cfg(test)]
mod tests {
    use crate::{OpenMarketOrder, add_order, get_order_book, remove_order};
    use crate::Side::Buy;
    use crate::Symbol::AAPL;
    use crate::OrderType::MKT;

    #[test]
    fn test_order() {
        let order = OpenMarketOrder{
            id: 1,
            amount: 10,
            symbol: AAPL,
            side: Buy,
        };
        add_order(order);
        let ob = get_order_book();
        assert_eq!(1, ob.len());
        remove_order(order);
        let ob = get_order_book();
        assert_eq!(0, ob.len());
    }

//    #[test]
//    fn test_match_order() {
//        let order = OpenMarketOrder{
//            id: 1,
//            amount: 10,
//            symbol: AAPL,
//            side: Buy,
//        };
//        add_order();
//        let ob = get_order_book();
//        assert_eq!(1, ob.len());
//        empty_order_book();
//    }
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
                        let res: Result<OpenMarketOrder
                        > = serde_json::from_str(&str_body);
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
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

use std::error;

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
    LIMIT,
    // TODO: limit, fok, etc.
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
pub struct OpenLimitOrder {
    id: u32,
    amount: u32,
    symbol: Symbol,
    price: u32,
    side: Side,
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
pub struct FilledLimitOrder {
    open_order: OpenLimitOrder,
    executed_price: u32,
}


type GenericError = Box<dyn std::error::Error + Send + Sync>;
type ResponseFuture = Box<dyn Future<Item = Response<Body>, Error = GenericError> + Send>;

use std::collections::{HashMap, VecDeque};
// Let's have separate order books per symbol as they are entirely independent and can
// be handled concurrently.
// A map can look up the respective order book for a given symbol.
// The order book needs to be organized by price level first and then by arrival time.
// Let's use a map[price level]fifo queue of orders
type OrderBookEntries = HashMap<u32, VecDeque<OpenLimitOrder>>;
type OrderBook = Arc<RwLock<OrderBookEntries>>;

// Back this "exchange" with an in-memory store
// We may want to look at redis?
lazy_static! {
    pub static ref ORDER_BOOK: OrderBook = Arc::new(RwLock::new(HashMap::new()));
}

pub fn add_order(t: OpenLimitOrder) {
    info!("adding Order {:?}", t);
    let ob = Arc::clone(&ORDER_BOOK);
    let mut lock = ob.write().unwrap(); // take the write lock
    // If we find an entry at that price point, add it to the queue
    // Otherwise create a queue at that price point.
    let price_point = lock.get_mut(&t.price);
    match price_point {
        Some(price_point) => {
            price_point.push_back(t);
        },
        None => {
            let mut orders: VecDeque<OpenLimitOrder> = VecDeque::new();
            orders.push_back(t);
            lock.insert(t.price, orders);
        }
    };
}

pub fn get_order_book() -> OrderBookEntries {
    let ob = Arc::clone(&ORDER_BOOK);
    let lock = ob.read().unwrap(); // take a read lock
    (*lock).clone()
}

pub fn index_of_order_in_queue(t: OpenLimitOrder, orders: &mut VecDeque<OpenLimitOrder>) -> Option<usize> {
    for (i, x) in orders.iter().enumerate() {
        if x.id == t.id {
            println!("found match");
            return Some(i as usize);
        }
    }
    return None
}

pub fn remove_order(t: OpenLimitOrder) {
    let ob = Arc::clone(&ORDER_BOOK);
    let mut lock = ob.write().unwrap(); // take a write lock
    let order_queue = lock.get_mut(&t.price);
    match order_queue {
        Some(order_queue) => {
            let index = index_of_order_in_queue(t, order_queue);
            match index {
                Some(index) => {
                    println!("removing item at {}, len {}", index, order_queue.len());
                    order_queue.remove(index);
                },
                None => {
                    println!("no such order");
                }
            };
        },
        None => {
            println!("no such order");
        }
    };
}

#[cfg(test)]
mod tests {
    use crate::{OpenLimitOrder, add_order, get_order_book, remove_order};
    use crate::Side::Buy;
    use crate::Symbol::AAPL;
    use crate::OrderType::LIMIT;

    #[test]
    fn test_order() {
        let order = OpenLimitOrder{
            id: 1,
            amount: 10,
            symbol: AAPL,
            side: Buy,
            price: 1,
        };
        add_order(order);
        let ob = get_order_book();
        let hm = ob.get(&order.price).unwrap();
        assert_eq!(hm[0].amount, order.amount);
        remove_order(order);
        let ob = get_order_book();
        let hm = ob.get(&order.price).unwrap();
        assert_eq!(hm.len(), 0);
    }

//    #[test]
//    fn test_match_order() {
//        let order = OpenLimitOrder{
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
                        let res: Result<OpenLimitOrder
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
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
use std::collections::VecDeque;

use std::error;

#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq)]
pub enum Side {
    Buy,
    Sell
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
enum Symbol {
    AAPL,
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

type OrderBook = Arc<RwLock<Vec<VecDeque<OpenLimitOrder>>>>;

// Back this "exchange" with an in-memory store
// We may want to look at redis?
lazy_static! {
    pub static ref BUY_ORDER_BOOK: OrderBook = Arc::new(RwLock::new(Vec::new()));
    pub static ref SELL_ORDER_BOOK: OrderBook = Arc::new(RwLock::new(Vec::new()));
}

pub fn add_order(t: OpenLimitOrder) {
    info!("adding Order {:?}", t);
    let mut maybe_ob = None;
    if t.side == Side::Buy {
        maybe_ob = Some(Arc::clone(&BUY_ORDER_BOOK));

    } else {
        maybe_ob = Some(Arc::clone(&SELL_ORDER_BOOK));
    }
    let ob = maybe_ob.unwrap();
    let mut order_book = ob.write().unwrap();

    // If we find an entry at that price point, add it to the queue
    // Otherwise create a queue at that price point.
    // TODO: binary search to the price point?
    let mut queue_index = None; // index for the queue at that price
    let mut insert_index = None; // index to insert new queue, should that price not exist (maintain sort)
    for (index, order_queue) in order_book.iter().enumerate() {
        if order_queue.front().unwrap().price == t.price {
            queue_index = Some(index);
        } else if order_queue.front().unwrap().price < t.price && t.side == Side::Buy {
            insert_index = Some(index);
        } else if order_queue.front().unwrap().price > t.price && t.side == Side::Sell {
            insert_index = Some(index);
        }
    }
    match queue_index {
        Some(queue_index) => {
            order_book[queue_index].push_back(t);
        },
        None => {
            // We'll need a new queue
            let mut orders: VecDeque<OpenLimitOrder> = VecDeque::new();
            orders.push_back(t);
            match insert_index {
                Some(insert_index) => {
                    // We know the spot to put this new queue
                    order_book.insert(insert_index, orders);
                },
                None => {
                    // Order book must be empty, just push the queue into the first spot
                    order_book.push(orders);
                }
            }
        }
    };
}

pub fn get_order_book(side: Side) -> Vec<VecDeque<OpenLimitOrder>> {
    if side == Side::Buy {
        let ob = Arc::clone(&BUY_ORDER_BOOK);
        let lock = ob.read().unwrap(); // take a read lock
        (*lock).clone()
    } else {
        let ob = Arc::clone(&SELL_ORDER_BOOK);
        let lock = ob.read().unwrap(); // take a read lock
        (*lock).clone()
    }
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
//
//pub fn remove_order(t: OpenLimitOrder) {
//    let ob = Arc::clone(&ORDER_BOOK);
//    let mut lock = ob.write().unwrap(); // take a write lock
//    let order_queue = lock.get_mut(&t.price);
//    match order_queue {
//        Some(order_queue) => {
//            let index = index_of_order_in_queue(t, order_queue);
//            match index {
//                Some(index) => {
//                    println!("removing item at {}, len {}", index, order_queue.len());
//                    order_queue.remove(index);
//                },
//                None => {
//                    println!("no such order");
//                }
//            };
//        },
//        None => {
//            println!("no such order");
//        }
//    };
//}

// If there's something at the exact price we want, drain the queue as much as we can
// If the queue runs out, drop to the next allowed price level.
// If we drain the whole order book, then return an error saying it can't be filled and leave the order
// Returns the set of orders on the other side which satisfy the order
//pub fn fill_order(t: OpenLimitOrder) -> Vec<OpenLimitOrder> {
//    let ob = Arc::clone(&ORDER_BOOK);
//    let mut lock = ob.write().unwrap(); // take a write lock
//    let order_queue = lock.get_mut(&t.price);
//    match order_queue {
//        Some(order_queue) => {
//
//        },
//        None => {
//            // drop down to the next
//        }
//    }
//
//}

#[cfg(test)]
mod tests {
    use crate::{OpenLimitOrder, add_order, get_order_book, VecDeque};
    use crate::Side::{Buy, Sell};
    use crate::Symbol::AAPL;

    fn assert_order(expected: &OpenLimitOrder, actual: &OpenLimitOrder) {
        assert_eq!(expected.amount, actual.amount);
        assert_eq!(expected.price, actual.price);
        assert_eq!(expected.side, actual.side);
    }

    fn assert_order_queue(expected: &VecDeque<OpenLimitOrder>, actual: &VecDeque<OpenLimitOrder>) {
        assert_eq!(expected.len(), actual.len());
        for i in 0..expected.len() {
            assert_order(expected.get(i).unwrap(), actual.get(i).unwrap());
        }
    }

    fn assert_order_book(expected: Vec<VecDeque<OpenLimitOrder>>, actual: Vec<VecDeque<OpenLimitOrder>>) {
        assert_eq!(expected.len(), actual.len());
        for i in 0..expected.len() {
            assert_order_queue(&expected[i], &actual[i])
        }
    }

    #[test]
    fn test_buy_order_book() {
        let bid = OpenLimitOrder {
            id: 1,
            amount: 10,
            symbol: AAPL,
            side: Buy,
            price: 5,
        };
        add_order(bid);
        assert_order_book(vec![VecDeque::from(vec![bid])], get_order_book(Buy));
        add_order(bid);
        assert_order_book(vec![VecDeque::from(vec![bid, bid])], get_order_book(Buy));
        let higher_bid = OpenLimitOrder {
            id: 1,
            amount: 9,
            symbol: AAPL,
            side: Buy,
            price: 6,
        };
        add_order(higher_bid);
        add_order(higher_bid);
        assert_order_book(vec![
                               VecDeque::from(vec![higher_bid, higher_bid]),
                               VecDeque::from(vec![bid, bid])],
                          get_order_book(Buy));
        let lower_bid = OpenLimitOrder{
            id: 1,
            amount: 9,
            symbol: AAPL,
            side: Buy,
            price: 4,
        };
        add_order(lower_bid);
        add_order(lower_bid);
        assert_order_book(vec![
            VecDeque::from(vec![higher_bid, higher_bid]),
            VecDeque::from(vec![bid, bid]),
            VecDeque::from(vec![lower_bid, lower_bid])],
                          get_order_book(Buy));
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
            let payload = serde_json::to_string(&get_order_book(Side::Buy)).unwrap();
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
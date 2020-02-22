extern crate env_logger;
extern crate futures;
extern crate hyper;
#[macro_use]
extern crate lazy_static;
extern crate log;

use hyper::client::HttpConnector;
use hyper::rt::Future;
use hyper::service::service_fn;
use hyper::StatusCode;
use hyper::{Body, Client, Method, Request, Response};

use std::collections::VecDeque;
use std::string::String;
use std::sync::{Arc, RwLock, RwLockWriteGuard};
use std::vec::Vec;

use futures::{future, Stream};
use log::info;
use serde_json::Result;
use std::collections::HashMap;

mod order_book;

type OrderBookRef = Arc<RwLock<order_book::OrderBook>>;
lazy_static! {
    static ref BUY: HashMap<order_book::Symbol, OrderBookRef> = {
        let mut buy = HashMap::new();
        buy.insert(
            order_book::Symbol::AAPL,
            Arc::new(RwLock::new(order_book::OrderBook::new(
                order_book::Side::Buy,
            ))),
        );
        buy.insert(
            order_book::Symbol::MSFT,
            Arc::new(RwLock::new(order_book::OrderBook::new(
                order_book::Side::Buy,
            ))),
        );
        buy.insert(
            order_book::Symbol::AMZN,
            Arc::new(RwLock::new(order_book::OrderBook::new(
                order_book::Side::Buy,
            ))),
        );

        buy
    };
    static ref SELL: HashMap<order_book::Symbol, OrderBookRef> = {
        let mut sell = HashMap::new();

        sell.insert(
            order_book::Symbol::AAPL,
            Arc::new(RwLock::new(order_book::OrderBook::new(
                order_book::Side::Sell,
            ))),
        );
        sell.insert(
            order_book::Symbol::MSFT,
            Arc::new(RwLock::new(order_book::OrderBook::new(
                order_book::Side::Sell,
            ))),
        );
        sell.insert(
            order_book::Symbol::AMZN,
            Arc::new(RwLock::new(order_book::OrderBook::new(
                order_book::Side::Sell,
            ))),
        );
        sell
    };
}

type GenericError = Box<dyn std::error::Error + Send + Sync>;
type ResponseFuture = Box<dyn Future<Item = Response<Body>, Error = GenericError> + Send>;

pub fn router(req: Request<Body>, _client: &Client<HttpConnector>) -> ResponseFuture {
    match (req.method(), req.uri().path()) {
        (&Method::POST, "/order") => {
            Box::new(req.into_body().concat2().from_err().and_then(|whole_body| {
                let str_body = String::from_utf8(whole_body.to_vec()).unwrap();
                info!("order requested {:?}", str_body);
                let order_request: Result<order_book::OpenLimitOrder> =
                    serde_json::from_str(&str_body);

                match order_request {
                    Ok(order_request) => {
                        let mut book: RwLockWriteGuard<order_book::OrderBook>;
                        let mut opposing_book: RwLockWriteGuard<order_book::OrderBook>;
                        if order_request.side == order_book::Side::Buy {
                            book = BUY.get(&order_request.symbol).unwrap().write().unwrap();
                            opposing_book =
                                SELL.get(&order_request.symbol).unwrap().write().unwrap();
                        } else {
                            book = SELL.get(&order_request.symbol).unwrap().write().unwrap();
                            opposing_book =
                                BUY.get(&order_request.symbol).unwrap().write().unwrap();
                        }

                        // If we can instantly match the order, then lets match it.
                        // If not, just leave it on the order book.
                        // Probably want to publish the matched order somewhere.
                        match opposing_book.fill_order(order_request) {
                            Ok(fr) => Box::new(future::ok(
                                Response::builder()
                                    .status(StatusCode::OK)
                                    .body(Body::from(serde_json::to_string(&fr).unwrap()))
                                    .unwrap(),
                            )),
                            Err(e) => {
                                info!("unable to fill order, leaving on the book");
                                match e {
                                    order_book::ERR_CANT_FILL_PRICE
                                    | order_book::ERR_CANT_FILL_SIZE => {
                                        match book.add_order(order_request) {
                                            Ok(order) => Box::new(future::ok(
                                                Response::builder()
                                                    .status(StatusCode::OK)
                                                    .body(Body::from(
                                                        serde_json::to_string(&order).unwrap(),
                                                    ))
                                                    .unwrap(),
                                            )),
                                            Err(order) => Box::new(future::ok(
                                                Response::builder()
                                                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                                                    .body(Body::from(
                                                        serde_json::to_string(&order).unwrap(),
                                                    ))
                                                    .unwrap(),
                                            )),
                                        }
                                    }
                                    e => Box::new(future::ok(
                                        Response::builder()
                                            .status(StatusCode::INTERNAL_SERVER_ERROR)
                                            .body(Body::from(serde_json::to_string(e).unwrap()))
                                            .unwrap(),
                                    )),
                                }
                            }
                        }
                    }
                    Err(order_request) => Box::new(future::ok(
                        Response::builder()
                            .status(StatusCode::BAD_REQUEST)
                            .body(Body::empty())
                            .unwrap(),
                    )),
                }
            }))
        }
        (&Method::GET, "/sells") => {
            let payload = String::new();
            let mut to_serialize: HashMap<
                order_book::Symbol,
                Vec<VecDeque<order_book::OpenLimitOrder>>,
            > = HashMap::new();
            for (symbol, book) in SELL.iter() {
                to_serialize.insert(*symbol, book.read().unwrap().get_book());
            }
            Box::new(future::ok(
                Response::builder()
                    .status(StatusCode::OK)
                    .body(Body::from(serde_json::to_string(&to_serialize).unwrap()))
                    .unwrap(),
            ))
        }
        (&Method::GET, "/buys") => {
            let payload = String::new();
            let mut to_serialize: HashMap<
                order_book::Symbol,
                Vec<VecDeque<order_book::OpenLimitOrder>>,
            > = HashMap::new();
            for (symbol, book) in BUY.iter() {
                to_serialize.insert(*symbol, book.read().unwrap().get_book());
            }
            Box::new(future::ok(
                Response::builder()
                    .status(StatusCode::OK)
                    .body(Body::from(serde_json::to_string(&to_serialize).unwrap()))
                    .unwrap(),
            ))
        }
        _ => Box::new(future::ok(
            Response::builder()
                .status(StatusCode::METHOD_NOT_ALLOWED)
                .body(Body::empty())
                .unwrap(),
        )),
    }
}

fn main() {
    env_logger::init();
    let address = "127.0.0.1:3000".parse().unwrap();

    let client = Client::new();

    let new_service = move || {
        // Move a clone of Client into the service_fn
        let client = client.clone();
        service_fn(move |req| router(req, &client))
    };
    let server = hyper::server::Server::bind(&address).serve(new_service);

    hyper::rt::run(server.map_err(|e| {
        eprintln!("server error: {}", e);
    }));
}

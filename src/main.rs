extern crate env_logger;
extern crate futures;
extern crate hyper;
#[macro_use]
extern crate lazy_static;
extern crate log;

use hyper::rt::Future;
use hyper::service::service_fn;
use hyper::{Body, Client, Method, Request, Response};
use hyper::client::HttpConnector;
use hyper::StatusCode;

use std::collections::VecDeque;
use std::error;
use std::ptr::null;
use std::str;
use std::string::String;
use std::sync::{Arc, RwLock};
use std::vec::Vec;

use futures::{future, Stream};
//use futures::future::Future;

use log::{info, warn};
use serde::{Deserialize, Serialize};
use serde_json::{Error, Result};

mod order_book;

static mut buy_ob: Option<order_book::OrderBookV2> = None;
static mut sell_ob: Option<order_book::OrderBookV2> = None;

type GenericError = Box<dyn std::error::Error + Send + Sync>;
type ResponseFuture = Box<dyn Future<Item = Response<Body>, Error = GenericError> + Send>;

pub fn router(req: Request<Body>, _client: &Client<HttpConnector>) -> ResponseFuture {
    match (req.method(), req.uri().path()) {
        (&Method::POST, "/") => {
            Box::new(
                req.into_body()
                    .concat2()
                    .from_err()
                    .and_then(|whole_body| {
                        let str_body = String::from_utf8(whole_body.to_vec()).unwrap();
                        let res: Result<order_book::OpenLimitOrder> = serde_json::from_str(&str_body);
                        match res {
                            Ok(v) => {
                                //add_order(v);
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
            //let payload = serde_json::to_string(&get_order_book(Side::Buy)).unwrap();
            let payload = serde_json::to_string("test").unwrap();
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

fn main() {
    env_logger::init();
    let address = "127.0.0.1:3000".parse().unwrap();

    let client = Client::new();
    unsafe {
        buy_ob = Some(order_book::OrderBookV2::new(order_book::Side::Buy));
        sell_ob = Some(order_book::OrderBookV2::new(order_book::Side::Sell));
    }

    let new_service = move || {
        // Move a clone of Client into the service_fn
        let client = client.clone();
        service_fn(move |req| router(req, &client))
    };
    let server = hyper::server::Server::bind(&address)
        .serve(new_service);

    hyper::rt::run(server.map_err(|e| {
        eprintln!("server error: {}", e);
    }));
}
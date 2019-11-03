extern crate env_logger;
extern crate futures;
extern crate hyper;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;

use core::fmt;
use std::collections::HashMap;
use std::fmt::Error;
use std::slice::SliceIndex;
use std::str;
use std::sync::{Arc, Mutex, MutexGuard, RwLock};
use std::vec::Vec;

use futures::{future, Stream};
use futures::future::Future;
use hyper::{Body, Chunk, Client, Method, Request, Response, Version};
use hyper::client::HttpConnector;
use hyper::service::{service_fn, service_fn_ok};
use hyper::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::Result;

#[derive(Serialize, Deserialize, Debug)]
pub struct Trade {
    amount: i32,
}

type GenericError = Box<dyn std::error::Error + Send + Sync>;
type ResponseFuture = Box<Future<Item = Response<Body>, Error = GenericError> + Send>;
type Trades = Arc<RwLock<Vec<Trade>>>;

lazy_static! {
    pub static ref TRADES: Trades = Arc::new(RwLock::new(Vec::new()));
}

fn add_trade(t: Trade) {
    let trades = Arc::clone(&TRADES);
    let mut lock = trades.write().unwrap();
    lock.push(t);
}

fn router(req: Request<Body>, _client: &Client<HttpConnector>) -> ResponseFuture {
    match (req.method(), req.uri().path()) {
        (&Method::POST, "/") => {
            Box::new(
                req.into_body()
                    .concat2() // concatenate all the chunks in the body
                    .from_err() // like try! for Result, but for Futures
                    .and_then(|whole_body| {
                        let str_body = String::from_utf8(whole_body.to_vec()).unwrap();
                        let res: Result<Trade> = serde_json::from_str(&str_body);
                        match res {
                            Ok(t) => {
                                println!("{:?}", t);
                                add_trade(t);
                            },
                            Err(t) => {
                                println!("invalid trade")
                            }
                            _ => {}
                        };
                        Box::new(future::ok(
                            Response::builder()
                                .status(StatusCode::OK)
                                .body(Body::from(""))
                                .unwrap(),
                        ))
                    }),
            )
        },
        (&Method::GET, "/") => {
            let trades = Arc::clone(&TRADES);
            let lock = trades.read().unwrap();
            let payload = serde_json::to_string(&*lock).unwrap();
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
    let  trades: Arc<Mutex<Vec<Trade>>> = Arc::new(Mutex::new(Vec::new()));

    let client = Client::new();

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
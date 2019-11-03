use hyper::rt::Future;
use hyper::service::service_fn;
use exchange::router;
use hyper::Client;

fn main() {
    env_logger::init();
    let address = "127.0.0.1:3000".parse().unwrap();

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
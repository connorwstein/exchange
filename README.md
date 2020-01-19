A first project in Rust - in-memory order book.

# Some helpful background reading:
https://www.chrisstucchio.com/blog/2012/hft_apology.html

# Design
- According to [https://www.chrisstucchio.com/blog/2012/hft_apology.html] the order book needs to be organized by price level first and then by arrival time. This suggests a sorted vector of queues per price as the
fundamental data structure.
- We can have a separate order book per symbol as they are entirely independent and can
be handled concurrently. A map can look up the respective order book for a given symbol.

# Example usage
```
RUST_LOG=debug cargo run


curl -H "Content-Type: application/json"
    -d '{"price": 3,
        "side": "Buy",
        "amount": 10,
        "symbol": "AAPL"}' localhost:3000/order
```

Run unit tests
```
RUST_BACKTRACE=1 cargo test -- --nocapture
```


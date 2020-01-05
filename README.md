A first project in Rust - in-memory order book.

Some helpful background reading:
https://www.chrisstucchio.com/blog/2012/hft_apology.html

Design
- Let's have separate order books per symbol as they are entirely independent and can
be handled concurrently.
- A map can look up the respective order book for a given symbol.
- The order book needs to be organized by price level first and then by arrival time. This suggests a sorted vector of queues per price as the
fundamental data structure.

To test:
RUST_BACKTRACE=1 cargo test -- --nocapture

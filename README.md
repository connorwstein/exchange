A first project to explore Rust: an in-memory order book.

### Design
According to [https://www.chrisstucchio.com/blog/2012/hft_apology.html] an order book needs to be organized by price level first and then by arrival time.
To simplify the order filling slightly, I've enforced that every order involved in a fill is above/below the requested limit for sells/buys.
Presumably in practice you could still fill an order if the average fill price satisfied the requested limit.   

We can have a separate order book per symbol as they are entirely independent and can be handled concurrently.
A map can look up the respective order book for a given symbol in O(1) time.
Each orderbook is a sorted vector of queues per price level, which gives us O(log(N)) to insert an order at an existing price level and
finding a set of fulfilling orders takes O(N) time. Time complexity of filling an order 
depends on a few things:
1. The order price level distribution. The worst case is a single order per price level, yielding
O(N * k) to fill where k is the number of orders on the other side of the trade required to fill.
The best case is just a single queue at a one price level, yielding O(k).
2. How often we need to remove queues once orders a price level are drained because that 
results in a vector shift (O(N)). 


### Examples
```
RUST_BACKTRACE=1 RUST_LOG=debug cargo run


curl -H "Content-Type: application/json" -d '{"price": 3, "side": "Sell", "amount": 5, "symbol": "AAPL"}' localhost:3000/order | jq
{
  "id": "ef1c4f22-ff16-4b40-9c92-881b1f1db8ca",
  "amount": 5,
  "symbol": "AAPL",
  "price": 3,
  "side": "Sell"
}

curl -H "Content-Type: application/json" -d '{"price": 3, "side": "Sell", "amount": 5, "symbol": "AAPL"}' localhost:3000/order | jq
{
  "id": "40bc6343-f2cf-486c-9dc6-8111ea3e69ac",
  "amount": 5,
  "symbol": "AAPL",
  "price": 3,
  "side": "Sell"
}


curl localhost:3000/sells | jq
{
  "AAPL": [
    [
      {
        "id": "ef1c4f22-ff16-4b40-9c92-881b1f1db8ca",
        "amount": 5,
        "symbol": "AAPL",
        "price": 3,
        "side": "Sell"
      },
      {
        "id": "40bc6343-f2cf-486c-9dc6-8111ea3e69ac",
        "amount": 5,
        "symbol": "AAPL",
        "price": 3,
        "side": "Sell"
      }
    ]
  ],
  "MSFT": [],
  "AMZN": []
}

curl -H "Content-Type: application/json" -d '{"price": 3, "side": "Buy", "amount": 7, "symbol": "AAPL"}' localhost:3000/order | jq
{
  "avg_price": 3
}

curl localhost:3000/sells
{
  "AMZN": [],
  "AAPL": [
    [
      {
        "id": "40bc6343-f2cf-486c-9dc6-8111ea3e69ac",
        "amount": 3,
        "symbol": "AAPL",
        "price": 3,
        "side": "Sell"
      }
    ]
  ],
  "MSFT": []
}

```

### Unit tests
```
RUST_BACKTRACE=1 cargo test -- --nocapture
```


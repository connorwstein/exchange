A first project in Rust - in-memory order book.

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


### Example
```
RUST_LOG=debug cargo run


curl -H "Content-Type: application/json" -d '{"price": 3, "side": "Buy", "amount": 10, "symbol": "AAPL"}' localhost:3000/order | jq
{
  "id": "51e562ce-284b-4562-a4bc-527215fa5128",
  "amount": 10,
  "symbol": "AAPL",
  "price": 3,
  "side": "Buy"
}

curl localhost:3000/buys
[
  [
    {
      "id": "5c730c40-c36f-4aac-bd50-69d7f4d8d886",
      "amount": 10,
      "symbol": "AAPL",
      "price": 3,
      "side": "Buy"
    }
  ]
]

curl -H "Content-Type: application/json" -d '{"price": 2, "side": "Sell", "amount": 5, "symbol": "AAPL"}' localhost:3000/fill | jq
{
  "avg_price": 3
}

curl localhost:3000/buys
[
  [
    {
      "id": "5c730c40-c36f-4aac-bd50-69d7f4d8d886",
      "amount": 5,
      "symbol": "AAPL",
      "price": 3,
      "side": "Buy"
    }
  ]
]

```

### Unit tests
```
RUST_BACKTRACE=1 cargo test -- --nocapture
```


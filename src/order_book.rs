use std::sync::{Arc, RwLock};
use std::collections::VecDeque;
use serde::{Deserialize, Serialize};
use log::{info, warn};
use uuid::{Uuid};


#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq)]
pub enum Side {
    Buy,
    Sell
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
pub enum Symbol {
    AAPL,
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
pub struct OpenLimitOrder {
    pub id: uuid::Uuid,
    pub amount: u32,
    pub symbol: Symbol,
    pub price: u32,
    pub side: Side,
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
pub struct FilledLimitOrder {
    open_order: OpenLimitOrder,
    executed_price: u32,
}


type OrderBook = Arc<RwLock<Vec<VecDeque<OpenLimitOrder>>>>;

// Back this "exchange" with an global in-memory store
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
        println!("index {:?} order queue {:?}", index, order_queue);
        if order_queue.front().unwrap().price == t.price {
            queue_index = Some(index);
            break
        } else if order_queue.front().unwrap().price < t.price && t.side == Side::Buy {
            insert_index = Some(index);
            break
        } else if order_queue.front().unwrap().price > t.price && t.side == Side::Sell {
            insert_index = Some(index);
            break
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

pub fn find_order(t: OpenLimitOrder, ob: &mut Vec<VecDeque<OpenLimitOrder>>) -> (Option<usize>, Option<usize>) {
    // TODO: optimze - can binary search to find the order
    for (i, order_queue) in ob.iter().enumerate() {
        for (j, order) in order_queue.iter().enumerate() {
            if order.id == t.id {
                println!("found order to remove id {}", t.id);
                return (Some(i as usize), Some(j as usize));
            }
        }
    }
    return (None, None);
}


pub fn remove_order(t: OpenLimitOrder)  {
    // TODO: way to get this in a function? struggling to implement a working get_order_book_mut fn
    let mut maybe_ob = None;
    if t.side == Side::Buy {
        maybe_ob = Some(Arc::clone(&BUY_ORDER_BOOK));

    } else {
        maybe_ob = Some(Arc::clone(&SELL_ORDER_BOOK));
    }
    let ob = maybe_ob.unwrap();
    let mut order_book = ob.write().unwrap();
    let (queue_index, order_index) = find_order(t, &mut *order_book);
    if queue_index.is_none() || order_index.is_none() {
        println!("no such order");
        return;
    }
    let res = order_book[queue_index.unwrap()].remove(order_index.unwrap());
    if res.is_none() {
        println!("error removing");
    }
    println!("removed order successfully");
    if order_book[queue_index.unwrap()].len() == 0 {
        println!("no more orders at price point {}", t.price);
        order_book.remove(queue_index.unwrap());
    }

}

pub fn valid_price(side: Side, to_fill_price: u32, candidate_order_price: u32) -> bool {
    if side == Side::Buy {
        return to_fill_price <= candidate_order_price;
    }
    return to_fill_price >= candidate_order_price;
}

// If there's something at the exact price we want, drain the queue as much as we can
// If the queue runs out, drop to the next allowed price level.
// TODO: If we drain the whole order book what do we do? It could become fillable as soon as other orders come through,
// do we just keep it lying around and check periodically seems tricky? Maybe just error out at the beginning
// Returns the set of orders on the other side which can satisfy the order
// The sum of the amounts in the returning orders is at least as great as the request.
// Defer to handling partial fills until later 
pub fn fill_order(to_fill: OpenLimitOrder) -> Result<Vec<OpenLimitOrder>, &'static str> {
    // Grab the a references order book of the opposite side:
    let mut maybe_ob = None;
    if to_fill.side == Side::Buy {
        maybe_ob = Some(Arc::clone(&SELL_ORDER_BOOK));
    } else {
        maybe_ob = Some(Arc::clone(&BUY_ORDER_BOOK));
    }
    let ob = maybe_ob.unwrap();

    // TODO: binary search to the order we want to start with
    let mut remaining: i32 = to_fill.amount as i32;
    let mut orders = Vec::new(); // vec puts this on the heap for us

    { // Inside here so we drop the write lock. More elegant way?
        let mut order_book = ob.write().unwrap();
        println!("orderbook size {}", order_book.len());

        if order_book.len() == 0 {
        println!("no orders");
        return Err("empty book");
    }
    // If the current price is no good break
    if !valid_price(to_fill.side, to_fill.price, order_book[0].front().unwrap().price) {
        println!("nothing available in book at valid price");
        return Err("cannot fill order");
    }

    // Drain each queue one by one as needed
    while valid_price(to_fill.side, to_fill.price, order_book[0].front().unwrap().price) {
        let order = order_book[0].pop_front();
        match order {
            Some(order) => {
                orders.push(order);
                println!("selecting order {:?}", order);
                remaining = remaining - order.amount as i32;
            },
            None => {
                println!("drained the whole queue at current price, moving to next price");
            }
        }
        if order_book[0].len() == 0 {
            order_book.remove(0);
        }
        if remaining <= 0 {
            println!("filled the order");
            break;
        }
        if order_book.len() == 0 {
            println!("drained the whole book without filling the order");
            // Add all the order back if we fail to fill
            for &i in orders.iter() {
                add_order(i);
            }
            return Err("failed to fill order, drained whole book");
        }
    }

    if remaining > 0 {
        return Err("unable to fill order at specified price");
    }

    if remaining == 0 {
        // Exact fill
        return Ok(orders);
    }
}

    // Need to split an order. Guaranteed that last order is > remaining
    let last_order = orders[orders.len() - 1];
    add_order(OpenLimitOrder{
        id: Uuid::new_v4(),
        price: last_order.price,
        side: last_order.side,
        amount: last_order.amount - remaining.abs() as u32,
        symbol: last_order.symbol,
    });
    orders.pop(); // remove that order
    // push on the remainder
    orders.push(OpenLimitOrder{
        id: Uuid::new_v4(),
        price: last_order.price,
        side: last_order.side,
        amount: remaining as u32,
        symbol: last_order.symbol,
    });
    return Ok(orders);
}

pub fn clear_order_book() {
    let ob = Arc::clone(&BUY_ORDER_BOOK);
    let mut lock = ob.write().unwrap(); // take a write lock
    lock.clear();
    let ob = Arc::clone(&SELL_ORDER_BOOK);
    let mut lock = ob.write().unwrap(); // take a write lock
    lock.clear();
}

#[cfg(test)]
mod tests {
    use crate::VecDeque;
    use crate::order_book::{Symbol, add_order, remove_order, get_order_book, OpenLimitOrder, Side, fill_order, clear_order_book};
    use uuid::Uuid;

    fn assert_order(expected: &OpenLimitOrder, actual: &OpenLimitOrder) {
        assert_eq!(expected.amount, actual.amount);
        assert_eq!(expected.price, actual.price);
        assert_eq!(expected.side, actual.side);
        assert_eq!(expected.id, actual.id);
    }

    fn assert_orders(expected: Vec<OpenLimitOrder>, actual: Vec<OpenLimitOrder>) {
        assert_eq!(expected.len(), actual.len());
        for i in 0..actual.len() {
            assert_order(&expected[i], &actual[i]);
        }
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
    fn test_fill_order() {
        let bids = vec![OpenLimitOrder {
            id: Uuid::new_v4(),
            amount: 10,
            symbol: Symbol::AAPL,
            side: Side::Buy,
            price: 5,
        }, OpenLimitOrder{
            id: Uuid::new_v4(),
            amount: 10,
            symbol: Symbol::AAPL,
            side: Side::Buy,
            price: 5,
        }];
        add_order(bids[0]);
        add_order(bids[1]);
        assert_order_book(vec![VecDeque::from(vec![bids[0], bids[1]])], get_order_book(Side::Buy));
        let result = fill_order(OpenLimitOrder{
            id: Uuid::new_v4(),
            amount: 11,
            symbol: Symbol::AAPL,
            side: Side::Sell,
            price: 5,
        });
        assert!(!result.is_err());
    }

    #[test]
    fn test_buy_order_book() {
        let bids = vec![OpenLimitOrder {
            id:  Uuid::new_v4(),
            amount: 10,
            symbol: Symbol::AAPL,
            side: Side::Buy,
            price: 5,
        }, OpenLimitOrder{
            id:  Uuid::new_v4(),
            amount: 10,
            symbol: Symbol::AAPL,
            side: Side::Buy,
            price: 5,
        }];
        add_order(bids[0]);
        assert_order_book(vec![VecDeque::from(vec![bids[0]])], get_order_book(Side::Buy));

        add_order(bids[1]);
        assert_order_book(vec![VecDeque::from(vec![bids[0], bids[1]])], get_order_book(Side::Buy));
        let higher_bids = vec![OpenLimitOrder {
            id:  Uuid::new_v4(),
            amount: 9,
            symbol: Symbol::AAPL,
            side: Side::Buy,
            price: 6,
        }, OpenLimitOrder {
            id:  Uuid::new_v4(),
            amount: 9,
            symbol: Symbol::AAPL,
            side: Side::Buy,
            price: 6,
        }];
        add_order(higher_bids[0]);
        add_order(higher_bids[1]);
        assert_order_book(vec![
            VecDeque::from(vec![higher_bids[0], higher_bids[1]]),
            VecDeque::from(vec![bids[0], bids[1]])],
                          get_order_book(Side::Buy));
        let lower_bids = vec![OpenLimitOrder{
            id:  Uuid::new_v4(),
            amount: 8,
            symbol: Symbol::AAPL,
            side: Side::Buy,
            price: 4,
        }, OpenLimitOrder{
            id:  Uuid::new_v4(),
            amount: 8,
            symbol: Symbol::AAPL,
            side: Side::Buy,
            price: 4,
        }];
        add_order(lower_bids[0]);
        add_order(lower_bids[1]);
        assert_order_book(vec![
            VecDeque::from(vec![higher_bids[0], higher_bids[1]]),
            VecDeque::from(vec![bids[0], bids[1]]),
            VecDeque::from(vec![lower_bids[0], lower_bids[1]])],
                          get_order_book(Side::Buy));

        remove_order(lower_bids[0]);
        assert_order_book(vec![
            VecDeque::from(vec![higher_bids[0], higher_bids[1]]),
            VecDeque::from(vec![bids[0], bids[1]]),
            VecDeque::from(vec![lower_bids[1]])],
                          get_order_book(Side::Buy));
        remove_order(lower_bids[1]);
        assert_order_book(vec![
            VecDeque::from(vec![higher_bids[0], higher_bids[1]]),
            VecDeque::from(vec![bids[0], bids[1]])],
                          get_order_book(Side::Buy));
        // Removing a non-existent order should have no effect
        remove_order(OpenLimitOrder{id: Uuid::new_v4(), amount: 10, symbol: Symbol::AAPL, price:1, side: Side::Buy});
        assert_order_book(vec![
            VecDeque::from(vec![higher_bids[0], higher_bids[1]]),
            VecDeque::from(vec![bids[0], bids[1]])],
                          get_order_book(Side::Buy));
        clear_order_book();
    }

    #[test]
    fn test_sell_order_book() {
        let asks = vec![OpenLimitOrder{
            id:  Uuid::new_v4(),
            amount: 10,
            symbol: Symbol::AAPL,
            side: Side::Sell,
            price: 5,
        }, OpenLimitOrder {
            id:  Uuid::new_v4(),
            amount: 10,
            symbol: Symbol::AAPL,
            side: Side::Sell,
            price: 5,
        }];
        add_order(asks[0]);
        assert_order_book(vec![VecDeque::from(vec![asks[0]])], get_order_book(Side::Sell));
        add_order(asks[1]);
        assert_order_book(vec![VecDeque::from(vec![asks[0], asks[1]])], get_order_book(Side::Sell));
        let higher_asks = vec![OpenLimitOrder{
            id:  Uuid::new_v4(),
            amount: 9,
            symbol: Symbol::AAPL,
            side: Side::Sell,
            price: 6,
        }, OpenLimitOrder {
            id:  Uuid::new_v4(),
            amount: 9,
            symbol: Symbol::AAPL,
            side: Side::Sell,
            price: 6,
        }];
        add_order(higher_asks[0]);
        add_order(higher_asks[1]);
        assert_order_book(vec![
            VecDeque::from(vec![asks[0], asks[1]]),
            VecDeque::from(vec![higher_asks[0], higher_asks[1]])],
                          get_order_book(Side::Sell));

        let lower_asks = vec![OpenLimitOrder{
            id:  Uuid::new_v4(),
            amount: 8,
            symbol: Symbol::AAPL,
            side: Side::Sell,
            price: 4,
        }, OpenLimitOrder{
            id:  Uuid::new_v4(),
            amount: 8,
            symbol: Symbol::AAPL,
            side: Side::Sell,
            price: 4,
        }];
        add_order(lower_asks[0]);
        add_order(lower_asks[1]);
        assert_order_book(vec![
            VecDeque::from(vec![lower_asks[0], lower_asks[1]]),
            VecDeque::from(vec![asks[0], asks[1]]),
            VecDeque::from(vec![higher_asks[0], higher_asks[1]])],
                          get_order_book(Side::Sell));
       clear_order_book();
    }
}

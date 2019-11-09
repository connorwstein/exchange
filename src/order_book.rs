use std::sync::{Arc, RwLock};
use std::collections::VecDeque;
use serde::{Deserialize, Serialize};
use log::{info, warn};


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
    use crate::VecDeque;
    use crate::order_book::{Symbol, add_order, remove_order, get_order_book, OpenLimitOrder, Side};

    fn assert_order(expected: &OpenLimitOrder, actual: &OpenLimitOrder) {
        assert_eq!(expected.amount, actual.amount);
        assert_eq!(expected.price, actual.price);
        assert_eq!(expected.side, actual.side);
        assert_eq!(expected.id, actual.id);

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
        let bids = vec![OpenLimitOrder {
            id: 1,
            amount: 10,
            symbol: Symbol::AAPL,
            side: Side::Buy,
            price: 5,
        }, OpenLimitOrder{
            id: 2,
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
            id: 3,
            amount: 9,
            symbol: Symbol::AAPL,
            side: Side::Buy,
            price: 6,
        }, OpenLimitOrder {
            id: 4,
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
            id: 5,
            amount: 8,
            symbol: Symbol::AAPL,
            side: Side::Buy,
            price: 4,
        }, OpenLimitOrder{
            id: 6,
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
        remove_order(OpenLimitOrder{id: 100, amount: 10, symbol: Symbol::AAPL, price:1, side: Side::Buy});
        assert_order_book(vec![
            VecDeque::from(vec![higher_bids[0], higher_bids[1]]),
            VecDeque::from(vec![bids[0], bids[1]])],
                          get_order_book(Side::Buy));
    }

    #[test]
    fn test_sell_order_book() {
        let asks = vec![OpenLimitOrder{
            id: 1,
            amount: 10,
            symbol: Symbol::AAPL,
            side: Side::Sell,
            price: 5,
        }, OpenLimitOrder {
            id: 2,
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
            id: 3,
            amount: 9,
            symbol: Symbol::AAPL,
            side: Side::Sell,
            price: 6,
        }, OpenLimitOrder {
            id: 4,
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
            id: 5,
            amount: 8,
            symbol: Symbol::AAPL,
            side: Side::Sell,
            price: 4,
        }, OpenLimitOrder{
            id: 6,
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

    }
}
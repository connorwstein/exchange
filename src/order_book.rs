use std::sync::{Arc, RwLock};
use std::collections::VecDeque;
use serde::{Deserialize, Serialize};
use log::{info, warn};
use uuid::{Uuid};
use std::boxed::{Box};


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

pub struct OrderBookV2 {
    book: Box<Vec<VecDeque<OpenLimitOrder>>>,
    side: Side,
}

impl OrderBookV2 {
    pub fn new(side: Side) -> OrderBookV2 {
        OrderBookV2{
            book: Box::new(Vec::new()),
            side: side,
        }
    }

    pub fn get_book(&self) -> Vec<VecDeque<OpenLimitOrder>> {
        return (*self.book).clone()
    }

    pub fn find_order(&self, t: OpenLimitOrder) -> (Option<usize>, Option<usize>) {
        // TODO: optimize - can binary search to find the order
        for (i, order_queue) in self.book.iter().enumerate() {
            for (j, order) in order_queue.iter().enumerate() {
                if order.id == t.id {
                    println!("found order, id {}", t.id);
                    return (Some(i as usize), Some(j as usize));
                }
            }
        }
        return (None, None);
    }

    pub fn remove_order(&mut self, t: OpenLimitOrder) -> Result<&'static str, &'static str> {
        let (queue_index, order_index) = self.find_order(t);
        if queue_index.is_none() || order_index.is_none() {
            return Err("no such order");
        }
        let res = self.book[queue_index.unwrap()].remove(order_index.unwrap());
        if res.is_none() {
            return Err("error removing");
        }
        if self.book[queue_index.unwrap()].len() == 0 {
            println!("no more orders at price point {}", t.price);
            self.book.remove(queue_index.unwrap());
        }
        return Ok("successfully removed order");
    }

    pub fn add_order(&mut self, t: OpenLimitOrder) -> Result<&'static str, &'static str> {
        if t.side != self.side {
            return Err("wrong side")
        }
        println!("adding Order {:?}", t);
        // If we find an entry at that price point, add it to the queue
        // Otherwise create a queue at that price point.
        let mut queue_index = None;
        let mut insert_index = None;

        for (index, order_queue) in self.book.iter().enumerate() {
            println!("index {:?} order queue {:?}", index, order_queue);
            if order_queue.front().unwrap().price == t.price {
                queue_index = Some(index);
                break
            } else if order_queue.front().unwrap().price < t.price && self.side == Side::Buy {
                insert_index = Some(index);
                break
            } else if order_queue.front().unwrap().price > t.price && self.side == Side::Sell {
                insert_index = Some(index);
                break
            }
        }

        match queue_index {
            Some(queue_index) => {
                // Existing orders at that price
                self.book[queue_index].push_back(t);
            },
            None => {
                // No existing orders at the price, create a new queue
                let mut orders: VecDeque<OpenLimitOrder> = VecDeque::new();
                orders.push_back(t);
                // Put the queue in the right place
                match insert_index {
                    Some(insert_index) => {
                        // We know the spot to put this new queue
                        self.book.insert(insert_index, orders);
                    },
                    None => {
                        // Order book must be empty, just push the queue into the first spot
                        self.book.push(orders);
                    }
                }
            }
        };
        return Ok("added order")
    }

    pub fn valid_price(&self, to_fill_price: u32, candidate_order_price: u32) -> bool {
        if self.side == Side::Buy {
            return to_fill_price <= candidate_order_price;
        }
        return to_fill_price >= candidate_order_price;
    }

    // Returns orders on the other side that were used to fill the order.
    // Removes any orders that were used to fill from the book.
    // If sum(orders returns) > to_fill, then the last order was only partially used to fill.
    pub fn fill_order(&mut self, to_fill: OpenLimitOrder) -> Result<Vec<OpenLimitOrder>, &'static str> {
        if to_fill.side == Side::Buy && self.side != Side::Sell {
            return Err("cannot fill buy order with sell book")
        }
        if to_fill.side == Side::Sell && self.side != Side::Buy {
            return Err("cannot fill sell order with buy  book")
        }

        println!("orderbook size {}", self.book.len());
        if self.book.len() == 0 {
            return Err("empty book");
        }

        // If the current price is no good break
        if !self.valid_price(to_fill.price, self.book[0].front().unwrap().price) {
            println!("nothing available in book at valid price");
            return Err("cannot fill order");
        }

        let mut remaining: i32 = to_fill.amount as i32;
        let mut orders = Vec::new();

        // Drain each queue one by one as needed
        while self.valid_price(to_fill.price, self.book[0].front().unwrap().price) {
            let order = self.book[0].pop_front();
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
            if self.book[0].len() == 0 {
                self.book.remove(0);
            }
            if remaining <= 0 {
                println!("filled the order");
                break;
            }
            if self.book.len() == 0 {
                println!("drained the whole book without filling the order");
                // Add all the order back if we fail to fill
                for &i in orders.iter() {
                    self.add_order(i);
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

        // Need to split an order.
        // Guaranteed that last order is > remaining.
        let last_order = orders[orders.len() - 1];
        self.add_order(OpenLimitOrder{
            id: Uuid::new_v4(),
            price: last_order.price,
            side: last_order.side,
            amount: last_order.amount - remaining.abs() as u32,
            symbol: last_order.symbol,
        });
        return Ok(orders);
    }
}

#[cfg(test)]
mod tests {
    use crate::VecDeque;
    use crate::order_book::{Symbol, OpenLimitOrder, Side, OrderBookV2};
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
    fn test_order_v2() {
        // Test structure: add all the orders,
        // assert book looks as expected.
        // remove all specified orders
        // assert book looks as expected.
        struct TestCase {
            add: Vec<OpenLimitOrder>,
            expectedAfterAdd: Vec<VecDeque<OpenLimitOrder>>,
            remove: Vec<OpenLimitOrder>,
            expectedAfterRemove: Vec<VecDeque<OpenLimitOrder>>,
        };
        let test_cases = vec![
            TestCase {
                add: vec![OpenLimitOrder {
                    id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
                    amount: 10,
                    symbol: Symbol::AAPL,
                    side: Side::Buy,
                    price: 5,
                }],
                expectedAfterAdd: vec![VecDeque::from(vec![OpenLimitOrder {
                    id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
                    amount: 10,
                    symbol: Symbol::AAPL,
                    side: Side::Buy,
                    price: 5,
                }])],
                remove: vec![OpenLimitOrder {
                    id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
                    amount: 10,
                    symbol: Symbol::AAPL,
                    side: Side::Buy,
                    price: 5,
                }],
                expectedAfterRemove: Vec::new()
            }
        ];
        let mut buy_ob = OrderBookV2::new(Side::Buy);
        for tc in test_cases.iter() {
            for &to_add in tc.add.iter() {
                buy_ob.add_order(to_add);
            }
            assert_order_book(buy_ob.get_book(), tc.expectedAfterAdd.clone());
            for &to_remove in tc.remove.iter() {
                buy_ob.remove_order(to_remove);
            }
            assert_order_book(buy_ob.get_book(), tc.expectedAfterRemove.clone());
        }
    }
}
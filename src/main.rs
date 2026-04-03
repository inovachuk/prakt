use derive_more::Display;

#[derive(Debug, Clone, PartialEq, Eq, Display)]
pub struct OrderId(String);

#[derive(Debug, Clone, PartialEq, Display)]
pub struct Amount(f64);

#[derive(Debug)]
pub struct New;

#[derive(Debug)]
pub struct Paid;

#[derive(Debug)]
pub struct Shipped;

#[derive(Debug)]
pub struct Order<State> {
    id: OrderId,
    amount: Amount,
    state: State,
}

impl Order<New> {
    pub fn new(id: OrderId, amount: Amount) -> Self {
        Self {
            id,
            amount,
            state: New,
        }
    }

    pub fn pay(self) -> Order<Paid> {
        println!("Замовлення {} на суму {} оплачено.", self.id, self.amount);
        Order {
            id: self.id,
            amount: self.amount,
            state: Paid,
        }
    }
}

impl Order<Paid> {
    pub fn ship(self) -> Order<Shipped> {
        println!("Замовлення {} відправлено.", self.id);
        Order {
            id: self.id,
            amount: self.amount,
            state: Shipped,
        }
    }
}

impl Order<Shipped> {
    pub fn deliver(self) {
        println!("Замовлення {} успішно доставлено клієнту.", self.id);
    }
}

fn main() {
    let order_id = OrderId("ORD-777".to_string());
    let amount = Amount(2500.50);

    let order_new = Order::new(order_id, amount);
    let order_paid = order_new.pay();
    let order_shipped = order_paid.ship();
    
    order_shipped.deliver();
}
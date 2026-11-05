#[derive(repay::Model)]
struct Request {
    #[repay(required)]
    amount: String,
}

fn main() {}

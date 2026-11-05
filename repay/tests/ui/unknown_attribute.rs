#[derive(repay::Model)]
struct Request {
    #[repay(secret)]
    card_number: String,
}

fn main() {}

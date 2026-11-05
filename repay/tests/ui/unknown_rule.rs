#[derive(repay::Model)]
struct Request {
    #[repay(rule = "no_emoji")]
    name: String,
}

fn main() {}

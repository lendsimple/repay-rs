#[derive(repay::Model)]
struct Request {
    #[repay(min = "zero")]
    status: i32,
}

fn main() {}

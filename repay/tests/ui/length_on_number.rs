#[derive(repay::Model)]
struct Request {
    #[repay(max_length = 10)]
    batch_id: i32,
}

fn main() {}

#[derive(repay::Model)]
#[repay(one_of = "card_number | card_token")]
struct Sale {
    card_number: String,
    card_token: Option<String>,
}

fn main() {}

#[derive(repay::Model)]
#[repay(one_of = "card_number | card_tokn")]
struct Sale {
    card_number: Option<String>,
    card_token: Option<String>,
}

fn main() {}

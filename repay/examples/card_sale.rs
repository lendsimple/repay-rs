//! A card sale in the sandbox with the async client.
//!
//! ```sh
//! REPAY_API_USER=... REPAY_SECURE_TOKEN=... REPAY_MERCHANT_ID=... \
//!     cargo run --example card_sale
//! ```

use repay::schema::CardSaleRequest;
use repay::types::{Amount, ExpDate};
use repay::{Client, Credentials, dec};

fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("set {name}"))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut builder = Client::builder(Credentials::new(
        env("REPAY_API_USER"),
        env("REPAY_SECURE_TOKEN"),
    ));
    if let Ok(merchant_id) = std::env::var("REPAY_MERCHANT_ID") {
        builder = builder.merchant_id(merchant_id.parse::<i32>()?);
    }
    let client = builder.build();

    let sale = CardSaleRequest::builder()
        .amount(Amount::from(dec!(19.99)))
        .card_number("4111111111111111")
        .exp_date(ExpDate::new(12, 2030)?)
        .cvv("123")
        .invoice_id("repay-rs-example")
        .build();
    let detail = client.card_transactions().sale(&sale).await?;
    println!(
        "transaction {:?}: {:?}",
        detail.transaction_id, detail.result_text
    );
    Ok(())
}

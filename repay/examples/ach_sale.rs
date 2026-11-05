//! An ACH debit in the sandbox, from a bank account or a stored ACH token.
//!
//! ```sh
//! REPAY_API_USER=... REPAY_SECURE_TOKEN=... REPAY_MERCHANT_ID=... \
//!     cargo run --example ach_sale [ach-token]
//! ```

use repay::schema::{AchAccount, AchSaleRequest};
use repay::types::Amount;
use repay::{Client, Credentials, dec};

fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("set {name}"))
}

/// A client for the sandbox, acting for `REPAY_MERCHANT_ID` when it is set.
fn client() -> Result<Client, Box<dyn std::error::Error>> {
    let mut builder = Client::builder(Credentials::new(
        env("REPAY_API_USER"),
        env("REPAY_SECURE_TOKEN"),
    ));
    if let Ok(merchant_id) = std::env::var("REPAY_MERCHANT_ID") {
        builder = builder.merchant_id(merchant_id.parse::<i32>()?);
    }
    Ok(builder.build())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = client()?;
    let amount = Amount::from(dec!(25.00));
    let mut sale = match std::env::args().nth(1) {
        Some(token) => AchSaleRequest::token(amount, token),
        None => AchSaleRequest::account(
            amount,
            AchAccount::new("123456789", "021000021", "Ann Smith").with_account_type("CHECKING"),
        ),
    };
    sale.invoice_id = Some("repay-rs-ach-example".into());
    let result = client.ach_transactions().sale(&sale).await?;
    println!("{}", serde_json::to_string_pretty(result.as_value())?);
    Ok(())
}

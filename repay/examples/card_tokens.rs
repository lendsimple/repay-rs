//! Stores a card as a token in the sandbox, reads it back, then deletes it.
//!
//! ```sh
//! REPAY_API_USER=... REPAY_SECURE_TOKEN=... REPAY_MERCHANT_ID=... \
//!     cargo run --example card_tokens
//! ```

use repay::schema::CreateCardTokenRequest;
use repay::types::{CardTokenKey, ExpDate};
use repay::{Client, Credentials};

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
    let card = CreateCardTokenRequest::builder()
        .card_number("4111111111111111")
        .exp_date(ExpDate::new(12, 2030)?)
        .name_on_card("Ann Smith")
        .nickname("repay-rs example")
        .build();
    let created = client.card_tokens().create(&card).await?;
    println!(
        "created: {}",
        serde_json::to_string_pretty(created.as_value())?
    );

    // The token response is open-ended JSON; its key identifies it.
    let Some(key) = created.get("card_token_key").and_then(|v| v.as_i64()) else {
        println!("no card_token_key in the response");
        return Ok(());
    };
    let key = CardTokenKey::new(i32::try_from(key)?);
    let token = client.card_tokens().get(key, &Default::default()).await?;
    println!(
        "read back: {}",
        serde_json::to_string_pretty(token.as_value())?
    );
    client.card_tokens().delete(key).await?;
    println!("deleted {key}");
    Ok(())
}

//! Creates a customer in the sandbox, then lists the merchant's customers.
//!
//! ```sh
//! REPAY_API_USER=... REPAY_SECURE_TOKEN=... REPAY_MERCHANT_ID=... \
//!     cargo run --example customers
//! ```

use repay::schema::{ContactRequest, CustomerRequest};
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
    let customer = CustomerRequest::builder()
        .customer_id("repay-rs-example")
        .customer_name("Ann Smith")
        .contact(
            ContactRequest::builder()
                .first_name("Ann")
                .last_name("Smith")
                .email("ann@example.com")
                .build(),
        )
        .build();
    let created = client.customers().create(&customer).await?;
    println!(
        "created: {}",
        serde_json::to_string_pretty(created.as_value())?
    );
    let customers = client.customers().list().await?;
    println!(
        "customers: {}",
        serde_json::to_string_pretty(customers.as_value())?
    );
    Ok(())
}

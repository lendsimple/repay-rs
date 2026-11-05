//! Uploads bulk disbursement funding adjustments (an internal API) and prints
//! the outcome: the result on success, or each failed row.
//!
//! ```sh
//! REPAY_API_USER=... REPAY_SECURE_TOKEN=... REPAY_MERCHANT_ID=... \
//!     cargo run --example bulk_adjustment -- <merchant-id> <transaction-id> <original> <adjustment>
//! ```

use repay::bulk::{BulkAdjustment, BulkOutcome};
use repay::types::Amount;
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
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [merchant_id, transaction_id, original, adjustment] = args.as_slice() else {
        return Err(
            "usage: bulk_adjustment <merchant-id> <transaction-id> <original> <adjustment>".into(),
        );
    };
    let mut adjustments = BulkAdjustment::new();
    adjustments.row(
        merchant_id.parse::<i32>()?,
        transaction_id.parse::<i32>()?,
        original.parse::<Amount>()?,
        adjustment.parse::<Amount>()?,
    );
    let upload = adjustments.to_upload()?;

    let client = client()?;
    match client
        .adjustments()
        .bulk_disbursement_funding(&upload)
        .await?
    {
        BulkOutcome::Completed(result) => {
            println!(
                "completed: {}",
                serde_json::to_string_pretty(result.as_value())?
            );
        }
        BulkOutcome::Failed(report) => {
            println!("failed rows ({}):", report.headers().join(", "));
            for row in report.rows() {
                println!("  {}", row.join(", "));
            }
        }
        other => println!("{other:?}"),
    }
    Ok(())
}

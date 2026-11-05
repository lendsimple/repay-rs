//! Smoke tests against the real Repay sandbox. They are ignored by default; run
//! them with sandbox credentials:
//!
//! ```sh
//! REPAY_API_USER=... REPAY_SECURE_TOKEN=... REPAY_MERCHANT_ID=... \
//!     cargo test --all-features --test sandbox -- --ignored --test-threads 1
//! ```
//!
//! They print what they learn about the API's behavior (declines, response
//! shapes), which the open items in `implementation.md` wait on.
#![cfg(all(feature = "reqwest", feature = "ureq"))]

use std::time::{SystemTime, UNIX_EPOCH};

use repay::operations::card_transactions::SearchParams;
use repay::schema::{
    CardCaptureRequest, CardRefundRequest, CardSaleRequest, CreateCardTokenRequest,
};
use repay::types::{Amount, CardTokenKey, ExpDate, TransactionId};
use repay::{Client, Credentials, Error, ExportFormat, dec};

fn var(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| {
        panic!(
            "the sandbox tests need REPAY_API_USER, REPAY_SECURE_TOKEN and \
             REPAY_MERCHANT_ID ({name} is not set)"
        )
    })
}

fn credentials() -> Credentials {
    Credentials::new(var("REPAY_API_USER"), var("REPAY_SECURE_TOKEN"))
}

fn merchant_id() -> i32 {
    var("REPAY_MERCHANT_ID")
        .parse()
        .expect("REPAY_MERCHANT_ID is a number")
}

fn client() -> Client {
    Client::builder(credentials())
        .merchant_id(merchant_id())
        .build()
}

/// Differs between runs, so the sandbox does not reject a charge as a duplicate.
fn invoice() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("repay-rs-{nanos}")
}

fn card_sale(amount: Amount) -> CardSaleRequest {
    let mut sale =
        CardSaleRequest::keyed(amount, "4111111111111111", ExpDate::new(12, 2030).unwrap())
            .with_cvv("123");
    sale.invoice_id = Some(invoice());
    sale
}

fn transaction_id(id: Option<TransactionId>) -> TransactionId {
    id.expect("the response has a transaction_id")
}

#[tokio::test]
#[ignore = "needs sandbox credentials"]
async fn health() {
    let status = client().health().check().await.unwrap();
    println!("health: {status}");
}

#[tokio::test]
#[ignore = "needs sandbox credentials"]
async fn card_sale_then_void() {
    let client = client();
    let sale = client
        .card_transactions()
        .sale(&card_sale(Amount::from(dec!(1.23))))
        .await
        .unwrap();
    println!("sale: {:?} {:?}", sale.result, sale.result_text);
    let id = transaction_id(sale.transaction_id);
    let void = client.card_transactions().void(id).await.unwrap();
    println!("void: {:?}", void.result_text);
}

#[tokio::test]
#[ignore = "needs sandbox credentials"]
async fn card_auth_capture_refund() {
    let client = client();
    let mut auth = repay::schema::CardAuthRequest::keyed(
        Amount::from(dec!(2.50)),
        "4111111111111111",
        ExpDate::new(12, 2030).unwrap(),
    )
    .with_cvv("123");
    auth.invoice_id = Some(invoice());
    let auth = client.card_transactions().auth(&auth).await.unwrap();
    let id = transaction_id(auth.transaction_id);

    let capture = CardCaptureRequest::builder()
        .amount(Amount::from(dec!(2.50)))
        .build();
    let captured = client
        .card_transactions()
        .capture(id, &capture)
        .await
        .unwrap();
    println!("capture: {:?}", captured.result_text);

    let refund = CardRefundRequest::builder()
        .amount(Amount::from(dec!(1.00)))
        .build();
    // A refund of an unsettled capture may be refused; print what the API says.
    match client.card_transactions().refund(id, &refund).await {
        Ok(refunded) => println!("refund: {:?}", refunded.result_text),
        Err(err) => println!("refund refused: {err}"),
    }
}

#[tokio::test]
#[ignore = "needs sandbox credentials"]
async fn card_token_create_get_delete() {
    let client = client();
    let card = CreateCardTokenRequest::builder()
        .card_number("4111111111111111")
        .exp_date(ExpDate::new(12, 2030).unwrap())
        .name_on_card("Repay Rs")
        .build();
    let created = client.card_tokens().create(&card).await.unwrap();
    println!("created token: {}", created.as_value());
    let key = created
        .get("card_token_key")
        .and_then(|v| v.as_i64())
        .expect("the response has a card_token_key");
    let key = CardTokenKey::new(i32::try_from(key).unwrap());
    let token = client
        .card_tokens()
        .get(key, &Default::default())
        .await
        .unwrap();
    println!("token: {}", token.as_value());
    client.card_tokens().delete(key).await.unwrap();
}

#[tokio::test]
#[ignore = "needs sandbox credentials"]
async fn card_search_and_csv_export() {
    let client = client();
    let params = SearchParams::builder().take(5).build();
    let page = client.card_transactions().search(&params).await.unwrap();
    println!("search: {}", page.as_value());
    let file = client
        .card_transactions()
        .search_export(&params, ExportFormat::Csv)
        .await
        .unwrap();
    println!(
        "export: {} ({:?}, {} bytes)",
        file.content_type,
        file.filename,
        file.bytes.len()
    );
}

#[tokio::test]
#[ignore = "needs sandbox credentials"]
async fn an_invalid_request_is_an_api_error() {
    // Skip client-side validation so the API sees a sale with no card.
    let client = Client::builder(credentials())
        .merchant_id(merchant_id())
        .validate_requests(false)
        .build();
    let sale = CardSaleRequest::builder()
        .amount(Amount::from(dec!(1)))
        .build();
    let err = client.card_transactions().sale(&sale).await.unwrap_err();
    println!("invalid sale: {err}");
    let Error::Api(api) = err else {
        panic!("expected an API error, got {err:?}");
    };
    assert_eq!(api.status(), 400);
}

#[test]
#[ignore = "needs sandbox credentials"]
fn blocking_client_health_and_sale() {
    let client = repay::blocking::Client::builder(credentials())
        .merchant_id(merchant_id())
        .build();
    println!("health: {}", client.health().check().unwrap());
    let sale = client
        .card_transactions()
        .sale(&card_sale(Amount::from(dec!(1.11))))
        .unwrap();
    println!("sale: {:?}", sale.result_text);
}

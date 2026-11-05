//! What the clients log. A separate test binary, because these tests install a
//! global `tracing` subscriber.
#![cfg(any(feature = "reqwest", feature = "ureq"))]

mod common;

use std::sync::OnceLock;

use common::{Answer, LogCapture, MockApi, credentials};
use repay::operations::card_transactions::SearchParams;

/// The card number and secure token the requests carry. (The CVV, `987`, could
/// appear by chance in a port number, so it is checked by its key.)
const SECRETS: [&str; 2] = ["4111111111111111", common::SECURE_TOKEN];

fn logs() -> &'static LogCapture {
    static LOGS: OnceLock<LogCapture> = OnceLock::new();
    LOGS.get_or_init(|| {
        let logs = LogCapture::default();
        tracing::subscriber::set_global_default(logs.subscriber()).unwrap();
        logs
    })
}

const SALE_PATH: &str = "/rgapi/v1/transactions/card/sale";
/// The response echoes the card number, as a careless API might.
const SALE_RESPONSE: &str = r#"{"result_text": "APPROVED", "card_number": "4111111111111111"}"#;
const SEARCH_PATH: &str =
    "/rgapi/v1/transactions/card/query/export?query.card_number=4111111111111111";

fn search_params() -> SearchParams {
    SearchParams::builder()
        .card_number("4111111111111111")
        .build()
}

fn assert_redacted(logs: &str) {
    assert!(
        logs.contains(r#"operation="card_transactions.sale""#),
        "{logs}"
    );
    assert!(
        logs.contains(r#"operation="card_transactions.search""#),
        "{logs}"
    );
    assert!(logs.contains(r#""cvv":"[REDACTED]""#), "{logs}");
    assert!(logs.contains("query.card_number=[REDACTED]"), "{logs}");
    for secret in SECRETS {
        assert!(!logs.contains(secret), "{secret} leaked into logs:\n{logs}");
    }
}

#[cfg(feature = "ureq")]
#[test]
fn blocking_client_logs_redact_secrets() {
    let logs = logs();
    let client = |api: &MockApi| {
        repay::blocking::Client::builder(credentials())
            .environment(api.environment())
            .build()
    };
    let api = MockApi::new("POST", SALE_PATH, Answer::json(200, SALE_RESPONSE));
    client(&api)
        .card_transactions()
        .sale(&common::card_sale())
        .unwrap();
    let api = MockApi::new("GET", SEARCH_PATH, Answer::json(200, "[]"));
    client(&api)
        .card_transactions()
        .search(&search_params())
        .unwrap();
    assert_redacted(&logs.contents());
}

#[cfg(feature = "reqwest")]
#[tokio::test]
async fn async_client_logs_redact_secrets() {
    let logs = logs();
    let client = |api: &MockApi| {
        repay::Client::builder(credentials())
            .environment(api.environment())
            .build()
    };
    let api = MockApi::new_async("POST", SALE_PATH, Answer::json(200, SALE_RESPONSE)).await;
    client(&api)
        .card_transactions()
        .sale(&common::card_sale())
        .await
        .unwrap();
    let api = MockApi::new_async("GET", SEARCH_PATH, Answer::json(200, "[]")).await;
    client(&api)
        .card_transactions()
        .search(&search_params())
        .await
        .unwrap();
    assert_redacted(&logs.contents());
}

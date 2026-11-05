//! The blocking client against a mock API.
#![cfg(feature = "ureq")]

#[macro_use]
mod common;

use std::time::Duration;

use common::{Answer, MERCHANT_ID, MockApi, credentials, fixture};
use repay::Error;
use repay::blocking::Client;
use repay::bulk::BulkOutcome;

fn client(api: &MockApi) -> Client {
    Client::builder(credentials())
        .environment(api.environment())
        .merchant_id(MERCHANT_ID)
        .build()
}

macro_rules! blocking_cases {
    ($(
        $name:ident: $resource:ident.$method:ident($($arg:expr),* $(,)?)
            => $http:literal $path:literal, $answer:expr;
    )*) => {$(
        #[test]
        fn $name() {
            let api = MockApi::new($http, $path, $answer);
            client(&api).$resource().$method($($arg),*).unwrap();
            let received = api.received();
            assert_eq!(received.header("rg-merchant-id"), Some("1234"));
        }
    )*};
}

client_cases!(blocking_cases);

#[test]
fn sends_json_with_its_content_type() {
    let api = MockApi::new(
        "POST",
        "/rgapi/v1/transactions/card/sale",
        Answer::json(200, "{}"),
    );
    client(&api)
        .card_transactions()
        .sale(&common::card_sale())
        .unwrap();
    let received = api.received();
    assert_eq!(received.header("content-type"), Some("application/json"));
    assert_eq!(received.header("accept"), Some("application/json"));
    let body: serde_json::Value = serde_json::from_slice(&received.body).unwrap();
    assert_eq!(body["card_number"], "4111111111111111");
    assert_eq!(body["exp_date"], "1230");
}

#[test]
fn uploads_files_as_multipart() {
    let api = MockApi::new(
        "POST",
        "/rgapi/v1/transactions/card/reversal/bulk",
        Answer::json(200, "{}"),
    );
    let upload = repay::types::Upload::csv("reversals.csv", "TransactionId\n1001\n");
    client(&api)
        .card_transactions()
        .bulk_reversal(&upload)
        .unwrap();
    let received = api.received();
    assert_eq!(
        received.header("content-type"),
        Some("multipart/form-data; boundary=repay-boundary")
    );
    let body = String::from_utf8(received.body).unwrap();
    assert!(body.contains("filename=\"reversals.csv\""), "{body}");
}

#[test]
fn an_error_response_is_an_api_error() {
    let api = MockApi::new(
        "GET",
        "/rgapi/v1/health",
        Answer::json(400, fixture("error_response.json")),
    );
    let err = client(&api).health().check().unwrap_err();
    let api_err = err.as_api().expect("an API error");
    assert!(api_err.is_validation());
    assert_eq!(api_err.message(), "The request is invalid.");
}

#[test]
fn an_html_error_page_is_a_status_error() {
    let api = MockApi::new(
        "GET",
        "/rgapi/v1/health",
        Answer::file("text/html", "<html>down</html>").status(503),
    );
    let err = client(&api).health().check().unwrap_err();
    assert!(matches!(err, Error::Status { status: 503, .. }), "{err:?}");
}

#[test]
fn a_slow_response_times_out() {
    let api = MockApi::new(
        "GET",
        "/rgapi/v1/health",
        Answer::json(200, r#""Healthy""#).delayed(Duration::from_secs(2)),
    );
    let client = Client::builder(credentials())
        .environment(api.environment())
        .timeout(Duration::from_millis(200))
        .build();
    let err = client.health().check().unwrap_err();
    let Error::Transport(err) = err else {
        panic!("expected a transport error, got {err:?}");
    };
    assert!(err.is_timeout(), "{err}");
}

#[test]
fn a_refused_connection_is_a_transport_error() {
    let client = Client::builder(credentials())
        .environment(repay::Environment::Custom(
            "http://127.0.0.1:1/rgapi".into(),
        ))
        .build();
    let err = client.health().check().unwrap_err();
    assert!(
        matches!(&err, Error::Transport(e) if !e.is_timeout()),
        "{err:?}"
    );
}

#[test]
fn an_invalid_request_is_never_sent() {
    let api = MockApi::new(
        "POST",
        "/rgapi/v1/openbatches/card",
        Answer::json(200, "{}"),
    );
    let client = client(&api).without_merchant_id();
    let err = client.batches().close_card(&common::batch()).unwrap_err();
    assert!(matches!(err, Error::Validation(_)), "{err:?}");
    api.assert_not_called();
}

#[test]
fn a_failed_bulk_upload_is_a_csv_report() {
    let api = MockApi::new(
        "POST",
        "/rgapi/v1/adjustments/disbursementfunding/bulk",
        Answer::file(
            "text/csv",
            "MerchantId,TransactionId,Error\n1234,1001,\"Exceeds the balance, 10.00\"\n",
        ),
    );
    let outcome = client(&api)
        .adjustments()
        .bulk_disbursement_funding(&common::adjustments())
        .unwrap();
    let BulkOutcome::Failed(report) = outcome else {
        panic!("expected a CSV report, got {outcome:?}");
    };
    assert_eq!(report.get(0, "Error"), Some("Exceeds the balance, 10.00"));
    assert_eq!(api.received().header("accept"), Some("*/*"));
}

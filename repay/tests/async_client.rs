//! The async client against a mock API.
#![cfg(feature = "reqwest")]

#[macro_use]
mod common;

use std::time::Duration;

use common::{Answer, MERCHANT_ID, MockApi, credentials, fixture};
use repay::{Client, Error};

fn client(api: &MockApi) -> Client {
    Client::builder(credentials())
        .environment(api.environment())
        .merchant_id(MERCHANT_ID)
        .build()
}

macro_rules! async_cases {
    ($(
        $name:ident: $resource:ident.$method:ident($($arg:expr),* $(,)?)
            => $http:literal $path:literal, $answer:expr;
    )*) => {$(
        #[tokio::test]
        async fn $name() {
            let api = MockApi::new_async($http, $path, $answer).await;
            client(&api).$resource().$method($($arg),*).await.unwrap();
            let received = api.received();
            assert_eq!(received.header("rg-merchant-id"), Some("1234"));
        }
    )*};
}

client_cases!(async_cases);

#[tokio::test]
async fn sends_json_with_its_content_type() {
    let api = MockApi::new_async(
        "POST",
        "/rgapi/v1/transactions/card/sale",
        Answer::json(200, "{}"),
    )
    .await;
    client(&api)
        .card_transactions()
        .sale(&common::card_sale())
        .await
        .unwrap();
    let received = api.received();
    assert_eq!(received.header("content-type"), Some("application/json"));
    assert_eq!(received.header("accept"), Some("application/json"));
    let body: serde_json::Value = serde_json::from_slice(&received.body).unwrap();
    assert_eq!(body["card_number"], "4111111111111111");
    assert_eq!(body["exp_date"], "1230");
}

#[tokio::test]
async fn uploads_files_as_multipart() {
    let api = MockApi::new_async(
        "POST",
        "/rgapi/v1/transactions/card/reversal/bulk",
        Answer::json(200, "{}"),
    )
    .await;
    let upload = repay::types::Upload::csv("reversals.csv", "TransactionId\n1001\n");
    client(&api)
        .card_transactions()
        .bulk_reversal(&upload)
        .await
        .unwrap();
    let received = api.received();
    assert_eq!(
        received.header("content-type"),
        Some("multipart/form-data; boundary=repay-boundary")
    );
    let body = String::from_utf8(received.body).unwrap();
    assert!(body.contains("filename=\"reversals.csv\""), "{body}");
}

#[tokio::test]
async fn an_error_response_is_an_api_error() {
    let api = MockApi::new_async(
        "GET",
        "/rgapi/v1/health",
        Answer::json(400, fixture("error_response.json")),
    )
    .await;
    let err = client(&api).health().check().await.unwrap_err();
    let api_err = err.as_api().expect("an API error");
    assert!(api_err.is_validation());
    assert_eq!(api_err.message(), "The request is invalid.");
}

#[tokio::test]
async fn an_html_error_page_is_a_status_error() {
    let api = MockApi::new_async(
        "GET",
        "/rgapi/v1/health",
        Answer::file("text/html", "<html>down</html>").status(503),
    )
    .await;
    let err = client(&api).health().check().await.unwrap_err();
    assert!(matches!(err, Error::Status { status: 503, .. }), "{err:?}");
}

#[tokio::test]
async fn a_slow_response_times_out() {
    let api = MockApi::new_async(
        "GET",
        "/rgapi/v1/health",
        Answer::json(200, r#""Healthy""#).delayed(Duration::from_secs(2)),
    )
    .await;
    let client = Client::builder(credentials())
        .environment(api.environment())
        .timeout(Duration::from_millis(200))
        .build();
    let err = client.health().check().await.unwrap_err();
    let Error::Transport(err) = err else {
        panic!("expected a transport error, got {err:?}");
    };
    assert!(err.is_timeout(), "{err}");
}

#[tokio::test]
async fn a_refused_connection_is_a_transport_error() {
    let client = Client::builder(credentials())
        .environment(repay::Environment::Custom(
            "http://127.0.0.1:1/rgapi".into(),
        ))
        .build();
    let err = client.health().check().await.unwrap_err();
    assert!(
        matches!(&err, Error::Transport(e) if !e.is_timeout()),
        "{err:?}"
    );
}

#[tokio::test]
async fn an_invalid_request_is_never_sent() {
    let api = MockApi::new_async(
        "POST",
        "/rgapi/v1/openbatches/card",
        Answer::json(200, "{}"),
    )
    .await;
    let client = client(&api).without_merchant_id();
    let err = client
        .batches()
        .close_card(&common::batch())
        .await
        .unwrap_err();
    assert!(matches!(err, Error::Validation(_)), "{err:?}");
    api.assert_not_called();
}

/// The futures can be spawned on a multi-threaded tokio runtime.
#[test]
fn futures_are_send() {
    fn assert_send<T: Send>(_: &T) {}
    let client = Client::new(credentials());
    let sale = common::card_sale();
    assert_send(&client.card_transactions().sale(&sale));
    let params = repay::operations::card_transactions::SearchParams::default();
    assert_send(
        &client
            .card_transactions()
            .search_export(&params, repay::ExportFormat::Csv),
    );
    assert_send(&client.ach_auth_forms().get_pdf(1));
}

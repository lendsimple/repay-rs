//! Decoding responses: JSON bodies, files, empty bodies, and each kind of error.

use repay::operations::{
    adjustments, batches, card_transactions, custom_fields, customers, fraud_velocity, health,
};
use repay::protocol;
use repay::schema::StatusCode;
use repay::transport::HttpResponse;
use repay::types::{MerchantId, TransactionId};
use repay::{Error, Export};

fn response(status: u16, content_type: Option<&str>, body: &str) -> HttpResponse {
    let headers = content_type
        .map(|ct| vec![("Content-Type".to_owned(), ct.to_owned())])
        .unwrap_or_default();
    HttpResponse::new(status, headers, body)
}

fn json(status: u16, body: &str) -> HttpResponse {
    response(status, Some("application/json; charset=utf-8"), body)
}

type Capture<'a> = card_transactions::Capture<'a>;

#[test]
fn a_transaction_detail() {
    let body = include_str!("data/responses/transaction_detail_card_response.json");
    let detail = protocol::decode::<Capture>(&json(200, body)).unwrap();
    assert_eq!(detail.transaction_id, Some(TransactionId::new(123456)));
    assert_eq!(detail.merchant_id, Some(MerchantId::new(1234)));
    assert_eq!(detail.result_text.as_deref(), Some("APPROVED"));
}

#[test]
fn a_201_is_a_success() {
    // Repay answers some declines with 201 and the transaction's details.
    let detail = protocol::decode::<Capture>(&json(201, r#"{"result_text": "DECLINED"}"#)).unwrap();
    assert_eq!(detail.result_text.as_deref(), Some("DECLINED"));
}

#[test]
fn the_batch_close_202_is_typed() {
    let body = include_str!("data/responses/close_card_settlement_batch_response.json");
    let closed = protocol::decode::<batches::CloseCard>(&json(202, body)).unwrap();
    assert_eq!(closed.batch_id, Some(3));
}

#[test]
fn a_list_response() {
    let fields = protocol::decode::<custom_fields::List>(&json(
        200,
        r#"[{"custom_field_id": 1}, {"custom_field_id": 2}]"#,
    ))
    .unwrap();
    assert_eq!(fields.len(), 2);
}

#[test]
fn an_error_response_is_an_api_error() {
    let body = include_str!("data/responses/error_response.json");
    let err = protocol::decode::<Capture>(&json(400, body)).unwrap_err();
    let api = err.as_api().expect("an API error");
    assert_eq!(api.status(), 400);
    assert_eq!(api.status_code(), Some(&StatusCode::BadRequest));
    assert!(api.is_validation());
    assert_eq!(api.validation_errors()[1].property.as_deref(), Some("zip"));
    assert_eq!(
        err.to_string(),
        "HTTP 400: The request is invalid. (2 validation errors)"
    );
}

#[test]
fn a_non_json_error_keeps_the_status_and_body() {
    let err = protocol::decode::<Capture>(&response(
        500,
        Some("text/html"),
        "<html>Server Error</html>",
    ))
    .unwrap_err();
    let Error::Status { status, body } = &err else {
        panic!("expected a status error, got {err:?}");
    };
    assert_eq!((*status, body.as_str()), (500, "<html>Server Error</html>"));
    assert_eq!(err.status(), Some(500));
}

#[test]
fn a_json_error_without_a_message_is_a_status_error() {
    let err = protocol::decode::<Capture>(&json(403, "{}")).unwrap_err();
    assert!(matches!(err, Error::Status { status: 403, .. }), "{err:?}");
}

#[test]
fn xml_on_a_json_operation_is_rejected() {
    let err =
        protocol::decode::<Capture>(&response(200, Some("application/xml"), "<a/>")).unwrap_err();
    assert!(
        matches!(&err, Error::UnexpectedContentType { status: 200, content_type } if content_type == "application/xml"),
        "{err:?}"
    );
}

#[test]
fn an_unreadable_body_is_a_decode_error() {
    let err = protocol::decode::<Capture>(&json(200, r#"{"transaction_id": "abc"}"#)).unwrap_err();
    let Error::Decode { status, body, .. } = &err else {
        panic!("expected a decode error, got {err:?}");
    };
    assert_eq!(
        (*status, body.as_str()),
        (200, r#"{"transaction_id": "abc"}"#)
    );
}

#[test]
fn empty_bodies() {
    // An operation without a response body ignores whatever comes back.
    protocol::decode::<fraud_velocity::Delete>(&response(204, None, "")).unwrap();
    // An open-ended JSON response reads an empty body as `null`.
    let null = protocol::decode::<customers::List>(&json(200, "")).unwrap();
    assert!(null.as_value().is_null());
}

#[test]
fn a_body_without_a_content_type_is_read_as_json() {
    let status = protocol::decode::<health::Check>(&response(200, None, "\"Healthy\"")).unwrap();
    assert_eq!(status, "Healthy");
}

#[test]
fn an_export_is_a_download() {
    let mut csv = response(200, Some("text/csv"), "transaction_id,amount\n1,19.99\n");
    csv.headers.push((
        "Content-Disposition".into(),
        r#"attachment; filename="transactions.csv""#.into(),
    ));
    type SearchExport<'a> = Export<'a, card_transactions::Search<'a>>;
    let file = protocol::decode::<SearchExport>(&csv).unwrap();
    assert_eq!(file.content_type, "text/csv");
    assert_eq!(file.filename.as_deref(), Some("transactions.csv"));
    assert_eq!(file.bytes, b"transaction_id,amount\n1,19.99\n");

    // JSON on an export (the API ignored the format) is kept as the file.
    let file = protocol::decode::<SearchExport>(&json(200, "[]")).unwrap();
    assert_eq!(file.content_type, "application/json");
    assert_eq!(file.bytes, b"[]");
}

#[test]
fn the_bulk_template_is_a_download() {
    let file = protocol::decode::<adjustments::BulkTemplate>(&response(
        200,
        Some("text/csv"),
        "MerchantId,TransactionId,OriginalAmount,AdjustmentAmount\n",
    ))
    .unwrap();
    assert_eq!(file.filename, None);
    assert!(file.bytes.starts_with(b"MerchantId,"));
}

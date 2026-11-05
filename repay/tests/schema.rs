//! The generated schema types: request JSON, lenient responses, enums and
//! redaction.

use repay::dec;
use repay::schema::{
    CardSaleRequest, CustomerBaseRequest, CustomerStatus, State, TransactionCustomField,
    TransactionDetailCardResponse,
};
use repay::types::{Amount, ExpDate, MerchantId, TransactionId};
use repay::validate::Validate;
use serde_json::json;

fn card_sale() -> CardSaleRequest {
    CardSaleRequest::builder()
        .amount(Amount::from(dec!(19.99)))
        .card_number("4111111111111111")
        .exp_date(ExpDate::new(12, 2030).unwrap())
        .cvv("123")
        .invoice_id("INV-1")
        .custom_fields(vec![
            TransactionCustomField::builder()
                .custom_field_name("loan")
                .value("L-1")
                .build(),
        ])
        .build()
}

#[test]
fn a_request_serializes_only_the_fields_that_are_set() {
    assert_eq!(
        serde_json::to_value(card_sale()).unwrap(),
        json!({
            "amount": 19.99,
            "card_number": "4111111111111111",
            "exp_date": "1230",
            "cvv": "123",
            "invoice_id": "INV-1",
            "custom_fields": [{"value": "L-1", "custom_field_name": "loan"}],
        })
    );
}

#[test]
fn enums_serialize_as_their_values() {
    let request = CustomerBaseRequest::builder()
        .customer_id("C-1")
        .status(CustomerStatus::Active)
        .build();
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        json!({"customer_id": "C-1", "status": "Active"})
    );
}

#[test]
fn a_request_checks_the_spec_constraints() {
    let request = CardSaleRequest::builder()
        .amount(Amount::from(dec!(1)))
        .name_on_card("<script>")
        .build();
    let error = request.validate().unwrap_err();
    assert_eq!(error.to_string(), "name_on_card: must not contain `<`");
}

#[test]
fn a_response_tolerates_nulls_and_unknown_fields() {
    let json = include_str!("data/transaction_detail_card_response.json");
    let response: TransactionDetailCardResponse = serde_json::from_str(json).unwrap();
    assert_eq!(response.transaction_id, Some(TransactionId::new(123456)));
    assert_eq!(response.merchant_id, Some(MerchantId::new(1234)));
    assert_eq!(response.auth_amount, Some(Amount::from(dec!(19.99))));
    assert_eq!(response.tip_amount, None);
    assert_eq!(response.custom_fields, None);
    assert_eq!(response.links, Some(Vec::new()));
    assert_eq!(
        response.timestamp.unwrap().as_str(),
        "2026-11-04T10:15:30.1234567"
    );
    assert_eq!(response.result_text.as_deref(), Some("APPROVED"));
}

#[test]
fn an_unknown_enum_value_is_kept() {
    let state: State = serde_json::from_str("\"ZZ\"").unwrap();
    assert_eq!(state, State::Other("ZZ".into()));
    let state: State = serde_json::from_str("\"MI\"").unwrap();
    assert_eq!(state, State::Mi);
}

#[test]
fn debug_redacts_card_data() {
    let debug = format!("{:?}", card_sale());
    assert!(!debug.contains("4111"), "{debug}");
    assert!(!debug.contains("1230"), "{debug}");
    assert!(debug.contains("cvv: [REDACTED]"), "{debug}");
    assert!(debug.contains("invoice_id: Some(\"INV-1\")"), "{debug}");
}

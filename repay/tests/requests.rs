//! Golden requests: the exact method, URL, headers and body each kind of
//! operation encodes to. Review changes with `cargo insta review`.

use std::collections::BTreeMap;

use repay::operations::{
    ach_auth_forms, ach_reporting, ach_tokens, ach_transactions, adjustments, batches, card_tokens,
    card_transactions, cash_transactions, contracts, custom_fields, customers, digital_wallet,
    digital_wallet_transactions, fraud_velocity, health, onboarding, reseller_reports,
    routing_rules, validation,
};
use repay::protocol::{self, Context};
use repay::schema::*;
use repay::transport::HttpRequest;
use repay::types::{Amount, DateTime, ExpDate, Upload};
use repay::{Credentials, Error, Export, ExportFormat, Operation, dec};

fn context() -> Context {
    Context::new(Credentials::new("api-user", "s3cret-token")).user_agent("repay-rs/test")
}

fn reseller() -> Context {
    context().merchant_id(1234)
}

/// The request as text: the request line, sorted headers (the secure token
/// replaced), and the body, pretty-printed when it is JSON.
fn render(request: &HttpRequest) -> String {
    let mut out = format!("{} {}\n", request.method, request.url);
    let mut headers = request.headers.clone();
    headers.sort();
    for (name, value) in headers {
        let value = if name == "rg-api-secure-token" {
            assert_eq!(value, "s3cret-token");
            "<token>".to_owned()
        } else {
            value
        };
        out.push_str(&format!("{name}: {value}\n"));
    }
    if !request.body.is_empty() {
        out.push('\n');
        if request.header("content-type") == Some("application/json") {
            let json: serde_json::Value = serde_json::from_slice(&request.body).unwrap();
            out.push_str(&serde_json::to_string_pretty(&json).unwrap());
        } else {
            out.push_str(&String::from_utf8_lossy(&request.body));
        }
        out.push('\n');
    }
    out
}

fn encode<O: Operation>(op: &O, context: &Context) -> String {
    render(&protocol::encode(op, context).unwrap())
}

fn date(text: &str) -> DateTime {
    DateTime::parse(text).unwrap()
}

#[test]
fn card_sale() {
    let body = CardSaleRequest::builder()
        .amount(Amount::from(dec!(19.99)))
        .card_number("4111111111111111")
        .exp_date(ExpDate::new(12, 2030).unwrap())
        .cvv("123")
        .invoice_id("INV-1")
        .surcharge(dec!(0.50).into())
        .build();
    insta::assert_snapshot!(encode(&card_transactions::Sale::new(&body), &reseller()));
}

#[test]
fn card_capture_has_a_typed_path_id() {
    let body = CardCaptureRequest::builder()
        .amount(Amount::from(dec!(10)))
        .build();
    insta::assert_snapshot!(encode(
        &card_transactions::Capture::new(987654, &body),
        &context()
    ));
}

#[test]
fn ach_void_takes_a_string_transaction_id() {
    insta::assert_snapshot!(encode(&ach_transactions::Void::new("ACH-55"), &context()));
}

#[test]
fn wallet_merchant_path_is_escaped_and_not_repeated_in_the_query() {
    let params = digital_wallet::GetMerchantParams::builder()
        .use_test_keys(true)
        .build();
    insta::assert_snapshot!(encode(
        &digital_wallet::GetMerchant::new("merchant one", &params),
        &context()
    ));
}

#[test]
fn card_search_sends_multi_and_date_parameters() {
    let params = card_transactions::SearchParams::builder()
        .start_date(date("2026-11-01T00:00:00"))
        .end_date(date("2026-11-04T23:59:59"))
        .card_holder("Ann & Bob")
        .custom_fields(vec!["loan=L-1".into(), "branch=7".into()])
        .take(50)
        .build();
    insta::assert_snapshot!(encode(&card_transactions::Search::new(&params), &context()));
}

#[test]
fn card_search_export() {
    let params = card_transactions::SearchParams::builder().take(10).build();
    let search = card_transactions::Search::new(&params);
    insta::assert_snapshot!(encode(&Export::new(&search, ExportFormat::Csv), &context()));
}

#[test]
fn query_by_date_has_an_enum_path_segment() {
    let body = CardTransactionsByDateQuery::builder()
        .start_date(date("2026-11-01"))
        .end_date(date("2026-11-04"))
        .build();
    let params = card_transactions::QueryByDateParams::default();
    insta::assert_snapshot!(encode(
        &card_transactions::QueryByDate::new(ReportType::SalesByDate, &body, &params),
        &context()
    ));
}

#[test]
fn ach_query_export_as_excel() {
    let body = AchTransactionQuery::builder()
        .start_date(date("2026-11-01"))
        .end_date(date("2026-11-04"))
        .build();
    let params = ach_reporting::QueryParams::builder()
        .skip(100)
        .take(50)
        .build();
    let query = ach_reporting::Query::new(&body, &params);
    insta::assert_snapshot!(encode(
        &Export::new(&query, ExportFormat::Excel),
        &context()
    ));
}

#[test]
fn auth_form_get_asks_for_json() {
    insta::assert_snapshot!(encode(&ach_auth_forms::Get::new(31), &context()));
}

#[test]
fn auth_form_get_export_asks_for_the_pdf() {
    let get = ach_auth_forms::Get::new(31);
    insta::assert_snapshot!(encode(&Export::new(&get, ExportFormat::Pdf), &context()));
}

#[test]
fn auth_form_find() {
    let params = ach_auth_forms::FindParams::builder()
        .transaction_id(42)
        .build();
    insta::assert_snapshot!(encode(&ach_auth_forms::Find::new(&params), &context()));
}

#[test]
fn bulk_reversal_uploads_a_csv() {
    let upload = Upload::csv("reversals.csv", "TransactionId\n1001\n1002\n");
    insta::assert_snapshot!(encode(
        &card_transactions::BulkReversal::new(&upload),
        &reseller()
    ));
}

#[test]
fn bulk_adjustment_uploads_the_documented_csv() {
    let mut adjustments = repay::bulk::BulkAdjustment::new();
    adjustments
        .row(
            1234,
            1001,
            Amount::from(dec!(100.00)),
            Amount::from(dec!(25.00)),
        )
        .row(1234, 1002, Amount::from(dec!(40)), Amount::from(dec!(40)));
    let upload = adjustments.to_upload().unwrap();
    insta::assert_snapshot!(encode(
        &adjustments::BulkDisbursementFunding::new(&upload),
        &context()
    ));
}

#[test]
fn health_check_has_no_body() {
    insta::assert_snapshot!(encode(&health::Check::new(), &context()));
}

#[test]
#[allow(deprecated)]
fn deprecated_reseller_query() {
    let body = CardTransactionQuery::builder().build();
    let params = reseller_reports::QueryTransactionsParams::default();
    insta::assert_snapshot!(encode(
        &reseller_reports::QueryTransactions::new(&body, &params),
        &reseller()
    ));
}

#[test]
fn card_transaction_list_never_asks_for_xml() {
    let params = card_transactions::ListParams::builder()
        .transaction_id("42")
        .take(5)
        .build();
    insta::assert_snapshot!(encode(&card_transactions::List::new(&params), &context()));
}

#[test]
fn card_token_create() {
    let body = CreateCardTokenRequest::builder()
        .card_number("4111111111111111")
        .exp_date(ExpDate::new(1, 2031).unwrap())
        .name_on_card("Ann Smith")
        .build();
    insta::assert_snapshot!(encode(&card_tokens::Create::new(&body), &context()));
}

#[test]
fn ach_token_patch() {
    let body = AchTokenBaseRequest::builder()
        .name_on_check("Ann Smith")
        .build();
    insta::assert_snapshot!(encode(&ach_tokens::Patch::new(77, &body), &context()));
}

#[test]
fn customer_card_token_update_has_two_path_ids() {
    let body = CreateCardTokenRequest::builder()
        .exp_date(ExpDate::new(2, 2032).unwrap())
        .build();
    insta::assert_snapshot!(encode(
        &customers::UpdateCardToken::new(5, 6, &body),
        &context()
    ));
}

#[test]
fn customer_list() {
    insta::assert_snapshot!(encode(&customers::List::new(), &reseller()));
}

#[test]
fn contract_create() {
    let body = ContractRequest::builder()
        .billing_period("Monthly")
        .billing_interval(1)
        .subtotal(Amount::from(dec!(25.00)))
        .customer_receipt(1)
        .build();
    insta::assert_snapshot!(encode(&contracts::Create::new(5, &body), &context()));
}

#[test]
fn ach_sale() {
    let body = AchSaleRequest::builder()
        .amount(Amount::from(dec!(100)))
        .ach_token("tok_ach")
        .build();
    insta::assert_snapshot!(encode(&ach_transactions::Sale::new(&body), &context()));
}

#[test]
fn nacha_threshold_report_export() {
    let params = ach_reporting::NachaThresholdReportParams::default();
    let report = ach_reporting::NachaThresholdReport::new(&params);
    insta::assert_snapshot!(encode(&Export::new(&report, ExportFormat::Csv), &context()));
}

#[test]
fn cash_sale() {
    let body = CashSaleRequest::builder()
        .amount(Amount::from(dec!(5)))
        .build();
    insta::assert_snapshot!(encode(&cash_transactions::Sale::new(&body), &reseller()));
}

#[test]
fn custom_field_list() {
    insta::assert_snapshot!(encode(&custom_fields::List::new(), &context()));
}

#[test]
fn apple_pay_sale() {
    let body = DigitalWalletRequest::builder()
        .authentication_payment_token("payment-token")
        .wallet_merchant_id("merchant-example")
        .build();
    insta::assert_snapshot!(encode(
        &digital_wallet_transactions::ApplePaySale::new(&body),
        &context()
    ));
}

#[test]
fn fraud_velocity_add_merchants_sends_a_json_array() {
    let merchants = vec![11, 12];
    insta::assert_snapshot!(encode(
        &fraud_velocity::AddMerchants::new("tmpl-1", &merchants),
        &context()
    ));
}

#[test]
fn fraud_velocity_delete() {
    insta::assert_snapshot!(encode(&fraud_velocity::Delete::new("tmpl-1"), &context()));
}

#[test]
fn routing_rule_rollback() {
    let body = TransRoutingRuleRollbackRequest::builder()
        .state_after_changes(false)
        .build();
    insta::assert_snapshot!(encode(
        &routing_rules::Rollback::new("8a0e0b2c-1d1f-4c55-9a43-2b2d1a8f0c11", &body),
        &context()
    ));
}

#[test]
fn ecash_merchant_create_sends_a_json_object() {
    let body = BTreeMap::from([("dba_name".to_owned(), "Example".to_owned())]);
    insta::assert_snapshot!(encode(
        &onboarding::CreateEcashMerchant::new("org-9", &body),
        &context()
    ));
}

#[test]
fn card_account_validation() {
    let body = CardAccountValidateRequest::builder()
        .primary_account_number("4111111111111111")
        .exp_date(ExpDate::new(12, 2030).unwrap())
        .build();
    insta::assert_snapshot!(encode(&validation::CardAccount::new(&body), &context()));
}

#[test]
fn batch_close_sends_the_merchant_id() {
    let body = CloseCardSettlementBatchRequest::builder()
        .batch_id(3)
        .build();
    insta::assert_snapshot!(encode(&batches::CloseCard::new(&body), &reseller()));
}

#[test]
fn disbursement_funding_adjustment() {
    let body = TransactionAdjustmentRequest::builder()
        .original_amount(Amount::from(dec!(10)))
        .adjustment_amount(Amount::from(dec!(-2.5)))
        .build();
    insta::assert_snapshot!(encode(
        &adjustments::DisbursementFunding::new(1001, &body),
        &context()
    ));
}

#[test]
fn the_api_version_and_environment_are_configurable() {
    let context = context()
        .api_version("2")
        .environment(repay::Environment::Custom(
            "http://localhost:8080/rgapi/".into(),
        ));
    let request = protocol::encode(&adjustments::BulkTemplate::new(), &context).unwrap();
    assert_eq!(
        request.url,
        "http://localhost:8080/rgapi/v2/adjustments/disbursementfunding/bulk/template"
    );
    assert_eq!(request.header("accept"), Some("*/*"));
}

#[test]
fn a_required_merchant_id_must_be_configured() {
    let body = CloseCardSettlementBatchRequest::builder()
        .batch_id(3)
        .build();
    let err = protocol::encode(&batches::CloseCard::new(&body), &context()).unwrap_err();
    let Error::Validation(err) = err else {
        panic!("expected a validation error, got {err:?}");
    };
    assert_eq!(err.to_string(), "rg-merchant-id: is required");
}

#[test]
fn an_invalid_body_or_empty_path_segment_is_not_sent() {
    let body = CardCaptureRequest::builder()
        .amount(Amount::from(dec!(1.005)))
        .build();
    let err = protocol::encode(&card_transactions::Capture::new(1, &body), &context()).unwrap_err();
    assert_eq!(
        err.to_string(),
        "invalid request: amount: must have at most 2 decimal places, not 1.005"
    );

    let err = protocol::encode(&ach_transactions::Void::new(""), &context()).unwrap_err();
    assert_eq!(
        err.to_string(),
        "invalid request: transaction_id: must not be empty"
    );

    // Without validation, the request is built as given.
    assert!(protocol::encode_unvalidated(&ach_transactions::Void::new(""), &context()).is_ok());
}

#[test]
fn operations_know_their_names() {
    assert_eq!(card_transactions::Sale::NAME, "card_transactions.sale");
    assert_eq!(
        card_transactions::Sale::ID,
        "CardTransactions_ProcessSaleAsync"
    );
    assert_eq!(
        <batches::CloseCard<'_> as Operation>::MERCHANT,
        repay::MerchantHeader::Required
    );
}

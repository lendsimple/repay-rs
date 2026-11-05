//! `#[derive(Model)]`: validation paths and messages, and redacted `Debug` output.

use repay::Model;
use repay::dec;
use repay::types::{Amount, ExpDate};
use repay::validate::Validate;

#[derive(Model)]
struct CustomField {
    #[repay(min_length = 1, max_length = 10)]
    name: String,
    #[repay(rule = "no_angle_bracket")]
    value: Option<String>,
}

#[derive(Model)]
struct Sale {
    amount: Amount,
    #[repay(sensitive)]
    card_number: String,
    #[repay(sensitive)]
    exp_date: Option<ExpDate>,
    #[repay(sensitive)]
    cvv: Option<String>,
    #[repay(required, max_length = 10)]
    zip: Option<String>,
    #[repay(min = "0", max = "2")]
    status: Option<i32>,
    #[repay(max_length = 3)]
    tags: Vec<String>,
    #[repay(rule = "wallet_merchant_id")]
    wallets: Option<Vec<String>>,
    custom_fields: Vec<CustomField>,
    #[repay(rule = "hostname")]
    r#type: Option<String>,
}

fn valid_sale() -> Sale {
    Sale {
        amount: Amount::from(dec!(19.99)),
        card_number: "4111111111111111".into(),
        exp_date: Some(ExpDate::new(12, 2030).unwrap()),
        cvv: None,
        zip: Some("49503".into()),
        status: Some(1),
        tags: vec!["web".into()],
        wallets: Some(vec!["merchant-1".into()]),
        custom_fields: vec![CustomField {
            name: "loan".into(),
            value: Some("L-1".into()),
        }],
        r#type: Some("pay.example.com".into()),
    }
}

fn messages(sale: &Sale) -> Vec<String> {
    match sale.validate() {
        Ok(()) => Vec::new(),
        Err(error) => error.violations().iter().map(ToString::to_string).collect(),
    }
}

#[test]
fn a_valid_value_passes() {
    assert!(valid_sale().validate().is_ok());
}

#[test]
fn every_violation_is_reported_with_its_path() {
    let mut sale = valid_sale();
    sale.amount = Amount::from(dec!(1.005));
    sale.zip = None;
    sale.status = Some(3);
    sale.tags = vec!["ok".into(), "toolong".into()];
    sale.wallets = Some(vec!["fine".into(), "has space".into()]);
    sale.custom_fields.push(CustomField {
        name: String::new(),
        value: Some("<b>".into()),
    });
    sale.r#type = Some("localhost".into());

    assert_eq!(
        messages(&sale),
        [
            "amount: must have at most 2 decimal places, not 1.005",
            "zip: is required",
            "status: must be at most 2, not 3",
            "tags[1]: must be at most 3 characters, not 7",
            "wallets[1]: must not contain whitespace or any of # % ^ & + * ( ) = [ ] { } : ' < > . / ? |",
            "custom_fields[1].name: must be at least 1 character, not 0",
            "custom_fields[1].value: must not contain `<`",
            "type: must be a domain name such as example.com",
        ]
    );
}

#[test]
fn missing_optional_values_are_not_checked() {
    let mut sale = valid_sale();
    sale.status = None;
    sale.wallets = None;
    sale.r#type = None;
    assert!(sale.validate().is_ok());
}

#[test]
fn debug_redacts_sensitive_fields() {
    let debug = format!("{:?}", valid_sale());
    assert!(!debug.contains("4111"), "{debug}");
    assert!(!debug.contains("1230"), "{debug}");
    assert!(debug.contains("card_number: [REDACTED]"), "{debug}");
    assert!(debug.contains("exp_date: [REDACTED]"), "{debug}");
    assert!(debug.contains("cvv: None"), "{debug}");
    assert!(debug.contains("zip: Some(\"49503\")"), "{debug}");
    assert!(debug.starts_with("Sale {"), "{debug}");
}

#[derive(Model)]
#[repay(one_of = "card_number exp_date | card_token")]
struct Payment {
    #[repay(sensitive)]
    card_number: Option<String>,
    exp_date: Option<ExpDate>,
    #[repay(max_length = 10)]
    card_token: Option<String>,
}

fn payment_messages(payment: &Payment) -> Vec<String> {
    match payment.validate() {
        Ok(()) => Vec::new(),
        Err(error) => error.violations().iter().map(ToString::to_string).collect(),
    }
}

#[test]
fn one_of_needs_exactly_one_complete_group() {
    let keyed = Payment {
        card_number: Some("4111111111111111".into()),
        exp_date: Some(ExpDate::new(12, 2030).unwrap()),
        card_token: None,
    };
    assert!(payment_messages(&keyed).is_empty());

    let neither = Payment {
        card_number: None,
        exp_date: None,
        card_token: None,
    };
    assert_eq!(
        payment_messages(&neither),
        ["set exactly one of card_number or card_token"]
    );

    let half_keyed_and_long_token = Payment {
        card_number: Some("4111111111111111".into()),
        exp_date: None,
        card_token: Some("a-very-long-token".into()),
    };
    assert_eq!(
        payment_messages(&half_keyed_and_long_token),
        [
            "card_token: must be at most 10 characters, not 17",
            "set exactly one of card_number or card_token, not card_number and card_token",
        ]
    );
}

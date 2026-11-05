//! Hand-written shortcuts for the most common requests.
//!
//! Each builds a complete request with one payment source, which is what the
//! spec's `one_of` checks require. Fields stay public, so a request made here can
//! still be adjusted (`sale.invoice_id = Some(..)`).

use std::fmt;

use super::{
    AchCreditPrenoteRequest, AchCreditRequest, AchDebitPrenoteRequest, AchRebillRequest,
    AchSaleRequest, CardAuthRequest, CardDisbursementRequest, CardSaleRequest,
    UnlinkedCardRefundRequest,
};
use crate::types::{Amount, ExpDate};

/// `keyed`, `token` and `device` constructors for card requests with an amount.
macro_rules! card_sources {
    ($($ty:ident: $($source:ident),+;)*) => {$(
        impl $ty {
            $(card_sources!(@$source $ty);)+
        }
    )*};

    (@keyed $ty:ident) => {
        /// A card entered by number and expiration date.
        pub fn keyed(amount: Amount, card_number: impl Into<String>, exp_date: ExpDate) -> Self {
            Self::builder()
                .amount(amount)
                .card_number(card_number)
                .exp_date(exp_date)
                .build()
        }
    };
    (@token $ty:ident) => {
        /// A stored card token.
        pub fn token(amount: Amount, card_token: impl Into<String>) -> Self {
            Self::builder().amount(amount).card_token(card_token).build()
        }
    };
    (@device $ty:ident) => {
        /// Encrypted card data from a payment device, in place of the card number,
        /// expiration date and CVV.
        pub fn device(amount: Amount, device_card_data: impl Into<String>) -> Self {
            Self::builder()
                .amount(amount)
                .device_card_data(device_card_data)
                .build()
        }
    };
    (@cvv $ty:ident) => {
        /// Adds the card's CVV.
        pub fn with_cvv(mut self, cvv: impl Into<String>) -> Self {
            self.cvv = Some(cvv.into());
            self
        }
    };
}

card_sources! {
    CardSaleRequest: keyed, token, device, cvv;
    CardAuthRequest: keyed, token, device, cvv;
    UnlinkedCardRefundRequest: keyed, token;
    CardDisbursementRequest: keyed, token, device;
}

/// A bank account for an ACH request.
///
/// `Debug` output hides the account number.
#[derive(Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct AchAccount {
    pub account_number: String,
    /// The routing number.
    pub transit_number: String,
    pub name_on_check: String,
    /// Such as `CHECKING` or `SAVINGS`.
    pub account_type: Option<String>,
    /// Such as `PERSONAL` or `BUSINESS`.
    pub check_type: Option<String>,
}

impl AchAccount {
    pub fn new(
        account_number: impl Into<String>,
        transit_number: impl Into<String>,
        name_on_check: impl Into<String>,
    ) -> Self {
        Self {
            account_number: account_number.into(),
            transit_number: transit_number.into(),
            name_on_check: name_on_check.into(),
            account_type: None,
            check_type: None,
        }
    }

    /// The account type, such as `CHECKING` or `SAVINGS`.
    pub fn with_account_type(mut self, account_type: impl Into<String>) -> Self {
        self.account_type = Some(account_type.into());
        self
    }

    /// The check type, such as `PERSONAL` or `BUSINESS`.
    pub fn with_check_type(mut self, check_type: impl Into<String>) -> Self {
        self.check_type = Some(check_type.into());
        self
    }
}

impl fmt::Debug for AchAccount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AchAccount")
            .field("account_number", &crate::__private::Redacted::new(true))
            .field("transit_number", &self.transit_number)
            .field("name_on_check", &self.name_on_check)
            .field("account_type", &self.account_type)
            .field("check_type", &self.check_type)
            .finish()
    }
}

/// `account` and `token` constructors for ACH requests, with or without an amount.
macro_rules! ach_sources {
    ($($ty:ident $(($amount:ident))?;)*) => {$(
        impl $ty {
            /// A bank account.
            pub fn account($($amount: Amount,)? account: AchAccount) -> Self {
                let mut request = Self::builder()
                    $(.amount($amount))?
                    .account_number(account.account_number)
                    .transit_number(account.transit_number)
                    .name_on_check(account.name_on_check)
                    .build();
                request.account_type = account.account_type;
                request.check_type = account.check_type;
                request
            }

            /// A stored ACH token.
            pub fn token($($amount: Amount,)? ach_token: impl Into<String>) -> Self {
                Self::builder()$(.amount($amount))?.ach_token(ach_token).build()
            }
        }
    )*};
}

ach_sources! {
    AchSaleRequest(amount);
    AchCreditRequest(amount);
    AchRebillRequest(amount);
    AchDebitPrenoteRequest;
    AchCreditPrenoteRequest;
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::dec;
    use crate::validate::Validate;

    fn amount() -> Amount {
        Amount::from(dec!(5))
    }

    #[test]
    fn card_shortcuts_set_one_valid_source() {
        let exp = ExpDate::new(12, 2030).unwrap();
        let keyed = CardSaleRequest::keyed(amount(), "4111111111111111", exp).with_cvv("123");
        assert_eq!(
            serde_json::to_value(&keyed).unwrap(),
            json!({"amount": 5.0, "card_number": "4111111111111111", "exp_date": "1230", "cvv": "123"})
        );
        let requests = [
            keyed,
            CardSaleRequest::token(amount(), "tok"),
            CardSaleRequest::device(amount(), "data"),
        ];
        for request in &requests {
            assert!(request.validate().is_ok(), "{request:?}");
        }
        assert!(CardAuthRequest::token(amount(), "tok").validate().is_ok());
        assert!(
            UnlinkedCardRefundRequest::keyed(amount(), "4111", exp)
                .validate()
                .is_ok()
        );
        assert!(
            CardDisbursementRequest::device(amount(), "d")
                .validate()
                .is_ok()
        );
    }

    #[test]
    fn ach_shortcuts_set_one_valid_source() {
        let account =
            AchAccount::new("123456789", "021000021", "Ann Smith").with_account_type("CHECKING");
        let sale = AchSaleRequest::account(amount(), account.clone());
        assert_eq!(
            serde_json::to_value(&sale).unwrap(),
            json!({"amount": 5.0, "account_number": "123456789", "transit_number": "021000021",
                   "name_on_check": "Ann Smith", "account_type": "CHECKING"})
        );
        assert!(sale.validate().is_ok());
        assert!(
            AchCreditRequest::token(amount(), "ach_tok")
                .validate()
                .is_ok()
        );
        assert!(AchDebitPrenoteRequest::account(account).validate().is_ok());
        assert!(AchCreditPrenoteRequest::token("ach_tok").validate().is_ok());
        assert!(
            AchRebillRequest::token(amount(), "ach_tok")
                .validate()
                .is_ok()
        );
    }

    #[test]
    fn shortcut_results_can_be_adjusted() {
        let mut sale = CardSaleRequest::token(amount(), "tok_4f2a").with_cvv("123");
        sale.invoice_id = Some("INV-1".into());
        assert!(sale.validate().is_ok());
        assert_eq!(sale.invoice_id.as_deref(), Some("INV-1"));
    }

    #[test]
    fn ach_account_debug_hides_the_account_number() {
        let debug = format!("{:?}", AchAccount::new("123456789", "021000021", "Ann"));
        assert!(!debug.contains("123456789"), "{debug}");
        assert!(debug.contains("021000021"), "{debug}");
    }
}

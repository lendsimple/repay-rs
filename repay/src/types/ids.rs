//! Typed IDs, so that a customer key cannot be passed where a card token key is
//! expected.

use std::fmt;

use serde::Deserializer;
use serde::de::{self, Visitor};

use crate::macros::{int_id, string_id};

int_id! {
    /// The ID of a transaction.
    TransactionId;
    /// The key of a customer.
    CustomerKey;
    /// The key of a stored card token.
    CardTokenKey;
    /// The key of a stored ACH token.
    AchTokenKey;
    /// The key of a customer's contract.
    ContractKey;
    /// The ID of an ACH authorization form.
    AuthFormId;
    /// The ID of an intelligent routing rule.
    RuleId;
    /// The ID of a merchant's custom field.
    CustomFieldId;
    /// The ID of a merchant.
    MerchantId;
}

string_id! {
    /// The ID of a fraud velocity template.
    TemplateId;
    /// The ID of an organization in payment services onboarding.
    OrgId;
    /// The key of a routing configuration audit entry, a UUID.
    AuditKey;
}

/// Reads an `i32` from a JSON number or a string of digits.
pub(crate) fn deserialize_i32<'de, D: Deserializer<'de>>(deserializer: D) -> Result<i32, D::Error> {
    deserializer.deserialize_any(I32Visitor)
}

struct I32Visitor;

impl Visitor<'_> for I32Visitor {
    type Value = i32;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("an ID, as a number or a string of digits")
    }

    fn visit_i64<E: de::Error>(self, value: i64) -> Result<i32, E> {
        i32::try_from(value).map_err(|_| E::custom(format!("ID {value} is out of range")))
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> Result<i32, E> {
        i32::try_from(value).map_err(|_| E::custom(format!("ID {value} is out of range")))
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<i32, E> {
        value
            .trim()
            .parse()
            .map_err(|_| E::custom(format!("invalid ID {value:?}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_numbers_and_strings() {
        let number: TransactionId = serde_json::from_str("123").unwrap();
        let string: TransactionId = serde_json::from_str("\"123\"").unwrap();
        assert_eq!(number, TransactionId::new(123));
        assert_eq!(string, number);
    }

    #[test]
    fn writes_numbers() {
        assert_eq!(serde_json::to_string(&CustomerKey::new(42)).unwrap(), "42");
        assert_eq!(CustomerKey::new(42).to_string(), "42");
    }

    #[test]
    fn rejects_other_values() {
        for json in ["\"abc\"", "1.5", "true", "9999999999", "-9999999999"] {
            assert!(
                serde_json::from_str::<CardTokenKey>(json).is_err(),
                "{json}"
            );
        }
    }

    #[test]
    fn string_ids_are_transparent() {
        let id: TemplateId = serde_json::from_str("\"tmpl-1\"").unwrap();
        assert_eq!(id.as_str(), "tmpl-1");
        assert_eq!(serde_json::to_string(&id).unwrap(), "\"tmpl-1\"");
    }
}

//! Money amounts.

use std::fmt;
use std::str::FromStr;

use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;
use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::validate::{Numeric, Path, Validate, Violation, violation};

/// The largest number of cents a JSON number (an IEEE double) holds exactly: 2^53.
const MAX_CENTS: i64 = 1 << 53;

/// An amount of money, held exactly as a [`Decimal`].
///
/// The API's amounts are JSON numbers. An `Amount` is sent as one, and is read from a
/// number or a string without the rounding a float would add: `19.99` reads as
/// exactly `19.99`. Validation checks that it has at most 2 decimal places.
///
/// ```
/// use repay::{dec, types::Amount};
///
/// let amount = Amount::from(dec!(19.99));
/// assert_eq!(amount, Amount::from_cents(1999));
/// assert_eq!(serde_json::to_string(&amount).unwrap(), "19.99");
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Amount(Decimal);

/// An amount the API sends and reads as a string, such as `"1.50"`.
///
/// Used for the few amount fields the schema declares as strings, such as
/// `surcharge`. It otherwise behaves like [`Amount`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AmountString(Decimal);

/// The text is not a decimal number.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid amount {0:?}: expected a decimal number such as 19.99")]
pub struct AmountError(String);

macro_rules! amount_api {
    ($name:ident) => {
        impl $name {
            /// `num` scaled down by `scale` decimal places: `new(1999, 2)` is 19.99.
            pub fn new(num: i64, scale: u32) -> Self {
                Self(Decimal::new(num, scale))
            }

            /// An amount of whole cents: `from_cents(1999)` is 19.99.
            pub fn from_cents(cents: i64) -> Self {
                Self(Decimal::new(cents, 2))
            }

            /// The amount as a [`Decimal`].
            pub fn as_decimal(&self) -> Decimal {
                self.0
            }
        }

        impl From<Decimal> for $name {
            fn from(value: Decimal) -> Self {
                Self(value)
            }
        }

        impl From<$name> for Decimal {
            fn from(value: $name) -> Self {
                value.0
            }
        }

        impl FromStr for $name {
            type Err = AmountError;

            fn from_str(text: &str) -> Result<Self, Self::Err> {
                parse(text).map(Self)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(&self.0, f)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                deserializer.deserialize_any(AmountVisitor).map(Self)
            }
        }

        impl Numeric for $name {
            fn to_decimal(&self) -> Decimal {
                self.0
            }
        }

        impl Validate for $name {
            fn validate_into(&self, path: &mut Path, out: &mut Vec<Violation>) {
                validate_amount(self.0, path, out);
            }
        }
    };
}

amount_api!(Amount);
amount_api!(AmountString);

impl Serialize for Amount {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        // A value of at most 2 decimal places and 2^53 cents converts exactly, and is
        // written in its shortest form (19.99, not 19.989999999999998).
        let value = self.0.to_f64().ok_or_else(|| {
            serde::ser::Error::custom(format!("amount {} does not fit a JSON number", self.0))
        })?;
        serializer.serialize_f64(value)
    }
}

impl Serialize for AmountString {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(&self.0)
    }
}

fn parse(text: &str) -> Result<Decimal, AmountError> {
    let trimmed = text.trim();
    Decimal::from_str(trimmed)
        .or_else(|_| Decimal::from_scientific(trimmed))
        .map_err(|_| AmountError(text.into()))
}

fn validate_amount(value: Decimal, path: &Path, out: &mut Vec<Violation>) {
    let normalized = value.normalize();
    if normalized.scale() > 2 {
        violation(
            path,
            out,
            format!("must have at most 2 decimal places, not {normalized}"),
        );
    } else if (normalized * Decimal::ONE_HUNDRED).abs() > Decimal::from(MAX_CENTS) {
        violation(path, out, format!("is too large: {normalized}"));
    }
}

struct AmountVisitor;

impl Visitor<'_> for AmountVisitor {
    type Value = Decimal;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("an amount, as a number or a string")
    }

    fn visit_i64<E: de::Error>(self, value: i64) -> Result<Decimal, E> {
        Ok(Decimal::from(value))
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Decimal, E> {
        Ok(Decimal::from(value))
    }

    fn visit_f64<E: de::Error>(self, value: f64) -> Result<Decimal, E> {
        if !value.is_finite() {
            return Err(E::custom(format!("invalid amount {value}")));
        }
        // `Display` writes the shortest text that reads back as the same double, which
        // for a JSON number such as 19.99 is the text that was sent.
        parse(&value.to_string()).map_err(E::custom)
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<Decimal, E> {
        parse(value).map_err(E::custom)
    }
}

#[cfg(test)]
mod tests {
    use rust_decimal::dec;

    use super::*;

    fn read(json: &str) -> Amount {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn numbers_read_exactly() {
        assert_eq!(read("19.99").as_decimal(), dec!(19.99));
        assert_eq!(read("0.1").as_decimal(), dec!(0.1));
        assert_eq!(read("0.3").as_decimal(), dec!(0.3));
        assert_eq!(read("45").as_decimal(), dec!(45));
        assert_eq!(read("-5.25").as_decimal(), dec!(-5.25));
        assert_eq!(read("1234567.89").as_decimal(), dec!(1234567.89));
    }

    #[test]
    fn strings_read_as_amounts() {
        assert_eq!(read("\"19.99\"").as_decimal(), dec!(19.99));
        assert_eq!(read("\" 7 \"").as_decimal(), dec!(7));
        assert!(serde_json::from_str::<Amount>("\"abc\"").is_err());
        assert!(serde_json::from_str::<Amount>("\"\"").is_err());
        assert!(serde_json::from_str::<Amount>("true").is_err());
    }

    #[test]
    fn amounts_write_as_short_numbers() {
        let write = |amount: Amount| serde_json::to_string(&amount).unwrap();
        assert_eq!(write(Amount::from_cents(1999)), "19.99");
        assert_eq!(write(Amount::from(dec!(0.1))), "0.1");
        assert_eq!(write(Amount::from(dec!(45.00))), "45.0");
        assert_eq!(write(Amount::from_cents(-525)), "-5.25");
    }

    #[test]
    fn amount_strings_keep_their_scale() {
        let surcharge = AmountString::from(dec!(1.50));
        assert_eq!(serde_json::to_string(&surcharge).unwrap(), "\"1.50\"");
        let read: AmountString = serde_json::from_str("2.5").unwrap();
        assert_eq!(read.as_decimal(), dec!(2.5));
    }

    #[test]
    fn validation_allows_two_decimal_places() {
        assert!(Amount::from(dec!(19.99)).validate().is_ok());
        assert!(Amount::from(dec!(19.990)).validate().is_ok());
        let error = Amount::from(dec!(19.999)).validate().unwrap_err();
        assert_eq!(
            error.to_string(),
            "must have at most 2 decimal places, not 19.999"
        );
    }

    #[test]
    fn validation_rejects_amounts_a_json_number_cannot_hold() {
        assert!(Amount::from_cents(MAX_CENTS).validate().is_ok());
        assert!(Amount::from_cents(MAX_CENTS + 1).validate().is_err());
        assert!(Amount::from_cents(-MAX_CENTS - 1).validate().is_err());
    }

    #[test]
    fn non_finite_numbers_are_rejected() {
        use serde::de::value::{Error, F64Deserializer};
        for value in [f64::NAN, f64::INFINITY] {
            assert!(Amount::deserialize(F64Deserializer::<Error>::new(value)).is_err());
        }
    }

    #[test]
    fn parses_from_text() {
        assert_eq!("19.99".parse::<Amount>().unwrap(), Amount::from_cents(1999));
        assert_eq!(
            "12,00".parse::<Amount>().unwrap_err().to_string(),
            "invalid amount \"12,00\": expected a decimal number such as 19.99"
        );
    }
}

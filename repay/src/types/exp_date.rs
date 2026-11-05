//! Card expiration dates.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A card's expiration month, sent as `MMYY` (December 2030 is `1230`).
///
/// ```
/// use repay::types::ExpDate;
///
/// let exp = ExpDate::new(12, 2030).unwrap();
/// assert_eq!(exp.to_string(), "1230");
/// assert_eq!("12/30".parse::<ExpDate>().unwrap(), exp);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExpDate {
    // Ordered year first, so that comparisons are chronological.
    year: u16,
    month: u8,
}

/// The month or year is out of range, or the text is not `MMYY` or `MM/YY`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ExpDateError {
    /// The month is not 1 to 12.
    #[error("invalid expiration month {0}: expected 1 to 12")]
    Month(u8),
    /// The year is not 2000 to 2099.
    #[error("invalid expiration year {0}: expected 2000 to 2099")]
    Year(u16),
    /// The text is not `MMYY` or `MM/YY`.
    #[error("invalid expiration date {0:?}: expected MMYY or MM/YY, such as 1230")]
    Format(String),
}

impl ExpDate {
    /// The given month (1 to 12) of the given year (2000 to 2099).
    pub fn new(month: u8, year: u16) -> Result<Self, ExpDateError> {
        if !(1..=12).contains(&month) {
            return Err(ExpDateError::Month(month));
        }
        if !(2000..=2099).contains(&year) {
            return Err(ExpDateError::Year(year));
        }
        Ok(Self { year, month })
    }

    /// The month, 1 to 12.
    pub fn month(&self) -> u8 {
        self.month
    }

    /// The year, such as 2030.
    pub fn year(&self) -> u16 {
        self.year
    }
}

impl FromStr for ExpDate {
    type Err = ExpDateError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let format = || ExpDateError::Format(text.into());
        let (month, year) = match text.split_once('/') {
            Some(parts) => parts,
            None if text.len() == 4 && text.is_char_boundary(2) => text.split_at(2),
            None => return Err(format()),
        };
        let two_digits = |part: &str| -> Result<u8, ExpDateError> {
            if part.len() == 2 && part.bytes().all(|b| b.is_ascii_digit()) {
                Ok(part.parse().expect("two ASCII digits"))
            } else {
                Err(format())
            }
        };
        Self::new(two_digits(month)?, 2000 + u16::from(two_digits(year)?))
    }
}

impl fmt::Display for ExpDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:02}{:02}", self.month, self.year % 100)
    }
}

impl Serialize for ExpDate {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for ExpDate {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}

crate::validate::no_constraints!(ExpDate);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_mmyy_and_mm_slash_yy() {
        let exp = ExpDate::new(12, 2030).unwrap();
        assert_eq!("1230".parse::<ExpDate>().unwrap(), exp);
        assert_eq!("12/30".parse::<ExpDate>().unwrap(), exp);
        assert_eq!("0127".parse::<ExpDate>().unwrap().month(), 1);
    }

    #[test]
    fn writes_mmyy() {
        let exp = ExpDate::new(3, 2031).unwrap();
        assert_eq!(serde_json::to_string(&exp).unwrap(), "\"0331\"");
        let read: ExpDate = serde_json::from_str("\"0331\"").unwrap();
        assert_eq!(read, exp);
    }

    #[test]
    fn rejects_out_of_range_values() {
        assert_eq!(ExpDate::new(13, 2030), Err(ExpDateError::Month(13)));
        assert_eq!(ExpDate::new(0, 2030), Err(ExpDateError::Month(0)));
        assert_eq!(ExpDate::new(1, 1999), Err(ExpDateError::Year(1999)));
        assert_eq!("1330".parse::<ExpDate>(), Err(ExpDateError::Month(13)));
    }

    #[test]
    fn rejects_other_text() {
        for text in [
            "", "123", "12345", "12/2030", "1/30", "ab/cd", "12-30", "１２30",
        ] {
            assert!(text.parse::<ExpDate>().is_err(), "{text}");
        }
    }

    #[test]
    fn orders_chronologically() {
        let dec_2030 = ExpDate::new(12, 2030).unwrap();
        let jan_2031 = ExpDate::new(1, 2031).unwrap();
        assert!(dec_2030 < jan_2031);
    }
}

//! Dates and times as the API writes them.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use time::format_description::BorrowedFormatItem;
use time::format_description::well_known::Rfc3339;
use time::macros::format_description;
use time::{Date, OffsetDateTime, PrimitiveDateTime, Time, UtcOffset};

const DATE: &[BorrowedFormatItem<'_>] = format_description!("[year]-[month]-[day]");
const LOCAL: &[BorrowedFormatItem<'_>] =
    format_description!("[year]-[month]-[day]T[hour]:[minute]:[second][optional [.[subsecond]]]");
const LOCAL_OUT: &[BorrowedFormatItem<'_>] =
    format_description!("[year]-[month]-[day]T[hour]:[minute]:[second]");
const LOCAL_OUT_SUBSECOND: &[BorrowedFormatItem<'_>] =
    format_description!("[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond]");

/// A date and time, with the offset from UTC when the API gives one.
///
/// The API is written in .NET, whose dates may or may not carry an offset:
/// `2026-11-04T15:01:00Z`, `2026-11-04T15:01:00-05:00` and
/// `2026-11-04T15:01:00.1234567` all occur. A `DateTime` reads each of these, and a
/// date alone (`2026-11-04`, as midnight). It keeps the text it was read from and
/// writes that text back unchanged.
///
/// ```
/// use repay::types::DateTime;
///
/// let local = DateTime::parse("2026-11-04T15:01:00.1234567").unwrap();
/// assert_eq!(local.offset(), None);
/// assert_eq!(local.as_str(), "2026-11-04T15:01:00.1234567");
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DateTime {
    datetime: PrimitiveDateTime,
    offset: Option<UtcOffset>,
    lexical: String,
}

/// The text is not a date and time the API uses.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
    "invalid date and time {0:?}: expected a date such as 2026-11-04, \
     optionally with a time such as T15:01:00 and an offset such as Z or -05:00"
)]
pub struct DateTimeError(String);

impl DateTime {
    /// Reads a date and time in any of the forms the API uses.
    pub fn parse(text: &str) -> Result<Self, DateTimeError> {
        let lexical = text.to_owned();
        if let Ok(value) = OffsetDateTime::parse(text, &Rfc3339) {
            return Ok(Self {
                datetime: PrimitiveDateTime::new(value.date(), value.time()),
                offset: Some(value.offset()),
                lexical,
            });
        }
        let datetime = PrimitiveDateTime::parse(text, LOCAL)
            .or_else(|_| Date::parse(text, DATE).map(|date| date.with_time(Time::MIDNIGHT)))
            .map_err(|_| DateTimeError(lexical.clone()))?;
        Ok(Self {
            datetime,
            offset: None,
            lexical,
        })
    }

    /// The date and time, without its offset.
    pub fn datetime(&self) -> PrimitiveDateTime {
        self.datetime
    }

    /// The offset from UTC, if the API gave one.
    pub fn offset(&self) -> Option<UtcOffset> {
        self.offset
    }

    /// The date and time with its offset, if the API gave one.
    pub fn to_offset_date_time(&self) -> Option<OffsetDateTime> {
        self.offset
            .map(|offset| self.datetime.assume_offset(offset))
    }

    /// The text as the API wrote it.
    pub fn as_str(&self) -> &str {
        &self.lexical
    }
}

impl From<OffsetDateTime> for DateTime {
    /// Writes the value as RFC 3339, such as `2026-11-04T15:01:00Z`.
    fn from(value: OffsetDateTime) -> Self {
        Self {
            datetime: PrimitiveDateTime::new(value.date(), value.time()),
            offset: Some(value.offset()),
            lexical: value
                .format(&Rfc3339)
                .expect("an OffsetDateTime with a four-digit year formats as RFC 3339"),
        }
    }
}

impl From<PrimitiveDateTime> for DateTime {
    /// Writes the value with no offset, such as `2026-11-04T15:01:00`, with
    /// fractional seconds only when it has them.
    fn from(value: PrimitiveDateTime) -> Self {
        let format = if value.nanosecond() == 0 {
            LOCAL_OUT
        } else {
            LOCAL_OUT_SUBSECOND
        };
        Self {
            datetime: value,
            offset: None,
            lexical: value
                .format(format)
                .expect("a PrimitiveDateTime with a four-digit year formats"),
        }
    }
}

impl From<Date> for DateTime {
    /// Midnight at the start of the date, with no offset.
    fn from(value: Date) -> Self {
        Self::from(value.with_time(Time::MIDNIGHT))
    }
}

impl std::str::FromStr for DateTime {
    type Err = DateTimeError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::parse(text)
    }
}

impl fmt::Display for DateTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.lexical)
    }
}

impl Serialize for DateTime {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.lexical)
    }
}

impl<'de> Deserialize<'de> for DateTime {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::parse(&text).map_err(serde::de::Error::custom)
    }
}

crate::validate::no_constraints!(DateTime);

#[cfg(test)]
mod tests {
    use time::macros::{date, datetime, offset};

    use super::*;

    #[test]
    fn reads_utc_and_offsets() {
        let utc = DateTime::parse("2026-11-04T15:01:00Z").unwrap();
        assert_eq!(utc.datetime(), datetime!(2026-11-04 15:01:00));
        assert_eq!(utc.offset(), Some(offset!(UTC)));

        let eastern = DateTime::parse("2026-11-04T15:01:00-05:00").unwrap();
        assert_eq!(
            eastern.to_offset_date_time(),
            Some(datetime!(2026-11-04 15:01:00 -05:00))
        );
    }

    #[test]
    fn reads_dotnet_local_times() {
        let local = DateTime::parse("2026-11-04T15:01:00").unwrap();
        assert_eq!(local.datetime(), datetime!(2026-11-04 15:01:00));
        assert_eq!(local.offset(), None);
        assert_eq!(local.to_offset_date_time(), None);

        let ticks = DateTime::parse("2026-11-04T15:01:00.1234567").unwrap();
        assert_eq!(ticks.datetime(), datetime!(2026-11-04 15:01:00.1234567));
    }

    #[test]
    fn reads_a_date_alone_as_midnight() {
        let day = DateTime::parse("2026-11-04").unwrap();
        assert_eq!(day.datetime(), datetime!(2026-11-04 0:00));
    }

    #[test]
    fn writes_back_the_text_it_read() {
        for text in [
            "2026-11-04T15:01:00Z",
            "2026-11-04T15:01:00.1234567",
            "2026-11-04T15:01:00+00:00",
            "2026-11-04",
        ] {
            let json = format!("\"{text}\"");
            let value: DateTime = serde_json::from_str(&json).unwrap();
            assert_eq!(serde_json::to_string(&value).unwrap(), json);
        }
    }

    #[test]
    fn writes_constructed_values() {
        let utc = DateTime::from(datetime!(2026-11-04 15:01:00 UTC));
        assert_eq!(utc.as_str(), "2026-11-04T15:01:00Z");
        let local = DateTime::from(datetime!(2026-11-04 15:01:00));
        assert_eq!(local.as_str(), "2026-11-04T15:01:00");
        let precise = DateTime::from(datetime!(2026-11-04 15:01:00.25));
        assert_eq!(precise.as_str(), "2026-11-04T15:01:00.25");
        let day = DateTime::from(date!(2026 - 11 - 04));
        assert_eq!(day.as_str(), "2026-11-04T00:00:00");
    }

    #[test]
    fn rejects_other_text() {
        for text in [
            "",
            "yesterday",
            "11/04/2026",
            "2026-13-01",
            "2026-11-04T25:00:00",
        ] {
            assert!(DateTime::parse(text).is_err(), "{text}");
        }
        assert!(serde_json::from_str::<DateTime>("\"soon\"").is_err());
    }
}

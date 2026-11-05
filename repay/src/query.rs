//! Path and query string encoding.

use std::fmt::{self, Display, Write as _};

use crate::types::DateTime;

/// Appends `value` to `out`, percent-encoding everything but RFC 3986's
/// unreserved characters (`A-Z a-z 0-9 - . _ ~`).
pub(crate) fn percent_encode(value: &str, out: &mut String) {
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(char::from(byte));
        } else {
            write!(out, "%{byte:02X}").expect("writing to a String");
        }
    }
}

/// Decodes `%XX` escapes; invalid escapes are kept as they are.
pub(crate) fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = |b: u8| char::from(b).to_digit(16);
        match (bytes[i], bytes.get(i + 1), bytes.get(i + 2)) {
            (b'%', Some(&h), Some(&l)) if hex(h).is_some() && hex(l).is_some() => {
                let byte = hex(h).expect("checked") * 16 + hex(l).expect("checked");
                out.push(u8::try_from(byte).expect("two hex digits fit in a byte"));
                i += 3;
            }
            (byte, _, _) => {
                out.push(byte);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Fills in a path template's `{name}` placeholders, percent-encoding each value
/// as one segment.
pub(crate) fn render_path(template: &str, params: &[(&str, &dyn Display)]) -> String {
    let mut out = String::with_capacity(template.len() + 16);
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let end = rest[start..]
            .find('}')
            .map(|i| start + i)
            .expect("generated templates close every placeholder");
        let name = &rest[start + 1..end];
        let (_, value) = params
            .iter()
            .find(|(n, _)| *n == name)
            .expect("generated operations supply every placeholder");
        percent_encode(&value.to_string(), &mut out);
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    out
}

/// Collects query parameters, with the API's names kept verbatim
/// (`query.start_date`).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct QueryWriter {
    pairs: Vec<(&'static str, String)>,
}

impl QueryWriter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds `name=value`, or one pair per element for a list.
    pub fn push<V: QueryValue + ?Sized>(&mut self, name: &'static str, value: &V) {
        value.write(name, self);
    }

    /// Adds `name=value` for a value that is present.
    pub fn push_opt<V: QueryValue>(&mut self, name: &'static str, value: Option<&V>) {
        if let Some(value) = value {
            self.push(name, value);
        }
    }

    fn pair(&mut self, name: &'static str, value: String) {
        self.pairs.push((name, value));
    }

    /// The pairs added so far.
    pub fn pairs(&self) -> &[(&'static str, String)] {
        &self.pairs
    }

    /// Whether no pair was added.
    pub fn is_empty(&self) -> bool {
        self.pairs.is_empty()
    }
}

impl fmt::Display for QueryWriter {
    /// The query string, without the leading `?`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut out = String::new();
        for (i, (name, value)) in self.pairs.iter().enumerate() {
            if i > 0 {
                out.push('&');
            }
            percent_encode(name, &mut out);
            out.push('=');
            percent_encode(value, &mut out);
        }
        f.write_str(&out)
    }
}

/// A value that can be sent as a query parameter.
pub trait QueryValue {
    #[doc(hidden)]
    fn write(&self, name: &'static str, query: &mut QueryWriter);
}

macro_rules! display_query_value {
    ($($ty:ty),*) => {$(
        impl QueryValue for $ty {
            fn write(&self, name: &'static str, query: &mut QueryWriter) {
                query.pair(name, self.to_string());
            }
        }
    )*};
}

display_query_value!(String, str, bool, i32, i64);

impl QueryValue for DateTime {
    fn write(&self, name: &'static str, query: &mut QueryWriter) {
        query.pair(name, self.as_str().to_owned());
    }
}

/// A list, sent as the same key repeated (`a=1&a=2`).
impl<T: QueryValue> QueryValue for Vec<T> {
    fn write(&self, name: &'static str, query: &mut QueryWriter) {
        for value in self {
            value.write(name, query);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_names_verbatim_and_encodes_values() {
        let mut query = QueryWriter::new();
        query.push(
            "query.start_date",
            &DateTime::parse("2026-11-04T10:15:00").unwrap(),
        );
        query.push("query.card_holder", &"Ann O'Neil & Co".to_owned());
        query.push("exclude_voids", &true);
        query.push("take", &50);
        assert_eq!(
            query.to_string(),
            "query.start_date=2026-11-04T10%3A15%3A00&query.card_holder=Ann%20O%27Neil%20%26%20Co\
             &exclude_voids=true&take=50"
        );
    }

    #[test]
    fn repeats_the_key_for_each_list_element() {
        let mut query = QueryWriter::new();
        query.push(
            "query.custom_fields",
            &vec!["a".to_owned(), "b=c".to_owned()],
        );
        assert_eq!(
            query.to_string(),
            "query.custom_fields=a&query.custom_fields=b%3Dc"
        );
    }

    #[test]
    fn skips_missing_values() {
        let mut query = QueryWriter::new();
        query.push_opt::<i32>("skip", None);
        assert!(query.is_empty());
        assert_eq!(query.to_string(), "");
    }

    #[test]
    fn renders_path_placeholders_as_segments() {
        let id = 42;
        let wallet = "a/b c".to_owned();
        assert_eq!(
            render_path(
                "/x/{transaction_id}/y/{request.wallet_merchant_id}",
                &[
                    ("transaction_id", &id),
                    ("request.wallet_merchant_id", &wallet)
                ]
            ),
            "/x/42/y/a%2Fb%20c"
        );
        assert_eq!(render_path("/health", &[]), "/health");
    }

    #[test]
    fn decodes_percent_escapes() {
        assert_eq!(percent_decode("a%20b%2Fc"), "a b/c");
        assert_eq!(percent_decode("100%"), "100%");
        assert_eq!(percent_decode("%e2%82%ac"), "€");
    }
}

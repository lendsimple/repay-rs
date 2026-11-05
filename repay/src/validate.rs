//! Checks of the schema's constraints: lengths, numeric ranges, patterns, required
//! values, and the precision of amounts.
//!
//! Every schema type implements [`Validate`]. Validating a request before it is sent
//! reports every problem with the path of the offending field, instead of the API's
//! first error.

use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use rust_decimal::Decimal;

/// A type whose values can be checked against the schema's constraints.
pub trait Validate {
    /// Checks `self` and everything inside it, collecting every violation.
    fn validate(&self) -> Result<(), ValidationError> {
        let mut path = Path::default();
        let mut violations = Vec::new();
        self.validate_into(&mut path, &mut violations);
        if violations.is_empty() {
            Ok(())
        } else {
            Err(ValidationError { violations })
        }
    }

    #[doc(hidden)]
    fn validate_into(&self, path: &mut Path, out: &mut Vec<Violation>);
}

/// One value that breaks a constraint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    /// Where the value is, by JSON field names: `custom_fields[0].field_value`.
    pub path: String,
    /// What is wrong with it.
    pub message: String,
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.path.is_empty() {
            write!(f, "{}", self.message)
        } else {
            write!(f, "{}: {}", self.path, self.message)
        }
    }
}

/// A value does not satisfy the schema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    violations: Vec<Violation>,
}

impl ValidationError {
    /// Every violation found, in field order. Never empty.
    pub fn violations(&self) -> &[Violation] {
        &self.violations
    }
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (first, rest) = self.violations.split_first().expect("never empty");
        write!(f, "{first}")?;
        match rest.len() {
            0 => Ok(()),
            1 => write!(f, " (and 1 more violation)"),
            n => write!(f, " (and {n} more violations)"),
        }
    }
}

impl std::error::Error for ValidationError {}

/// The location being validated, maintained by derived code.
#[doc(hidden)]
#[derive(Debug, Default)]
pub struct Path {
    segments: Vec<Segment>,
}

#[derive(Debug)]
enum Segment {
    Field(&'static str),
    Index(usize),
}

impl Path {
    pub fn push_field(&mut self, name: &'static str) {
        self.segments.push(Segment::Field(name));
    }

    pub fn push_index(&mut self, index: usize) {
        self.segments.push(Segment::Index(index));
    }

    pub fn pop(&mut self) {
        self.segments.pop();
    }

    fn render(&self) -> String {
        let mut out = String::new();
        for segment in &self.segments {
            match segment {
                Segment::Field(name) => {
                    if !out.is_empty() {
                        out.push('.');
                    }
                    out.push_str(name);
                }
                Segment::Index(i) => out.push_str(&format!("[{i}]")),
            }
        }
        out
    }
}

pub(crate) fn violation(path: &Path, out: &mut Vec<Violation>, message: String) {
    out.push(Violation {
        path: path.render(),
        message,
    });
}

impl<T: Validate> Validate for Option<T> {
    fn validate_into(&self, path: &mut Path, out: &mut Vec<Violation>) {
        if let Some(value) = self {
            value.validate_into(path, out);
        }
    }
}

impl<T: Validate> Validate for Vec<T> {
    fn validate_into(&self, path: &mut Path, out: &mut Vec<Violation>) {
        for (index, value) in self.iter().enumerate() {
            path.push_index(index);
            value.validate_into(path, out);
            path.pop();
        }
    }
}

impl<T: Validate> Validate for Box<T> {
    fn validate_into(&self, path: &mut Path, out: &mut Vec<Violation>) {
        (**self).validate_into(path, out);
    }
}

/// Types with no constraints of their own.
macro_rules! no_constraints {
    ($($ty:ty),* $(,)?) => {$(
        impl $crate::validate::Validate for $ty {
            fn validate_into(
                &self,
                _path: &mut $crate::validate::Path,
                _out: &mut ::std::vec::Vec<$crate::validate::Violation>,
            ) {
            }
        }
    )*};
}

pub(crate) use no_constraints;

no_constraints!(
    String,
    bool,
    i32,
    i64,
    f64,
    Decimal,
    BTreeMap<String, String>,
);

/// A value that `min` and `max` bounds compare against.
#[doc(hidden)]
pub trait Numeric {
    fn to_decimal(&self) -> Decimal;
}

impl Numeric for i32 {
    fn to_decimal(&self) -> Decimal {
        Decimal::from(*self)
    }
}

impl Numeric for i64 {
    fn to_decimal(&self) -> Decimal {
        Decimal::from(*self)
    }
}

impl Numeric for Decimal {
    fn to_decimal(&self) -> Decimal {
        *self
    }
}

// Checks called by derived code. Lengths count characters.

#[doc(hidden)]
pub fn check_required(present: bool, path: &Path, out: &mut Vec<Violation>) {
    if !present {
        violation(path, out, "is required".into());
    }
}

#[doc(hidden)]
pub fn check_length(
    value: &str,
    min: Option<usize>,
    max: Option<usize>,
    path: &Path,
    out: &mut Vec<Violation>,
) {
    let len = value.chars().count();
    if let Some(min) = min
        && len < min
    {
        violation(
            path,
            out,
            format!("must be at least {}, not {len}", characters(min)),
        );
    }
    if let Some(max) = max
        && len > max
    {
        violation(
            path,
            out,
            format!("must be at most {}, not {len}", characters(max)),
        );
    }
}

fn characters(n: usize) -> String {
    if n == 1 {
        "1 character".into()
    } else {
        format!("{n} characters")
    }
}

/// `min` and `max` are decimal numbers, checked when the derive expands.
#[doc(hidden)]
pub fn check_range(
    value: Decimal,
    min: Option<&str>,
    max: Option<&str>,
    path: &Path,
    out: &mut Vec<Violation>,
) {
    let bound = |text: &str| {
        Decimal::from_str(text)
            .unwrap_or_else(|_| panic!("bound {text:?} must be a decimal number"))
    };
    if let Some(min) = min
        && value < bound(min)
    {
        violation(path, out, format!("must be at least {min}, not {value}"));
    }
    if let Some(max) = max
        && value > bound(max)
    {
        violation(path, out, format!("must be at most {max}, not {value}"));
    }
}

#[doc(hidden)]
pub fn check_rule(
    value: &str,
    rule: fn(&str) -> Result<(), &'static str>,
    path: &Path,
    out: &mut Vec<Violation>,
) {
    if let Err(message) = rule(value) {
        violation(path, out, message.into());
    }
}

/// Checks that stand in for the schema's regular expressions.
///
/// The schema's patterns use lookaround, which the `regex` crate does not support,
/// so each is written out by hand. A rule returns what is wrong with the value.
pub mod rules {
    /// No `<` anywhere, so the value cannot hold markup.
    ///
    /// Schema pattern: `^(?!.*<).*$`.
    pub fn no_angle_bracket(value: &str) -> Result<(), &'static str> {
        if value.contains('<') {
            Err("must not contain `<`")
        } else {
            Ok(())
        }
    }

    /// A digital wallet merchant ID: not empty, with no whitespace and none of
    /// ``# % ^ & + * ( ) = [ ] { } : ' < > . / ? |``.
    ///
    /// Schema pattern: `^[^#%^&+*()\=\[\]{}:'<>./?|\s]+$`.
    pub fn wallet_merchant_id(value: &str) -> Result<(), &'static str> {
        const FORBIDDEN: &str = "#%^&+*()=[]{}:'<>./?|";
        if value.is_empty() {
            Err("must not be empty")
        } else if value
            .chars()
            .any(|c| c.is_whitespace() || FORBIDDEN.contains(c))
        {
            Err("must not contain whitespace or any of # % ^ & + * ( ) = [ ] { } : ' < > . / ? |")
        } else {
            Ok(())
        }
    }

    /// A domain name of at least two labels, such as `example.com`, with an optional
    /// trailing dot. The whole name is at most 253 characters; each label is 1 to 63
    /// letters, digits, `-` or `_`, does not start or end with `-`, and has `_` only
    /// as its first character (as in `_dmarc.example.com`).
    ///
    /// Schema pattern:
    /// `^(?=.{1,253}\.?$)(?:(?!-|[^.]+_)[A-Za-z0-9-_]{1,63}(?<!-)(?:\.|$)){2,}$`.
    pub fn hostname(value: &str) -> Result<(), &'static str> {
        const INVALID: &str = "must be a domain name such as example.com";
        let name = value.strip_suffix('.').unwrap_or(value);
        if name.is_empty() || name.len() > 253 {
            return Err(INVALID);
        }
        let labels: Vec<&str> = name.split('.').collect();
        if labels.len() < 2 {
            return Err(INVALID);
        }
        let valid_label = |label: &str| {
            (1..=63).contains(&label.len())
                && label
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
                && !label.starts_with('-')
                && !label.ends_with('-')
                && !label[1..].contains('_')
        };
        if labels.into_iter().all(valid_label) {
            Ok(())
        } else {
            Err(INVALID)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::rules::*;
    use super::*;

    #[test]
    fn no_angle_bracket_rejects_markup() {
        assert!(no_angle_bracket("Ellen Johnson").is_ok());
        assert!(no_angle_bracket("").is_ok());
        assert!(no_angle_bracket("<script>").is_err());
        assert!(no_angle_bracket("a < b").is_err());
    }

    #[test]
    fn wallet_merchant_id_rejects_forbidden_characters() {
        assert!(wallet_merchant_id("merchant-123_ABC").is_ok());
        assert!(wallet_merchant_id("").is_err());
        assert!(wallet_merchant_id("a b").is_err());
        assert!(wallet_merchant_id("a\tb").is_err());
        for c in "#%^&+*()=[]{}:'<>./?|".chars() {
            assert!(wallet_merchant_id(&format!("a{c}b")).is_err(), "{c}");
        }
    }

    #[test]
    fn hostname_accepts_domain_names() {
        for ok in [
            "example.com",
            "example.com.",
            "pay.example.co.uk",
            "_dmarc.example.com",
            "my-shop.example",
            "a1.b2",
        ] {
            assert!(hostname(ok).is_ok(), "{ok}");
        }
    }

    #[test]
    fn hostname_rejects_other_text() {
        let long_label = format!("{}.com", "a".repeat(64));
        let long_name = format!("{}.com", vec!["a".repeat(63); 4].join("."));
        for bad in [
            "",
            ".",
            "localhost",
            "a_b.com",
            "-a.com",
            "a-.com",
            "a..com",
            ".example.com",
            "exa mple.com",
            "example.com/path",
            long_label.as_str(),
            long_name.as_str(),
        ] {
            assert!(hostname(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn validation_error_reports_the_first_and_counts_the_rest() {
        let one = |path: &str| Violation {
            path: path.into(),
            message: "is required".into(),
        };
        let error = ValidationError {
            violations: vec![one("amount")],
        };
        assert_eq!(error.to_string(), "amount: is required");
        let error = ValidationError {
            violations: vec![one("amount"), one("zip"), one("street")],
        };
        assert_eq!(
            error.to_string(),
            "amount: is required (and 2 more violations)"
        );
    }

    #[test]
    fn range_bounds_are_inclusive() {
        let mut out = Vec::new();
        let path = Path::default();
        check_range(Decimal::from(0), Some("0"), Some("2"), &path, &mut out);
        check_range(Decimal::from(2), Some("0"), Some("2"), &path, &mut out);
        assert!(out.is_empty());
        check_range(Decimal::from(3), Some("0"), Some("2"), &path, &mut out);
        assert_eq!(out[0].message, "must be at most 2, not 3");
    }
}

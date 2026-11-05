//! A typed client for the Repay Gateway API (Repay Online).
//!
//! This release provides the values the schema is built on:
//!
//! - [`types`]: exact money amounts, the API's dates and times, card expiration
//!   dates, typed IDs, and the open-ended [`Json`](types::Json) and
//!   [`Download`](types::Download) responses.
//! - [`validate`]: checks of the schema's constraints, collecting every violation
//!   with the path of the field.
//!
//! The schema's request and response types and the clients follow in later releases.

// docs.rs builds with `--cfg docsrs` and labels items that need a feature.
#![cfg_attr(docsrs, feature(doc_cfg))]

// Lets derived code refer to `::repay::...` inside this crate too.
extern crate self as repay;

mod macros;
pub mod types;
pub mod validate;

pub use rust_decimal::{Decimal, dec};

/// Implements `Validate` and a redacting `Debug` for a schema struct.
#[doc(hidden)]
pub use repay_derive::Model;

/// Items used by derived code. Not part of the public API.
#[doc(hidden)]
pub mod __private {
    use std::fmt;

    /// Stands in for a sensitive value in `Debug` output.
    pub struct Redacted(bool);

    impl Redacted {
        /// `present` is whether there is a value to hide.
        pub fn new(present: bool) -> Self {
            Self(present)
        }
    }

    impl fmt::Debug for Redacted {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str(if self.0 { "[REDACTED]" } else { "None" })
        }
    }
}

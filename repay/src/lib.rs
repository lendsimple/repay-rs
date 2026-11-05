//! A typed client for the Repay Gateway API (Repay Online).
//!
//! This release provides the API's types and a protocol layer without I/O:
//!
//! - [`operations`]: a type for every operation, such as
//!   [`card_transactions::Capture`](operations::card_transactions::Capture).
//! - [`protocol`]: [`encode`](protocol::encode) an operation as an HTTP request,
//!   send it with any HTTP client, and [`decode`](protocol::decode) the response.
//! - [`schema`]: every request and response type, generated from the vendored
//!   Swagger specs. Requests have builders.
//! - [`types`]: exact money amounts, the API's dates and times, card expiration
//!   dates, typed IDs, and the open-ended [`Json`](types::Json) and
//!   [`Download`](types::Download) responses.
//! - [`validate`]: checks of the schema's constraints, collecting every violation
//!   with the path of the field.
//!
//! The clients follow in later releases.

// docs.rs builds with `--cfg docsrs` and labels items that need a feature.
#![cfg_attr(docsrs, feature(doc_cfg))]

// Lets derived code refer to `::repay::...` inside this crate too.
extern crate self as repay;

mod credentials;
mod error;
mod macros;
mod multipart;
mod operation;
pub mod operations;
pub mod protocol;
pub mod query;
pub mod schema;
pub mod transport;
pub mod types;
pub mod validate;

pub use credentials::{Credentials, Environment, PRODUCTION_URL, SANDBOX_URL};
pub use error::{ApiError, Error, TransportError};
pub use operation::{Body, Export, ExportFormat, Exportable, MerchantHeader, Operation};

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

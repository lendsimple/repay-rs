//! What every API operation has in common, and the file exports of report
//! operations.

use std::fmt::Display;

use crate::error::Error;
use crate::query::QueryWriter;
use crate::transport::{HttpResponse, Method};
use crate::types::Download;
use crate::validate::{Path, Validate, ValidationError, Violation};

/// One call to the API: its route, parameters and body, and the response it
/// expects.
///
/// Every operation of the API has a type in [`operations`](crate::operations),
/// such as [`card_transactions::Sale`](crate::operations::card_transactions::Sale).
/// [`protocol::encode`](crate::protocol::encode) turns one into an HTTP request,
/// and [`protocol::decode`](crate::protocol::decode) reads the answer.
pub trait Operation {
    /// What a successful call returns.
    type Response;

    /// The spec's operation ID, such as `CardTransactions_ProcessSaleAsync`.
    const ID: &'static str;

    /// The operation's name in the client, such as `card_transactions.sale`.
    const NAME: &'static str;

    /// The HTTP method.
    const METHOD: Method;

    /// Whether the spec declares the `rg-merchant-id` header.
    const MERCHANT: MerchantHeader;

    #[doc(hidden)]
    const EXPORT: Option<ExportParam>;

    /// The path after the version, such as `/transactions/card/42/capture`.
    #[doc(hidden)]
    fn path(&self) -> String;

    /// Writes the operation's own query parameters.
    #[doc(hidden)]
    fn params(&self, query: &mut QueryWriter);

    /// Writes the whole query: the parameters and, for an operation whose
    /// response is a PDF unless asked otherwise, `pdf=false`.
    #[doc(hidden)]
    fn query(&self, query: &mut QueryWriter) {
        self.params(query);
        if let Some(ExportParam {
            kind: ExportKind::Pdf,
            wire,
        }) = Self::EXPORT
        {
            query.push(wire, &false);
        }
    }

    #[doc(hidden)]
    fn body(&self) -> Result<Option<Body>, Error>;

    /// Checks the path parameters and body against the spec's constraints.
    fn validate(&self) -> Result<(), ValidationError>;

    /// Reads a successful (2xx) response.
    #[doc(hidden)]
    fn decode(response: &HttpResponse) -> Result<Self::Response, Error>;
}

/// Whether an operation's spec declares the `rg-merchant-id` header. The clients
/// send a configured merchant ID with every request; an operation that
/// [requires](MerchantHeader::Required) one fails validation without it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MerchantHeader {
    /// Not declared.
    None,
    /// Declared and optional, for reseller users acting for a merchant.
    Optional,
    /// Declared and required.
    Required,
}

/// A request body, encoded.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Body {
    pub content_type: String,
    pub bytes: Vec<u8>,
}

/// The parameter that switches an operation's response to a file.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExportParam {
    pub kind: ExportKind,
    pub wire: &'static str,
}

#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportKind {
    /// `format=csv|excel|pdf`.
    Format,
    /// `pdf=true`.
    Pdf,
}

/// An operation whose response can be a file instead of JSON; see [`Export`].
pub trait Exportable: Operation {}

/// The file formats of exports. Each report supports some of them; its
/// documentation lists which.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ExportFormat {
    Csv,
    Excel,
    Pdf,
}

impl ExportFormat {
    /// The value of the `format` parameter.
    pub fn as_str(self) -> &'static str {
        match self {
            ExportFormat::Csv => "csv",
            ExportFormat::Excel => "excel",
            ExportFormat::Pdf => "pdf",
        }
    }
}

/// An exportable operation asked for a file, which it returns as a
/// [`Download`].
///
/// For the ACH authorization form lookups, which return a PDF or JSON, the
/// format is ignored and the file is always a PDF.
#[derive(Debug, Clone)]
pub struct Export<'a, O> {
    op: &'a O,
    format: ExportFormat,
}

impl<'a, O: Exportable> Export<'a, O> {
    /// `op`, returning a file in `format`.
    pub fn new(op: &'a O, format: ExportFormat) -> Self {
        Self { op, format }
    }
}

impl<O: Exportable> Operation for Export<'_, O> {
    type Response = Download;
    const ID: &'static str = O::ID;
    const NAME: &'static str = O::NAME;
    const METHOD: Method = O::METHOD;
    const MERCHANT: MerchantHeader = O::MERCHANT;
    const EXPORT: Option<ExportParam> = O::EXPORT;

    fn path(&self) -> String {
        self.op.path()
    }

    fn params(&self, query: &mut QueryWriter) {
        self.op.params(query);
    }

    fn query(&self, query: &mut QueryWriter) {
        self.op.params(query);
        let param = O::EXPORT.expect("exportable operations have an export parameter");
        match param.kind {
            ExportKind::Format => query.push(param.wire, self.format.as_str()),
            ExportKind::Pdf => query.push(param.wire, &true),
        }
    }

    fn body(&self) -> Result<Option<Body>, Error> {
        self.op.body()
    }

    fn validate(&self) -> Result<(), ValidationError> {
        self.op.validate()
    }

    fn decode(response: &HttpResponse) -> Result<Download, Error> {
        Ok(crate::protocol::decode_download(response))
    }
}

/// Checks that a path parameter is not empty, which would change the route.
#[doc(hidden)]
pub fn check_segment(value: &dyn Display, name: &'static str, out: &mut Vec<Violation>) {
    if value.to_string().is_empty() {
        out.push(Violation {
            path: name.to_owned(),
            message: "must not be empty".to_owned(),
        });
    }
}

/// Checks a body, collecting its violations.
#[doc(hidden)]
pub fn check_body<B: Validate + ?Sized>(body: &B, out: &mut Vec<Violation>) {
    body.validate_into(&mut Path::default(), out);
}

#[doc(hidden)]
pub fn finish(violations: Vec<Violation>) -> Result<(), ValidationError> {
    match ValidationError::from_violations(violations) {
        Some(err) => Err(err),
        None => Ok(()),
    }
}

/// A JSON body.
#[doc(hidden)]
pub fn json_body<B: serde::Serialize + ?Sized>(body: &B) -> Result<Option<Body>, Error> {
    Ok(Some(Body {
        content_type: "application/json".to_owned(),
        bytes: serde_json::to_vec(body).map_err(Error::Encode)?,
    }))
}

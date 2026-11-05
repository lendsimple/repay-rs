//! Encoding requests and decoding responses, without any I/O.
//!
//! The clients are thin layers over [`encode`] and [`decode`], which can also
//! drive any other HTTP client:
//!
//! ```
//! use repay::operations::card_transactions::Capture;
//! use repay::protocol::{self, Context};
//! use repay::schema::CardCaptureRequest;
//! use repay::transport::HttpResponse;
//! use repay::{Credentials, Operation};
//!
//! let context = Context::new(Credentials::new("api-user", "secure-token")).merchant_id(1234);
//! let body = CardCaptureRequest::builder().build();
//! let capture = Capture::new(987654, &body);
//!
//! let request = protocol::encode(&capture, &context)?;
//! assert_eq!(
//!     request.url,
//!     "https://api.sandbox.repayonline.com/rgapi/v1/transactions/card/987654/capture"
//! );
//! assert_eq!(request.header("rg-merchant-id"), Some("1234"));
//!
//! // ... send `request`, then read the answer:
//! let response = HttpResponse::new(
//!     200,
//!     vec![("Content-Type".into(), "application/json".into())],
//!     r#"{"transaction_id": 987654, "result_text": "APPROVED"}"#,
//! );
//! let detail = protocol::decode::<Capture>(&response)?;
//! assert_eq!(detail.result_text.as_deref(), Some("APPROVED"));
//! # Ok::<(), repay::Error>(())
//! ```

use serde::de::DeserializeOwned;

use crate::credentials::{Credentials, Environment};
use crate::error::{ApiError, Error, body_excerpt};
use crate::operation::{MerchantHeader, Operation};
use crate::query::{QueryWriter, percent_decode};
use crate::schema::ErrorResponse;
use crate::transport::{HttpRequest, HttpResponse};
use crate::types::{Download, MerchantId};
use crate::validate::{ValidationError, Violation};

/// The `User-Agent` sent unless set otherwise.
pub const DEFAULT_USER_AGENT: &str = concat!("repay-rs/", env!("CARGO_PKG_VERSION"));

/// The API version requests use unless set otherwise.
pub const DEFAULT_API_VERSION: &str = "1";

/// What every request needs besides the operation: where to send it, and who
/// sends it.
#[derive(Debug, Clone)]
pub struct Context {
    credentials: Credentials,
    environment: Environment,
    api_version: String,
    merchant_id: Option<MerchantId>,
    user_agent: String,
}

impl Context {
    /// Requests to the sandbox, API version 1, with no merchant ID.
    pub fn new(credentials: Credentials) -> Self {
        Self {
            credentials,
            environment: Environment::default(),
            api_version: DEFAULT_API_VERSION.to_owned(),
            merchant_id: None,
            user_agent: DEFAULT_USER_AGENT.to_owned(),
        }
    }

    /// Sends requests to `environment`. The default is the sandbox.
    pub fn environment(mut self, environment: Environment) -> Self {
        self.environment = environment;
        self
    }

    /// The API version in request paths (`/v1/...`). The default is `1`.
    pub fn api_version(mut self, version: impl Into<String>) -> Self {
        self.api_version = version.into();
        self
    }

    /// The merchant to act for, sent as `rg-merchant-id` with every request.
    /// Reseller users need it; others can leave it out.
    pub fn merchant_id(mut self, merchant_id: impl Into<MerchantId>) -> Self {
        self.merchant_id = Some(merchant_id.into());
        self
    }

    /// Sends no merchant ID.
    pub fn without_merchant_id(mut self) -> Self {
        self.merchant_id = None;
        self
    }

    /// The `User-Agent` header. The default is `repay-rs/<version>`.
    pub fn user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.user_agent = user_agent.into();
        self
    }

    pub fn credentials(&self) -> &Credentials {
        &self.credentials
    }

    pub fn get_environment(&self) -> &Environment {
        &self.environment
    }

    pub fn get_api_version(&self) -> &str {
        &self.api_version
    }

    pub fn get_merchant_id(&self) -> Option<MerchantId> {
        self.merchant_id
    }

    pub fn get_user_agent(&self) -> &str {
        &self.user_agent
    }
}

/// Validates `op`, then builds its HTTP request.
///
/// Fails with [`Error::Validation`] if the body or a path parameter breaks the
/// spec's constraints, or if the operation requires a merchant ID and `context`
/// has none.
pub fn encode<O: Operation>(op: &O, context: &Context) -> Result<HttpRequest, Error> {
    let mut violations = op
        .validate()
        .err()
        .map(ValidationError::into_violations)
        .unwrap_or_default();
    if O::MERCHANT == MerchantHeader::Required && context.merchant_id.is_none() {
        violations.push(Violation {
            path: "rg-merchant-id".to_owned(),
            message: "is required".to_owned(),
        });
    }
    if let Some(err) = ValidationError::from_violations(violations) {
        return Err(err.into());
    }
    encode_unvalidated(op, context)
}

/// Like [`encode`], without validating `op` first.
pub fn encode_unvalidated<O: Operation>(op: &O, context: &Context) -> Result<HttpRequest, Error> {
    let mut url = format!(
        "{}/v{}{}",
        context.environment.base_url(),
        context.api_version,
        op.path()
    );
    let mut query = QueryWriter::new();
    op.query(&mut query);
    if !query.is_empty() {
        url.push('?');
        url.push_str(&query.to_string());
    }

    let mut headers = vec![
        ("accept", "application/json".to_owned()),
        ("rg-api-user", context.credentials.api_user().to_owned()),
        (
            "rg-api-secure-token",
            context.credentials.expose_token().to_owned(),
        ),
        ("user-agent", context.user_agent.clone()),
    ];
    if let Some(merchant_id) = context.merchant_id {
        headers.push(("rg-merchant-id", merchant_id.to_string()));
    }
    let body = match op.body()? {
        Some(body) => {
            headers.push(("content-type", body.content_type));
            body.bytes
        }
        None => Vec::new(),
    };
    Ok(HttpRequest {
        method: O::METHOD,
        url,
        headers,
        body,
    })
}

/// Reads the response to `O`.
///
/// A 2xx status is a success (Repay answers some declines with 201; check the
/// transaction's result). Any other status fails with [`Error::Api`] when the body
/// is an `ErrorResponse`, and with [`Error::Status`] otherwise.
pub fn decode<O: Operation>(response: &HttpResponse) -> Result<O::Response, Error> {
    if (200..300).contains(&response.status) {
        O::decode(response)
    } else {
        Err(error(response))
    }
}

fn error(response: &HttpResponse) -> Error {
    let json = response.content_type().is_none_or(|ct| is_json(&ct));
    if json
        && let Ok(body) = serde_json::from_slice::<ErrorResponse>(&response.body)
        && (body.message.is_some()
            || body
                .validation_errors
                .as_ref()
                .is_some_and(|v| !v.is_empty()))
    {
        return ApiError::new(response.status, body).into();
    }
    Error::Status {
        status: response.status,
        body: body_excerpt(&response.body),
    }
}

fn is_json(media_type: &str) -> bool {
    matches!(media_type, "application/json" | "text/json") || media_type.ends_with("+json")
}

/// Reads a JSON body. An empty body reads as `null`, and a body without a
/// `Content-Type` is tried as JSON.
#[doc(hidden)]
pub fn decode_json<T: DeserializeOwned>(response: &HttpResponse) -> Result<T, Error> {
    if let Some(content_type) = response.content_type()
        && !is_json(&content_type)
    {
        return Err(Error::UnexpectedContentType {
            status: response.status,
            content_type,
        });
    }
    let body: &[u8] = if response.body.iter().all(u8::is_ascii_whitespace) {
        b"null"
    } else {
        &response.body
    };
    serde_json::from_slice(body).map_err(|source| Error::Decode {
        source,
        status: response.status,
        body: body_excerpt(&response.body),
    })
}

/// Reads a file, whatever its type.
pub(crate) fn decode_download(response: &HttpResponse) -> Download {
    let content_type = response
        .content_type()
        .unwrap_or_else(|| "application/octet-stream".to_owned());
    let filename = response.header("content-disposition").and_then(filename);
    Download::new(content_type, filename, response.body.clone())
}

/// The file name in a `Content-Disposition` header, preferring the RFC 6266
/// `filename*` form.
fn filename(header: &str) -> Option<String> {
    let mut plain = None;
    let mut extended = None;
    for param in header.split(';').skip(1) {
        let Some((name, value)) = param.split_once('=') else {
            continue;
        };
        let value = value.trim();
        match name.trim().to_ascii_lowercase().as_str() {
            "filename*" => {
                // charset'language'percent-encoded
                let encoded = value.rsplit_once('\'').map_or(value, |(_, v)| v);
                extended = Some(percent_decode(encoded));
            }
            "filename" => {
                let unquoted = value
                    .strip_prefix('"')
                    .and_then(|v| v.strip_suffix('"'))
                    .map_or_else(|| value.to_owned(), |v| v.replace("\\\"", "\""));
                plain = Some(unquoted);
            }
            _ => {}
        }
    }
    extended.or(plain).filter(|name| !name.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_content_disposition_file_names() {
        assert_eq!(
            filename(r#"attachment; filename="report.csv""#).as_deref(),
            Some("report.csv")
        );
        assert_eq!(
            filename(r#"attachment; filename="a.csv"; filename*=UTF-8''a%20b.csv"#).as_deref(),
            Some("a b.csv")
        );
        assert_eq!(
            filename("attachment; filename=plain.pdf").as_deref(),
            Some("plain.pdf")
        );
        assert_eq!(filename("inline"), None);
    }
}

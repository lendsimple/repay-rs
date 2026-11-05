//! How the clients send requests over HTTP.
//!
//! [`protocol::encode`](crate::protocol::encode) turns an operation into an
//! [`HttpRequest`]; a [`Transport`] sends it and returns the [`HttpResponse`],
//! which [`protocol::decode`](crate::protocol::decode) reads. Everything else
//! (encoding, validation, decoding, logging) stays in the client.
//!
//! The async [`Client`](crate::Client) uses a [`Transport`], by default
//! `ReqwestTransport` (with the `reqwest` feature); the
//! [`blocking::Client`](crate::blocking::Client) uses a
//! [`blocking::Transport`](crate::blocking::Transport), by default `UreqTransport`
//! (with the `ureq` feature). On WebAssembly, `ReqwestTransport` uses the
//! JavaScript `fetch` API, and the traits do not require `Send`.
//!
//! To use another HTTP client, implement the trait:
//!
//! ```
//! use repay::TransportError;
//! use repay::transport::{HttpRequest, HttpResponse, Transport};
//!
//! struct MyTransport;
//!
//! impl Transport for MyTransport {
//!     async fn send(&self, request: HttpRequest) -> Result<HttpResponse, TransportError> {
//!         // Send `request.method` to `request.url` with `request.headers` and
//!         // `request.body`, within `request.timeout`.
//!         # let _ = request;
//!         # Ok(HttpResponse::new(200, Vec::new(), Vec::new()))
//!     }
//! }
//! ```

use std::fmt;
use std::future::Future;
use std::time::Duration;

use crate::error::TransportError;

/// The HTTP methods the API uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Method {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

impl Method {
    /// The method's name, such as `"POST"`.
    pub fn as_str(self) -> &'static str {
        match self {
            Method::Get => "GET",
            Method::Post => "POST",
            Method::Put => "PUT",
            Method::Patch => "PATCH",
            Method::Delete => "DELETE",
        }
    }
}

impl fmt::Display for Method {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A request to the API, ready to send.
///
/// `Debug` output hides the secure token.
#[derive(Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct HttpRequest {
    pub method: Method,
    /// The full URL, with the query string.
    pub url: String,
    /// Header names (lowercase) and values, including the credentials.
    pub headers: Vec<(&'static str, String)>,
    /// The body; empty when the request has none.
    pub body: Vec<u8>,
    /// How long the request may take in total, including reading the response.
    /// The clients always set it; `None` from [`protocol::encode`](crate::protocol::encode).
    pub timeout: Option<Duration>,
}

impl HttpRequest {
    /// The value of the header `name` (ignoring case), if present.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

/// Headers whose values `Debug` output hides.
pub(crate) const SECRET_HEADERS: &[&str] = &["rg-api-secure-token"];

impl fmt::Debug for HttpRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let headers: Vec<(&str, &str)> = self
            .headers
            .iter()
            .map(|(name, value)| {
                let secret = SECRET_HEADERS.iter().any(|s| s.eq_ignore_ascii_case(name));
                (*name, if secret { "[REDACTED]" } else { value.as_str() })
            })
            .collect();
        f.debug_struct("HttpRequest")
            .field("method", &self.method)
            .field("url", &self.url)
            .field("headers", &headers)
            .field("body", &format_args!("{} bytes", self.body.len()))
            .field("timeout", &self.timeout)
            .finish()
    }
}

/// The API's answer to an [`HttpRequest`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct HttpResponse {
    pub status: u16,
    /// Header names and values, as received.
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl HttpResponse {
    pub fn new(status: u16, headers: Vec<(String, String)>, body: impl Into<Vec<u8>>) -> Self {
        Self {
            status,
            headers,
            body: body.into(),
        }
    }

    /// The value of the header `name` (ignoring case), if present.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    /// The media type, without parameters and in lowercase, such as
    /// `application/json`.
    pub fn content_type(&self) -> Option<String> {
        self.header("content-type")
            .map(|v| {
                v.split(';')
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_ascii_lowercase()
            })
            .filter(|v| !v.is_empty())
    }
}

/// Sends requests for the async [`Client`](crate::Client).
///
/// The future must be `Send`, so that client calls can be spawned on a
/// multi-threaded runtime. (On WebAssembly, where there are no threads to send it
/// to and `fetch` futures are not `Send`, it need not be.)
///
/// Return a [`TransportError`] only when no response was received (a connection,
/// TLS or timeout failure); an HTTP error status is a response.
#[cfg(not(target_arch = "wasm32"))]
pub trait Transport: Send + Sync {
    fn send(
        &self,
        request: HttpRequest,
    ) -> impl Future<Output = Result<HttpResponse, TransportError>> + Send;
}

/// Sends requests for the async [`Client`](crate::Client).
///
/// On WebAssembly the future need not be `Send`: there are no threads to send it
/// to, and JavaScript `fetch` futures are not `Send`.
///
/// Return a [`TransportError`] only when no response was received (a connection,
/// TLS or timeout failure); an HTTP error status is a response.
#[cfg(target_arch = "wasm32")]
pub trait Transport {
    fn send(
        &self,
        request: HttpRequest,
    ) -> impl Future<Output = Result<HttpResponse, TransportError>>;
}

/// A [`Transport`] built on [reqwest](https://docs.rs/reqwest), which runs on
/// tokio (or on `fetch` in WebAssembly).
#[cfg(feature = "reqwest")]
#[derive(Debug, Clone)]
pub struct ReqwestTransport {
    client: reqwest::Client,
}

#[cfg(feature = "reqwest")]
impl Default for ReqwestTransport {
    /// A transport with its own connection pool.
    ///
    /// # Panics
    ///
    /// Like `reqwest::Client::new`, if the TLS backend cannot be initialized.
    fn default() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

#[cfg(feature = "reqwest")]
impl From<reqwest::Client> for ReqwestTransport {
    /// Uses an existing reqwest client, for example to share its connection pool
    /// or proxy settings.
    fn from(client: reqwest::Client) -> Self {
        Self { client }
    }
}

#[cfg(feature = "reqwest")]
impl Transport for ReqwestTransport {
    async fn send(&self, request: HttpRequest) -> Result<HttpResponse, TransportError> {
        let error = |err: reqwest::Error| {
            if err.is_timeout() {
                TransportError::timeout(err)
            } else {
                TransportError::new(err)
            }
        };
        let method = match request.method {
            Method::Get => reqwest::Method::GET,
            Method::Post => reqwest::Method::POST,
            Method::Put => reqwest::Method::PUT,
            Method::Patch => reqwest::Method::PATCH,
            Method::Delete => reqwest::Method::DELETE,
        };
        let mut builder = self.client.request(method, &request.url);
        for (name, value) in &request.headers {
            builder = builder.header(*name, value);
        }
        if let Some(timeout) = request.timeout {
            builder = builder.timeout(timeout);
        }
        if !request.body.is_empty() {
            builder = builder.body(request.body);
        }
        let response = builder.send().await.map_err(error)?;
        let status = response.status().as_u16();
        let headers = response
            .headers()
            .iter()
            .map(|(name, value)| {
                let value = String::from_utf8_lossy(value.as_bytes()).into_owned();
                (name.as_str().to_owned(), value)
            })
            .collect();
        let body = response.bytes().await.map_err(error)?;
        Ok(HttpResponse::new(status, headers, body.to_vec()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_hides_the_secure_token() {
        let request = HttpRequest {
            method: Method::Get,
            url: "https://example.com/v1/health".into(),
            headers: vec![
                ("rg-api-user", "user".into()),
                ("rg-api-secure-token", "s3cret".into()),
            ],
            body: Vec::new(),
            timeout: None,
        };
        let debug = format!("{request:?}");
        assert!(!debug.contains("s3cret"), "{debug}");
        assert!(
            debug.contains(r#"("rg-api-secure-token", "[REDACTED]")"#),
            "{debug}"
        );
    }

    #[test]
    fn reads_headers_ignoring_case() {
        let response = HttpResponse::new(
            200,
            vec![(
                "Content-Type".into(),
                "Application/JSON; charset=utf-8".into(),
            )],
            "{}",
        );
        assert_eq!(
            response.header("content-type"),
            Some("Application/JSON; charset=utf-8")
        );
        assert_eq!(response.content_type().as_deref(), Some("application/json"));
        assert_eq!(HttpResponse::new(200, Vec::new(), "").content_type(), None);
    }
}

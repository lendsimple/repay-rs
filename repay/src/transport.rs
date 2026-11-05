//! The HTTP requests and responses the protocol encodes and decodes.
//!
//! [`protocol::encode`](crate::protocol::encode) turns an operation into an
//! [`HttpRequest`]; any HTTP client can send it, and
//! [`protocol::decode`](crate::protocol::decode) reads the [`HttpResponse`].

use std::fmt;

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

//! Errors returned by the client.

use std::fmt;

use crate::schema::{ErrorResponse, StatusCode, ValidationErrorResponse};
use crate::validate::ValidationError;

/// How much of a response body an error keeps, for diagnosis.
const MAX_BODY: usize = 4096;

/// Why a request failed.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The API answered with an error status and an `ErrorResponse`.
    #[error(transparent)]
    Api(Box<ApiError>),

    /// The request breaks the schema's constraints, so it was not sent.
    #[error("invalid request: {0}")]
    Validation(#[from] ValidationError),

    /// The request body could not be written as JSON.
    #[error("could not write the request body: {0}")]
    Encode(#[source] serde_json::Error),

    /// A successful response could not be read as the expected type.
    #[error("could not read the response (HTTP {status}): {source}")]
    Decode {
        #[source]
        source: serde_json::Error,
        status: u16,
        /// The start of the response body.
        body: String,
    },

    /// A successful response was not JSON, as the operation expects.
    #[error("expected a JSON response, got {content_type} (HTTP {status})")]
    UnexpectedContentType { status: u16, content_type: String },

    /// The API answered with an error status and a body that is not an
    /// `ErrorResponse`.
    #[error("HTTP status {status}")]
    Status {
        status: u16,
        /// The start of the response body, which may help diagnose the failure.
        body: String,
    },

    /// The request could not be sent or its response not received: a connection,
    /// TLS or timeout failure.
    #[error("HTTP request failed: {0}")]
    Transport(#[from] TransportError),
}

impl Error {
    /// The API error, if the API reported one.
    pub fn as_api(&self) -> Option<&ApiError> {
        match self {
            Error::Api(err) => Some(err),
            _ => None,
        }
    }

    /// The HTTP status of the response, if one was received.
    pub fn status(&self) -> Option<u16> {
        match self {
            Error::Api(err) => Some(err.status),
            Error::Decode { status, .. }
            | Error::UnexpectedContentType { status, .. }
            | Error::Status { status, .. } => Some(*status),
            Error::Validation(_) | Error::Encode(_) | Error::Transport(_) => None,
        }
    }
}

impl From<ApiError> for Error {
    fn from(err: ApiError) -> Self {
        Error::Api(Box::new(err))
    }
}

/// The start of a body, as text, for an error.
pub(crate) fn body_excerpt(body: &[u8]) -> String {
    let text = String::from_utf8_lossy(&body[..body.len().min(MAX_BODY)]).into_owned();
    if body.len() > MAX_BODY {
        format!("{text}…")
    } else {
        text
    }
}

/// A failure of the HTTP transport underneath a client.
#[derive(Debug, thiserror::Error)]
#[error("{source}")]
pub struct TransportError {
    source: Box<dyn std::error::Error + Send + Sync>,
    timeout: bool,
}

impl TransportError {
    /// A failure to send a request or receive its response.
    pub fn new(source: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> Self {
        Self {
            source: source.into(),
            timeout: false,
        }
    }

    /// A request that timed out.
    pub fn timeout(source: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> Self {
        Self {
            source: source.into(),
            timeout: true,
        }
    }

    /// Whether the request timed out.
    pub fn is_timeout(&self) -> bool {
        self.timeout
    }
}

/// An error the API reported: an error status with an `ErrorResponse` body.
///
/// Displays as `HTTP 400: <message>`, with the number of validation errors.
///
/// ```no_run
/// # fn handle(err: repay::ApiError) {
/// for e in err.validation_errors() {
///     println!("{:?}: {:?}", e.property, e.message);
/// }
/// # }
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ApiError {
    status: u16,
    response: ErrorResponse,
}

impl ApiError {
    pub(crate) fn new(status: u16, response: ErrorResponse) -> Self {
        Self { status, response }
    }

    /// The HTTP status.
    pub fn status(&self) -> u16 {
        self.status
    }

    /// The API's message; empty if there is none.
    pub fn message(&self) -> &str {
        self.response.message.as_deref().unwrap_or("")
    }

    /// The status the API reports in the body, such as `BadRequest`.
    pub fn status_code(&self) -> Option<&StatusCode> {
        self.response.status_code.as_ref()
    }

    /// The fields the API rejected, if any.
    pub fn validation_errors(&self) -> &[ValidationErrorResponse] {
        self.response.validation_errors.as_deref().unwrap_or(&[])
    }

    /// Whether the API rejected the request's fields.
    pub fn is_validation(&self) -> bool {
        !self.validation_errors().is_empty()
    }

    /// The whole response.
    pub fn response(&self) -> &ErrorResponse {
        &self.response
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "HTTP {}", self.status)?;
        if !self.message().is_empty() {
            write!(f, ": {}", self.message())?;
        }
        match self.validation_errors().len() {
            0 => Ok(()),
            1 => f.write_str(" (1 validation error)"),
            n => write!(f, " ({n} validation errors)"),
        }
    }
}

impl std::error::Error for ApiError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn api_error(json: &str) -> ApiError {
        ApiError::new(400, serde_json::from_str(json).unwrap())
    }

    #[test]
    fn displays_the_status_message_and_validation_errors() {
        let err = api_error(
            r#"{"message": "The request is invalid.", "status_code": "BadRequest",
                "validation_errors": [{"property": "amount", "message": "Required"},
                                      {"property": "zip", "message": "Too long"}]}"#,
        );
        assert_eq!(
            err.to_string(),
            "HTTP 400: The request is invalid. (2 validation errors)"
        );
        assert!(err.is_validation());
        assert_eq!(err.status_code(), Some(&StatusCode::BadRequest));
        assert_eq!(api_error("{}").to_string(), "HTTP 400");
    }

    #[test]
    fn truncates_long_bodies() {
        assert_eq!(body_excerpt(b"short"), "short");
        let long = vec![b'a'; MAX_BODY + 10];
        assert_eq!(body_excerpt(&long).chars().count(), MAX_BODY + 1);
    }

    #[test]
    fn status_of_each_error() {
        let err = Error::Status {
            status: 502,
            body: String::new(),
        };
        assert_eq!(err.status(), Some(502));
        assert_eq!(Error::from(api_error("{}")).status(), Some(400));
    }
}

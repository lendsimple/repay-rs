//! What the async and blocking clients share: settings, logging, and the calls
//! into the protocol.

use std::time::Duration;

use serde_json::Value;

use crate::credentials::Credentials;
use crate::error::Error;
use crate::operation::Operation;
use crate::protocol::{self, Context};
use crate::schema::SENSITIVE_FIELDS;
use crate::transport::{HttpRequest, HttpResponse};

/// How long a request may take, including reading the response, unless set
/// otherwise.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

const REDACTED: &str = "[REDACTED]";

/// A client's settings.
#[derive(Debug, Clone)]
pub(crate) struct Config {
    pub context: Context,
    pub timeout: Duration,
    pub validate_requests: bool,
}

impl Config {
    pub fn new(credentials: Credentials) -> Self {
        Self {
            context: Context::new(credentials),
            timeout: DEFAULT_TIMEOUT,
            validate_requests: true,
        }
    }

    /// The HTTP request for `op`, validated unless validation is turned off.
    pub fn encode<O: Operation>(&self, op: &O) -> Result<HttpRequest, Error> {
        let mut request = if self.validate_requests {
            protocol::encode(op, &self.context)
        } else {
            protocol::encode_unvalidated(op, &self.context)
        }
        .inspect_err(|err| tracing::debug!(error = %err, "request not sent"))?;
        request.timeout = Some(self.timeout);
        tracing::debug!(method = %request.method, url = %redact_url(&request.url), "sending request");
        if tracing::enabled!(tracing::Level::TRACE) {
            let body = redact_body(request.header("content-type"), &request.body);
            tracing::trace!(%body, "request body");
        }
        Ok(request)
    }

    pub fn decode<O: Operation>(&self, response: &HttpResponse) -> Result<O::Response, Error> {
        tracing::debug!(status = response.status, "received response");
        if tracing::enabled!(tracing::Level::TRACE) {
            let body = redact_body(response.header("content-type"), &response.body);
            tracing::trace!(%body, "response body");
        }
        let result = protocol::decode::<O>(response);
        if let Err(err) = &result {
            tracing::debug!(error = %err, "request failed");
        }
        result
    }
}

/// The span every request runs in.
pub(crate) fn request_span<O: Operation>() -> tracing::Span {
    tracing::debug_span!("repay", operation = O::NAME)
}

fn is_sensitive(name: &str) -> bool {
    let bare = name
        .strip_prefix("query.")
        .or_else(|| name.strip_prefix("request."))
        .unwrap_or(name);
    SENSITIVE_FIELDS.contains(&bare)
}

/// A body for logging: JSON with every sensitive field's value replaced by
/// `[REDACTED]`, or a summary of anything else.
pub(crate) fn redact_body(content_type: Option<&str>, body: &[u8]) -> String {
    if body.is_empty() {
        return String::new();
    }
    match serde_json::from_slice::<Value>(body) {
        Ok(mut json) => {
            redact_json(&mut json);
            json.to_string()
        }
        Err(_) => format!(
            "<{} bytes of {}>",
            body.len(),
            content_type.unwrap_or("unknown type")
        ),
    }
}

fn redact_json(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for (key, value) in map.iter_mut() {
                if is_sensitive(key) && !value.is_null() {
                    *value = Value::String(REDACTED.to_owned());
                } else {
                    redact_json(value);
                }
            }
        }
        Value::Array(items) => items.iter_mut().for_each(redact_json),
        _ => {}
    }
}

/// A URL for logging, with sensitive query values replaced by `[REDACTED]`.
pub(crate) fn redact_url(url: &str) -> String {
    let Some((base, query)) = url.split_once('?') else {
        return url.to_owned();
    };
    let pairs: Vec<String> = query
        .split('&')
        .map(|pair| match pair.split_once('=') {
            Some((name, _)) if is_sensitive(name) => format!("{name}={REDACTED}"),
            _ => pair.to_owned(),
        })
        .collect();
    format!("{base}?{}", pairs.join("&"))
}

/// The builder settings both clients have, as methods on a builder whose
/// `config` field is a [`Config`].
macro_rules! common_builder_methods {
    () => {
        /// Sends requests to `environment`. The default is the sandbox.
        pub fn environment(mut self, environment: $crate::Environment) -> Self {
            self.config.context.environment = environment;
            self
        }

        /// The API version in request paths (`/v1/...`). The default is `1`.
        pub fn api_version(mut self, version: impl Into<String>) -> Self {
            self.config.context.api_version = version.into();
            self
        }

        /// The merchant to act for, sent as `rg-merchant-id` with every request.
        /// Reseller users need it; others can leave it out. A client can switch
        /// merchants per call with `with_merchant_id`.
        pub fn merchant_id(mut self, merchant_id: impl Into<$crate::types::MerchantId>) -> Self {
            self.config.context.merchant_id = Some(merchant_id.into());
            self
        }

        /// How long a request may take in total. The default is 60 seconds.
        pub fn timeout(mut self, timeout: ::std::time::Duration) -> Self {
            self.config.timeout = timeout;
            self
        }

        /// The `User-Agent` header. The default is `repay-rs/<version>`.
        pub fn user_agent(mut self, user_agent: impl Into<String>) -> Self {
            self.config.context.user_agent = user_agent.into();
            self
        }

        /// Whether to check requests against the spec's constraints before sending
        /// them (the default), failing with
        /// [`Error::Validation`](crate::Error::Validation).
        pub fn validate_requests(mut self, validate: bool) -> Self {
            self.config.validate_requests = validate;
            self
        }
    };
}

pub(crate) use common_builder_methods;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_sensitive_json_fields_at_any_depth() {
        let body = br#"{"amount": 1.5, "card_number": "4111111111111111", "cvv": null,
                        "tokens": [{"card_token": "tok", "name": "x"}],
                        "details": {"account_number": 123}}"#;
        assert_eq!(
            redact_body(Some("application/json"), body),
            r#"{"amount":1.5,"card_number":"[REDACTED]","cvv":null,"details":{"account_number":"[REDACTED]"},"tokens":[{"card_token":"[REDACTED]","name":"x"}]}"#
        );
    }

    #[test]
    fn summarizes_other_bodies() {
        assert_eq!(
            redact_body(Some("text/csv"), b"a,b\n1,2\n"),
            "<8 bytes of text/csv>"
        );
        assert_eq!(redact_body(None, b""), "");
    }

    #[test]
    fn redacts_sensitive_query_values() {
        assert_eq!(
            redact_url("https://x/v1/q?take=5&query.card_number=4111&query.card_holder=Ann"),
            "https://x/v1/q?take=5&query.card_number=[REDACTED]&query.card_holder=Ann"
        );
        assert_eq!(redact_url("https://x/v1/health"), "https://x/v1/health");
    }
}

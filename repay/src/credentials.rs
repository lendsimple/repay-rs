//! API credentials and the API's environments.

use std::fmt;

use secrecy::{ExposeSecret, SecretString};

/// The `rg-api-user` and `rg-api-secure-token` headers every request carries.
///
/// The secure token is held in a [`SecretString`] and never appears in `Debug`
/// output. It grants full access to the merchant's account: keep it on a server, and
/// never ship it to a browser.
///
/// ```
/// use repay::Credentials;
///
/// let credentials = Credentials::new("api-user", "secure-token");
/// assert_eq!(credentials.api_user(), "api-user");
/// assert!(!format!("{credentials:?}").contains("secure-token"));
/// ```
#[derive(Clone)]
pub struct Credentials {
    api_user: String,
    secure_token: SecretString,
}

impl Credentials {
    /// The API user and its secure token.
    pub fn new(api_user: impl Into<String>, secure_token: impl Into<SecretString>) -> Self {
        Self {
            api_user: api_user.into(),
            secure_token: secure_token.into(),
        }
    }

    /// The API user, sent as `rg-api-user`.
    pub fn api_user(&self) -> &str {
        &self.api_user
    }

    /// The secure token, sent as `rg-api-secure-token`.
    pub fn secure_token(&self) -> &SecretString {
        &self.secure_token
    }

    pub(crate) fn expose_token(&self) -> &str {
        self.secure_token.expose_secret()
    }
}

impl fmt::Debug for Credentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Credentials")
            .field("api_user", &self.api_user)
            .finish_non_exhaustive()
    }
}

/// The sandbox's base URL, for testing with a sandbox account.
pub const SANDBOX_URL: &str = "https://api.sandbox.repayonline.com/rgapi";

/// The production base URL, which moves real money.
pub const PRODUCTION_URL: &str = "https://api.repayonline.com/rgapi";

/// Which Repay environment to send requests to.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum Environment {
    /// The sandbox, for testing. Sandbox credentials do not work in production.
    #[default]
    Sandbox,
    /// The production environment, which moves real money.
    Production,
    /// Another base URL, such as a mock server in tests. Requests go to
    /// `<url>/v<version>/<path>`.
    Custom(String),
}

impl Environment {
    /// The base URL, without the API version or a trailing `/`.
    pub fn base_url(&self) -> &str {
        match self {
            Environment::Sandbox => SANDBOX_URL,
            Environment::Production => PRODUCTION_URL,
            Environment::Custom(url) => url.trim_end_matches('/'),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_hides_the_token() {
        let debug = format!("{:?}", Credentials::new("user", "s3cret"));
        assert_eq!(debug, r#"Credentials { api_user: "user", .. }"#);
    }

    #[test]
    fn base_urls() {
        assert_eq!(Environment::default().base_url(), SANDBOX_URL);
        assert_eq!(
            Environment::Custom("http://localhost:1234/rgapi/".into()).base_url(),
            "http://localhost:1234/rgapi"
        );
    }
}

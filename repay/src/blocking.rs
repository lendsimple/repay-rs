//! The blocking client, which needs no async runtime, and its resource handles.
//!
//! Do not call it from async code: it blocks the thread. In a tokio task, use the
//! async [`Client`](crate::Client), or wrap calls in `tokio::task::spawn_blocking`.

use std::fmt;
use std::sync::Arc;

use crate::config::{Config, common_builder_methods, request_span};
use crate::credentials::Credentials;
use crate::error::{Error, TransportError};
use crate::operation::Operation;
use crate::operations::blocking_handles;
use crate::schema::operations::operation_table;
use crate::transport::{HttpRequest, HttpResponse};
use crate::types::MerchantId;

/// Sends requests for the blocking [`Client`].
///
/// Return a [`TransportError`] only when no response was received (a connection,
/// TLS or timeout failure); an HTTP error status is a response. See
/// [`transport`](crate::transport) for the async counterpart and an example.
#[cfg(not(target_arch = "wasm32"))]
pub trait Transport: Send + Sync {
    fn send(&self, request: HttpRequest) -> Result<HttpResponse, TransportError>;
}

/// Sends requests for the blocking [`Client`].
///
/// On WebAssembly the transport need not be `Send` or `Sync`: there are no threads
/// to share it with, and host HTTP APIs (such as WASI's) are often single-threaded.
#[cfg(target_arch = "wasm32")]
pub trait Transport {
    fn send(&self, request: HttpRequest) -> Result<HttpResponse, TransportError>;
}

/// A blocking Repay client.
///
/// It sends requests through a [`Transport`], by default a [`UreqTransport`]
/// (with the `ureq` feature). Use [`with_transport`](Client::with_transport) to
/// send them with another HTTP client.
///
/// Cloning is cheap and clones share one transport, so a client can be shared
/// across threads. Operations are grouped by resource:
///
/// ```no_run
/// # #[cfg(feature = "ureq")]
/// # fn run() -> Result<(), repay::Error> {
/// use repay::blocking::Client;
/// use repay::schema::CardSaleRequest;
/// use repay::types::{Amount, ExpDate};
/// use repay::{Credentials, dec};
///
/// let client = Client::builder(Credentials::new("api-user", "secure-token"))
///     .merchant_id(1234)
///     .build();
/// let sale = CardSaleRequest::builder()
///     .amount(Amount::from(dec!(19.99)))
///     .card_number("4111111111111111")
///     .exp_date(ExpDate::new(12, 2030).unwrap())
///     .build();
/// let detail = client.card_transactions().sale(&sale)?;
/// println!("{:?}", detail.result_text);
/// # Ok(())
/// # }
/// ```
#[cfg(feature = "ureq")]
pub struct Client<T = UreqTransport> {
    transport: Arc<T>,
    config: Arc<Config>,
}

/// A blocking Repay client.
///
/// It sends requests through a [`Transport`]; enable the `ureq` feature for a
/// default one. Cloning is cheap and clones share one transport.
#[cfg(not(feature = "ureq"))]
pub struct Client<T> {
    transport: Arc<T>,
    config: Arc<Config>,
}

#[cfg(feature = "ureq")]
impl Client {
    /// A client for the sandbox with default settings and a [`UreqTransport`].
    pub fn new(credentials: Credentials) -> Self {
        Self::builder(credentials).build()
    }

    /// A builder to configure the environment, merchant, timeout and so on.
    pub fn builder(credentials: Credentials) -> ClientBuilder {
        ClientBuilder::new(credentials, UreqTransport::default())
    }
}

impl<T: Transport> Client<T> {
    /// A client for the sandbox with default settings, sending requests through
    /// `transport`. Use [`ClientBuilder::new`] to change the settings.
    pub fn with_transport(credentials: Credentials, transport: T) -> Self {
        ClientBuilder::new(credentials, transport).build()
    }

    /// The transport requests are sent through.
    pub fn transport(&self) -> &T {
        &self.transport
    }

    /// A client that acts for `merchant_id` (sent as `rg-merchant-id`), sharing
    /// this one's transport.
    pub fn with_merchant_id(&self, merchant_id: impl Into<MerchantId>) -> Self {
        let mut config = Config::clone(&self.config);
        config.context.merchant_id = Some(merchant_id.into());
        self.with_config(config)
    }

    /// A client that sends no merchant ID, sharing this one's transport.
    pub fn without_merchant_id(&self) -> Self {
        let mut config = Config::clone(&self.config);
        config.context.merchant_id = None;
        self.with_config(config)
    }

    fn with_config(&self, config: Config) -> Self {
        Self {
            transport: Arc::clone(&self.transport),
            config: Arc::new(config),
        }
    }

    /// Sends any operation, such as an [`Export`](crate::Export). The resource
    /// methods, such as [`card_transactions`](Self::card_transactions), call this.
    pub fn execute<O: Operation>(&self, op: &O) -> Result<O::Response, Error> {
        let _span = request_span::<O>().entered();
        let request = self.config.encode(op)?;
        let response = self.transport.send(request)?;
        self.config.decode::<O>(&response)
    }
}

impl<T> Clone for Client<T> {
    fn clone(&self) -> Self {
        Self {
            transport: Arc::clone(&self.transport),
            config: Arc::clone(&self.config),
        }
    }
}

impl<T> fmt::Debug for Client<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

/// Configures a blocking [`Client`].
#[cfg(feature = "ureq")]
#[must_use]
pub struct ClientBuilder<T = UreqTransport> {
    config: Config,
    transport: T,
}

/// Configures a blocking [`Client`].
#[cfg(not(feature = "ureq"))]
#[must_use]
pub struct ClientBuilder<T> {
    config: Config,
    transport: T,
}

impl<T> ClientBuilder<T> {
    /// A builder for a client that sends requests through `transport`.
    pub fn new(credentials: Credentials, transport: T) -> Self {
        Self {
            config: Config::new(credentials),
            transport,
        }
    }

    common_builder_methods!();
}

impl<T: Transport> ClientBuilder<T> {
    pub fn build(self) -> Client<T> {
        Client {
            transport: Arc::new(self.transport),
            config: Arc::new(self.config),
        }
    }
}

impl<T> fmt::Debug for ClientBuilder<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClientBuilder")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

/// A [`Transport`] built on [ureq](https://docs.rs/ureq).
#[cfg(feature = "ureq")]
#[derive(Debug, Clone)]
pub struct UreqTransport {
    agent: ureq::Agent,
}

/// Larger than any response: exports of a whole query are the largest.
#[cfg(feature = "ureq")]
const MAX_RESPONSE_BYTES: u64 = 100 * 1024 * 1024;

#[cfg(feature = "ureq")]
impl Default for UreqTransport {
    /// A transport with its own connection pool.
    fn default() -> Self {
        Self {
            agent: ureq::Agent::new_with_defaults(),
        }
    }
}

#[cfg(feature = "ureq")]
impl From<ureq::Agent> for UreqTransport {
    /// Uses an existing ureq agent, for example to share its connection pool or
    /// proxy settings.
    fn from(agent: ureq::Agent) -> Self {
        Self { agent }
    }
}

#[cfg(feature = "ureq")]
impl Transport for UreqTransport {
    fn send(&self, request: HttpRequest) -> Result<HttpResponse, TransportError> {
        let mut builder = ureq::http::Request::builder()
            .method(request.method.as_str())
            .uri(&request.url);
        for (name, value) in &request.headers {
            builder = builder.header(*name, value);
        }
        // A request without a body sends none, rather than an empty one.
        let mut response = if request.body.is_empty() {
            self.run(builder.body(()), request.timeout)
        } else {
            self.run(builder.body(request.body), request.timeout)
        }?;
        let status = response.status().as_u16();
        let headers = response
            .headers()
            .iter()
            .map(|(name, value)| {
                let value = String::from_utf8_lossy(value.as_bytes()).into_owned();
                (name.as_str().to_owned(), value)
            })
            .collect();
        let body = response
            .body_mut()
            .with_config()
            .limit(MAX_RESPONSE_BYTES)
            .read_to_vec()
            .map_err(ureq_error)?;
        Ok(HttpResponse::new(status, headers, body))
    }
}

#[cfg(feature = "ureq")]
impl UreqTransport {
    fn run<B: ureq::AsSendBody>(
        &self,
        request: Result<ureq::http::Request<B>, ureq::http::Error>,
        timeout: Option<std::time::Duration>,
    ) -> Result<ureq::http::Response<ureq::Body>, TransportError> {
        let request = self
            .agent
            .configure_request(request.map_err(TransportError::new)?)
            .http_status_as_error(false)
            .timeout_global(timeout)
            .build();
        self.agent.run(request).map_err(ureq_error)
    }
}

#[cfg(feature = "ureq")]
fn ureq_error(err: ureq::Error) -> TransportError {
    if matches!(err, ureq::Error::Timeout(_)) {
        TransportError::timeout(err)
    } else {
        TransportError::new(err)
    }
}

operation_table!(blocking_handles);

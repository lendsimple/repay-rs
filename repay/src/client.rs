//! The async client and its resource handles.

use std::fmt;
use std::sync::Arc;

use tracing::Instrument;

use crate::config::{Config, common_builder_methods, request_span};
use crate::credentials::Credentials;
use crate::error::Error;
use crate::operation::Operation;
use crate::operations::async_handles;
use crate::schema::operations::operation_table;
#[cfg(feature = "reqwest")]
use crate::transport::ReqwestTransport;
use crate::transport::Transport;
use crate::types::MerchantId;

/// An async Repay client.
///
/// It sends requests through a [`Transport`], by default a [`ReqwestTransport`]
/// (with the `reqwest` feature). Use [`with_transport`](Client::with_transport) to
/// send them with another HTTP client.
///
/// Cloning is cheap and clones share one transport, so a client can be shared
/// across tasks. Operations are grouped by resource:
///
/// ```no_run
/// # #[cfg(feature = "reqwest")]
/// # async fn run() -> Result<(), repay::Error> {
/// use repay::schema::CardSaleRequest;
/// use repay::types::{Amount, ExpDate};
/// use repay::{Client, Credentials, dec};
///
/// let client = Client::builder(Credentials::new("api-user", "secure-token"))
///     .merchant_id(1234)
///     .build();
/// let sale = CardSaleRequest::builder()
///     .amount(Amount::from(dec!(19.99)))
///     .card_number("4111111111111111")
///     .exp_date(ExpDate::new(12, 2030).unwrap())
///     .build();
/// let detail = client.card_transactions().sale(&sale).await?;
/// println!("{:?}", detail.result_text);
/// # Ok(())
/// # }
/// ```
#[cfg(feature = "reqwest")]
pub struct Client<T = ReqwestTransport> {
    transport: Arc<T>,
    config: Arc<Config>,
}

/// An async Repay client.
///
/// It sends requests through a [`Transport`]; enable the `reqwest` feature for a
/// default one. Cloning is cheap and clones share one transport.
#[cfg(not(feature = "reqwest"))]
pub struct Client<T> {
    transport: Arc<T>,
    config: Arc<Config>,
}

#[cfg(feature = "reqwest")]
impl Client {
    /// A client for the sandbox with default settings and a [`ReqwestTransport`].
    ///
    /// # Panics
    ///
    /// Like `reqwest::Client::new`, if the TLS backend cannot be initialized.
    pub fn new(credentials: Credentials) -> Self {
        Self::builder(credentials).build()
    }

    /// A builder to configure the environment, merchant, timeout and so on.
    pub fn builder(credentials: Credentials) -> ClientBuilder {
        ClientBuilder::new(credentials, ReqwestTransport::default())
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
    pub async fn execute<O: Operation + Sync>(&self, op: &O) -> Result<O::Response, Error> {
        let config = &self.config;
        async move {
            let request = config.encode(op)?;
            let response = self.transport.send(request).await?;
            config.decode::<O>(&response)
        }
        .instrument(request_span::<O>())
        .await
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

/// Configures a [`Client`].
#[cfg(feature = "reqwest")]
#[must_use]
pub struct ClientBuilder<T = ReqwestTransport> {
    config: Config,
    transport: T,
}

/// Configures a [`Client`].
#[cfg(not(feature = "reqwest"))]
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

operation_table!(async_handles);

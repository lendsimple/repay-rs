//! Both clients with custom, in-memory transports: no HTTP, no cargo features, and
//! for the async client no tokio (a minimal executor drives it).

mod common;

use std::future::Future;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Wake};
use std::time::Duration;

use common::credentials;
use repay::operations::card_transactions;
use repay::schema::{CardCaptureRequest, CloseCardSettlementBatchRequest};
use repay::transport::{HttpRequest, HttpResponse, Method};
use repay::types::Amount;
use repay::{
    ClientBuilder, DEFAULT_TIMEOUT, DEFAULT_USER_AGENT, Error, ExportFormat, TransportError,
    blocking, dec,
};

type Client = repay::Client<FakeTransport>;

/// Answers every request with the same result and records what was sent.
#[derive(Clone)]
struct FakeTransport {
    answer: Arc<dyn Fn() -> Result<HttpResponse, TransportError> + Send + Sync>,
    sent: Arc<Mutex<Vec<HttpRequest>>>,
}

impl FakeTransport {
    fn json(status: u16, body: &str) -> Self {
        let body = body.to_owned();
        Self::with(move || {
            Ok(HttpResponse::new(
                status,
                vec![("content-type".into(), "application/json".into())],
                body.clone(),
            ))
        })
    }

    fn with(
        answer: impl Fn() -> Result<HttpResponse, TransportError> + Send + Sync + 'static,
    ) -> Self {
        Self {
            answer: Arc::new(answer),
            sent: Arc::default(),
        }
    }

    fn answer(&self, request: HttpRequest) -> Result<HttpResponse, TransportError> {
        self.sent.lock().unwrap().push(request);
        (self.answer)()
    }

    fn only_request(&self) -> HttpRequest {
        let sent = self.sent.lock().unwrap();
        assert_eq!(sent.len(), 1, "expected exactly one request");
        sent[0].clone()
    }

    fn sent(&self) -> usize {
        self.sent.lock().unwrap().len()
    }
}

impl repay::transport::Transport for FakeTransport {
    async fn send(&self, request: HttpRequest) -> Result<HttpResponse, TransportError> {
        self.answer(request)
    }
}

impl blocking::Transport for FakeTransport {
    fn send(&self, request: HttpRequest) -> Result<HttpResponse, TransportError> {
        self.answer(request)
    }
}

/// Runs a future to completion on the current thread, without an async runtime.
fn block_on<F: Future>(future: F) -> F::Output {
    struct Unpark(std::thread::Thread);
    impl Wake for Unpark {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }
    let waker = Arc::new(Unpark(std::thread::current())).into();
    let mut cx = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
        if let Poll::Ready(output) = future.as_mut().poll(&mut cx) {
            return output;
        }
        std::thread::park();
    }
}

fn capture_body() -> CardCaptureRequest {
    CardCaptureRequest::builder()
        .amount(Amount::from(dec!(10)))
        .build()
}

const APPROVED: &str = r#"{"transaction_id": 42, "result_text": "APPROVED"}"#;

#[test]
fn async_client_sends_through_a_custom_transport() {
    let transport = FakeTransport::json(200, APPROVED);
    let client = Client::with_transport(credentials(), transport.clone());
    let detail = block_on(client.card_transactions().capture(42, &capture_body())).unwrap();
    assert_eq!(detail.result_text.as_deref(), Some("APPROVED"));

    let sent = transport.only_request();
    assert_eq!(sent.method, Method::Post);
    assert_eq!(
        sent.url,
        "https://api.sandbox.repayonline.com/rgapi/v1/transactions/card/42/capture"
    );
    assert_eq!(sent.timeout, Some(DEFAULT_TIMEOUT));
    assert_eq!(sent.header("user-agent"), Some(DEFAULT_USER_AGENT));
    assert_eq!(sent.header("rg-api-secure-token"), Some("s3cret-token"));
    assert_eq!(sent.header("rg-merchant-id"), None);
    assert_eq!(sent.body, br#"{"amount":10.0}"#);
}

#[test]
fn blocking_client_sends_through_a_custom_transport() {
    let transport = FakeTransport::json(200, APPROVED);
    let client = blocking::Client::with_transport(credentials(), transport.clone());
    let detail = client
        .card_transactions()
        .capture(42, &capture_body())
        .unwrap();
    assert_eq!(detail.transaction_id.map(|id| id.get()), Some(42));
    assert_eq!(transport.only_request().method, Method::Post);
}

#[test]
fn builder_settings_reach_the_request() {
    let transport = FakeTransport::json(200, APPROVED);
    let client = ClientBuilder::new(credentials(), transport.clone())
        .environment(repay::Environment::Production)
        .api_version("2")
        .merchant_id(1234)
        .timeout(Duration::from_secs(5))
        .user_agent("my-app/1.0")
        .build();
    block_on(client.card_transactions().capture(42, &capture_body())).unwrap();

    let sent = transport.only_request();
    assert_eq!(
        sent.url,
        "https://api.repayonline.com/rgapi/v2/transactions/card/42/capture"
    );
    assert_eq!(sent.timeout, Some(Duration::from_secs(5)));
    assert_eq!(sent.header("user-agent"), Some("my-app/1.0"));
    assert_eq!(sent.header("rg-merchant-id"), Some("1234"));
}

#[test]
fn a_client_can_act_for_another_merchant() {
    let transport = FakeTransport::json(200, APPROVED);
    let client = ClientBuilder::new(credentials(), transport.clone())
        .merchant_id(1)
        .build();
    block_on(
        client
            .with_merchant_id(2)
            .card_transactions()
            .capture(42, &capture_body()),
    )
    .unwrap();
    block_on(
        client
            .without_merchant_id()
            .card_transactions()
            .capture(42, &capture_body()),
    )
    .unwrap();
    block_on(client.card_transactions().capture(42, &capture_body())).unwrap();

    let sent = transport.sent.lock().unwrap();
    let merchants: Vec<_> = sent.iter().map(|r| r.header("rg-merchant-id")).collect();
    assert_eq!(merchants, [Some("2"), None, Some("1")]);
}

#[test]
fn query_parameters_and_exports() {
    let transport = FakeTransport::with(|| {
        Ok(HttpResponse::new(
            200,
            vec![("content-type".into(), "text/csv".into())],
            "transaction_id\n42\n",
        ))
    });
    let client = Client::with_transport(credentials(), transport.clone());
    let params = card_transactions::SearchParams::builder().take(5).build();
    let file = block_on(
        client
            .card_transactions()
            .search_export(&params, ExportFormat::Csv),
    )
    .unwrap();
    assert_eq!(file.bytes, b"transaction_id\n42\n");
    let sent = transport.only_request();
    assert!(
        sent.url
            .ends_with("/transactions/card/query/export?take=5&format=csv")
    );
    assert_eq!(sent.header("accept"), Some("*/*"));
}

#[test]
fn an_invalid_request_is_not_sent() {
    let transport = FakeTransport::json(200, "{}");
    let client = blocking::Client::with_transport(credentials(), transport.clone());
    let body = CardCaptureRequest::builder()
        .amount(Amount::from(dec!(0.001)))
        .build();
    let err = client.card_transactions().capture(42, &body).unwrap_err();
    assert!(matches!(err, Error::Validation(_)), "{err:?}");

    // A required merchant ID is checked too.
    let body = CloseCardSettlementBatchRequest::builder()
        .batch_id(1)
        .build();
    let err = client.batches().close_card(&body).unwrap_err();
    assert_eq!(
        err.to_string(),
        "invalid request: rg-merchant-id: is required"
    );
    assert_eq!(transport.sent(), 0);
}

#[test]
fn validation_can_be_turned_off() {
    let transport = FakeTransport::json(200, "{}");
    let client = blocking::ClientBuilder::new(credentials(), transport.clone())
        .validate_requests(false)
        .build();
    let body = CloseCardSettlementBatchRequest::builder()
        .batch_id(1)
        .build();
    client.batches().close_card(&body).unwrap();
    assert_eq!(transport.sent(), 1);
}

#[test]
fn transport_errors_are_returned() {
    let transport = FakeTransport::with(|| Err(TransportError::timeout("too slow")));
    let client = Client::with_transport(credentials(), transport);
    let err = block_on(client.health().check()).unwrap_err();
    let Error::Transport(err) = err else {
        panic!("expected a transport error, got {err:?}");
    };
    assert!(err.is_timeout());
}

#[test]
fn debug_output_hides_the_secure_token() {
    let client = Client::with_transport(credentials(), FakeTransport::json(200, "{}"));
    let debug = format!("{client:?}");
    assert!(debug.contains("api-user"), "{debug}");
    assert!(!debug.contains("s3cret-token"), "{debug}");
}

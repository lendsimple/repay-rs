//! Helpers shared by integration tests.

// Each test crate compiles this module and uses only part of it.
#![allow(dead_code, unused_macros)]

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use repay::Credentials;

pub const API_USER: &str = "api-user";
pub const SECURE_TOKEN: &str = "s3cret-token";

pub fn credentials() -> Credentials {
    Credentials::new(API_USER, SECURE_TOKEN)
}

pub fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/responses")
}

pub fn fixture(name: &str) -> Vec<u8> {
    let path = data_dir().join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()))
}

/// What a mock API received.
#[derive(Debug, Clone, Default)]
pub struct Received {
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Received {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

/// A mock Repay API answering one route with one canned response, and recording
/// the request. It matches the method, path, query, and the client's credential,
/// `Accept` and `User-Agent` headers; anything else gets mockito's 501.
pub struct MockApi {
    server: mockito::ServerGuard,
    mock: mockito::Mock,
    received: Arc<Mutex<Option<Received>>>,
}

/// One canned answer.
pub struct Answer {
    pub status: usize,
    pub content_type: &'static str,
    pub body: Vec<u8>,
    pub delay: std::time::Duration,
}

impl Answer {
    pub fn json(status: usize, body: impl Into<Vec<u8>>) -> Self {
        Self {
            status,
            content_type: "application/json; charset=utf-8",
            body: body.into(),
            delay: std::time::Duration::ZERO,
        }
    }

    pub fn file(content_type: &'static str, body: impl Into<Vec<u8>>) -> Self {
        Self {
            status: 200,
            content_type,
            body: body.into(),
            delay: std::time::Duration::ZERO,
        }
    }

    pub fn delayed(mut self, delay: std::time::Duration) -> Self {
        self.delay = delay;
        self
    }
}

impl MockApi {
    fn mock(
        server: &mut mockito::ServerGuard,
        method: &str,
        path_and_query: &str,
        answer: Answer,
    ) -> (mockito::Mock, Arc<Mutex<Option<Received>>>) {
        let received = Arc::new(Mutex::new(None));
        let captured = Arc::clone(&received);
        let (path, query) = path_and_query
            .split_once('?')
            .unwrap_or((path_and_query, ""));
        let query = if query.is_empty() {
            mockito::Matcher::Missing
        } else {
            mockito::Matcher::Exact(query.to_owned())
        };
        let Answer {
            status,
            content_type,
            body,
            delay,
        } = answer;
        let mock = server
            .mock(method, path)
            .match_query(query)
            .match_header("rg-api-user", API_USER)
            .match_header("rg-api-secure-token", SECURE_TOKEN)
            .match_header("accept", mockito::Matcher::Any)
            .match_header("user-agent", mockito::Matcher::Regex("^repay-rs/".into()))
            .with_status(status)
            .with_header("content-type", content_type)
            .with_body_from_request(move |request| {
                *captured.lock().unwrap() = Some(Received {
                    headers: request
                        .headers()
                        .iter()
                        .map(|(n, v)| (n.to_string(), v.to_str().unwrap_or("").to_owned()))
                        .collect(),
                    body: request.body().unwrap().clone(),
                });
                std::thread::sleep(delay);
                body.clone()
            });
        (mock, received)
    }

    pub fn new(method: &str, path_and_query: &str, answer: Answer) -> Self {
        let mut server = mockito::Server::new();
        let (mock, received) = Self::mock(&mut server, method, path_and_query, answer);
        Self {
            mock: mock.create(),
            server,
            received,
        }
    }

    pub async fn new_async(method: &str, path_and_query: &str, answer: Answer) -> Self {
        let mut server = mockito::Server::new_async().await;
        let (mock, received) = Self::mock(&mut server, method, path_and_query, answer);
        Self {
            mock: mock.create_async().await,
            server,
            received,
        }
    }

    /// The API's base URL, as an environment.
    pub fn environment(&self) -> repay::Environment {
        repay::Environment::Custom(format!("{}/rgapi", self.server.url()))
    }

    /// The request received; panics if there was none.
    pub fn received(&self) -> Received {
        self.mock.assert();
        self.received
            .lock()
            .unwrap()
            .clone()
            .expect("the client sent a request")
    }

    /// Checks that no request reached the mock.
    pub fn assert_not_called(&self) {
        assert!(
            self.received.lock().unwrap().is_none(),
            "the request was sent"
        );
    }
}

/// Captures `tracing` output, for checking what the clients log.
#[derive(Clone, Default)]
pub struct LogCapture(Arc<Mutex<Vec<u8>>>);

impl LogCapture {
    /// A subscriber that writes every event, at all levels, here.
    pub fn subscriber(&self) -> impl tracing::Subscriber + Send + Sync + 'static {
        let writer = self.clone();
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::TRACE)
            .with_ansi(false)
            .with_writer(move || writer.clone())
            .finish()
    }

    pub fn contents(&self) -> String {
        String::from_utf8(self.0.lock().unwrap().clone()).unwrap()
    }
}

impl std::io::Write for LogCapture {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

# repay-gateway

[![CI](https://github.com/lendsimple/repay-rs/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/lendsimple/repay-rs/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/repay-gateway.svg)](https://crates.io/crates/repay-gateway)
[![docs.rs](https://docs.rs/repay-gateway/badge.svg)](https://docs.rs/repay-gateway)

A typed Rust client for the [Repay](https://www.repay.com/) Gateway API (Repay Online),
with async and blocking clients, for native targets and WebAssembly.

- Every request and response type, and all 138 operations, are generated from Repay's
  own Swagger specifications: card and ACH transactions, tokens, customers, contracts,
  reports, digital wallets, fraud velocity, routing rules, onboarding, and the
  internal Adjustments API.
- Requests are checked before they are sent: the spec's lengths and patterns, money
  amounts with at most two decimal places, and exactly one payment source (a card
  number with its expiration date, a token, or device data).
- Card numbers, CVVs, tokens and the secure token never appear in `Debug` output or
  logs.
- The clients are thin layers over a protocol layer without I/O, so any HTTP client
  can send the requests.

```toml
[dependencies]
repay-gateway = "0.1"
```

The library is imported as `repay` (the package name `repay` was already taken on
crates.io): `use repay::Client;`.

| Feature | Default | |
|---|---|---|
| `reqwest` | yes | `ReqwestTransport`: the async `Client` sends with reqwest, on tokio (or `fetch` in WebAssembly) |
| `ureq` | | `UreqTransport`: the `blocking::Client` sends with ureq, with no async runtime |
| `rustls` | yes | TLS with rustls |
| `native-tls` | | TLS with the platform's library |

The clients themselves need no feature: you can give them any HTTP client (see
[Other HTTP clients](#other-http-clients)).

The minimum supported Rust version is 1.88.

## A card sale

```rust,no_run
use repay::schema::CardSaleRequest;
use repay::types::{Amount, ExpDate};
use repay::{Client, Credentials, Environment, dec};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::builder(Credentials::new("api user", "secure token"))
        .environment(Environment::Sandbox)
        .merchant_id(1234)
        .build();

    let mut sale = CardSaleRequest::keyed(
        Amount::from(dec!(19.99)),
        "4111111111111111",
        ExpDate::new(12, 2030)?,
    )
    .with_cvv("123");
    sale.invoice_id = Some("INV-1".into());

    let detail = client.card_transactions().sale(&sale).await?;
    println!("{:?}: {:?}", detail.transaction_id, detail.result_text);
    Ok(())
}
```

Operations are grouped by resource, as in Repay's documentation:
`client.card_transactions()`, `ach_transactions()`, `card_tokens()`, `customers()`,
`contracts()`, `ach_reporting()`, and so on. Each method takes the path parameters,
then the body, then the query parameters, if the operation has them.

Requests have builders (`CardSaleRequest::builder()`), and shortcuts for the usual
payment sources: `CardSaleRequest::keyed`, `::token` and `::device`, and
`AchSaleRequest::account` and `::token`. A request with no payment source, or two,
fails validation before it is sent.

### The blocking client

With the `ureq` feature:

```rust,no_run
use repay::blocking::Client;
use repay::schema::{AchAccount, AchSaleRequest};
use repay::types::Amount;
use repay::{Credentials, dec};

fn main() -> Result<(), repay::Error> {
    let client = Client::builder(Credentials::new("api user", "secure token"))
        .merchant_id(1234)
        .build();
    let account = AchAccount::new("123456789", "021000021", "Ann Smith")
        .with_account_type("CHECKING");
    let result = client
        .ach_transactions()
        .sale(&AchSaleRequest::account(Amount::from(dec!(25)), account))?;
    println!("{}", result.as_value());
    Ok(())
}
```

Do not call the blocking client from async code: it blocks the thread.

### Query parameters, reports and files

Operations with query parameters take a parameters struct with a builder; pass
`&Default::default()` for none. Reports can also be fetched as files:

```rust,no_run
use repay::operations::card_transactions::SearchParams;
use repay::{Client, Credentials, ExportFormat};

# async fn run() -> Result<(), repay::Error> {
let client = Client::new(Credentials::new("api user", "secure token"));
let params = SearchParams::builder()
    .card_holder("Ann Smith")
    .take(50)
    .build();

let page = client.card_transactions().search(&params).await?;
let csv = client
    .card_transactions()
    .search_export(&params, ExportFormat::Csv)
    .await?;
std::fs::write(csv.filename.as_deref().unwrap_or("transactions.csv"), &csv.bytes)
    .expect("writable");
# let _ = page;
# Ok(())
# }
```

Many responses are typed (`TransactionDetailCardResponse`, ...). Where the spec does
not describe a response, it is a [`Json`](https://docs.rs/repay-gateway/latest/repay/types/struct.Json.html)
value: read it with `get`, or with `deserialize::<T>()` into your own type.

### Bulk adjustments

The internal Adjustments API takes a CSV file. `BulkAdjustment` writes it, after the
same checks the API applies, and the answer is either the result or the rows that
failed:

```rust,no_run
use repay::bulk::{BulkAdjustment, BulkOutcome};
use repay::types::Amount;
use repay::{Client, Credentials, dec};

# async fn run() -> Result<(), Box<dyn std::error::Error>> {
let client = Client::new(Credentials::new("api user", "secure token"));
let mut adjustments = BulkAdjustment::new();
adjustments.row(1234, 987654, Amount::from(dec!(100)), Amount::from(dec!(25)));

match client
    .adjustments()
    .bulk_disbursement_funding(&adjustments.to_upload()?)
    .await?
{
    BulkOutcome::Completed(result) => println!("{}", result.as_value()),
    BulkOutcome::Failed(report) => {
        for row in 0..report.rows().len() {
            println!("{:?}", report.get(row, "Error"));
        }
    }
    _ => {}
}
# Ok(())
# }
```

### Acting for a merchant

Reseller users act for a merchant by sending its ID as the `rg-merchant-id` header.
Set it on the client (`.merchant_id(1234)`), and switch it for a call with
`client.with_merchant_id(5678)`, which shares the connection pool. Operations that
require it fail validation without it.

## Errors

Every call returns `Result<_, repay::Error>`:

- `Error::Validation`: the request breaks the spec's constraints, so it was not sent.
  Each violation names the field, such as `exp_date: is required with card_number`.
- `Error::Api`: the API answered with an error status and an `ErrorResponse`.
  `ApiError` gives the status, the message, and `validation_errors()`.
- `Error::Status`: an error status without an `ErrorResponse`, such as a gateway's
  HTML page, with the start of the body.
- `Error::Decode` and `Error::UnexpectedContentType`: a successful response that could
  not be read.
- `Error::Transport`: no response arrived (connection, TLS, or timeout; see
  `TransportError::is_timeout`).

A declined transaction is not an error: Repay answers it as a successful response
(some operations with HTTP 201), and the transaction's `result` and `result_text`
say what happened.

## Other HTTP clients

The clients send requests through a `Transport` (async) or `blocking::Transport`. To
use another HTTP client, implement it and pass it to `Client::with_transport`:

```rust
use repay::TransportError;
use repay::transport::{HttpRequest, HttpResponse, Transport};

struct MyTransport;

impl Transport for MyTransport {
    async fn send(&self, request: HttpRequest) -> Result<HttpResponse, TransportError> {
        // Send `request.method` to `request.url` with `request.headers` and
        // `request.body`, within `request.timeout`.
        # let _ = request;
        # Ok(HttpResponse::new(200, Vec::new(), Vec::new()))
    }
}
```

Or skip the clients: `repay::protocol::encode` turns an operation, such as
`repay::operations::card_transactions::Sale`, into an `HttpRequest`, and
`protocol::decode` reads the response.

## WebAssembly

The crate builds for `wasm32-unknown-unknown`, where `ReqwestTransport` uses the
JavaScript `fetch` API, and for WASI (`wasm32-wasip2`) with a transport of your own.
On WebAssembly the transport traits do not require `Send`.

The secure token grants full access to the merchant's account. Use the WebAssembly
builds on servers, edge workers and WASI hosts; never ship the token to a browser.
Browser code should call your own backend, which calls Repay.

## Logging

The clients log through [`tracing`](https://docs.rs/tracing): every request runs in a
`repay` span with the operation's name (`operation="card_transactions.sale"`). At
`DEBUG` they log the method, URL, status and errors; at `TRACE`, the bodies. Card
numbers, CVVs, expiration dates, account numbers and tokens are replaced by
`[REDACTED]` in both, and headers (which carry the secure token) are never logged.

## Development

The schema code in `repay/src/schema/` is generated from the vendored specs in
`schema/`:

```sh
cargo xtask codegen          # regenerate after changing schema/*.json or schema/overrides.toml
cargo xtask codegen --check  # CI: the committed code is up to date
cargo xtask drift            # compare the vendored specs with the sandbox's
cargo xtask fetch            # download the sandbox's specs over the vendored ones
```

`schema/overrides.toml` records every correction to the specs, each with its reason.
The sandbox tests need credentials:

```sh
REPAY_API_USER=... REPAY_SECURE_TOKEN=... REPAY_MERCHANT_ID=... \
    cargo test --all-features --test sandbox -- --ignored --test-threads 1
```

## License

MIT

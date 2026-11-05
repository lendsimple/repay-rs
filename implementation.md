# repay-rs — Implementation Plan

## Context

This is an idiomatic Rust client for the **Repay Gateway API** ("RGAPI", Repay Online). It works on native targets and on `wasm32`. It mirrors the architecture of `~/Web/authorizenet-rs` (v0.2.0):
- sans-IO protocol core;
- pluggable transports;
- `operation_table!` macro DSL;
- bon builders;
- committed, checked codegen;
- the same CI and release setup.

The main difference is that Repay is a JSON/REST API (many paths and verbs) rather than a single XML POST endpoint. So serde replaces the XML runtime, and each operation needs a method, a path template, query and header parameters, and a body.

**Specs (both Swagger 2.0, served by Swashbuckle):**

| Spec | Source | Size |
|---|---|---|
| Public `v1` | `https://api.sandbox.repayonline.com/rgapi/v1/swagger` | 104 paths / 134 operations / 132 definitions |
| Internal `v1-internal` (= current `repay.json`) | `https://api.sandbox.repayonline.com/rgapi/v1-internal/swagger` | 4 operations (Adjustments) / 20 definitions |

All 20 internal definitions except `TransactionAdjustmentRequest` are identical to their public counterparts. The codegen merges both specs.

### Decisions already made

| Topic | Decision |
|---|---|
| Scope | Full public v1 spec plus the internal Adjustments operations. The design must make adding endpoints a matter of re-running codegen. |
| Untyped responses | About 73 operations declare `200: {type: object}`. These return `Json` (a `serde_json::Value` newtype with `.deserialize::<T>()`). No capture-and-type effort for now. |
| Money | `Amount` is a `rust_decimal::Decimal` newtype, sent as a JSON number, and reads numbers losslessly (see Types). |
| API version | `v{version}` path segment; the client config holds `api_version`, default `"1"`, overridable via the builder. |
| Clients | Async (reqwest, default; wasm via fetch) and blocking (ureq). Both are generated from one operation table, with pluggable `Transport` traits (`Send` bounds on native, none on wasm32). |
| Bulk / CSV endpoints | Modelled from the descriptions: we build the multipart body ourselves, and the response is an enum chosen by Content-Type. |
| Naming | Crate `repay`, plus `repay-derive` (redacted `Debug` and `Validate`). Workspace `repay-rs`, repo `lendsimple/repay-rs`. |
| Toolchain | Edition 2024, resolver 3, MSRV 1.88, MIT license, `#[non_exhaustive]` everywhere public. Same as authorizenet-rs. |

## What the spec tells us (subtleties to honor)

### Transport and protocol
- **Base URL:** `https://{host}/rgapi/v{version}/…`. The sandbox host is `api.sandbox.repayonline.com`. The production host is *not in the spec*; we assume `api.repayonline.com` and verify it (open item).
- **Auth is plain headers, not `securityDefinitions`:**
  - `rg-api-user` and `rg-api-secure-token` are required on **every** operation, including `/health`.
  - `rg-merchant-id` is optional on 12 operations and required on 1. It is a client-level default with a per-call override.
- **Content negotiation:**
  - Every operation `produces` JSON *and* XML, and request bodies accept JSON, XML or form data.
  - We must send `Accept: application/json` and `Content-Type: application/json` explicitly, or the server may answer in XML.
  - 50 operations list `consumes: []`. These have no body, or a multipart body (see Bulk).
- **Responses:**
  - 73 operations declare only `200`; 57 declare `200` + `400` (`ErrorResponse`).
  - A few have extras: `500`; `201` (fraud template create); `202` (`CloseCardSettlementBatchResponse` — the *typed* body is on 202, while 200 is untyped); `404` (ITR warning context).
  - Descriptions mention undeclared 403, 404, 415 and 500 codes.
  - Decoding is therefore driven by Content-Type plus status, not only by the declared codes.
- **`ErrorResponse`:** `{ message, status_code, validation_errors[{property, message, custom_field, error_code}], customer_ids }`. `status_code` is a .NET `HttpStatusCode` *name* string ("BadRequest"), not a number.

### Parameters
- **ASP.NET model-binding names:** query names such as `request.transaction_id` and `query.start_date` must be sent verbatim.
- **Duplicated wallet merchant id:** `/digitalwallet/merchants/{request.wallet_merchant_id}` declares `request.wallet_merchant_id` as **both** a path and a (required, patterned) query parameter. We send the path value only and drop the query duplicate through an override; whether the server needs both is an open item.
- **`collectionFormat: multi`:** `query.custom_fields` is a multi array with empty `items`, so it is sent as repeated keys. serde_urlencoded can't do this, so we use a small custom query writer.
- **Pagination:** `skip`/`take` on 11 operations; cursor-based on card-token events.
- **`format` query param:** on 11 report operations: `csv` / `excel` / `pdf` / null = JSON. This changes the response media type, so it is modelled as separate *export* methods returning `Download`, not as a raw string.
- **Inconsistent ID types:** `transaction_id` is `int32` on 18 paths but `string` on ACH void. `audit_key` is a uuid. `template_id` and `org_id` are strings.
- **Path enum:** `report_type` is a path enum (`Default, Card, Check, Cash, ActiveSafe, SalesByDate, …`).
- **Dates in query parameters:** they are `date-time`, but some (`start_date`/`end_date` on history rules) are plain strings.

### Schema quirks
- **`required` is unreliable:**
  - `AchSaleRequest` marks `account_number`, `transit_number`, `name_on_check` *and* `ach_token` as required. The descriptions say these are alternatives ("Required if AchToken is not provided").
  - `CardSaleRequest` has no `required`, but needs *either* `card_number`+`exp_date` *or* `card_token` *or* `device_card_data` (or `magnetic_stripe_data`).
  - These become source enums through overrides (see Ergonomics). The other `required` lists (33 definitions) are kept, but reviewed one by one.
- **Untyped `card_token`:** it is `type: object` in the card sale, auth, refund, disbursement and account-validate requests, but a string in responses. It is retyped to `String`.
- **Inconsistent amount types:** amounts are `number/double`, except `surcharge` (a *string*, on `CardSaleRequest`/`CardAuthRequest`) and `total_amount` in the query models (a string filter).
- **Enums (36 properties):**
  - `Address.state`: 77 values including `Unknown`.
  - `country_code`: Unknown/CAN/USA.
  - `currency`: Unknown/CAD/USD.
  - Name-match codes `"0"/"1"/"2"`, each with a parallel `_text` enum ("Match", "Partial Match", "No Match").
  - OFAC codes `"0"/"1"`.
  - `ContractRequest.status` is a string enum but *also* has `minimum: 0, maximum: 2`, which is .NET enum leakage. Verify whether it is serialized as a string or an int.
  - Every enum gets an `Other(String)` fallback, and repeated value sets are deduplicated into shared enums.
- **Patterns use lookaround**, which the `regex` crate doesn't support, so each one maps to a named hand-written validator:
  - `^(?!.*<).*$` (16×) → "no `<`";
  - the wallet-merchant-id deny-set (5×);
  - one hostname pattern.
- **`.NET` leakage and naming:**
  - Unreferenced definitions are skipped: `CancellationToken`, `WaitHandle`, `SafeWaitHandle`, `GetCardTransactionsRequest`, `DigitalWalletUriRequest`.
  - Generic names `TransactionsReportResponse[ReconciliationCardTransactionData]` / `[…RegionalTransactionDetailData]` become `TransactionsReportResponse<T>`.
- **Field casing:**
  - All properties are snake_case.
  - The two schema `example`s use camelCase, which suggests the server binds case-insensitively. We always send snake_case.
- **Free-form maps:** `additionalProperties: string` maps (`validation_errors`, `account_returns_history`, the onboarding endpoints' `Dictionary<string,string>` bodies) become `BTreeMap<String, String>`. `RuleAction.params` / `RuleCondition.value` become `Json`.
- **`readOnly` (23 properties):** response-only, so they are left out of request builders.
- **Undocumented formats:**
  - `date-time` values come from .NET and may lack an offset.
  - `exp_date` is `MMYY` (example `"1230"`).
  - `date_of_birth` is "MM/DD/YYYY or similar".
- **Deprecated:** `ResellerReports_QueryTransactions` is deprecated, so it gets `#[deprecated]`.

### Bulk and files
- **Multipart CSV with no declared file parameter:**
  - Adjustments bulk (internal) needs columns `MerchantId, TransactionId, OriginalAmount, AdjustmentAmount`, header optional, one file. On full success it returns JSON (`BulkTransactionProcessingResult`); on partial or full failure, a **CSV** with per-row errors.
  - Card bulk reversal returns 415 if the body is not multipart.
- **Template download:** `…/bulk/template` returns a CSV file.
- **PDF:** auth forms take a `pdf` flag that switches to PDF output.

## Architecture

### Workspace layout
```
repay-rs/
├── Cargo.toml                # workspace (repay, repay-derive, xtask)
├── .cargo/config.toml        # alias xtask; REPAY_UI_TESTS=1
├── schema/
│   ├── repay-v1.json           # vendored public spec
│   ├── repay-v1-internal.json  # vendored internal spec (moved from ./repay.json)
│   ├── overrides.toml          # every entry commented with its reason; stale entry = error
│   └── README.md               # source URLs, fetch date, SHA-256, update steps
├── repay/
│   ├── src/
│   │   ├── lib.rs  config.rs  credentials.rs  error.rs  protocol.rs  transport.rs
│   │   ├── client.rs  blocking.rs  operations.rs  query.rs  multipart.rs
│   │   ├── types/ (amount.rs, datetime.rs, ids.rs, json.rs, download.rs)
│   │   ├── validate.rs
│   │   └── schema/ (models.rs, enums.rs, operations.rs  ← @generated; ext.rs hand-written)
│   ├── tests/  examples/
├── repay-derive/             # #[derive(Model)]: redacted Debug + Validate
├── xtask/                    # codegen [--check], fetch, drift
├── scripts/test-packaged-crate.sh
├── deny.toml  CHANGELOG.md  RELEASING.md  README.md  LICENSE
└── .github/workflows/{ci,release}.yml, dependabot.yml
```

### Dependencies (`repay`)
- **Wire format:** `serde` (derive) and `serde_json`.
- **Builders, money, dates:** `bon` (builders), `rust_decimal` (`serde`), `time` (parsing/formatting/macros).
- **Errors and secrets:** `thiserror` 2, `secrecy` (`SecretString`).
- **Logging:** `tracing`.
- **CSV:** `csv`, which is pure Rust and wasm-safe, for bulk rows and CSV error reports.
- **HTTP (optional):** `reqwest` (`default-features = false`) and `ureq` 3.
- **Features:** `default = ["reqwest", "rustls"]`, `ureq`, `rustls`, `native-tls`. The TLS features use the `?/` weak syntax.
- **Dev dependencies:** `mockito`, `pretty_assertions`, `tokio`, `tracing-subscriber`, `trybuild`, `wasm-bindgen-test`.
- **Not used, deliberately:**
  - no `rand`: multipart boundaries are deterministic, with a collision check;
  - no `serde_urlencoded`: it can't encode multi arrays;
  - no `regex`: the patterns use lookaround.

### Layer 1: Runtime types and the derive DSL
- **`Amount(Decimal)`:**
  - Serialize: check the scale (≤ 2 fraction digits, |value| < 2^53 cents), then write it as a JSON number. ryu's shortest form gives `19.99`.
  - Deserialize: accept a number or a string. A number goes through ryu's shortest repr and then `Decimal::from_str`, so `19.99` stays exact (`from_f64` can't guarantee that).
  - `AmountString` is the same type, serialized as a string; used for `surcharge`.
  - Re-export `Decimal` and `rust_decimal::dec!`.
- **`DateTime`:** keeps the lexical form and the parsed value, like authorizenet's `XmlDateTime`. It parses RFC 3339 *or* an ISO date-time without an offset, round-trips exactly, and offers `From<OffsetDateTime | PrimitiveDateTime | Date>`.
- **`ExpDate`:**
  - `ExpDate::new(month, year)` writes `MMYY`;
  - `FromStr` accepts `MMYY` and `MM/YY`.
- **ID newtypes:**
  - `TransactionId`, `CustomerKey`, `CardTokenKey`, `AchTokenKey`, `ContractKey`, `AuthFormId`, `RuleId`, `CustomFieldId` (i32), `TemplateId`/`OrgId` (String), `AuditKey` (uuid string).
  - Deserialization is lenient (number or string).
  - Path parameters are typed, so the compiler catches mixing up a customer key with a token key.
- **`Json(serde_json::Value)`:** used for untyped responses. It offers `get`, `as_value`, `into_inner`, and `deserialize::<T: DeserializeOwned>()` for users who bring their own types.
- **`Download { content_type, filename (from Content-Disposition), bytes }`:** used for CSV, Excel and PDF outputs.
- **Enums:**
  - Generated with serde (`rename` per variant), plus `Other(String)` via a custom `Deserialize`.
  - `as_str()` and `Display`.
  - `#[non_exhaustive]`.
- **`repay-derive` → `#[derive(Model)]`** with the helper attribute `#[repay(...)]`:
  - field attributes: `sensitive`, `min_length`, `max_length`, `min`, `max`, `rule = "no_angle_bracket" | "wallet_merchant_id" | "hostname"`, `required` (for an `Option` field that the overrides make mandatory but that stays `Option` on the wire);
  - it generates a **redacted `Debug`** (`[REDACTED]`) and a **`Validate`** that collects `Violation { path, message }`;
  - it rejects generics and conflicting attributes with errors at the right span, covered by trybuild UI tests;
  - wire (de)serialization stays with serde. The derive does only what serde can't.

### Layer 2: Codegen (`cargo xtask codegen [--check]`)
- **Pipeline:**
  1. `swagger.rs` reads both specs with serde into an IR.
  2. `model.rs` merges them and applies `overrides.toml` (`deny_unknown_fields`; a stale key is an error).
  3. `emit.rs` builds output with `quote!`, `syn`, and `prettyplease`.
- **Generated output** goes to `repay/src/schema/{models,enums,operations}.rs`, with an `@generated` header, `#[rustfmt::skip]`, and committed to the repo.
- **What `overrides.toml` covers:**
  - `[operations.<operationId>]`: `resource = "card_transactions"`, `method = "sale"`, and optionally `response = "Json" | "<Type>"`, `response_status = 202`, `export = true` (`format` param), `multipart = "csv"`, `drop_query = [...]`.
  - `skip_definitions`.
  - `[rename]`, including the generic names.
  - `[retype]`: `card_token` → `String`, `surcharge` → `AmountString`, `*_amount`/`amount`/`*fee*`/`surcharge` doubles → `Amount`, ID fields → newtypes, `exp_date` → `ExpDate` (in requests).
  - `[required]` fixes.
  - `[sources.X]` for one-of groups that are flattened into the request.
  - `sensitive`.
  - `[enums]` for dedupe and renames.
  - `[patterns]` mapping each pattern to a rule name.
- **Model output:**
  - **Requests:** `#[derive(Clone, PartialEq, Serialize, Model, bon::Builder)]`, `#[non_exhaustive]`, `#[serde(skip_serializing_if = "Option::is_none")]` on optional fields, `Vec` fields defaulting to empty. `readOnly` fields are dropped.
  - **Responses:** `Deserialize`, with every field `Option` / `#[serde(default)]`, and lenient about unknown fields.
  - Docs come from the spec descriptions, with `\r\n` and HTML entities cleaned up.
- **`operations.rs`** emits the `operation_table!` invocation body (see Layer 3), plus one request-descriptor struct per operation.
- **`cargo xtask fetch`** re-downloads both specs and updates `schema/README.md` (date, SHA-256). Fetching is never part of the build.
- **`cargo xtask drift`** prints the operations and definitions added or removed between the vendored and the live spec.
- insta snapshots of the IR cover the tricky definitions.

### Layer 3: Operations DSL and sans-IO core
- **The `Operation` trait:**
  ```rust
  pub trait Operation {
      type Response: DecodeResponse;            // T: Deserialize | Json | Download | BulkOutcome | ()
      const ID: &'static str;                   // "CardTransactions_ProcessSaleAsync"
      const METHOD: Method;
      fn path(&self, out: &mut PathWriter);     // writes "/transactions/card/123/capture" (escaped)
      fn query(&self, q: &mut QueryWriter) {}   // verbatim ASP.NET names, multi arrays
      fn body(&self) -> Body<'_> { Body::None } // Json(&dyn erased Serialize) | Multipart(..)
      fn merchant_id(&self) -> MerchantHeader { MerchantHeader::None } // Optional/Required
  }
  ```
- **`protocol.rs`** (pure, unit-tested):
  - `encode(&op, &config) -> HttpRequest`: URL = base + `/v{version}` + path + query. Headers: auth, `Accept: application/json`, content type, user agent, and an `rg-merchant-id` override.
  - `decode<R>(status, headers, body) -> Result<R, Error>`:
    - 2xx: decode by Content-Type (JSON / CSV / PDF / other).
    - Non-2xx with a JSON `ErrorResponse` body: `Error::Api`.
    - Otherwise: `Error::Status`.
  - Each body is validated before it is sent (`validate_requests(true)` by default).
- **The `operation_table!` callback macro** (one table, consumed by several macros):
  ```rust
  operation_table! {
      card_transactions => CardTransactions {
          /// Process a card sale.
          sale(POST "/transactions/card/sale", body: CardSaleRequest) -> TransactionDetailCardResponse;
          /// Capture a prior authorization.
          capture(POST "/transactions/card/{transaction_id}/capture",
                  transaction_id: TransactionId, body: CardCaptureRequest) -> TransactionDetailCardResponse;
          /// Query card transactions (CSV/Excel/PDF export via `query_export`).
          query(POST "/transactions/card/query", body: CardTransactionQuery,
                query: Paging) -> Json, export;
          …
      }
      …
  }
  ```
  - `impl_operations!` generates the descriptor structs and the `Operation` impls.
  - `resource_handles!(Client, async)` / `(blocking)` generate `client.card_transactions().capture(id, &req)`:
    - path params are positional arguments;
    - then the body;
    - then an optional query struct (`*Params` with a bon builder) when the operation has query parameters;
    - doc comments flow through.
  - `export` generates a sibling `<method>_export(.., ExportFormat)` that returns `Download`.
  - The generic escape hatch is `client.execute(&op)`.
- **Resource handles** (named from tags): `card_transactions`, `card_tokens`, `ach_tokens`, `ach_transactions`, `ach_reporting`, `ach_batches`, `ach_auth_forms`, `cash_transactions`, `customers` (with nested `customers().card_tokens(key)`-style methods for the customer-scoped routes), `contracts`, `custom_fields`, `digital_wallet`, `digital_wallet_transactions`, `fraud_velocity`, `routing_rules`, `onboarding`, `reseller_reports`, `validation` (card/ACH account validate, funds transfer inquiry, BIN, bank info), `batches` (card settlement close), `adjustments` (internal), `health`.

### Layer 4: Clients and transports
- **Transport types:**
  - `HttpRequest<'a> { method, url, headers: &[(&str, Cow<str>)], body: Vec<u8>, timeout }`
  - `HttpResponse { status, headers: Vec<(String, String)>, body }`
  - authorizenet's version had no method or headers; we need both.
- **Transport traits:** a cfg-split async `Transport` (RPITIT, `Send + Sync` on native) and `blocking::Transport`. `TransportError` has `new`/`timeout`/`is_timeout`.
- **Built-in transports:**
  - `ReqwestTransport`: on wasm32 it uses fetch automatically; timeouts are applied per request.
  - `UreqTransport`: `http_status_as_error(false)` and a 100 MiB body limit.
- **`Client<T = ReqwestTransport>`:**
  - wraps an `Arc<Inner>` and is `Clone`;
  - `Client::new(credentials)`, `Client::builder(creds).environment(..).api_version("1").merchant_id(..).timeout(..).user_agent(..).validate_requests(..).build()`, `with_transport`;
  - the builder methods are shared by both clients via `common_builder_methods!`.
- **`Credentials::new(api_user, secure_token: impl Into<SecretString>)`:** its `Debug` shows only the user. The README warns that secure tokens must never be shipped to browsers; for browser use, proxy through your backend (wasm targets are for edge/WASI/workers).
- **`Environment`:** `Sandbox` (default), `Production`, `Custom(String)`. The URLs are also exposed as constants.
- **Tracing:** `debug_span!("repay", op = ID)`. Bodies are logged at TRACE after redaction (sensitive JSON keys masked, plus `rg-api-secure-token`).

### Errors
```rust
#[non_exhaustive] pub enum Error {
    Api(Box<ApiError>),          // HTTP status + ErrorResponse (message, status_code, validation_errors, customer_ids)
    Validation(ValidationError), // client-side, request not sent
    Decode { source: serde_json::Error, status: u16, body: String /* truncated */ },
    UnexpectedContentType { status: u16, content_type: String },
    Status { status: u16, body: String },
    Transport(TransportError),
}
```
- `ApiError` has `status()`, `message()`, `validation_errors()`, and `is_validation()`.
- A card decline that arrives as a 2xx `TransactionDetailCardResponse` is *not* an error. Helpers `is_approved()`, `is_declined()` and `result()` live in `ext.rs`. Confirm the decline status in the sandbox (open item).

### Ergonomics (hand-written `schema/ext.rs`)
- **Payment-source enums** (via `[sources]`, flattened):
  - `CardSource::Keyed { card_number, exp_date, cvv }`, `::Token { card_token }`, `::Device { device_card_data, test_key }`, `::Swiped { magnetic_stripe_data }`;
  - `AchSource::Account { account_number, transit_number, name_on_check, account_type, check_type }`, `::Token { ach_token }`.
- **Shortcuts:**
  - `CardSaleRequest::keyed(amount, card, exp)` / `::token(amount, token)` return builders.
  - `CardAuthRequest` is similar.
  - `AchSaleRequest::token(..)`.
- **Bulk:**
  - `BulkAdjustment::new().row(merchant_id, transaction_id, original, adjustment)` writes a CSV, which is sent as a multipart file.
  - The response is `BulkOutcome::Success(Json)` or `BulkOutcome::Failed(CsvReport { rows: Vec<BulkRowResult>, raw: Vec<u8> })`.
- **Export:** `ExportFormat::{Csv, Excel, Pdf}`.
- **Pagination:** `Paging { skip, take }`. A cursor helper is provided for card-token events.

Usage:
```rust
let client = repay::Client::builder(Credentials::new("user", token)).merchant_id(1234).build();
let tx = client.card_transactions()
    .sale(&CardSaleRequest::keyed(dec!(19.99), "4111111111111111", ExpDate::new(12, 2030)?)
        .invoice_id("INV-1").build()).await?;
let page: Json = client.ach_reporting().query(&AchTransactionQuery::builder()
    .start_date(start).end_date(end).build(), Paging::take(50)).await?;
```

## Stages

## Stage 1: Workspace, runtime types, derive
**Goal**:
- Workspace skeleton matching authorizenet-rs: Cargo files, `.cargo/config.toml`, `.gitattributes`, deny.toml, LICENSE, CHANGELOG, CI fmt/clippy.
- Vendor both specs into `schema/` with `README.md` (move `./repay.json`).
- `Amount`, `AmountString`, `DateTime`, `ExpDate`, the ID newtypes, `Json`, `Download`.
- The enum `Other` pattern.
- `repay-derive` `#[derive(Model)]` (redacted Debug + Validate) and the validators: lengths, min/max, `no_angle_bracket`, `wallet_merchant_id`, `hostname`.

**Success Criteria**:
- `cargo test --workspace` and clippy `-D warnings` are clean.
- The wasm32 clippy check passes for `--no-default-features`.

**Tests**:
- Amount: 19.99, 0.1+0.2-style inputs, string input, scale > 2 rejected, negative amounts.
- DateTime: with and without offset, exact round-trip.
- ExpDate: MMYY and MM/YY.
- ID lenient deserialization.
- Enum unknown value goes to `Other`.
- Redaction; violation paths.
- trybuild UI errors.

**Status**: Complete

**Notes (as built)**:
- The workspace has `repay` and `repay-derive` only; `xtask` joins in Stage 2, along with the `xtask` cargo alias.
- **String enums:** `string_enum!` is a crate-internal declarative macro rather than a derive. Codegen emits invocations of it. The same goes for the ID newtypes, `int_id!` / `string_id!`. `MerchantId` was added to the ID list.
- **Derive scope:** `#[derive(Model)]` covers structs with named fields only. Enum support, for the flattened source enums, is added in Stage 5.
- **Derive attributes:**
  - `sensitive`, `required`, `min_length`, `max_length`, `min`, `max`, `rule`.
  - A bound must be a decimal string and a rule a valid identifier; both are checked at expansion.
  - A constraint on an unsupported type fails on the field's type: length and rule need `AsRef<str>`, a range needs `validate::Numeric`.
  - An unknown rule fails on the rule's string.
- **Redaction:** a redacted `Option` prints `None` or `[REDACTED]`. The helper type is `repay::__private::Redacted`.
- **Amount deserializing:** reads f64 through `Display` (Rust's shortest round-trip form) rather than the `ryu` crate, so there is no extra dependency.
- **Amount serializing:** an amount serializes as an f64, so `45.00` writes as `45.0`.
- **Amount validation:** `Amount::validate` checks that there are at most 2 decimal places and at most 2^53 cents.
- **Amount strings:** `AmountString` keeps its scale (`"1.50"`).
- **`DateTime` formatting:** `From<PrimitiveDateTime>` writes fractional seconds only when it has any.
- **`ExpDate` ordering:** `ExpDate` orders chronologically.
- **`string_enum!` warnings:** it was `#[allow(unused_macros)]` until the generated schema used it (the allow was removed in Stage 2).
- **CI jobs:** fmt (plus the LICENSE copies), clippy, test (3 OS), msrv 1.88, minimal-versions, wasm (both targets), docs, deny. Feature-matrix, codegen and package jobs come in later stages.

## Stage 2: Swagger codegen
**Goal**:
- `xtask` with `codegen [--check]`, `fetch` and `drift`.
- The IR, merged specs, and `overrides.toml` covering every quirk in "What the spec tells us".
- Generate models, enums and the operation table body.

**Success Criteria**:
- All 132 + 1 definitions are emitted or explicitly skipped.
- All 138 operations are mapped to a resource and method; an unmapped operation is a codegen error.
- `--check` is clean, and the generated code compiles.

**Tests**:
- insta snapshots of the IR for `CardSaleRequest`, `AchSaleRequest` (sources), `TransactionsReportResponse<T>`, `Address` (enum), and `ErrorResponse`.
- A stale override fails.
- `--check` detects a hand edit.

**Status**: Complete

**Notes (as built)**:
- **xtask layout:** `swagger.rs` (serde for the Swagger subset, `deny_unknown_fields`, merge), `model.rs` (IR + overrides), `emit.rs`, `drift.rs` (`fetch`/`drift`). Properties keep spec order (indexmap).
- **Counts:** 133 definitions → 128 structs + 5 skipped; 20 enums; 138 operations in 20 resources.
- **Generic reports:** `TransactionsReportResponse[X]` becomes two concrete structs, `ReconciliationCardTransactionsReport` and `RegionalTransactionDetailReport`, rather than `<T>`: the derive rejects generics, and concrete is simpler.
- **Source enums deferred:** `AchSaleRequest` and the other ACH/card requests have their "alternative" fields made optional through `[required.remove]`. `CardSource`/`AchSource` come in Stage 5, together with enum support in the derive.
- **Roles:** a definition is Request, Response or Both by reachability.
  - Requests: `Serialize + Model + bon::Builder` (`on(String, into)`), `Option` fields skipped when `None`, `Vec` fields defaulted and skipped when empty.
  - Responses: `Deserialize + Default + Model`, every field `Option` (arrays too).
  - Both (the 6 fraud-velocity definitions): both derives.
  - Facets (`max_length`, rules, …) are emitted only for requests, and lengths and rules only on `String` fields. Responses get `sensitive` only.
- **Retyping (`overrides.toml`):**
  - `[retype.by_name]` applies only where the spec's type fits the target (an ID to integers, `Amount` to numbers). That is how the ACH void's string `transaction_id` stays a `String`.
  - `[retype.requests]` (`exp_date` → `ExpDate`, `card_token` → `String`) applies to requests only, so that a response never fails on an unexpected format.
  - `[retype.fields]` makes `CardSaleRequest.surcharge`/`CardAuthRequest.surcharge` `AmountString`.
- **Enums:** `[enums]` maps each enum name to the properties that share it; an unnamed enum property, or two properties with different values under one name, is an error. `"0"/"1"` values get names from `[enum_variants]`.
- **Response type:** taken from the first 2xx with a `$ref` or an array (so the 202 batch close and the 201 template create are typed with no override). Otherwise `Json`; no schema → `()`.
- **Bodies:** the two fraud-velocity merchant bodies stay `Vec<i32>` (they are merchant keys, not transaction IDs).
- **Docs:** cleaned of `\r\n`, entities, the .NET XML doc tags (`<list>`, `<item>`, `<para>`, `<b>`) and `{Namespace.Type}` references, and "Gets or sets" is dropped. Markup is escaped outside code spans.
- **`operation_table!` format** (the input of Stage 3's macros), one block per resource:
  ```rust
  card_transactions {
      /// docs
      capture {
          id: "CardTransactions_ProcessCapture",
          method: POST,
          path: "/transactions/card/{transaction_id}/capture",
          path_params: { transaction_id: $crate::types::TransactionId = "transaction_id", },
          query: { start_date: $crate::types::DateTime = "query.start_date", custom_fields: ::std::vec::Vec<::std::string::String> = "query.custom_fields" multi, },
          body: json($crate::schema::CardCaptureRequest),   // or `none`, `multipart`
          merchant: optional,                                 // or `none`, `required`
          response: $crate::schema::TransactionDetailCardResponse, // or `$crate::types::Json`, `()`
          export: false,
          deprecated: false,
      }
  }
  ```
  Every key is always present and types are fully qualified. The `format` parameter is removed and sets `export: true`. Path placeholders keep the wire names (`{request.wallet_merchant_id}`). A test-only consumer in `schema/mod.rs` checks that every type resolves and that IDs and routes are unique.
- **`fetch`/`drift`:** they ignore response `examples`, because the server stamps the current time into one example on every request. `drift` found no other changes on 2026-11-04.
- **Generated files:** `#[rustfmt::skip]` in `schema/mod.rs`, `linguist-generated` in `.gitattributes`, and a CI `codegen` job.

## Stage 3: Operations DSL, sans-IO protocol, errors
**Goal**:
- The `Operation` trait, `PathWriter` (percent-encoding), and `QueryWriter` (verbatim names, multi arrays, RFC 3339 dates).
- The `operation_table!`, `impl_operations!` and `resource_handles!` macros.
- `protocol::{encode, decode}`, `Error`/`ApiError`, and request validation.

**Success Criteria**:
- Every operation encodes to the exact method, URL and headers.
- Decode covers JSON, CSV, PDF, `ErrorResponse`, non-JSON errors, and empty 200s.

**Tests**:
- Golden request JSON for roughly 40 representative operations (`tests/data/requests/*.json`).
- Response fixtures: `TransactionDetailCardResponse`, `ErrorResponse` with validation_errors, the 202 batch close, and a CSV failure report.
- Query: `query.custom_fields` multi, and the wallet-id duplicate dropped.
- `Accept` is always sent.

**Status**: Complete

**Notes (as built)**:
- **Merchant ID (decided with the user):** a merchant ID configured in the `Context` is sent as `rg-merchant-id` on *every* request. Many operations that don't declare the header say resellers must send it. `MerchantHeader::Required` operations (the batch close) fail validation with `rg-merchant-id: is required` when none is set.
- **Moved to Stage 4:** `resource_handles!`, `Config` with its builder, and tracing. `protocol::Context` holds the credentials, environment, API version (default `"1"`), merchant ID and user agent; Stage 4's `Config` wraps it.
- **Table format, revised:**
  - Optional parts are `[...]` lists of zero or one item, so `macro_rules` matches them with `$(...)?`.
  - Entries are `method => Name { … }`, with docs on the path and query parameters.
  - The keys are `query: [NameParams {…}]`, `body: [json T]` / `[multipart]` / `[]`, `response: json T` / `empty` / `download`, `export: [format "format"]` / `[pdf "pdf"]` / `[]`, and `deprecated: true|false` (matched as `tt`).
- **Operations (`repay::operations`):** `impl_operations!` generates a module per resource.
  - Each operation gets a type, such as `card_transactions::Capture<'a>`:
    - path params are owned;
    - the body and the query params are borrowed;
    - there is a `PhantomData` marker;
    - `new(path params: impl Into<T>…, &body?, &params?)`.
  - Each set of query parameters gets a `NameParams` struct (bon builder, `Default`, every field `Option`; `multi` lists are `Option<Vec<T>>`, sent as repeated keys).
  - Deprecated operations get `#[deprecated]` on `new`.
- **`Operation` trait:**
  - `Response`, `ID`, `NAME` (`card_transactions.sale`), `METHOD`, `MERCHANT`, and hidden `EXPORT`/`ACCEPT`;
  - hidden `path`/`params`/`query`/`body`/`decode`;
  - public `validate`, which checks the body and that path segments are non-empty.
- **No `PathWriter` type:** `query::render_path` fills in the template placeholders and percent-encodes each value as one segment.
- **Exports:** `Export::new(&op, ExportFormat)` for `Exportable` operations. It adds `format=csv|excel|pdf`, sends `Accept: */*`, and returns `Download`.
  - **PDF flag:** the two ACH auth-form GETs default to a PDF, so their plain operation sends `pdf=false`, and their `Export` sends `pdf=true` (the format is ignored).
  - **`request.format`:** dropped from `CardTransactions_GetTransactions` (it selects XML).
  - **Bulk template:** `adjustments.bulk_template` is a `download` by override.
- **Multipart:** implemented here rather than in Stage 5: one `file` part, with a deterministic boundary `repay-boundary` that is lengthened on collision. `types::Upload` is its input.
- **Decoding:**
  - Any 2xx (including the 201 decline and the 202 batch close) is a success.
  - JSON operations accept `application/json`, `text/json`, `+json`, or a missing Content-Type; an empty body reads as `null`.
  - Non-2xx with a JSON `ErrorResponse` carrying a message or validation errors becomes `Error::Api`; anything else becomes `Error::Status`.
  - Error bodies are truncated to 4 KiB.
  - A `Download`'s filename comes from Content-Disposition (`filename*` preferred).
- **`Error` variants:** `Api`, `Validation`, `Encode`, `Decode { source, status, body }`, `UnexpectedContentType`, `Status`, `Transport`.
- **Tests:**
  - insta request snapshots (`repay/tests/requests.rs`, 30 operations) instead of JSON files;
  - response fixtures in `repay/tests/data/responses/`.
  - The CSV failure report fixture moves to Stage 5, with the bulk outcome type.

## Stage 4: Async + blocking clients, transports, wasm
**Goal**:
- `Transport` traits, `ReqwestTransport`, `UreqTransport`.
- `Client` and `blocking::Client` with builders (a `Config` wrapping `protocol::Context`, plus timeout and `validate_requests`), tracing and redaction (`SENSITIVE_FIELDS` in JSON bodies).
- `resource_handles!`, consuming `operation_table!`: `client.card_transactions().capture(id, &body)`, `*_export` siblings for `Exportable` operations, `client.execute(&op)`, and `client.with_merchant_id(..)` for a per-call merchant.

**Success Criteria**:
- The mockito suites pass for both clients.
- The custom-transport test passes with no features.
- `futures_are_send`.
- wasm32-unknown-unknown builds (no features, and `reqwest`); wasm32-wasip2 builds (no features).
- ~~A `wasm-bindgen-test` smoke test with a fake transport runs under node.~~ Dropped (decided with the user): wasm is verified by clippy builds only, as in authorizenet-rs.

**Tests**:
- Per-operation case tables (`client_cases!`) checking method, path, headers and body.
- Timeouts; connection failure; status errors; an invalid request is never sent.
- Logs never contain the secure token, PAN or CVV.

**Status**: Complete

**Notes (as built)**:
- **Table additions:**
  - each resource is `card_transactions => CardTransactions { … }`, naming its handle type;
  - export entries name their client method: `[format "format" search_export]` or `[pdf "pdf" get_pdf]`. macro_rules cannot build identifiers.
- **Transports (`repay::transport`):**
  - `HttpRequest` gained `timeout: Option<Duration>`; `protocol::encode` leaves it `None` and the clients always set it.
  - The async `Transport` and `blocking::Transport` take `HttpRequest` by value; they are `Send + Sync` on native and unbounded on wasm32.
  - `ReqwestTransport` sends the method, headers, a body only when there is one, and a per-request timeout.
  - `UreqTransport` builds an `http::Request` (with a `()` body when empty, so a GET has none), sets `http_status_as_error(false)` and `timeout_global`, and caps bodies at 100 MiB.
- **Clients:**
  - `Client<T = ReqwestTransport>` and `blocking::Client<T = UreqTransport>` hold `Arc<T>` + `Arc<Config>`.
  - `with_merchant_id`/`without_merchant_id` return a client sharing the transport.
  - `execute(&op)` runs any `Operation`, including `Export`.
  - Builders share `common_builder_methods!`: `environment`, `api_version`, `merchant_id`, `timeout`, `user_agent`, `validate_requests`.
  - `Config` (crate-private) wraps `protocol::Context`. `DEFAULT_TIMEOUT` is 60s.
- **Handles:**
  - `resource_handles!` is invoked via the `async_handles!`/`blocking_handles!` wrappers.
  - Handle types live in `repay::client` and `repay::blocking` (for example, `repay::client::CardTransactions`).
  - Each method takes path params (`impl Into<T>`), then `&body`, then `&NameParams` (always required; pass `&Default::default()`).
  - Export siblings add `format: ExportFormat`; `*_pdf` methods take no format.
  - Deprecated operations are `#[deprecated]` on both the type's `new` and the method.
- **Logging:**
  - `debug_span!("repay", operation = NAME)`.
  - DEBUG logs the method and the URL, with sensitive query values (such as `query.card_number`) redacted.
  - TRACE logs JSON bodies with every `SENSITIVE_FIELDS` key redacted at any depth; other bodies appear as `<n bytes of type>`.
  - Headers are never logged.
- **Features:** `default = ["reqwest", "rustls"]`, `ureq`, `rustls`, `native-tls` (weak `?/` deps).
- **Tests:**
  - `tests/common` has `MockApi`, which matches the method, path, exact query, credentials and user agent, and captures headers and body.
  - `client_cases!` has 16 cases shared by `async_client.rs` and `blocking_client.rs`.
  - `custom_transport.rs` runs with no features and no tokio.
  - `logging.rs`; `futures_are_send`.
- **Examples:** `card_sale` (reqwest) and `card_sale_blocking` (ureq) read `REPAY_API_USER`, `REPAY_SECURE_TOKEN` and `REPAY_MERCHANT_ID`. They have not yet been run against the sandbox.
- **CI:** feature matrices for clippy and test; msrv, minimal-versions and docs use `--all-features`; wasm covers `wasm32-unknown-unknown` (none / `reqwest`) and `wasm32-wasip2` (none).

## Stage 5: Bulk, exports, ergonomics, sandbox
**Goal**:
- `BulkAdjustment` CSV builder (producing a `types::Upload`); `BulkOutcome` (JSON success or CSV failure report). The multipart encoder, template download, exports and auth-form PDF flag were done in Stage 3.
- Source enums and shortcuts in `ext.rs`; paging helpers.
- `#[ignore]`d sandbox tests.
- Examples: `card_sale`, `card_sale_blocking`, `ach_sale`, `card_tokens`, `customers`, `bulk_adjustment`.

**Success Criteria**:
- The multipart body is byte-exact against a golden file, and the boundary never appears in the content.
- Sandbox tests pass when `REPAY_API_USER`, `REPAY_SECURE_TOKEN` and `REPAY_MERCHANT_ID` are set. They panic if the variables are missing. (Written and compiled; not yet run, because there are no credentials here. The user runs them.)

**Tests**:
- Multipart golden; CSV report parsing with error rows.
- Export Content-Disposition filename parsing.
- ~~Each source enum serializes the right flattened fields.~~ Replaced by `one_of` validation and shortcut tests (decided with the user).
- Sandbox: sale → capture → refund → void; token create/get/delete; health.

**Status**: Complete (sandbox run pending)

**Notes (as built)**:
- **Payment sources (decided with the user):** no flattened source enums, and the derive has no enum support.
  - **`[one_of]`:** a codegen section emits a container attribute such as `#[repay(one_of = "card_number exp_date | card_token | device_card_data | magnetic_stripe_data")]`, and the derive generates `validate::check_one_of`. Exactly one group's key (its first field) must be set, and then the rest of that group. Messages: `set exactly one of card_number, card_token, device_card_data or magnetic_stripe_data[, not a and b]` and `exp_date: is required with card_number`.
  - **Covered:** card sale, auth, unlinked refund, disbursement, account validation, and the 5 ACH requests.
  - **Excluded:** `CreateCardTokenRequest`, because the token *update* operations reuse it with only an expiry.
  - **Derive:** it now accepts this container attribute (any other container attribute is an error), with UI tests.
- **Shortcuts (`schema/ext.rs`):**
  - `CardSaleRequest`/`CardAuthRequest::{keyed, token, device}` and `.with_cvv`;
  - `UnlinkedCardRefundRequest::{keyed, token}`;
  - `CardDisbursementRequest::{keyed, token, device}`;
  - `{AchSale, AchCredit, AchRebill}Request::{account(amount, AchAccount), token(amount, token)}`, and the prenotes without an amount;
  - `AchAccount`, whose `Debug` hides the account number.
  - They return the value, not a builder; fields are public for adjusting.
- **Bulk (`repay::bulk`):**
  - `BulkAdjustment` (`row`, then `to_upload() -> Result<Upload, ValidationError>`, checked like the API: at least one row, positive amounts, adjustment ≤ original, no duplicate merchant/transaction);
  - `BulkReversal` (a `TransactionId` column, assumed);
  - `BulkOutcome::{Completed(Json), Failed(CsvReport)}`; `CsvReport` has `headers`, `rows`, `column`, `get(row, name)` and `raw`, and reads with the `csv` crate.
  - The `response = "download" | "bulk"` override replaces `download = true`; both bulk operations are `bulk` and send `Accept: */*`.
- **Dropped:** `Paging` and the cursor helper. The generated `*Params` structs already carry `skip`/`take`/`cursor`.
- **Examples:** `card_sale` and `card_sale_blocking` (now using shortcuts), `ach_sale`, `card_tokens`, `customers`, `bulk_adjustment`.
- **Sandbox (`tests/sandbox.rs`):** health, sale→void, auth→capture→refund, token create/get/delete, search plus CSV export, an API validation error, and the blocking client. They print what they learn for the open items.

## Stage 6: Hardening, docs, release prep
**Goal**:
- README (features, usage, errors, WebAssembly and security notes, logging, custom transports). Its code blocks run as doctests.
- Rustdoc examples; `docs.rs` config.
- CI: the full matrix (features × OS, MSRV, minimal-versions, wasm, codegen check, docs, package, package tests, deny), with SHA-pinned actions.
- `release.yml` (GitHub Release + Trusted Publishing), `RELEASING.md`, dependabot.

**Success Criteria**:
- CI is green.
- `scripts/test-packaged-crate.sh` passes.
- `cargo publish --dry-run --workspace` passes.

**Tests**: README doctests; the packaged-crate test.

**Status**: Not Started

## Verification (end-to-end)
1. `cargo xtask codegen --check`
2. `cargo test --workspace --all-features`, then `--no-default-features`, then `--features ureq,rustls`
3. `cargo clippy --workspace --all-targets --all-features -- -D warnings`
4. `cargo clippy -p repay --lib --target wasm32-unknown-unknown --features reqwest -- -D warnings` (and `--no-default-features`; plus `wasm32-wasip2`)
5. `wasm-pack test --node repay` (smoke test)
6. `REPAY_API_USER=… REPAY_SECURE_TOKEN=… REPAY_MERCHANT_ID=… cargo test --all-features --test sandbox -- --ignored`
7. `cargo run --example card_sale` against the sandbox
8. `./scripts/test-packaged-crate.sh` and `cargo publish --dry-run --workspace`

## Open items to resolve during implementation (not blocking)
- The production host URL. Assumed to be `https://api.repayonline.com/rgapi`.
- Declines: is a declined sale a 200 with a `result`, or a 400 `ErrorResponse`?
- Does `ContractRequest.status` serialize as a string or an int?
- Is `card_token` in requests a string or a number?
- Does `/digitalwallet/merchants/{id}` also require the query duplicate?
- The multipart form field name for the CSV file (we assume `file`), the exact CSV report columns, and the bulk reversal file's column (we assume `TransactionId`, and that it answers JSON or a CSV like the adjustments).
- The card token response's key field (the `card_tokens` example and sandbox test read `card_token_key`).
- The real shape of `BulkTransactionProcessingResult` (kept as `Json` for now).
- The `date-time` format the server accepts in query parameters (we assume RFC 3339 / ISO 8601 without an offset).
- Exports send `Accept: */*`; confirm the server honours `format` with that, and which formats each report supports.
- Declines: several descriptions say "Returns HTTP 200 if approved or HTTP 201 otherwise", so a 201 with a transaction body likely means declined.
- Later: typing the ~73 `Json` responses (card/ACH tokens, customers, contracts, query pages) from captured fixtures, without breaking changes (we add `*_typed` methods or switch in 0.x).

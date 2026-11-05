# Changelog

All notable changes to `repay` and `repay-derive` are recorded here.
The two crates are released together, with the same version.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and
the crates follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- The Repay Gateway API's Swagger specifications, public and internal, vendored in
  `schema/`.
- `types::Amount`, an exact money amount sent as a JSON number and read from a
  number or a string without float rounding, and `types::AmountString` for the
  amounts the API sends as strings.
- `types::DateTime`, which reads the API's dates with or without an offset or
  fractional seconds and writes back the text it read.
- `types::ExpDate`, a card expiration date sent as `MMYY`.
- Typed IDs (`TransactionId`, `CustomerKey`, `CardTokenKey`, ...), read from numbers
  or strings.
- `types::Json` for responses the schema leaves open, and `types::Download` for
  file responses.
- `validate::Validate`, which collects every violation of the schema's constraints
  with the path of the field, and the `validate::rules` that stand in for the
  schema's patterns.
- `repay-derive`, whose `Model` derive implements `Validate` and a `Debug` that
  redacts card numbers and other secrets.
- `schema`: every request and response type of the API, generated from the specs.
  Requests have builders and check the spec's constraints; responses accept
  `null` for any field and ignore fields they do not know. Enums keep values they
  do not list in `Other`.
- `operations`: a type for every API operation (`card_transactions::Capture`,
  ...), with query parameter builders, and `Export` for reports returned as CSV,
  Excel or PDF.
- `protocol::encode` and `protocol::decode`, which turn an operation into an HTTP
  request and read the response without doing any I/O, so any HTTP client can
  send requests.
- `Credentials`, `Environment`, and `Error` with `ApiError` for the API's error
  responses.
- `Client`, an async client (on reqwest by default, which also runs in
  WebAssembly on `fetch`), and `blocking::Client` (on ureq, with the `ureq`
  feature), with a method per operation: `client.card_transactions().sale(&sale)`.
  Both accept a custom `Transport`, act for a merchant (`merchant_id`,
  `with_merchant_id`), and log through `tracing` with card data and the secure
  token redacted.

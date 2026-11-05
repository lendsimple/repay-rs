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

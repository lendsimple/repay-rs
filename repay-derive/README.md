# repay-gateway-derive

Derive macros for [`repay-gateway`](https://crates.io/crates/repay-gateway): the
validation and redacted `Debug` of its generated schema types. They call into
`repay-gateway`'s internals, so the two crates are released together with the same
version; depend on `repay-gateway` rather than on this crate.

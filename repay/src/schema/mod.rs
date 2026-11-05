//! The API's request and response types, generated from the vendored Swagger
//! specs.
//!
//! Regenerate with `cargo xtask codegen` after changing `schema/*.json` or
//! `schema/overrides.toml`.
//!
//! Request types have builders (`CardSaleRequest::builder()`), and check the
//! spec's constraints with [`Validate`](crate::validate::Validate). Response types
//! leave every field optional, because the API sends `null` freely.

#[rustfmt::skip]
mod enums;
#[rustfmt::skip]
mod models;
#[rustfmt::skip]
// The clients (a later release) consume the table.
#[allow(unused_macros, unused_imports)]
pub(crate) mod operations;

pub use enums::*;
pub use models::*;

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::operations::operation_table;

    /// Lists every operation, and names every type the table mentions so that a
    /// wrong name fails to compile.
    macro_rules! list_operations {
        ($(
            $resource:ident => $handle:ident {$(
                $(#[doc = $doc:literal])*
                $method:ident => $name:ident {
                    id: $id:literal,
                    method: $http:ident,
                    path: $path:literal,
                    path_params: {$(
                        $(#[doc = $pdoc:literal])*
                        $p:ident: $pty:ty = $pwire:literal,
                    )*},
                    query: [$($params:ident {$(
                        $(#[doc = $qdoc:literal])*
                        $q:ident: $qty:ty = $qwire:literal $($multi:ident)?,
                    )*})?],
                    body: [$($body:ident $($bty:ty)?)?],
                    merchant: $merchant:ident,
                    response: $response:ident $($rty:ty)?,
                    export: [$($export:ident $ewire:literal $export_method:ident)?],
                    deprecated: $deprecated:literal,
                }
            )*}
        )*) => {
            const OPERATIONS: &[(&str, &str, &str, &str)] = &[
                $($((stringify!($resource), $id, stringify!($http), $path),)*)*
            ];

            #[allow(dead_code, unused_parens, clippy::type_complexity)]
            fn types() {
                $($(
                    let _: Option<($($pty,)* $($($qty,)*)? $($($bty,)?)? $($rty)?)>;
                )*)*
            }
        };
    }

    operation_table!(list_operations);

    #[test]
    fn every_operation_is_listed_once() {
        assert_eq!(OPERATIONS.len(), 138);
        let ids: HashSet<_> = OPERATIONS.iter().map(|op| op.1).collect();
        assert_eq!(ids.len(), OPERATIONS.len());
        let routes: HashSet<_> = OPERATIONS.iter().map(|op| (op.2, op.3)).collect();
        assert_eq!(routes.len(), OPERATIONS.len());
    }

    #[test]
    fn paths_are_relative_to_the_version() {
        for (_, id, _, path) in OPERATIONS {
            assert!(
                path.starts_with('/') && !path.contains("{version}"),
                "{id}: {path}"
            );
        }
    }
}

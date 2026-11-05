//! Every operation of the API, by resource: one type per operation, such as
//! [`card_transactions::Capture`], and one per set of query parameters, such as
//! [`card_transactions::SearchParams`].
//!
//! Each type implements [`Operation`](crate::Operation), so it can be encoded
//! with [`protocol::encode`](crate::protocol::encode) and sent with any HTTP
//! client. The types are generated from `operation_table!`, the list
//! `cargo xtask codegen` writes from the spec.

/// Generates a module per resource, with each operation's type, its
/// `Operation` impl, and its query parameters' type.
macro_rules! impl_operations {
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
                deprecated: $deprecated:tt,
            }
        )*}
    )*) => {$(
        #[doc = concat!("The `", stringify!($resource), "` operations.")]
        pub mod $resource {$(
            $(
                #[doc = concat!("Query parameters of [`", stringify!($name), "`].")]
                #[derive(Clone, Debug, Default, PartialEq, ::bon::Builder)]
                #[builder(on(::std::string::String, into))]
                #[non_exhaustive]
                pub struct $params {$(
                    $(#[doc = $qdoc])*
                    #[doc = ""]
                    #[doc = concat!("Sent as `", $qwire, "`.")]
                    pub $q: ::core::option::Option<$qty>,
                )*}
            )?

            $(#[doc = $doc])*
            #[doc = ""]
            #[doc = concat!("`", stringify!($http), " ", $path, "`")]
            #[derive(Clone, Debug)]
            pub struct $name<'a> {
                $($p: $pty,)*
                $(body: &'a $crate::operations::impl_operations!(@body_ty $body $($bty)?),)?
                $(params: &'a $params,)?
                marker: ::core::marker::PhantomData<&'a ()>,
            }

            impl<'a> $name<'a> {
                $crate::operations::impl_operations!(@deprecated $deprecated
                    #[doc = concat!(
                        "The `", stringify!($resource), ".", stringify!($method), "` operation."
                    )]
                    #[allow(
                        clippy::new_without_default,
                        clippy::too_many_arguments,
                        clippy::redundant_field_names
                    )]
                    pub fn new(
                        $($p: impl ::core::convert::Into<$pty>,)*
                        $(body: &'a $crate::operations::impl_operations!(@body_ty $body $($bty)?),)?
                        $(params: &'a $params,)?
                    ) -> Self {
                        Self {
                            $($p: $p.into(),)*
                            $(body: $crate::operations::impl_operations!(@id body $body),)?
                            $(params: $crate::operations::impl_operations!(@id params $params),)?
                            marker: ::core::marker::PhantomData,
                        }
                    }
                );
            }

            impl $crate::operation::Operation for $name<'_> {
                type Response = $crate::operations::impl_operations!(@response_ty $response $($rty)?);
                const ID: &'static str = $id;
                const NAME: &'static str = concat!(stringify!($resource), ".", stringify!($method));
                const METHOD: $crate::transport::Method = $crate::operations::impl_operations!(@method $http);
                const MERCHANT: $crate::operation::MerchantHeader =
                    $crate::operations::impl_operations!(@merchant $merchant);
                const EXPORT: ::core::option::Option<$crate::operation::ExportParam> =
                    $crate::operations::impl_operations!(@export $($export $ewire)?);
                $crate::operations::impl_operations!(@accept $response);

                fn path(&self) -> ::std::string::String {
                    $crate::query::render_path($path, &[$(($pwire, &self.$p),)*])
                }

                #[allow(unused_variables)]
                fn params(&self, query: &mut $crate::query::QueryWriter) {
                    $($(
                        query.push_opt($qwire, self.params.$q.as_ref());
                    )*)?
                }

                fn body(&self) -> ::core::result::Result<
                    ::core::option::Option<$crate::operation::Body>,
                    $crate::Error,
                > {
                    $crate::operations::impl_operations!(@encode_body self $($body)?)
                }

                fn validate(&self) -> ::core::result::Result<(), $crate::validate::ValidationError> {
                    #[allow(unused_mut)]
                    let mut violations = ::std::vec::Vec::new();
                    $($crate::operation::check_segment(&self.$p, $pwire, &mut violations);)*
                    $(
                        let _ = stringify!($body);
                        $crate::operation::check_body(self.body, &mut violations);
                    )?
                    $crate::operation::finish(violations)
                }

                fn decode(
                    response: &$crate::transport::HttpResponse,
                ) -> ::core::result::Result<Self::Response, $crate::Error> {
                    $crate::operations::impl_operations!(@decode response $response)
                }
            }

            $($crate::operations::impl_operations!(@exportable $name $export);)?
        )*}
    )*};

    (@body_ty json $ty:ty) => { $ty };
    (@body_ty multipart) => { $crate::types::Upload };

    (@response_ty json $ty:ty) => { $ty };
    (@response_ty empty) => { () };
    (@response_ty download) => { $crate::types::Download };

    (@accept download) => { const ACCEPT: &'static str = "*/*"; };
    (@accept $json_or_empty:ident) => {};

    (@method GET) => { $crate::transport::Method::Get };
    (@method POST) => { $crate::transport::Method::Post };
    (@method PUT) => { $crate::transport::Method::Put };
    (@method PATCH) => { $crate::transport::Method::Patch };
    (@method DELETE) => { $crate::transport::Method::Delete };

    (@merchant none) => { $crate::operation::MerchantHeader::None };
    (@merchant optional) => { $crate::operation::MerchantHeader::Optional };
    (@merchant required) => { $crate::operation::MerchantHeader::Required };

    (@export) => { ::core::option::Option::None };
    (@export format $wire:literal) => {
        ::core::option::Option::Some($crate::operation::ExportParam {
            kind: $crate::operation::ExportKind::Format,
            wire: $wire,
        })
    };
    (@export pdf $wire:literal) => {
        ::core::option::Option::Some($crate::operation::ExportParam {
            kind: $crate::operation::ExportKind::Pdf,
            wire: $wire,
        })
    };

    (@exportable $name:ident $kind:ident) => {
        impl $crate::operation::Exportable for $name<'_> {}
    };

    (@encode_body $self:ident) => { ::core::result::Result::Ok(::core::option::Option::None) };
    (@encode_body $self:ident json) => { $crate::operation::json_body($self.body) };
    (@encode_body $self:ident multipart) => {
        ::core::result::Result::Ok(::core::option::Option::Some($crate::multipart::encode($self.body)))
    };

    (@decode $response:ident json) => { $crate::protocol::decode_json($response) };
    (@decode $response:ident download) => {
        ::core::result::Result::Ok($crate::protocol::decode_download($response))
    };
    (@decode $response:ident empty) => {{
        let _ = $response;
        ::core::result::Result::Ok(())
    }};

    // `$name`, in a repetition driven by `$_driver`.
    (@id $name:ident $_driver:ident) => { $name };

    (@deprecated true $($item:tt)*) => { #[deprecated = "deprecated in the API"] $($item)* };
    (@deprecated false $($item:tt)*) => { $($item)* };
}

pub(crate) use impl_operations;

/// Generates a client's resource handles and their methods, one per operation.
///
/// Arguments: `[Client, TransportTrait, async]` or `[Client, TransportTrait,
/// blocking]`, then the table, where `Client<T>` has an `execute` method for any
/// `T: TransportTrait`.
macro_rules! resource_handles {
    (
        [$client:ident, $transport:path, $mode:ident]
        $(
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
                    query: [$($params:ident { $($query:tt)* })?],
                    body: [$($body:ident $($bty:ty)?)?],
                    merchant: $merchant:ident,
                    response: $response:ident $($rty:ty)?,
                    export: [$($export:ident $ewire:literal $export_method:ident)?],
                    deprecated: $deprecated:tt,
                }
            )*}
        )*
    ) => {
        impl<T: $transport> $client<T> {
            $(
                #[doc = concat!(
                    "The `", stringify!($resource), "` operations: see [`", stringify!($handle), "`]."
                )]
                pub fn $resource(&self) -> $handle<'_, T> {
                    $handle { client: self }
                }
            )*
        }

        $(
            #[doc = concat!(
                "The `", stringify!($resource), "` operations, from [`", stringify!($client),
                "::", stringify!($resource), "`]."
            )]
            pub struct $handle<'a, T> {
                client: &'a $client<T>,
            }

            impl<T> Clone for $handle<'_, T> {
                fn clone(&self) -> Self {
                    *self
                }
            }

            impl<T> Copy for $handle<'_, T> {}

            impl<T> ::core::fmt::Debug for $handle<'_, T> {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    f.debug_struct(stringify!($handle)).finish_non_exhaustive()
                }
            }

            impl<T: $transport> $handle<'_, T> {$(
                $crate::operations::resource_handles!(@item $mode $deprecated
                    [
                        $(#[doc = $doc])*
                        #[doc = ""]
                        #[doc = concat!(
                            "Sends [`", stringify!($resource), "::", stringify!($name),
                            "`](crate::operations::", stringify!($resource), "::",
                            stringify!($name), ")."
                        )]
                    ]
                    pub fn $method(
                        &self,
                        $($p: impl ::core::convert::Into<$pty>,)*
                        $(body: &$crate::operations::impl_operations!(@body_ty $body $($bty)?),)?
                        $(params: &$crate::operations::$resource::$params,)?
                    ) -> ::core::result::Result<
                        $crate::operations::impl_operations!(@response_ty $response $($rty)?),
                        $crate::Error,
                    > {
                        #[allow(deprecated)]
                        let op = $crate::operations::$resource::$name::new(
                            $($p,)*
                            $($crate::operations::impl_operations!(@id body $body),)?
                            $($crate::operations::impl_operations!(@id params $params),)?
                        );
                        $crate::operations::resource_handles!(@await $mode self.client.execute(&op))
                    }
                );

                $crate::operations::resource_handles!(@export [$($export $export_method)?]
                    $mode $deprecated $resource $name
                    [$($p: $pty,)*]
                    [$(body: &$crate::operations::impl_operations!(@body_ty $body $($bty)?),)?]
                    [$(params: &$crate::operations::$resource::$params,)?]
                    [
                        $($p,)*
                        $($crate::operations::impl_operations!(@id body $body),)?
                        $($crate::operations::impl_operations!(@id params $params),)?
                    ]
                );
            )*}
        )*
    };

    (@export [] $($rest:tt)*) => {};

    // The file of a report that has a `format` parameter.
    (@export [format $method:ident] $mode:ident $deprecated:tt $resource:ident $name:ident
        [$($p:ident: $pty:ty,)*] [$($body:tt)*] [$($params:tt)*] [$($arg:tt)*]
    ) => {
        $crate::operations::resource_handles!(@item $mode $deprecated
            [
                #[doc = concat!(
                    "Sends [`", stringify!($resource), "::", stringify!($name),
                    "`](crate::operations::", stringify!($resource), "::", stringify!($name),
                    ") for a file in `format` (CSV, Excel or PDF), instead of JSON."
                )]
            ]
            pub fn $method(
                &self,
                $($p: impl ::core::convert::Into<$pty>,)*
                $($body)*
                $($params)*
                format: $crate::ExportFormat,
            ) -> ::core::result::Result<$crate::types::Download, $crate::Error> {
                #[allow(deprecated)]
                let op = $crate::operations::$resource::$name::new($($arg)*);
                let export = $crate::Export::new(&op, format);
                $crate::operations::resource_handles!(@await $mode self.client.execute(&export))
            }
        );
    };

    // The PDF of an operation whose `pdf` flag switches from JSON.
    (@export [pdf $method:ident] $mode:ident $deprecated:tt $resource:ident $name:ident
        [$($p:ident: $pty:ty,)*] [$($body:tt)*] [$($params:tt)*] [$($arg:tt)*]
    ) => {
        $crate::operations::resource_handles!(@item $mode $deprecated
            [
                #[doc = concat!(
                    "Sends [`", stringify!($resource), "::", stringify!($name),
                    "`](crate::operations::", stringify!($resource), "::", stringify!($name),
                    ") for the PDF, instead of JSON."
                )]
            ]
            pub fn $method(
                &self,
                $($p: impl ::core::convert::Into<$pty>,)*
                $($body)*
                $($params)*
            ) -> ::core::result::Result<$crate::types::Download, $crate::Error> {
                #[allow(deprecated)]
                let op = $crate::operations::$resource::$name::new($($arg)*);
                let export = $crate::Export::new(&op, $crate::ExportFormat::Pdf);
                $crate::operations::resource_handles!(@await $mode self.client.execute(&export))
            }
        );
    };

    (@item async $deprecated:tt [$($attr:tt)*] pub fn $($rest:tt)*) => {
        $crate::operations::impl_operations!(@deprecated $deprecated
            $($attr)* pub async fn $($rest)*
        );
    };
    (@item blocking $deprecated:tt [$($attr:tt)*] pub fn $($rest:tt)*) => {
        $crate::operations::impl_operations!(@deprecated $deprecated
            $($attr)* pub fn $($rest)*
        );
    };

    (@await async $($call:tt)*) => { $($call)*.await };
    (@await blocking $($call:tt)*) => { $($call)* };
}

pub(crate) use resource_handles;

/// `resource_handles!` for the async [`Client`](crate::Client).
macro_rules! async_handles {
    ($($table:tt)*) => {
        $crate::operations::resource_handles!(
            [Client, $crate::transport::Transport, async] $($table)*
        );
    };
}

pub(crate) use async_handles;

/// `resource_handles!` for the [`blocking::Client`](crate::blocking::Client).
macro_rules! blocking_handles {
    ($($table:tt)*) => {
        $crate::operations::resource_handles!(
            [Client, $crate::blocking::Transport, blocking] $($table)*
        );
    };
}

pub(crate) use blocking_handles;

crate::schema::operations::operation_table!(impl_operations);

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use crate::operation::Operation;

    macro_rules! names {
        ($($resource:ident => $handle:ident {$(
            $(#[doc = $doc:literal])*
            $method:ident => $name:ident { $($entry:tt)* }
        )*})*) => {
            const NAMES: &[(&str, &str)] = &[$($(
                (
                    <super::$resource::$name<'_> as Operation>::NAME,
                    <super::$resource::$name<'_> as Operation>::ID,
                ),
            )*)*];
        };
    }

    crate::schema::operations::operation_table!(names);

    #[test]
    fn every_operation_has_a_unique_name_and_id() {
        assert_eq!(NAMES.len(), 138);
        assert_eq!(NAMES.iter().map(|n| n.0).collect::<HashSet<_>>().len(), 138);
        assert_eq!(NAMES.iter().map(|n| n.1).collect::<HashSet<_>>().len(), 138);
        assert!(NAMES.contains(&(
            "card_transactions.sale",
            "CardTransactions_ProcessSaleAsync"
        )));
    }
}

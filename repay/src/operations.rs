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

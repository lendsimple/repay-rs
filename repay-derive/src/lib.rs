//! Derive macros for the [`repay-gateway`](https://docs.rs/repay-gateway) crate
//! (imported as `repay`).
//!
//! They are used by the crate's generated schema types and are not intended as a
//! general framework. Serialization stays with serde; these derives add what serde
//! does not: validation of the Repay schema's constraints, and a `Debug` that hides
//! secrets.
//!
//! # `#[derive(Model)]`
//!
//! On a struct with named fields, implements `repay::validate::Validate` and
//! `Debug`. Do not derive `Debug` separately.
//!
//! The field's type decides how a constraint applies: to the value of a `T`, to the
//! value of an `Option<T>` when it is `Some`, and to each item of a `Vec<T>` or
//! `Option<Vec<T>>`. Every field is also validated recursively through its own
//! `Validate` impl.
//!
//! Field attributes, inside `#[repay(...)]`:
//! - `sensitive`: shown as `[REDACTED]` in `Debug` output.
//! - `required`: on an `Option<T>`, the API needs a value although the wire format
//!   allows leaving it out.
//! - `min_length = N`, `max_length = N`: length in characters, for string values.
//! - `min = "0"`, `max = "1.5"`: inclusive bounds, for numeric values.
//! - `rule = "name"`: a check from `repay::validate::rules`, for string values.
//!
//! Struct attribute, which may repeat:
//! - `one_of = "card_number exp_date | card_token"`: alternatives. Exactly one
//!   group's first field must be set, and then the group's other fields. Every
//!   field named must be an `Option<T>`.

mod attrs;
mod cardinality;
mod model;

use proc_macro::TokenStream;
use syn::{Data, DeriveInput, parse_macro_input};

#[proc_macro_derive(Model, attributes(repay))]
pub fn derive_model(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let result = if !input.generics.params.is_empty() {
        Err(syn::Error::new_spanned(
            &input.generics,
            "Model does not support generic types",
        ))
    } else {
        match &input.data {
            Data::Struct(data) => model::expand(&input, data),
            Data::Enum(_) | Data::Union(_) => Err(syn::Error::new_spanned(
                &input.ident,
                "Model can only be derived for structs with named fields",
            )),
        }
    };
    result.unwrap_or_else(syn::Error::into_compile_error).into()
}

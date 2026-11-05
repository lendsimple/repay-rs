//! `#[derive(Model)]` on structs: `Validate` and a redacting `Debug`.

use proc_macro2::{Literal, TokenStream};
use quote::{quote, quote_spanned};
use syn::ext::IdentExt;
use syn::spanned::Spanned;
use syn::{DataStruct, DeriveInput, Fields, LitStr, Result};

use crate::attrs::{FieldAttrs, reject_container_attrs};
use crate::cardinality::Cardinality;

pub fn expand(input: &DeriveInput, data: &DataStruct) -> Result<TokenStream> {
    reject_container_attrs(&input.attrs)?;
    let Fields::Named(fields) = &data.fields else {
        return Err(syn::Error::new_spanned(
            &input.ident,
            "Model can only be derived for structs with named fields",
        ));
    };

    let mut validations = Vec::new();
    let mut debug_fields = Vec::new();
    for field in &fields.named {
        let attrs = FieldAttrs::parse(&field.attrs)?;
        let ident = field.ident.as_ref().expect("named field");
        let name = ident.unraw().to_string();
        let card = Cardinality::of(&field.ty);
        validations.push(validation(ident, &name, &card, &attrs)?);
        debug_fields.push(debug_field(ident, &name, &card, &attrs));
    }

    let ident = &input.ident;
    let type_name = ident.to_string();
    Ok(quote! {
        #[automatically_derived]
        impl ::repay::validate::Validate for #ident {
            fn validate_into(
                &self,
                path: &mut ::repay::validate::Path,
                out: &mut ::std::vec::Vec<::repay::validate::Violation>,
            ) {
                #(#validations)*
            }
        }

        #[automatically_derived]
        impl ::core::fmt::Debug for #ident {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                f.debug_struct(#type_name)
                    #(#debug_fields)*
                    .finish()
            }
        }
    })
}

/// Checks one field: `required`, its constraints on each value it holds, then the
/// value's own `Validate` impl.
fn validation(
    ident: &syn::Ident,
    name: &str,
    card: &Cardinality<'_>,
    attrs: &FieldAttrs,
) -> Result<TokenStream> {
    let v = quote!(::repay::validate);
    let required = match (attrs.required, card) {
        (None, _) => quote!(),
        (Some(_), Cardinality::Optional(_) | Cardinality::OptionalList(_)) => {
            quote!(#v::check_required(self.#ident.is_some(), path, out);)
        }
        (Some(span), _) => {
            return Err(syn::Error::new(
                span,
                "`required` is only valid on `Option<T>` fields",
            ));
        }
    };

    let checks = value_checks(card, attrs);
    let constrained = if checks.is_empty() {
        quote!()
    } else {
        match card {
            Cardinality::Required(_) => quote! {
                let value = &self.#ident;
                #checks
            },
            Cardinality::Optional(_) => quote! {
                if let ::core::option::Option::Some(value) = &self.#ident {
                    #checks
                }
            },
            Cardinality::List(_) => quote! {
                for (index, value) in self.#ident.iter().enumerate() {
                    path.push_index(index);
                    #checks
                    path.pop();
                }
            },
            Cardinality::OptionalList(_) => quote! {
                if let ::core::option::Option::Some(values) = &self.#ident {
                    for (index, value) in values.iter().enumerate() {
                        path.push_index(index);
                        #checks
                        path.pop();
                    }
                }
            },
        }
    };

    Ok(quote! {
        path.push_field(#name);
        #required
        #constrained
        #v::Validate::validate_into(&self.#ident, path, out);
        path.pop();
    })
}

/// Constraint checks of one value, bound to `value` (a reference). Each is spanned on
/// the value's type, so a constraint on a type that does not support it is reported
/// there.
fn value_checks(card: &Cardinality<'_>, attrs: &FieldAttrs) -> TokenStream {
    let ty = match card {
        Cardinality::Required(ty)
        | Cardinality::Optional(ty)
        | Cardinality::List(ty)
        | Cardinality::OptionalList(ty) => *ty,
    };
    let span = ty.span();
    let v = quote!(::repay::validate);
    let as_str = quote_spanned!(span=> ::core::convert::AsRef::<str>::as_ref(value));
    let mut checks = Vec::new();

    if attrs.min_length.is_some() || attrs.max_length.is_some() {
        let (min, max) = (opt_usize(attrs.min_length), opt_usize(attrs.max_length));
        checks.push(quote!(#v::check_length(#as_str, #min, #max, path, out);));
    }
    if attrs.min.is_some() || attrs.max.is_some() {
        let (min, max) = (opt_str(&attrs.min), opt_str(&attrs.max));
        let numeric = quote_spanned!(span=> #v::Numeric::to_decimal(value));
        checks.push(quote!(#v::check_range(#numeric, #min, #max, path, out);));
    }
    if let Some(rule) = &attrs.rule {
        let func = syn::Ident::new(&rule.value(), rule.span());
        let func = quote_spanned!(rule.span()=> #v::rules::#func);
        checks.push(quote!(#v::check_rule(#as_str, #func, path, out);));
    }
    quote!(#(#checks)*)
}

fn debug_field(
    ident: &syn::Ident,
    name: &str,
    card: &Cardinality<'_>,
    attrs: &FieldAttrs,
) -> TokenStream {
    if !attrs.sensitive {
        return quote!(.field(#name, &self.#ident));
    }
    let present = match card {
        Cardinality::Optional(_) | Cardinality::OptionalList(_) => {
            quote!(self.#ident.is_some())
        }
        Cardinality::Required(_) | Cardinality::List(_) => quote!(true),
    };
    quote!(.field(#name, &::repay::__private::Redacted::new(#present)))
}

fn opt_usize(value: Option<usize>) -> TokenStream {
    match value {
        Some(n) => {
            let n = Literal::usize_unsuffixed(n);
            quote!(::core::option::Option::Some(#n))
        }
        None => quote!(::core::option::Option::None),
    }
}

fn opt_str(value: &Option<LitStr>) -> TokenStream {
    match value {
        Some(s) => quote!(::core::option::Option::Some(#s)),
        None => quote!(::core::option::Option::None),
    }
}

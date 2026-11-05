//! Parsing of `#[repay(...)]` attributes.

use proc_macro2::Span;
use syn::meta::ParseNestedMeta;
use syn::{Attribute, LitInt, LitStr, Result};

/// Attributes on a struct field.
#[derive(Default)]
pub struct FieldAttrs {
    pub sensitive: bool,
    pub required: Option<Span>,
    pub min_length: Option<usize>,
    pub max_length: Option<usize>,
    pub min: Option<LitStr>,
    pub max: Option<LitStr>,
    pub rule: Option<LitStr>,
}

impl FieldAttrs {
    pub fn parse(attrs: &[Attribute]) -> Result<Self> {
        let mut out = Self::default();
        for attr in repay_attrs(attrs) {
            attr.parse_nested_meta(|meta| {
                let int = || -> Result<usize> { meta.value()?.parse::<LitInt>()?.base10_parse() };
                if meta.path.is_ident("sensitive") {
                    out.sensitive = true;
                } else if meta.path.is_ident("required") {
                    out.required = Some(meta.path.segments[0].ident.span());
                } else if meta.path.is_ident("min_length") {
                    out.min_length = Some(int()?);
                } else if meta.path.is_ident("max_length") {
                    out.max_length = Some(int()?);
                } else if meta.path.is_ident("min") {
                    out.min = Some(number(&meta)?);
                } else if meta.path.is_ident("max") {
                    out.max = Some(number(&meta)?);
                } else if meta.path.is_ident("rule") {
                    out.rule = Some(rule(&meta)?);
                } else {
                    return Err(unknown(&meta));
                }
                Ok(())
            })?;
        }
        Ok(out)
    }
}

/// Rejects `#[repay(...)]` where no attributes are supported.
pub fn reject_container_attrs(attrs: &[Attribute]) -> Result<()> {
    match repay_attrs(attrs).next() {
        Some(attr) => Err(syn::Error::new_spanned(
            attr,
            "`#[repay(...)]` attributes go on fields, not on the struct",
        )),
        None => Ok(()),
    }
}

/// A decimal number written as a string, such as `"0"` or `"-1.5"`.
fn number(meta: &ParseNestedMeta) -> Result<LitStr> {
    let lit: LitStr = meta.value()?.parse()?;
    let text = lit.value();
    let digits = text.strip_prefix('-').unwrap_or(&text);
    let (whole, fraction) = digits.split_once('.').unwrap_or((digits, "0"));
    let all_digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    if all_digits(whole) && all_digits(fraction) {
        Ok(lit)
    } else {
        Err(syn::Error::new(
            lit.span(),
            "expected a decimal number, such as \"0\" or \"1.5\"",
        ))
    }
}

/// The name of a function in `repay::validate::rules`.
fn rule(meta: &ParseNestedMeta) -> Result<LitStr> {
    let lit: LitStr = meta.value()?.parse()?;
    if syn::parse_str::<syn::Ident>(&lit.value()).is_err() {
        return Err(syn::Error::new(
            lit.span(),
            "expected the name of a rule in `repay::validate::rules`",
        ));
    }
    Ok(lit)
}

fn repay_attrs(attrs: &[Attribute]) -> impl Iterator<Item = &Attribute> {
    attrs.iter().filter(|a| a.path().is_ident("repay"))
}

fn unknown(meta: &ParseNestedMeta) -> syn::Error {
    let name = meta
        .path
        .get_ident()
        .map(ToString::to_string)
        .unwrap_or_else(|| "?".into());
    meta.error(format!("unknown repay attribute `{name}`"))
}

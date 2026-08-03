//! Serde integration feature module.
//!
//! This module handles serde attribute parsing and field name transformation
//! when the "serde" feature is enabled.

#[cfg(test)]
use crate::rename_rule::resolve_rename_rule;
use proc_macro2::{Delimiter, TokenTree};
use syn::meta::ParseNestedMeta;
use syn::{Attribute, Error, LitStr, Meta, Token};

/// Message of the rejection raised for a serde attribute hidden behind `cfg_attr`.
const CFG_ATTR_SERDE_REJECTION: &str = "cfg_attr-wrapped serde attribute is invisible to \
     model_schema and will be silently ignored by the generator; write #[serde(...)] \
     unconditionally (serde attrs are inert without the serde derive)";

/// Metadata for serde attributes applied to a struct or enum.
#[derive(Clone, Debug, Default)]
pub struct SerdeTypeMeta {
    /// The rejection raised when the walk met a `cfg_attr`-wrapped serde attribute.
    pub cfg_attr_rejection: Option<Error>,
    pub content: Option<String>, // e.g., "value" for adjacently tagged enums
    pub rename_all: Option<String>, // e.g., "camelCase"
    pub tag: Option<String>,     // e.g., "behaviorType"
    pub untagged: bool,          // Whether the enum is `#[serde(untagged)]`
}

/// Metadata for serde attributes applied to a field.
#[derive(Clone, Debug, Default)]
pub struct SerdeFieldMeta {
    /// The rejection raised when the walk met a `cfg_attr`-wrapped serde attribute.
    pub cfg_attr_rejection: Option<Error>,
    pub flatten: bool, // Whether the field is `#[serde(flatten)]`
    /// Whether a `None` is left out of the serialized output entirely. `skip_deserializing`
    /// does not qualify: it still writes `null` on the way out.
    pub omits_none: bool,
    pub rename: Option<String>, // e.g., "new_name"
    pub skip: bool,             // Whether to skip the field
}

/// The rejection for a `cfg_attr` carrying a `serde(...)` attribute, or `None` for every other
/// `cfg_attr` — gated derives and gated docs stay legal.
///
/// A `cfg_attr` on a field or a variant is expanded only after the attribute proc-macro has been
/// handed the item, so a serde attribute written inside one arrives here unexpanded and would
/// otherwise be walked past in silence. (An item's own attribute list is resolved by rustc first,
/// so those arrive already expanded or already stripped and never reach this check.) Reading the
/// payload would mean applying it in builds where the consumer's cfg predicate is false, and that
/// predicate cannot be evaluated from a proc macro, so the attribute is rejected, not guessed at.
fn cfg_attr_serde_rejection(attr: &Attribute) -> Option<Error> {
    let Meta::List(list) = &attr.meta else {
        return None;
    };
    // Only the top level of the payload is scanned: a `serde` naming the cfg predicate
    // (`cfg_attr(feature = "serde", ..)`) is a string literal, and one naming a nested predicate
    // sits inside a group, so neither can be mistaken for the `serde(...)` attribute itself.
    let mut previous_is_serde = false;
    for token in list.tokens.clone() {
        if previous_is_serde
            && matches!(&token, TokenTree::Group(group) if group.delimiter() == Delimiter::Parenthesis)
        {
            return Some(Error::new_spanned(attr, CFG_ATTR_SERDE_REJECTION));
        }
        previous_is_serde = matches!(&token, TokenTree::Ident(ident) if ident == "serde");
    }
    None
}

/// Parses serde attributes from a struct or enum.
pub fn parse_serde_type_attributes(attrs: &[Attribute]) -> SerdeTypeMeta {
    let mut meta = SerdeTypeMeta::default();

    for attr in attrs {
        if attr.path().is_ident("cfg_attr") && meta.cfg_attr_rejection.is_none() {
            meta.cfg_attr_rejection = cfg_attr_serde_rejection(attr);
        } else if attr.path().is_ident("serde") {
            attr.parse_nested_meta(|nested| {
                // Handle `tag = "value"`
                if nested.path.is_ident("tag") {
                    let value = nested.value()?;
                    let lit: LitStr = value.parse()?;
                    meta.tag = Some(lit.value());
                }
                // Handle `content = "value"` for adjacently tagged enums
                else if nested.path.is_ident("content") {
                    let value = nested.value()?;
                    let lit: LitStr = value.parse()?;
                    meta.content = Some(lit.value());
                }
                // Handle `rename_all = "value"`
                else if nested.path.is_ident("rename_all") {
                    let value = nested.value()?;
                    let lit: LitStr = value.parse()?;
                    meta.rename_all = Some(lit.value());
                }
                // Handle `untagged`
                else if nested.path.is_ident("untagged") {
                    meta.untagged = true;
                } else {
                    // Ignore other serde attributes.
                }
                Ok(())
            })
            .unwrap_or_else(|e| {
                log::trace!("Failed to parse serde type attribute: {e}");
            });
        } else {
            // Ignore attributes that are neither serde nor a cfg_attr wrapper.
        }
    }

    meta
}

/// Parses serde attributes from a field.
pub fn parse_serde_field_attributes(attrs: &[Attribute]) -> SerdeFieldMeta {
    let mut meta = SerdeFieldMeta::default();

    for attr in attrs {
        if attr.path().is_ident("cfg_attr") && meta.cfg_attr_rejection.is_none() {
            meta.cfg_attr_rejection = cfg_attr_serde_rejection(attr);
        } else if attr.path().is_ident("serde") {
            attr.parse_nested_meta(|nested| {
                // Handle `rename = "value"`
                if nested.path.is_ident("rename") {
                    let value = nested.value()?;
                    let lit: LitStr = value.parse()?;
                    meta.rename = Some(lit.value());
                }
                // Handle `skip` or `skip_serializing_if`
                else if nested.path.is_ident("skip")
                    || nested.path.is_ident("skip_serializing")
                    || nested.path.is_ident("skip_serializing_if")
                {
                    meta.skip = true;
                    meta.omits_none = true;
                }
                // `skip_deserializing` belongs to the `skip` lump but never suppresses a
                // serialized `null`, so it must not set `omits_none`.
                else if nested.path.is_ident("skip_deserializing") {
                    meta.skip = true;
                }
                // Handle `flatten`
                else if nested.path.is_ident("flatten") {
                    meta.flatten = true;
                } else {
                    // Ignore other serde attributes.
                }
                consume_unread_value(&nested)?;
                Ok(())
            })
            .unwrap_or_else(|e| {
                log::trace!("Failed to parse serde field attribute: {e}");
            });
        } else {
            // Ignore attributes that are neither serde nor a cfg_attr wrapper.
        }
    }

    meta
}

/// Consumes the value of a `key = value` the walk had no use for.
///
/// An unread value ends the walk on the comma that follows it, taking every attribute written
/// after it along — so which attributes a field is read by would otherwise depend on the order
/// someone happened to write them in.
fn consume_unread_value(nested: &ParseNestedMeta<'_>) -> syn::Result<()> {
    if nested.input.peek(Token![=]) {
        nested.value()?.parse::<syn::Expr>()?;
    }
    Ok(())
}

/// Whether the field writes a value for itself when its key is missing, in either spelling
/// (`default` and `default = "path"`).
///
/// Asked of the attributes rather than carried on [`SerdeFieldMeta`]: nothing the generated
/// surfaces describe turns on it — it is read where a serde hook is written, to decide whether
/// that hook has a missing key to answer for.
pub fn has_serde_default(attrs: &[Attribute]) -> bool {
    let mut defaulted = false;
    for attr in attrs {
        if !attr.path().is_ident("serde") {
            continue;
        }
        attr.parse_nested_meta(|nested| {
            defaulted |= nested.path.is_ident("default");
            consume_unread_value(&nested)?;
            Ok(())
        })
        .unwrap_or_else(|e| {
            log::trace!("Failed to parse serde field attribute: {e}");
        });
    }
    defaulted
}

/// Applies serde `rename_all` transformation to a field name.
#[cfg(test)]
pub fn apply_rename_all(field_name: &str, rename_all: Option<&str>) -> String {
    resolve_rename_rule(rename_all).apply_to_field(field_name)
}

/// Get the final field name after applying serde transformations.
#[cfg(test)]
pub fn get_final_field_name(
    original_name: &str,
    field_meta: &SerdeFieldMeta,
    type_meta: &SerdeTypeMeta,
) -> String {
    // If field has explicit rename, use that
    if let Some(rename) = &field_meta.rename {
        return rename.clone();
    }

    // Otherwise apply rename_all transformation
    apply_rename_all(original_name, type_meta.rename_all.as_deref())
}

#[cfg(test)]
mod tests;

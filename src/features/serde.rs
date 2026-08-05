//! Serde integration feature module.
//!
//! This module handles serde attribute parsing and field name transformation.
//!
//! Most of it is gated on the "serde" feature: renaming, tagging, and the guards are all things
//! the feature buys. The omission walk at the top is not. Whether a key reaches the serialized
//! object is a fact about the wire, the attribute stating it is on the field under every toggle,
//! and every build emits surfaces that claim to describe that wire — so a build that could not
//! read the attribute would describe a different wire than the one serde writes.

#[cfg(all(test, feature = "serde"))]
use crate::rename_rule::resolve_rename_rule;
#[cfg(feature = "serde")]
use proc_macro2::{Delimiter, TokenTree};
use syn::meta::ParseNestedMeta;
use syn::{Attribute, Token};
#[cfg(feature = "serde")]
use syn::{Error, LitStr, Meta};

/// Message of the rejection raised for a serde attribute hidden behind `cfg_attr`.
#[cfg(feature = "serde")]
const CFG_ATTR_SERDE_REJECTION: &str = "cfg_attr-wrapped serde attribute is invisible to \
     model_schema and will be silently ignored by the generator; write #[serde(...)] \
     unconditionally (serde attrs are inert without the serde derive)";

/// What a field's serde attributes say about the key it writes.
///
/// Read in every build, unlike the rest of this module. The three questions are asked together
/// because one walk answers all of them and because the guard that polices the combination needs
/// them side by side: an attribute that drops the key while serde still insists on reading the
/// field is only coherent when something writes the value the missing key does not carry.
#[derive(Clone, Copy, Debug, Default)]
pub struct SerdeKeyOmission {
    /// A `default` in either spelling (`default` or `default = "path"`) — the field supplies a
    /// value for itself when the key is missing.
    pub defaulted: bool,
    /// A `skip`, `skip_serializing` or `skip_serializing_if` leaves the key out of the output.
    pub omits_key: bool,
    /// A `skip` also stops serde reading the field, which is its own answer to a missing key and
    /// so needs no `default` written beside it. The other two omission spellings still read.
    pub skips_deserializing: bool,
}

impl SerdeKeyOmission {
    /// Whether the attributes take the member out of both of serde's directions at once: nothing
    /// serde writes carries it, and nothing serde reads keeps what a payload put there.
    ///
    /// The conjunction is what is read rather than the word `skip`, because `skip_serializing` and
    /// `skip_deserializing` written side by side are that same wire spelled out.
    pub const fn absent_from_wire(self) -> bool {
        self.omits_key && self.skips_deserializing
    }

    /// Whether the attributes take the member out of exactly one of serde's two directions, which
    /// leaves what serde writes and what serde reads two different payloads.
    pub const fn drops_one_direction_only(self) -> bool {
        self.omits_key != self.skips_deserializing
    }
}

/// Metadata for serde attributes applied to a struct or enum.
#[cfg(feature = "serde")]
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
///
/// Whether the field's key is omitted is not here: [`parse_serde_key_omission`] answers that in
/// every build, and one answer read from one place is what keeps the surfaces from disagreeing
/// across the feature toggle.
#[cfg(feature = "serde")]
#[derive(Clone, Debug, Default)]
pub struct SerdeFieldMeta {
    /// The rejection raised when the walk met a `cfg_attr`-wrapped serde attribute.
    pub cfg_attr_rejection: Option<Error>,
    pub flatten: bool,          // Whether the field is `#[serde(flatten)]`
    pub rename: Option<String>, // e.g., "new_name"
    pub skip: bool,             // Whether to skip the field
}

/// Walks the serde attributes for exactly the keys [`SerdeKeyOmission`] names, ignoring every
/// other one.
///
/// Attributes wrapped in a `cfg_attr` are not reached, the same way nothing else in this module
/// reaches them — a predicate a proc macro cannot evaluate is not one this walk may guess at.
pub fn parse_serde_key_omission(attrs: &[Attribute]) -> SerdeKeyOmission {
    let mut omission = SerdeKeyOmission::default();

    for attr in attrs {
        if !attr.path().is_ident("serde") {
            continue;
        }
        attr.parse_nested_meta(|nested| {
            if nested.path.is_ident("skip") {
                omission.omits_key = true;
                omission.skips_deserializing = true;
            } else if nested.path.is_ident("skip_serializing")
                || nested.path.is_ident("skip_serializing_if")
            {
                omission.omits_key = true;
            } else if nested.path.is_ident("skip_deserializing") {
                omission.skips_deserializing = true;
            } else if nested.path.is_ident("default") {
                omission.defaulted = true;
            } else {
                // Every other serde key is someone else's business.
            }
            consume_unread_value(&nested)?;
            Ok(())
        })
        .unwrap_or_else(|e| {
            log::trace!("Failed to parse serde key-omission attribute: {e}");
        });
    }

    omission
}

/// Consumes the value of a `key = value` the walk had no use for.
///
/// An unread value ends the walk on the comma that follows it, taking every attribute written
/// after it along — so which attributes a declaration is read by would otherwise depend on the
/// order someone happened to write them in.
fn consume_unread_value(nested: &ParseNestedMeta<'_>) -> syn::Result<()> {
    if nested.input.peek(Token![=]) {
        nested.value()?.parse::<syn::Expr>()?;
    }
    Ok(())
}

/// Whether the field writes a value for itself when its key is missing, in either spelling
/// (`default` and `default = "path"`).
pub fn has_serde_default(attrs: &[Attribute]) -> bool {
    parse_serde_key_omission(attrs).defaulted
}

/// The rejection for a `cfg_attr` carrying a `serde(...)` attribute, or `None` for every other
/// `cfg_attr` — gated derives and gated docs stay legal.
#[cfg(feature = "serde")]
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
#[cfg(feature = "serde")]
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
                consume_unread_value(&nested)?;
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
#[cfg(feature = "serde")]
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
                // Handle the whole `skip` lump
                else if nested.path.is_ident("skip")
                    || nested.path.is_ident("skip_serializing")
                    || nested.path.is_ident("skip_serializing_if")
                    || nested.path.is_ident("skip_deserializing")
                {
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

/// Applies serde `rename_all` transformation to a field name.
#[cfg(all(test, feature = "serde"))]
pub fn apply_rename_all(field_name: &str, rename_all: Option<&str>) -> String {
    resolve_rename_rule(rename_all).apply_to_field(field_name)
}

/// Get the final field name after applying serde transformations.
#[cfg(all(test, feature = "serde"))]
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
mod key_omission_tests;

#[cfg(test)]
#[cfg(feature = "serde")]
mod tests;

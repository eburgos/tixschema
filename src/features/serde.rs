//! Serde integration feature module.
//!
//! This module handles serde attribute parsing and field name transformation
//! when the "serde" feature is enabled.

#[cfg(test)]
use crate::rename_rule::resolve_rename_rule;
use syn::{Attribute, LitStr};

/// Metadata for serde attributes applied to a struct or enum.
#[derive(Clone, Debug, Default)]
pub struct SerdeTypeMeta {
    pub content: Option<String>, // e.g., "value" for adjacently tagged enums
    pub rename_all: Option<String>, // e.g., "camelCase"
    pub tag: Option<String>,     // e.g., "behaviorType"
    pub untagged: bool,          // Whether the enum is `#[serde(untagged)]`
}

/// Metadata for serde attributes applied to a field.
#[derive(Clone, Debug, Default)]
pub struct SerdeFieldMeta {
    pub flatten: bool, // Whether the field is `#[serde(flatten)]`
    /// Whether a `None` is left out of the serialized output entirely. `skip_deserializing`
    /// does not qualify: it still writes `null` on the way out.
    pub omits_none: bool,
    pub rename: Option<String>, // e.g., "new_name"
    pub skip: bool,             // Whether to skip the field
}

/// Parses serde attributes from a struct or enum.
pub fn parse_serde_type_attributes(attrs: &[Attribute]) -> SerdeTypeMeta {
    let mut meta = SerdeTypeMeta::default();

    for attr in attrs {
        if attr.path().is_ident("serde") {
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
        }
    }

    meta
}

/// Parses serde attributes from a field.
pub fn parse_serde_field_attributes(attrs: &[Attribute]) -> SerdeFieldMeta {
    let mut meta = SerdeFieldMeta::default();

    for attr in attrs {
        if attr.path().is_ident("serde") {
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
                Ok(())
            })
            .unwrap_or_else(|e| {
                log::trace!("Failed to parse serde field attribute: {e}");
            });
        }
    }

    meta
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

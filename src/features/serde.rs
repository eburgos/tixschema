//! Serde integration feature module
//! 
//! This module handles serde attribute parsing and field name transformation
//! when the "serde" feature is enabled.

use syn::{Attribute, LitStr};

/// Metadata for serde attributes applied to a struct or enum.
#[derive(Clone, Debug, Default)]
pub struct SerdeTypeMeta {
    pub tag: Option<String>,        // e.g., "behaviorType"
    pub rename_all: Option<String>, // e.g., "camelCase"
}

/// Metadata for serde attributes applied to a field.
#[derive(Clone, Debug, Default)]
pub struct SerdeFieldMeta {
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
                // Handle `rename_all = "value"`
                else if nested.path.is_ident("rename_all") {
                    let value = nested.value()?;
                    let lit: LitStr = value.parse()?;
                    meta.rename_all = Some(lit.value());
                }
                Ok(())
            })
            .unwrap_or_else(|e| {
                if std::env::var("RUST_LOG") == Ok(String::from("trace")) {
                    println!("Failed to parse serde type attribute: {e}");
                }
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
                    || nested.path.is_ident("skip_deserializing")
                    || nested.path.is_ident("skip_serializing_if")
                {
                    meta.skip = true;
                }
                Ok(())
            })
            .unwrap_or_else(|e| {
                if std::env::var("RUST_LOG") == Ok(String::from("trace")) {
                    println!("Failed to parse serde field attribute: {e}");
                }
            });
        }
    }

    meta
}

/// Applies serde rename_all transformation to a field name
#[cfg(test)]
pub fn apply_rename_all(field_name: &str, rename_all: &Option<String>) -> String {
    match rename_all.as_deref() {
        Some("camelCase") => to_camel_case(field_name),
        Some("PascalCase") => to_pascal_case(field_name),
        Some("snake_case") => field_name.to_string(), // Already snake_case
        Some("SCREAMING_SNAKE_CASE") => field_name.to_uppercase(),
        Some("kebab-case") => to_kebab_case(field_name),
        Some("lowercase") => field_name.to_lowercase(),
        Some("UPPERCASE") => field_name.to_uppercase(),
        _ => field_name.to_string(),
    }
}

/// Get the final field name after applying serde transformations
#[cfg(test)]
pub fn get_final_field_name(
    original_name: String,
    field_meta: &SerdeFieldMeta,
    type_meta: &SerdeTypeMeta,
) -> String {
    // If field has explicit rename, use that
    if let Some(rename) = &field_meta.rename {
        return rename.clone();
    }

    // Otherwise apply rename_all transformation
    apply_rename_all(&original_name, &type_meta.rename_all)
}

/// Convert snake_case to camelCase
#[cfg(test)]
fn to_camel_case(s: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = false;

    for (i, c) in s.chars().enumerate() {
        if c == '_' {
            capitalize_next = true;
        } else if capitalize_next && i > 0 {
            result.push(c.to_ascii_uppercase());
            capitalize_next = false;
        } else {
            result.push(c);
            capitalize_next = false;
        }
    }

    result
}

/// Convert snake_case to PascalCase
#[cfg(test)]
fn to_pascal_case(s: &str) -> String {
    let camel = to_camel_case(s);
    if let Some(first_char) = camel.chars().next() {
        format!("{}{}", first_char.to_ascii_uppercase(), &camel[1..])
    } else {
        camel
    }
}

/// Convert snake_case to kebab-case
#[cfg(test)]
fn to_kebab_case(s: &str) -> String {
    s.replace('_', "-")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rename_all_transformations() {
        // Test camelCase
        assert_eq!(apply_rename_all("user_name", &Some("camelCase".to_string())), "userName");
        assert_eq!(apply_rename_all("first_name", &Some("camelCase".to_string())), "firstName");
        
        // Test PascalCase
        assert_eq!(apply_rename_all("user_name", &Some("PascalCase".to_string())), "UserName");
        
        // Test kebab-case
        assert_eq!(apply_rename_all("user_name", &Some("kebab-case".to_string())), "user-name");
        
        // Test no transformation
        assert_eq!(apply_rename_all("user_name", &None), "user_name");
    }

    #[test]
    fn test_final_field_name() {
        let type_meta = SerdeTypeMeta {
            tag: None,
            rename_all: Some("camelCase".to_string()),
        };

        // Test field with explicit rename
        let field_meta_with_rename = SerdeFieldMeta {
            rename: Some("customName".to_string()),
            skip: false,
        };
        assert_eq!(
            get_final_field_name("field_name".to_string(), &field_meta_with_rename, &type_meta),
            "customName"
        );

        // Test field with rename_all
        let field_meta_no_rename = SerdeFieldMeta {
            rename: None,
            skip: false,
        };
        assert_eq!(
            get_final_field_name("field_name".to_string(), &field_meta_no_rename, &type_meta),
            "fieldName"
        );
    }
} 
//! Model schema property feature module
//! 
//! This module handles parsing of model_schema_prop attributes for field-level customization
//! of TypeScript type and Zod schema generation.

use syn::{Attribute, LitStr, Type};

/// Metadata for model_schema_prop attributes applied to a field.
#[derive(Clone, Debug, Default)]
pub struct ModelSchemaPropMeta {
    pub as_type: Option<String>,    // e.g., "String" from as = String
    pub literal: Option<String>,    // e.g., "ProDoctivity" from literal = "ProDoctivity"
    pub min_length: Option<usize>,  // e.g., 1 from minLength = 1
}

/// Parses model_schema_prop attributes from a field.
pub fn parse_model_schema_prop_attributes(attrs: &[Attribute]) -> ModelSchemaPropMeta {
    let mut meta = ModelSchemaPropMeta::default();

    for attr in attrs {
        if attr.path().is_ident("model_schema_prop") {
            attr.parse_nested_meta(|nested| {
                // Handle `as = Type`
                if nested.path.is_ident("as") {
                    let value = nested.value()?;
                    if let Ok(ty) = value.parse::<Type>() {
                        // Convert the type to a string representation
                        meta.as_type = Some(quote::quote!(#ty).to_string());
                    }
                }
                // Handle `literal = "value"`
                else if nested.path.is_ident("literal") {
                    let value = nested.value()?;
                    let lit: LitStr = value.parse()?;
                    meta.literal = Some(lit.value());
                }
                // Handle `minLength = N`
                else if nested.path.is_ident("minLength") {
                    let value = nested.value()?;
                    let lit = value.parse::<syn::LitInt>()?;
                    if let Ok(min_len) = lit.base10_parse::<usize>() {
                        meta.min_length = Some(min_len);
                    }
                }
                Ok(())
            })
            .unwrap_or_else(|e| {
                if std::env::var("RUST_LOG") == Ok(String::from("trace")) {
                    println!("Failed to parse model_schema_prop attribute: {e}");
                }
            });
        }
    }

    meta
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    #[test]
    fn test_parse_empty_attributes() {
        let attrs: Vec<Attribute> = vec![];
        let meta = parse_model_schema_prop_attributes(&attrs);
        assert!(meta.as_type.is_none());
        assert!(meta.literal.is_none());
        assert!(meta.min_length.is_none());
    }

    #[test]
    fn test_parse_as_type() {
        let attr: Attribute = parse_quote! { #[model_schema_prop(as = String)] };
        let meta = parse_model_schema_prop_attributes(&[attr]);
        assert!(meta.as_type.is_some());
        assert_eq!(meta.as_type.unwrap(), "String");
        assert!(meta.literal.is_none());
        assert!(meta.min_length.is_none());
    }

    #[test]
    fn test_parse_literal() {
        let attr: Attribute = parse_quote! { #[model_schema_prop(literal = "ProDoctivity")] };
        let meta = parse_model_schema_prop_attributes(&[attr]);
        assert!(meta.as_type.is_none());
        assert!(meta.literal.is_some());
        assert_eq!(meta.literal.unwrap(), "ProDoctivity");
        assert!(meta.min_length.is_none());
    }

    #[test]
    fn test_parse_both_as_and_literal() {
        let attr: Attribute = parse_quote! { #[model_schema_prop(as = String, literal = "ProDoctivity")] };
        let meta = parse_model_schema_prop_attributes(&[attr]);
        assert!(meta.as_type.is_some());
        assert_eq!(meta.as_type.unwrap(), "String");
        assert!(meta.literal.is_some());
        assert_eq!(meta.literal.unwrap(), "ProDoctivity");
        assert!(meta.min_length.is_none());
    }

    #[test]
    fn test_parse_min_length() {
        let attr: Attribute = parse_quote! { #[model_schema_prop(minLength = 1)] };
        let meta = parse_model_schema_prop_attributes(&[attr]);
        assert!(meta.as_type.is_none());
        assert!(meta.literal.is_none());
        assert!(meta.min_length.is_some());
        assert_eq!(meta.min_length.unwrap(), 1);
    }

    #[test]
    fn test_parse_as_and_min_length() {
        let attr: Attribute = parse_quote! { #[model_schema_prop(as = String, minLength = 5)] };
        let meta = parse_model_schema_prop_attributes(&[attr]);
        assert!(meta.as_type.is_some());
        assert_eq!(meta.as_type.unwrap(), "String");
        assert!(meta.literal.is_none());
        assert!(meta.min_length.is_some());
        assert_eq!(meta.min_length.unwrap(), 5);
    }

    #[test]
    fn test_parse_all_attributes() {
        let attr: Attribute = parse_quote! { #[model_schema_prop(as = String, literal = "test", minLength = 3)] };
        let meta = parse_model_schema_prop_attributes(&[attr]);
        assert!(meta.as_type.is_some());
        assert_eq!(meta.as_type.unwrap(), "String");
        assert!(meta.literal.is_some());
        assert_eq!(meta.literal.unwrap(), "test");
        assert!(meta.min_length.is_some());
        assert_eq!(meta.min_length.unwrap(), 3);
    }
} 
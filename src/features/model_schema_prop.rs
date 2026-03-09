//! Model schema property feature module
//!
//! This module handles parsing of model_schema_prop attributes for field-level customization
//! of TypeScript type and Zod schema generation.

use syn::{Attribute, LitStr, Type};

/// Metadata for `model_schema_prop` attributes applied to a field.
///
/// # Supported attributes
///
/// ## String constraints
///
/// - `pattern = "regex"` — validates the string matches the regex pattern.
///   - Zod: `.check(z.regex(/regex/))`
///   - JSON Schema: `"pattern"`
///   - Rust: auto-generates a `deserialize_{field}` serde hook and a `validate_{field}_value()` static function
///
/// - `minLength = N` — minimum string length (inclusive).
///   - Zod: `.min(N)`
///   - JSON Schema: `"minLength"`
///   - Rust: serde validator
///
/// - `maxLength = N` — maximum string length (inclusive).
///   - Zod: `.max(N)`
///   - JSON Schema: `"maxLength"`
///   - Rust: serde validator
///
/// ## Numeric constraints
///
/// - `minimum = N` — minimum value for numeric fields (integer or float).
///   - Zod: `.min(N)`
///   - JSON Schema: `"minimum"`
///   - Rust: serde validator
///
/// - `maximum = N` — maximum value for numeric fields (integer or float).
///   - Zod: `.max(N)`
///   - JSON Schema: `"maximum"`
///   - Rust: serde validator
///
/// ## Type overrides
///
/// - `as = Type` — override the TypeScript/Zod type emitted for this field.
/// - `literal = "value"` — emit as a string literal type instead of `string`.
///
/// ## Zod preprocessing
///
/// - `preprocess = ["fn1", "fn2"]` — wrap the Zod schema with `z.preprocess()` calls (Zod-only, no Rust-side effect).
///   Multiple functions are nested: `z.preprocess(fn1, z.preprocess(fn2, innerSchema))`.
///
/// # Example
///
/// ```rust
/// use tixschema::{model_schema, model_schema_prop};
/// use serde::{Deserialize, Serialize};
///
/// #[model_schema()]
/// #[derive(Serialize, Deserialize)]
/// pub struct User {
///     #[model_schema_prop(minLength = 3, maxLength = 50, pattern = "^[a-z]+$")]
///     pub username: String,
///
///     #[model_schema_prop(minimum = 0, maximum = 120)]
///     pub age: u32,
///
///     #[model_schema_prop(pattern = "^[0-9a-fA-F]{24}$")]
///     pub id: String,
/// }
/// ```
#[derive(Clone, Debug, Default)]
pub struct ModelSchemaPropMeta {
    pub as_type: Option<String>,    // e.g., "String" from as = String
    pub literal: Option<String>,    // e.g., "Tixena" from literal = "Tixena"
    pub min_length: Option<usize>,  // e.g., 1 from minLength = 1
    pub max_length: Option<usize>,  // e.g., 50 from maxLength = 50
    pub pattern: Option<String>,    // e.g., "^[0-9a-fA-F]{24}$" from pattern = "^[0-9a-fA-F]{24}$"
    pub minimum: Option<f64>,       // e.g., 0.0 from minimum = 0
    pub maximum: Option<f64>,       // e.g., 100.0 from maximum = 100
    pub preprocess: Vec<String>,    // e.g., ["epoch_to_date", "trim"] from preprocess = ["epoch_to_date", "trim"]
}

/// Parses `model_schema_prop` attributes from a field.
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
                // Handle `maxLength = N`
                else if nested.path.is_ident("maxLength") {
                    let value = nested.value()?;
                    let lit = value.parse::<syn::LitInt>()?;
                    if let Ok(max_len) = lit.base10_parse::<usize>() {
                        meta.max_length = Some(max_len);
                    }
                }
                // Handle `minimum = N` (integer or float)
                else if nested.path.is_ident("minimum") {
                    let value = nested.value()?;
                    let lit: syn::Lit = value.parse()?;
                    match lit {
                        syn::Lit::Int(li) => {
                            if let Ok(n) = li.base10_parse::<i64>() {
                                meta.minimum = Some(n as f64);
                            }
                        }
                        syn::Lit::Float(lf) => {
                            if let Ok(n) = lf.base10_parse::<f64>() {
                                meta.minimum = Some(n);
                            }
                        }
                        _ => {}
                    }
                }
                // Handle `maximum = N` (integer or float)
                else if nested.path.is_ident("maximum") {
                    let value = nested.value()?;
                    let lit: syn::Lit = value.parse()?;
                    match lit {
                        syn::Lit::Int(li) => {
                            if let Ok(n) = li.base10_parse::<i64>() {
                                meta.maximum = Some(n as f64);
                            }
                        }
                        syn::Lit::Float(lf) => {
                            if let Ok(n) = lf.base10_parse::<f64>() {
                                meta.maximum = Some(n);
                            }
                        }
                        _ => {}
                    }
                }
                // Handle `pattern = "regex"`
                else if nested.path.is_ident("pattern") {
                    let value = nested.value()?;
                    let lit: LitStr = value.parse()?;
                    meta.pattern = Some(lit.value());
                }
                // Handle `preprocess = ["fn1", "fn2"]`
                else if nested.path.is_ident("preprocess") {
                    let value = nested.value()?;
                    let arr: syn::ExprArray = value.parse()?;
                    let fns: Vec<String> = arr
                        .elems
                        .iter()
                        .filter_map(|elem| {
                            if let syn::Expr::Lit(expr_lit) = elem
                                && let syn::Lit::Str(s) = &expr_lit.lit
                            {
                                return Some(s.value());
                            }
                            None
                        })
                        .collect();
                    meta.preprocess = fns;
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
        let attr: Attribute = parse_quote! { #[model_schema_prop(literal = "Tixena")] };
        let meta = parse_model_schema_prop_attributes(&[attr]);
        assert!(meta.as_type.is_none());
        assert!(meta.literal.is_some());
        assert_eq!(meta.literal.unwrap(), "Tixena");
        assert!(meta.min_length.is_none());
    }

    #[test]
    fn test_parse_both_as_and_literal() {
        let attr: Attribute =
            parse_quote! { #[model_schema_prop(as = String, literal = "Tixena")] };
        let meta = parse_model_schema_prop_attributes(&[attr]);
        assert!(meta.as_type.is_some());
        assert_eq!(meta.as_type.unwrap(), "String");
        assert!(meta.literal.is_some());
        assert_eq!(meta.literal.unwrap(), "Tixena");
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
        let attr: Attribute =
            parse_quote! { #[model_schema_prop(as = String, literal = "test", minLength = 3)] };
        let meta = parse_model_schema_prop_attributes(&[attr]);
        assert!(meta.as_type.is_some());
        assert_eq!(meta.as_type.unwrap(), "String");
        assert!(meta.literal.is_some());
        assert_eq!(meta.literal.unwrap(), "test");
        assert!(meta.min_length.is_some());
        assert_eq!(meta.min_length.unwrap(), 3);
    }
}

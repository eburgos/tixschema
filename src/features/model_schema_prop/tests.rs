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
    let attr: Attribute = parse_quote! { #[model_schema_prop(as = String, literal = "Tixena")] };
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

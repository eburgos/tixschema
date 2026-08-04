use super::*;
use crate::field_type::{FieldDef, FieldDefType};

#[test]
fn test_format_docs() {
    assert_eq!(GenerationUtils::format_docs(""), "");
    assert_eq!(GenerationUtils::format_docs("Simple doc"), " * Simple doc");
    assert_eq!(
        GenerationUtils::format_docs("Line 1\nLine 2"),
        " * Line 1\n * Line 2"
    );
}

#[test]
fn test_typescript_field_formatting() {
    let field = FieldDef {
        name: "test_field".to_owned(),
        docs: "Test documentation".to_owned(),
        field_type: FieldDefType::String,
        array_depth: 0,
        array_lengths: Vec::new(),
        model_schema_prop_meta: None,
        nullable_levels: Vec::new(),
        absent_from_wire: false,
        omits_value: false,
        #[cfg(feature = "jsonschema")]
        type_span: proc_macro2::Span::call_site(),
    };

    let formatted = GenerationUtils::format_typescript_field(&field);
    assert!(formatted.contains("test_field"));
    assert!(formatted.contains("string"));
    assert!(formatted.contains("Test documentation"));
}

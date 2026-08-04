use super::*;
use crate::field_type::{FieldDef, FieldDefType};

#[test]
fn test_generate_struct_type_empty() {
    let result = TypeScriptGenerator::generate_struct_type("TestJson", &[], "Test docs");
    assert!(result.contains("export type Test"));
    assert!(result.contains("Record<string, never>"));
    assert!(result.contains("Test docs"));
}

#[test]
fn test_generate_struct_type_with_fields() {
    let fields = vec![
        FieldDef {
            name: "id".to_owned(),
            docs: "ID field".to_owned(),
            field_type: FieldDefType::String,
            array_depth: 0,
            array_lengths: Vec::new(),
            model_schema_prop_meta: None,
            nullable_levels: Vec::new(),
            omits_value: false,
            #[cfg(feature = "jsonschema")]
            type_span: proc_macro2::Span::call_site(),
        },
        FieldDef {
            name: "name".to_owned(),
            docs: "Name field".to_owned(),
            field_type: FieldDefType::String,
            array_depth: 0,
            array_lengths: Vec::new(),
            model_schema_prop_meta: None,
            nullable_levels: vec![0],
            omits_value: false,
            #[cfg(feature = "jsonschema")]
            type_span: proc_macro2::Span::call_site(),
        },
    ];

    let result = TypeScriptGenerator::generate_struct_type("UserJson", &fields, "User struct");
    assert!(result.contains("export type User"));
    assert!(result.contains("id: string"));
    assert!(result.contains("name: string | undefined"));
    assert!(result.contains("User struct"));
}

#[test]
fn test_generate_plain_enum_type() {
    let options = vec!["active".to_owned(), "inactive".to_owned()];
    let result =
        TypeScriptGenerator::generate_plain_enum_type("StatusJson", &options, "Status enum");

    assert!(result.contains("export type Status"));
    assert!(result.contains("\"active\" | \"inactive\""));
    assert!(result.contains("Status enum"));
}

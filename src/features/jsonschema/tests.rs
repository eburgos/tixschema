use super::*;

#[test]
fn test_should_generate_json_schema() {
    assert!(should_generate_json_schema());
}

#[test]
fn test_json_schema_method_generation() {
    let fields = vec![];
    let method = generate_struct_json_schema_method(&fields, &[], None);
    let method_str = method.to_string();

    assert!(method_str.contains("json_schema"));
    assert!(method_str.contains("serde_json"));
    assert!(method_str.contains("properties"));
    assert!(method_str.contains("required"));
}

#[test]
fn test_json_schema_method_flatten_emits_merge() {
    let fields = vec![];
    let no_flatten = generate_struct_json_schema_method(&fields, &[], None).to_string();
    let with_flatten = generate_struct_json_schema_method(
        &fields,
        &[quote::quote! { serde_json::json!({ "type": "object" }) }],
        None,
    )
    .to_string();

    assert!(!no_flatten.contains("merge_object_schemas"));
    assert!(with_flatten.contains("merge_object_schemas"));
    assert!(with_flatten.contains("oneOf"));
}

/// A body that names itself is only reachable from under `$defs`, so the method must root the
/// document there — in both the plain and the flattened shape, which spell their bodies apart.
#[test]
fn test_json_schema_method_roots_a_self_naming_body_under_defs() {
    let fields = vec![];
    let flattened = [quote::quote! { serde_json::json!({ "type": "object" }) }];

    for flatten in [[].as_slice(), flattened.as_slice()] {
        let method = generate_struct_json_schema_method(&fields, flatten, Some("Node")).to_string();

        assert!(method.contains("\"$defs\""), "no $defs: {method}");
        assert!(method.contains("\"#/$defs/Node\""), "no pointer: {method}");
        assert!(method.contains("\"Node\""), "not keyed by name: {method}");
    }
}

/// The pointer and the `$defs` key are two spellings of one name; a document whose reference
/// pointed anywhere but at its own entry would resolve to nothing.
#[test]
fn test_self_reference_value_points_at_the_defs_entry() {
    let reference = self_reference_value("Node").to_string();
    let document = recursive_document("Node", &quote::quote! { body }).to_string();

    assert!(reference.contains("\"#/$defs/Node\""), "{reference}");
    assert!(document.contains("\"#/$defs/Node\""), "{document}");
}

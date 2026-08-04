use super::*;

#[test]
fn test_should_generate_json_schema() {
    assert!(should_generate_json_schema());
}

#[test]
fn test_json_schema_method_generation() {
    let fields = vec![];
    let method = generate_struct_json_schema_method(&fields, &[], "Node");
    let method_str = method.to_string();

    assert!(method_str.contains("json_schema"));
    assert!(method_str.contains("serde_json"));
    assert!(method_str.contains("properties"));
    assert!(method_str.contains("required"));
}

#[test]
fn test_json_schema_method_flatten_emits_merge() {
    let fields = vec![];
    let no_flatten = generate_struct_json_schema_method(&fields, &[], "Node").to_string();
    let with_flatten = generate_struct_json_schema_method(
        &fields,
        &[MergedSource {
            label: "Base".to_owned(),
            value: quote::quote! { serde_json::json!({ "type": "object" }) },
        }],
        "Node",
    )
    .to_string();

    assert!(!no_flatten.contains("merge_object_schemas"));
    assert!(with_flatten.contains("merge_object_schemas"));
    assert!(with_flatten.contains("oneOf"));
}

/// The wrapper the merge writes is the one its source used, which is only knowable once the source
/// has described itself. So the spelling is read off the source beside its branches and carried
/// into the wrapping, rather than fixed where the document is written.
#[test]
fn test_the_merge_wraps_branches_in_the_spelling_its_source_used() {
    let merge = generate_struct_json_schema_method(
        &[],
        &[MergedSource {
            label: "Base".to_owned(),
            value: quote::quote! { serde_json::json!({ "type": "object" }) },
        }],
        "Node",
    )
    .to_string();

    assert!(
        merge.contains("let Some ((spelling , branches)) = union_branches (body)"),
        "{merge}"
    );
    assert!(
        merge.contains("Branches :: Union (spelling , expanded)"),
        "{merge}"
    );
    assert!(
        merge.contains("Merged :: Union (spelling , merged)"),
        "{merge}"
    );
    assert!(merge.contains("\"anyOf\""), "{merge}");
}

/// A branch that is itself a union carries no members either, so the questions the whole merged body
/// was asked are asked of it too rather than once. The descent is bounded by the names it resolved
/// on the way down: a name reached twice on one path names a type no finite value inhabits.
#[test]
fn test_the_merge_expands_branches_to_a_fixed_point_under_a_path_terminator() {
    let merge = generate_struct_json_schema_method(
        &[],
        &[MergedSource {
            label: "Base".to_owned(),
            value: quote::quote! { serde_json::json!({ "type": "object" }) },
        }],
        "Node",
    )
    .to_string();

    assert!(
        merge.contains(
            "let below = expanded_branches (branch , hoisted_defs , expanding , position , label)"
        ),
        "{merge}"
    );
    assert!(
        merge.contains("if expanding . contains (& name)"),
        "{merge}"
    );
    assert!(
        merge.contains("closes a flatten cycle through nested unions"),
        "{merge}"
    );
}

/// The two methods are one mechanism: the guarded one is what siblings call, and the entry point
/// is what turns the definitions it collected into a document root.
#[test]
fn test_json_schema_methods_pair_an_entry_point_with_a_guarded_body() {
    let methods = json_schema_methods("Node", &quote::quote! { body }).to_string();

    assert!(methods.contains("pub fn json_schema ()"), "{methods}");
    assert!(methods.contains("pub fn json_schema_within"), "{methods}");
    assert!(methods.contains("in_flight"), "{methods}");
    assert!(methods.contains("\"$defs\""), "no $defs: {methods}");
}

/// The pointer and the `$defs` key are two spellings of one name; a document whose reference
/// pointed anywhere but at the entry it hoists would resolve to nothing.
#[test]
fn test_the_deferred_reference_points_at_the_hoisted_defs_entry() {
    let methods = json_schema_methods("Node", &quote::quote! { body }).to_string();

    assert!(
        methods.contains("\"#/$defs/Node\""),
        "no pointer: {methods}"
    );
    assert!(methods.contains("\"Node\""), "not keyed by name: {methods}");
}

/// A plain enum names nothing, but a type that names it reaches it through the guarded method
/// like any other sibling.
#[test]
fn test_plain_enum_publishes_the_guarded_method_too() {
    let method = generate_plain_enum_json_schema_method(&[quote::quote! { "a" }], "Flag");

    assert!(
        method.to_string().contains("pub fn json_schema_within"),
        "{method}"
    );
}

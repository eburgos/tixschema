use super::*;

#[test]
fn test_should_generate_json_schema() {
    assert!(should_generate_json_schema());
}

#[test]
fn test_json_schema_method_generation() {
    let fields = vec![];
    let method = generate_struct_json_schema_method(&fields, &[], "Node", &[]);
    let method_str = method.to_string();

    assert!(method_str.contains("json_schema"));
    assert!(method_str.contains("serde_json"));
    assert!(method_str.contains("properties"));
    assert!(method_str.contains("required"));
}

#[test]
fn test_json_schema_method_flatten_emits_merge() {
    let fields = vec![];
    let no_flatten = generate_struct_json_schema_method(&fields, &[], "Node", &[]).to_string();
    let with_flatten = generate_struct_json_schema_method(
        &fields,
        &[MergedSource {
            label: "Base".to_owned(),
            optional: false,
            value: quote::quote! { serde_json::json!({ "type": "object" }) },
        }],
        "Node",
        &[],
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
            optional: false,
            value: quote::quote! { serde_json::json!({ "type": "object" }) },
        }],
        "Node",
        &[],
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

/// Whether a source was reached through an `Option` is knowable only where the field was read, and
/// the merge that answers for it runs where the document is written. So the answer is carried into
/// the merge beside the label and the schema, as the constant it is, and the merge offers the
/// source's absence beside the source wherever it is set.
#[test]
fn test_an_optional_merged_source_carries_its_absence_into_the_merge() {
    let merge_of = |optional| {
        generate_struct_json_schema_method(
            &[],
            &[MergedSource {
                label: "Base".to_owned(),
                optional,
                value: quote::quote! { serde_json::json!({ "type": "object" }) },
            }],
            "Node",
            &[],
        )
        .to_string()
    };

    let optional = merge_of(true);
    assert!(optional.contains("(\"Base\" , true ,"), "{optional}");
    assert!(
        optional.contains("if * optional { source . or_absent () } else { source }"),
        "{optional}"
    );
    assert!(
        optional.contains("Self :: Union (\"anyOf\" , vec ! [self , Self :: Absent])"),
        "{optional}"
    );

    let required = merge_of(false);
    assert!(required.contains("(\"Base\" , false ,"), "{required}");
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
            optional: false,
            value: quote::quote! { serde_json::json!({ "type": "object" }) },
        }],
        "Node",
        &[],
    )
    .to_string();

    assert!(
        merge.contains(
            "None => expanded_branches (branch , hoisted_defs , expanding , position , label)"
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

/// One branch of the choice the edge itself offers is read before the descent rather than by it: a
/// unit variant of an exclusive union is the one key set the description does not spell out, serde
/// writing the variant's name as a key where the document pins it as a bare string.
#[test]
fn test_the_merge_reads_a_tagged_unit_variant_at_the_edges_own_depth() {
    let merge = generate_struct_json_schema_method(
        &[],
        &[MergedSource {
            label: "Base".to_owned(),
            optional: false,
            value: quote::quote! { serde_json::json!({ "type": "object" }) },
        }],
        "Node",
        &[],
    )
    .to_string();

    assert!(
        merge.contains("(position . len () == 1 && spelling == \"oneOf\")"),
        "{merge}"
    );
    assert!(
        merge.contains("Some (name) => Some (Branches :: Tagged (name))"),
        "{merge}"
    );
    assert!(
        merge.contains("schema . get (\"const\") ? . as_str ()"),
        "{merge}"
    );
}

/// The two methods are one mechanism: the guarded one is what siblings call, and the entry point
/// is what turns the definitions it collected into a document root.
#[test]
fn test_json_schema_methods_pair_an_entry_point_with_a_guarded_body() {
    let methods = json_schema_methods("Node", &quote::quote! { body }, &[]).to_string();

    assert!(methods.contains("pub fn json_schema ()"), "{methods}");
    assert!(methods.contains("pub fn json_schema_within"), "{methods}");
    assert!(methods.contains("in_flight"), "{methods}");
    assert!(methods.contains("\"$defs\""), "no $defs: {methods}");
}

/// The pointer and the `$defs` key are two spellings of one name; a document whose reference
/// pointed anywhere but at the entry it hoists would resolve to nothing. A name declaring no
/// parameter always fills at an empty list, so the key it resolves to at runtime is the bare
/// name — but the pointer is built from that runtime `key`, not from a literal embedding the name
/// carries at macro-expansion time, since a generic name's key also carries the filling that
/// built it (see the filling-keyed tests in the whole-type generic fixtures).
#[test]
fn test_the_deferred_reference_points_at_the_hoisted_defs_entry() {
    let methods = json_schema_methods("Node", &quote::quote! { body }, &[]).to_string();

    assert!(
        methods.contains("\"#/$defs/\""),
        "no pointer prefix: {methods}"
    );
    assert!(methods.contains("\"Node\""), "not keyed by name: {methods}");
    assert!(
        methods.contains("format ! (\"{}{key}\""),
        "the pointer is not built off the same key the entry is hoisted under: {methods}"
    );
}

/// A plain enum names nothing, but a type that names it reaches it through the guarded method
/// like any other sibling.
#[test]
fn test_plain_enum_publishes_the_guarded_method_too() {
    let method = generate_plain_enum_json_schema_method(&[quote::quote! { "a" }], "Flag", &[]);

    assert!(
        method.to_string().contains("pub fn json_schema_within"),
        "{method}"
    );
}

/// A pointer resolves to one body, so the name a frame puts in flight travels with the filling
/// that body is being written at — which is what a re-entry has to read to tell the cycle a
/// reference describes from the one it cannot.
#[test]
fn test_the_in_flight_recording_carries_the_filling_beside_the_name() {
    let parameters = vec![SchemaParameter {
        binding: proc_macro2::Ident::new("_arg_value_type", proc_macro2::Span::call_site()),
        default: quote::quote! { serde_json::json!({ "type": "string" }) },
    }];
    let methods = json_schema_methods("Node", &quote::quote! { body }, &parameters).to_string();

    assert!(
        methods.contains("in_flight : & mut Vec < (& 'static str , Vec < serde_json :: Value >) >"),
        "{methods}"
    );
    assert!(
        methods.contains(
            "let filling : Vec < serde_json :: Value > = vec ! [_arg_value_type . clone ()]"
        ),
        "{methods}"
    );
    assert!(
        methods.contains("in_flight . push ((\"Node\" , filling))"),
        "{methods}"
    );
}

/// The two ends a re-entered name can take: the same filling is the cycle the pointer describes,
/// and any other is a body the document has no second place to hold.
#[test]
fn test_a_re_entered_name_is_read_against_the_filling_in_flight() {
    let methods = json_schema_methods("Node", &quote::quote! { body }, &[]).to_string();

    assert!(
        methods.contains("in_flight . iter () . find (| (named , _) | * named == \"Node\")"),
        "{methods}"
    );
    assert!(
        methods.contains("if * in_flight_filling != filling"),
        "{methods}"
    );
    assert!(
        methods.contains("a document holds one definition per name"),
        "the refusal does not state the limitation: {methods}"
    );
    assert!(
        methods.contains("key the definitions by name and filling"),
        "the refusal does not state the way past it: {methods}"
    );
}

use serde::{Deserialize, Serialize};
use tixschema::model_schema;

// One enum per tagging, each carrying only the container-level `rename_all_fields`, so what the
// surfaces name for the members can come from nowhere else.
#[model_schema()]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all_fields = "camelCase")]
enum ExternalFieldCasing {
    Made { another_one: u32, my_field: u32 },
}

#[model_schema()]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all_fields = "camelCase")]
enum InternalFieldCasing {
    Made { another_one: u32, my_field: u32 },
}

#[model_schema()]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "payload", rename_all_fields = "camelCase")]
enum AdjacentFieldCasing {
    Made { another_one: u32, my_field: u32 },
}

#[model_schema()]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(untagged, rename_all_fields = "camelCase")]
enum UntaggedFieldCasing {
    Made { another_one: u32, my_field: u32 },
}

// The three steps of serde's precedence on one enum: `Marked` overrides the container with its own
// rule, `Renamed` overrides both with the member's own key, and `Plain` takes the container's.
#[model_schema()]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all_fields = "camelCase")]
enum PrecedenceFieldCasing {
    #[serde(rename_all = "PascalCase")]
    Marked {
        my_field: u32,
    },
    Plain {
        my_field: u32,
    },
    Renamed {
        #[serde(rename = "explicit_key")]
        my_field: u32,
    },
}

// The two container rules reach two different things and neither falls back to the other.
#[model_schema()]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE", rename_all_fields = "camelCase")]
enum IndependentCasing {
    MadeVariant { my_field: u32 },
}

#[model_schema()]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
enum VariantNameCasingOnly {
    MadeVariant { my_field: u32 },
}

#[model_schema()]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all_fields = "camelCase")]
enum TupleSlotCasing {
    Made(u32),
}

#[model_schema()]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all_fields = "camelCase")]
enum PlainVariantCasing {
    AlphaOne,
    BetaTwo,
}

/// The keys serde wrote in the object at `pointer` of the document for `value`, sorted so a key
/// set can be compared against one read off a schema.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn wire_keys<T>(value: &T, pointer: &str) -> Vec<String>
where
    T: Serialize,
{
    let payload = serde_json::to_value(value).unwrap();
    sorted_keys(payload.pointer(pointer).unwrap())
}

#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn sorted_keys(object: &serde_json::Value) -> Vec<String> {
    let mut keys: Vec<String> = object.as_object().unwrap().keys().cloned().collect();
    keys.sort();
    keys
}

#[cfg(any(feature = "typescript", feature = "zod"))]
fn assert_renders_wire_keys(rendered: &str, keys: &[String]) {
    for key in keys {
        assert!(
            rendered.contains(key.as_str()),
            "the surface omits `{key}`, a key serde wrote:\n{rendered}"
        );
    }
    assert!(
        !rendered.contains("my_field") && !rendered.contains("another_one"),
        "the surface names a member at its raw Rust ident:\n{rendered}"
    );
}

/// Whether a closed document (`additionalProperties: false`) accepts `payload`: every key on the
/// wire is named, and every required key is on the wire. A `oneOf` needs exactly one accepting
/// branch, an `anyOf` at least one.
#[cfg(feature = "jsonschema")]
fn closed_document_accepts(schema: &serde_json::Value, payload: &serde_json::Value) -> bool {
    if let Some(branches) = schema.get("oneOf") {
        return accepting_branches(branches, payload) == 1;
    }
    if let Some(branches) = schema.get("anyOf") {
        return accepting_branches(branches, payload) >= 1;
    }
    let written = payload.as_object().unwrap();
    let named = schema["properties"].as_object().unwrap();
    let required = schema["required"].as_array().unwrap();
    written.keys().all(|key| named.contains_key(key))
        && required
            .iter()
            .all(|key| written.contains_key(key.as_str().unwrap()))
}

#[cfg(feature = "jsonschema")]
fn accepting_branches(branches: &serde_json::Value, payload: &serde_json::Value) -> usize {
    branches
        .as_array()
        .unwrap()
        .iter()
        .filter(|branch| closed_document_accepts(branch, payload))
        .count()
}

#[cfg(feature = "jsonschema")]
fn assert_accepts<T>(schema: &serde_json::Value, value: &T)
where
    T: Serialize,
{
    let payload = serde_json::to_value(value).unwrap();
    assert!(
        closed_document_accepts(schema, &payload),
        "{payload} is rejected by {schema}"
    );
    assert!(
        !serde_json::to_string(schema).unwrap().contains("my_field"),
        "the document names a member at its raw Rust ident: {schema}"
    );
}

fn external_value() -> ExternalFieldCasing {
    ExternalFieldCasing::Made {
        another_one: 2,
        my_field: 1,
    }
}

fn internal_value() -> InternalFieldCasing {
    InternalFieldCasing::Made {
        another_one: 2,
        my_field: 1,
    }
}

fn adjacent_value() -> AdjacentFieldCasing {
    AdjacentFieldCasing::Made {
        another_one: 2,
        my_field: 1,
    }
}

fn untagged_value() -> UntaggedFieldCasing {
    UntaggedFieldCasing::Made {
        another_one: 2,
        my_field: 1,
    }
}

#[test]
fn externally_tagged_members_reach_the_wire_cased() {
    let value = external_value();
    let payload = serde_json::to_value(&value).unwrap();
    assert_eq!(
        payload,
        serde_json::json!({ "Made": { "anotherOne": 2_i64, "myField": 1_i64 } })
    );
    assert_eq!(
        serde_json::from_value::<ExternalFieldCasing>(payload).unwrap(),
        value
    );
}

#[test]
fn internally_tagged_members_reach_the_wire_cased() {
    assert_eq!(
        serde_json::to_value(internal_value()).unwrap(),
        serde_json::json!({ "kind": "Made", "anotherOne": 2_i64, "myField": 1_i64 })
    );
}

#[test]
fn adjacently_tagged_members_reach_the_wire_cased() {
    assert_eq!(
        serde_json::to_value(adjacent_value()).unwrap(),
        serde_json::json!({ "kind": "Made", "payload": { "anotherOne": 2_i64, "myField": 1_i64 } })
    );
}

#[test]
fn untagged_members_reach_the_wire_cased() {
    let value = untagged_value();
    let payload = serde_json::to_value(&value).unwrap();
    assert_eq!(
        payload,
        serde_json::json!({ "anotherOne": 2_i64, "myField": 1_i64 })
    );
    assert_eq!(
        serde_json::from_value::<UntaggedFieldCasing>(payload).unwrap(),
        value
    );
}

#[test]
fn precedence_reaches_the_wire_as_serde_resolves_it() {
    assert_eq!(
        serde_json::to_value(PrecedenceFieldCasing::Marked { my_field: 1 }).unwrap(),
        serde_json::json!({ "Marked": { "MyField": 1_i64 } })
    );
    assert_eq!(
        serde_json::to_value(PrecedenceFieldCasing::Plain { my_field: 1 }).unwrap(),
        serde_json::json!({ "Plain": { "myField": 1_i64 } })
    );
    assert_eq!(
        serde_json::to_value(PrecedenceFieldCasing::Renamed { my_field: 1 }).unwrap(),
        serde_json::json!({ "Renamed": { "explicit_key": 1_i64 } })
    );
}

#[test]
fn the_two_container_rules_reach_the_wire_independently() {
    assert_eq!(
        serde_json::to_value(IndependentCasing::MadeVariant { my_field: 1 }).unwrap(),
        serde_json::json!({ "MADE_VARIANT": { "myField": 1_i64 } })
    );
}

/// The container's `rename_all` renames variant names only, which is what makes
/// `rename_all_fields` a separate rule rather than an extension of it.
#[test]
fn variant_name_casing_leaves_members_at_their_idents_on_the_wire() {
    assert_eq!(
        serde_json::to_value(VariantNameCasingOnly::MadeVariant { my_field: 1 }).unwrap(),
        serde_json::json!({ "madeVariant": { "my_field": 1_i64 } })
    );
}

#[test]
fn a_tuple_variant_carries_no_member_to_case() {
    assert_eq!(
        serde_json::to_value(TupleSlotCasing::Made(1)).unwrap(),
        serde_json::json!({ "Made": 1_i64 })
    );
}

#[test]
fn a_plain_variant_carries_no_member_to_case() {
    assert_eq!(
        serde_json::to_value(PlainVariantCasing::AlphaOne).unwrap(),
        serde_json::json!("AlphaOne")
    );
    assert_eq!(
        PlainVariantCasing::enum_members(),
        ["AlphaOne", "BetaTwo"],
        "rename_all_fields names nothing a plain enum writes"
    );
}

#[cfg(feature = "typescript")]
#[test]
fn externally_tagged_typescript_names_the_keys_serde_wrote() {
    assert_renders_wire_keys(
        &ExternalFieldCasing::ts_definition(),
        &wire_keys(&external_value(), "/Made"),
    );
}

#[cfg(feature = "typescript")]
#[test]
fn internally_tagged_typescript_names_the_keys_serde_wrote() {
    assert_renders_wire_keys(
        &InternalFieldCasing::ts_definition(),
        &wire_keys(&internal_value(), ""),
    );
}

#[cfg(feature = "typescript")]
#[test]
fn adjacently_tagged_typescript_names_the_keys_serde_wrote() {
    assert_renders_wire_keys(
        &AdjacentFieldCasing::ts_definition(),
        &wire_keys(&adjacent_value(), "/payload"),
    );
}

#[cfg(feature = "typescript")]
#[test]
fn untagged_typescript_names_the_keys_serde_wrote() {
    assert_renders_wire_keys(
        &UntaggedFieldCasing::ts_definition(),
        &wire_keys(&untagged_value(), ""),
    );
}

#[cfg(feature = "typescript")]
#[test]
fn precedence_typescript_matches_serde() {
    let ts = PrecedenceFieldCasing::ts_definition();
    assert!(ts.contains("MyField: number"), "Got:\n{ts}");
    assert!(ts.contains("myField: number"), "Got:\n{ts}");
    assert!(ts.contains("explicit_key: number"), "Got:\n{ts}");
}

#[cfg(feature = "typescript")]
#[test]
fn independent_typescript_matches_serde() {
    let ts = IndependentCasing::ts_definition();
    assert!(ts.contains("\"MADE_VARIANT\": {"), "Got:\n{ts}");
    assert!(ts.contains("myField: number"), "Got:\n{ts}");
    assert!(!ts.contains("my_field"), "Got:\n{ts}");
}

#[cfg(feature = "typescript")]
#[test]
fn variant_name_casing_typescript_leaves_members_at_their_idents() {
    let ts = VariantNameCasingOnly::ts_definition();
    assert!(ts.contains("\"madeVariant\": {"), "Got:\n{ts}");
    assert!(ts.contains("my_field: number"), "Got:\n{ts}");
}

#[cfg(feature = "zod")]
#[test]
fn externally_tagged_zod_names_the_keys_serde_wrote() {
    assert_renders_wire_keys(
        &ExternalFieldCasing::zod_schema(),
        &wire_keys(&external_value(), "/Made"),
    );
}

#[cfg(feature = "zod")]
#[test]
fn internally_tagged_zod_names_the_keys_serde_wrote() {
    assert_renders_wire_keys(
        &InternalFieldCasing::zod_schema(),
        &wire_keys(&internal_value(), ""),
    );
}

#[cfg(feature = "zod")]
#[test]
fn adjacently_tagged_zod_names_the_keys_serde_wrote() {
    assert_renders_wire_keys(
        &AdjacentFieldCasing::zod_schema(),
        &wire_keys(&adjacent_value(), "/payload"),
    );
}

#[cfg(feature = "zod")]
#[test]
fn untagged_zod_names_the_keys_serde_wrote() {
    assert_renders_wire_keys(
        &UntaggedFieldCasing::zod_schema(),
        &wire_keys(&untagged_value(), ""),
    );
}

#[cfg(feature = "zod")]
#[test]
fn precedence_zod_matches_serde() {
    let zod = PrecedenceFieldCasing::zod_schema();
    assert!(zod.contains("MyField: z.number().int()"), "Got:\n{zod}");
    assert!(zod.contains("myField: z.number().int()"), "Got:\n{zod}");
    assert!(
        zod.contains("explicit_key: z.number().int()"),
        "Got:\n{zod}"
    );
}

#[cfg(feature = "zod")]
#[test]
fn variant_name_casing_zod_leaves_members_at_their_idents() {
    let zod = VariantNameCasingOnly::zod_schema();
    assert!(zod.contains("my_field: z.number().int()"), "Got:\n{zod}");
}

#[cfg(feature = "jsonschema")]
#[test]
fn externally_tagged_json_schema_names_the_keys_serde_wrote() {
    let schema = ExternalFieldCasing::json_schema();
    let value = external_value();
    assert_accepts(&schema, &value);
    assert_eq!(
        sorted_keys(&schema["oneOf"][0]["properties"]["Made"]["properties"]),
        wire_keys(&value, "/Made")
    );
    assert_eq!(
        sorted_keys(&schema["oneOf"][0]["properties"]["Made"]["properties"]),
        {
            let mut required: Vec<String> = schema["oneOf"][0]["properties"]["Made"]["required"]
                .as_array()
                .unwrap()
                .iter()
                .map(|key| key.as_str().unwrap().to_owned())
                .collect();
            required.sort();
            required
        }
    );
}

#[cfg(feature = "jsonschema")]
#[test]
fn internally_tagged_json_schema_names_the_keys_serde_wrote() {
    let schema = InternalFieldCasing::json_schema();
    let value = internal_value();
    assert_accepts(&schema, &value);
    assert_eq!(
        sorted_keys(&schema["oneOf"][0]["properties"]),
        wire_keys(&value, "")
    );
}

#[cfg(feature = "jsonschema")]
#[test]
fn adjacently_tagged_json_schema_names_the_keys_serde_wrote() {
    let schema = AdjacentFieldCasing::json_schema();
    let value = adjacent_value();
    assert_accepts(&schema, &value);
    assert_eq!(
        sorted_keys(&schema["oneOf"][0]["properties"]["payload"]["properties"]),
        wire_keys(&value, "/payload")
    );
}

#[cfg(feature = "jsonschema")]
#[test]
fn untagged_json_schema_names_the_keys_serde_wrote() {
    let schema = UntaggedFieldCasing::json_schema();
    let value = untagged_value();
    assert_accepts(&schema, &value);
    assert_eq!(
        sorted_keys(&schema["anyOf"][0]["properties"]),
        wire_keys(&value, "")
    );
}

#[cfg(feature = "jsonschema")]
#[test]
fn precedence_json_schema_matches_serde() {
    let schema = PrecedenceFieldCasing::json_schema();
    for value in [
        PrecedenceFieldCasing::Marked { my_field: 1 },
        PrecedenceFieldCasing::Plain { my_field: 1 },
        PrecedenceFieldCasing::Renamed { my_field: 1 },
    ] {
        let payload = serde_json::to_value(&value).unwrap();
        assert!(
            closed_document_accepts(&schema, &payload),
            "{payload} is rejected by {schema}"
        );
    }
}

#[cfg(feature = "jsonschema")]
#[test]
fn variant_name_casing_json_schema_leaves_members_at_their_idents() {
    let schema = VariantNameCasingOnly::json_schema();
    let value = VariantNameCasingOnly::MadeVariant { my_field: 1 };
    assert!(closed_document_accepts(
        &schema,
        &serde_json::to_value(&value).unwrap()
    ));
    assert_eq!(
        sorted_keys(&schema["oneOf"][0]["properties"]["madeVariant"]["properties"]),
        wire_keys(&value, "/madeVariant")
    );
}

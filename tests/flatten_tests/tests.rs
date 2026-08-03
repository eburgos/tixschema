use serde::{Deserialize, Serialize};
use tixschema::model_schema;

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct BasePart {
    owner: String,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
struct DataElementSampleValueEntry {
    data_element_id: String,
    #[serde(flatten)]
    variant: DataElementSampleValueVariant,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "dataType")]
enum DataElementSampleValueVariant {
    Alphanumeric {
        #[serde(rename = "sampleValues")]
        sample_values: Vec<String>,
    },
    Logical {
        #[serde(rename = "sampleValues")]
        sample_values: Vec<bool>,
    },
    Numeric {
        #[serde(rename = "sampleValues")]
        sample_values: Vec<i64>,
    },
}

/// Two bases that flatten each other: neither body exists when the other's merge asks for it, and
/// no finite value inhabits either type.
#[cfg(feature = "jsonschema")]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct CycleFirst {
    first_own: String,
    #[serde(flatten)]
    second: CycleSecond,
}

#[cfg(feature = "jsonschema")]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct CycleSecond {
    #[serde(flatten)]
    first: Box<CycleFirst>,
    second_own: String,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct ExtraPart {
    priority: i64,
}

/// A base that names itself, written only where something asks what it describes as: what a
/// flattened branch that is a reference rather than an object contributes to its container.
#[cfg(feature = "jsonschema")]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct FlatHolder {
    #[serde(flatten)]
    base: FlatNode,
    extra: String,
}

#[cfg(feature = "jsonschema")]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct FlatNode {
    children: Vec<Self>,
    val: String,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct FlattenOnly {
    #[serde(flatten)]
    variant: DataElementSampleValueVariant,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct MultiFlatten {
    #[serde(flatten)]
    base: BasePart,
    #[serde(flatten)]
    extra: ExtraPart,
    id: String,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct NoFlatten {
    id: String,
    name: String,
}

/// A plain enum flattened into a struct. Declared without `#[model_schema()]`: a plain enum writes
/// its own variant name rather than an object, so the crate refuses the declaration and the wire
/// form is only readable from a plain serde type.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
enum FlatHue {
    Red,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct FlatOverEnum {
    own: String,
    #[serde(flatten)]
    tone: FlatHue,
}

/// A newtype over a `String` flattened into a struct: serde writes it as the string it wraps, and
/// the registry cannot tell it apart from a struct — both register as having no enum members. So
/// the declaration compiles and the divergence is left for the merge to catch.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct FlatSlug(String);

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct FlatOverBrand {
    own: String,
    #[serde(flatten)]
    slug: FlatSlug,
}

/// An untagged enum every member of which serde writes as an object, and the struct that flattens
/// it. serde writes whichever member matched into the object the struct is writing, so what the
/// struct writes is one key set per member.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct FlatFirst {
    a: String,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct FlatSecond {
    b: bool,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
enum FlatEither {
    First(FlatFirst),
    Second(FlatSecond),
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct FlatOverUntagged {
    #[serde(flatten)]
    either: FlatEither,
    own: String,
}

/// The same shape with one member serde writes as a string rather than an object. The union names
/// no type of its own, so nothing before the merge can tell the two apart.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
enum FlatScalarEither {
    Obj(FlatFirst),
    Text(String),
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct FlatOverScalarUntagged {
    #[serde(flatten)]
    either: FlatScalarEither,
    own: String,
}

/// Whether `payload` is accepted by a document every branch of which is an object closed by
/// `additionalProperties: false`: some branch names every key the payload carries and requires no
/// key it does not.
#[cfg(feature = "jsonschema")]
fn closed_document_accepts(schema: &serde_json::Value, payload: &serde_json::Value) -> bool {
    let written = payload.as_object().unwrap();
    let branches = schema.get("oneOf").map_or_else(
        || vec![schema.clone()],
        |one_of| one_of.as_array().unwrap().clone(),
    );
    branches.iter().any(|branch| {
        let named = branch["properties"].as_object().unwrap();
        let required = branch["required"].as_array().unwrap();
        written.keys().all(|key| named.contains_key(key))
            && required
                .iter()
                .all(|key| written.contains_key(key.as_str().unwrap()))
    })
}

#[test]
fn test_flatten_structs_constructible() {
    let base = BasePart {
        owner: String::new(),
    };
    assert!(base.owner.is_empty());
    let extra = ExtraPart { priority: 0 };
    assert_eq!(extra.priority, 0_i64);
    let flatten_only = FlattenOnly {
        variant: DataElementSampleValueVariant::Alphanumeric {
            sample_values: Vec::new(),
        },
    };
    assert!(matches!(
        flatten_only.variant,
        DataElementSampleValueVariant::Alphanumeric { .. }
    ));
    let multi = MultiFlatten {
        base: BasePart {
            owner: String::new(),
        },
        extra: ExtraPart { priority: 0 },
        id: String::new(),
    };
    assert!(multi.id.is_empty());
    let no_flatten = NoFlatten {
        id: String::new(),
        name: String::new(),
    };
    assert!(no_flatten.id.is_empty());
}

// ========================================================================
// TypeScript
// ========================================================================

#[test]
#[cfg(feature = "typescript")]
fn test_flatten_typescript_intersection() {
    let ts = DataElementSampleValueEntry::ts_definition();
    assert!(ts.contains("export type DataElementSampleValueEntry = {"));
    assert!(ts.contains("dataElementId: string;"));
    assert!(ts.contains("} & DataElementSampleValueVariant;"));
    assert!(!ts.contains("variant:"));
}

#[test]
#[cfg(feature = "typescript")]
fn test_flatten_typescript_multiple() {
    let ts = MultiFlatten::ts_definition();
    assert!(ts.contains("id: string;"));
    assert!(ts.contains("} & BasePart & ExtraPart;"));
    assert!(!ts.contains("base:"));
    assert!(!ts.contains("extra:"));
}

#[test]
#[cfg(feature = "typescript")]
fn test_flatten_only_typescript_is_alias() {
    let ts = FlattenOnly::ts_definition();
    assert!(ts.contains("export type FlattenOnly = DataElementSampleValueVariant;"));
    assert!(!ts.contains("Record<string, never>"));
    assert!(!ts.contains("variant:"));
}

#[test]
#[cfg(feature = "typescript")]
fn test_no_flatten_typescript_unchanged() {
    let ts = NoFlatten::ts_definition();
    assert!(ts.contains("export type NoFlatten = {"));
    assert!(ts.contains("id: string;"));
    assert!(ts.contains("name: string;"));
    assert!(!ts.contains(" & "));
}

#[test]
#[cfg(feature = "typescript")]
fn test_flatten_variant_keeps_pascal_discriminator_and_camel_fields() {
    let ts = DataElementSampleValueVariant::ts_definition();
    assert!(ts.contains("dataType: \"Numeric\""));
    assert!(ts.contains("sampleValues:"));
    assert!(!ts.contains("sample_values"));
}

// ========================================================================
// Zod
// ========================================================================

#[test]
#[cfg(feature = "zod")]
fn test_flatten_zod_intersection() {
    let zod = DataElementSampleValueEntry::zod_schema();
    assert!(zod.contains("z.strictObject({"));
    assert!(zod.contains("dataElementId:"));
    assert!(zod.contains("}).and(DataElementSampleValueVariant$Schema);"));
    assert!(!zod.contains("variant:"));
}

#[test]
#[cfg(feature = "zod")]
fn test_flatten_zod_multiple_chained() {
    let zod = MultiFlatten::zod_schema();
    assert!(zod.contains("}).and(BasePart$Schema).and(ExtraPart$Schema);"));
}

#[test]
#[cfg(feature = "zod")]
fn test_no_flatten_zod_unchanged() {
    let zod = NoFlatten::zod_schema();
    assert!(zod.contains("z.strictObject({"));
    assert!(!zod.contains(".and("));
}

// ========================================================================
// JSON Schema
// ========================================================================

#[test]
#[cfg(feature = "jsonschema")]
fn test_flatten_json_schema_distributes_base_into_variants() {
    let schema = DataElementSampleValueEntry::json_schema();
    let one_of = schema["oneOf"].as_array().unwrap();
    assert_eq!(one_of.len(), 3);
    for branch in one_of {
        assert_eq!(
            branch["additionalProperties"],
            serde_json::Value::Bool(false)
        );
        let props = branch["properties"].as_object().unwrap();
        assert!(props.contains_key("dataElementId"));
        assert!(props.contains_key("dataType"));
        assert!(props.contains_key("sampleValues"));
        let req = branch["required"].as_array().unwrap();
        assert!(req.iter().any(|v| v.as_str() == Some("dataElementId")));
        assert!(req.iter().any(|v| v.as_str() == Some("dataType")));
        assert!(req.iter().any(|v| v.as_str() == Some("sampleValues")));
    }
}

#[test]
#[cfg(feature = "jsonschema")]
fn test_multi_flatten_json_schema_merges_plain_structs() {
    let schema = MultiFlatten::json_schema();
    assert!(schema.get("oneOf").is_none());
    assert_eq!(
        schema["additionalProperties"],
        serde_json::Value::Bool(false)
    );
    let props = schema["properties"].as_object().unwrap();
    assert!(props.contains_key("id"));
    assert!(props.contains_key("owner"));
    assert!(props.contains_key("priority"));
}

#[test]
#[cfg(feature = "jsonschema")]
fn test_flatten_only_json_schema_is_union() {
    let schema = FlattenOnly::json_schema();
    let one_of = schema["oneOf"].as_array().unwrap();
    assert_eq!(one_of.len(), 3);
    for branch in one_of {
        let props = branch["properties"].as_object().unwrap();
        assert!(props.contains_key("dataType"));
        assert!(props.contains_key("sampleValues"));
        assert!(!props.contains_key("dataElementId"));
    }
}

#[test]
#[cfg(feature = "jsonschema")]
fn test_no_flatten_json_schema_closes_additional_properties() {
    let schema = NoFlatten::json_schema();
    assert_eq!(
        schema["additionalProperties"],
        serde_json::Value::Bool(false)
    );
    assert!(schema.get("oneOf").is_none());
}

#[test]
#[cfg(feature = "jsonschema")]
fn test_flatten_recursive_base_contributes_its_fields() {
    let schema = FlatHolder::json_schema();
    assert!(schema.get("oneOf").is_none());
    assert_eq!(
        schema["additionalProperties"],
        serde_json::Value::Bool(false)
    );
    let props = schema["properties"].as_object().unwrap();
    assert!(props.contains_key("children"));
    assert!(props.contains_key("val"));
    assert!(props.contains_key("extra"));
    let req = schema["required"].as_array().unwrap();
    for name in ["children", "val", "extra"] {
        assert!(
            req.iter().any(|v| v.as_str() == Some(name)),
            "{name} missing from {req:?}"
        );
    }
}

/// The base's own self-reference is written from the container's root, so it has to resolve there.
#[test]
#[cfg(feature = "jsonschema")]
fn test_flatten_recursive_base_self_reference_resolves_from_the_container() {
    let schema = FlatHolder::json_schema();
    let reference = schema["properties"]["children"]["items"]["$ref"]
        .as_str()
        .unwrap();
    let pointer = reference.strip_prefix('#').unwrap();
    let resolved = schema.pointer(pointer).unwrap();
    let resolved_props = resolved["properties"].as_object().unwrap();
    assert!(resolved_props.contains_key("children"));
    assert!(resolved_props.contains_key("val"));
}

/// A cycle closed through flatten edges has no body to merge at either end, so it is named rather
/// than described as the closed object over whatever fields happened to be written first.
#[test]
#[cfg(feature = "jsonschema")]
#[should_panic(
    expected = "`CycleSecond`: `#[serde(flatten)]` of `CycleFirst` closes a flatten cycle"
)]
fn test_flatten_cycle_is_rejected_rather_than_described() {
    assert!(CycleFirst::json_schema().is_object());
}

/// The rejection is the cycle's, not the entry point's: asking either end names the edge that
/// closes it.
#[test]
#[cfg(feature = "jsonschema")]
#[should_panic(
    expected = "`CycleFirst`: `#[serde(flatten)]` of `CycleSecond` closes a flatten cycle"
)]
fn test_flatten_cycle_is_rejected_from_either_end() {
    assert!(CycleSecond::json_schema().is_object());
}

/// Flattening a base that does not name itself writes the document it wrote before, byte for byte.
#[test]
#[cfg(feature = "jsonschema")]
fn test_non_recursive_flatten_documents_are_byte_identical() {
    assert_eq!(
        serde_json::to_string(&DataElementSampleValueEntry::json_schema()).unwrap(),
        r#"{"type":"object","oneOf":[{"type":"object","properties":{"dataElementId":{"type":"string"},"dataType":{"type":"string","const":"Alphanumeric"},"sampleValues":{"type":"array","items":{"type":"string"}}},"required":["dataElementId","dataType","sampleValues"],"additionalProperties":false},{"type":"object","properties":{"dataElementId":{"type":"string"},"dataType":{"type":"string","const":"Logical"},"sampleValues":{"type":"array","items":{"type":"boolean"}}},"required":["dataElementId","dataType","sampleValues"],"additionalProperties":false},{"type":"object","properties":{"dataElementId":{"type":"string"},"dataType":{"type":"string","const":"Numeric"},"sampleValues":{"type":"array","items":{"type":"integer"}}},"required":["dataElementId","dataType","sampleValues"],"additionalProperties":false}]}"#
    );
    assert_eq!(
        serde_json::to_string(&MultiFlatten::json_schema()).unwrap(),
        r#"{"type":"object","properties":{"id":{"type":"string"},"owner":{"type":"string"},"priority":{"type":"integer"}},"required":["id","owner","priority"],"additionalProperties":false}"#
    );
    assert_eq!(
        serde_json::to_string(&FlattenOnly::json_schema()).unwrap(),
        r#"{"type":"object","oneOf":[{"type":"object","properties":{"dataType":{"type":"string","const":"Alphanumeric"},"sampleValues":{"type":"array","items":{"type":"string"}}},"required":["dataType","sampleValues"],"additionalProperties":false},{"type":"object","properties":{"dataType":{"type":"string","const":"Logical"},"sampleValues":{"type":"array","items":{"type":"boolean"}}},"required":["dataType","sampleValues"],"additionalProperties":false},{"type":"object","properties":{"dataType":{"type":"string","const":"Numeric"},"sampleValues":{"type":"array","items":{"type":"integer"}}},"required":["dataType","sampleValues"],"additionalProperties":false}]}"#
    );
    assert_eq!(
        serde_json::to_string(&NoFlatten::json_schema()).unwrap(),
        r#"{"type":"object","additionalProperties":false,"properties":{"id":{"type":"string"},"name":{"type":"string"}},"required":["id","name"]}"#
    );
}

// ========================================================================
// Serialization round-trip
// ========================================================================

#[test]
fn test_flatten_serialization_is_flat() {
    let entry = DataElementSampleValueEntry {
        data_element_id: "abc".to_owned(),
        variant: DataElementSampleValueVariant::Numeric {
            sample_values: vec![1_i64, 2_i64, 3_i64],
        },
    };
    let json = serde_json::to_value(&entry).unwrap();
    assert_eq!(json["dataElementId"], "abc");
    assert_eq!(json["dataType"], "Numeric");
    assert_eq!(
        json["sampleValues"],
        serde_json::json!([1_i64, 2_i64, 3_i64])
    );
    assert!(json.get("variant").is_none());

    let back: DataElementSampleValueEntry = serde_json::from_value(json).unwrap();
    assert_eq!(back, entry);
}

#[test]
#[cfg(feature = "jsonschema")]
fn test_flatten_recursive_base_serializes_flat() {
    let holder = FlatHolder {
        base: FlatNode {
            children: vec![FlatNode {
                children: Vec::new(),
                val: "leaf".to_owned(),
            }],
            val: "root".to_owned(),
        },
        extra: "x".to_owned(),
    };
    let json = serde_json::to_value(&holder).unwrap();
    assert_eq!(json["val"], "root");
    assert_eq!(json["extra"], "x");
    assert_eq!(json["children"][0]["val"], "leaf");
    assert!(json.get("base").is_none());

    let back: FlatHolder = serde_json::from_value(json).unwrap();
    assert_eq!(back, holder);
}

/// What serde writes for a flattened plain enum: the enum's own variant name, as a key holding
/// null. No schema closed around the struct's remaining fields names that key, which is why the
/// declaration is refused rather than described.
#[test]
fn test_flattening_a_plain_enum_writes_a_key_the_struct_does_not_name() {
    assert_eq!(
        serde_json::to_value(FlatOverEnum {
            own: "o".to_owned(),
            tone: FlatHue::Red,
        })
        .unwrap(),
        serde_json::json!({ "own": "o", "Red": null })
    );
}

/// And what serde writes for a flattened newtype that reaches the wire as a string — nothing. A
/// name got this one past the declaration guard; the value never reaches the wire at all.
#[test]
fn test_flattening_a_string_newtype_is_unserializable() {
    let refusal = serde_json::to_value(FlatOverBrand {
        own: "o".to_owned(),
        slug: FlatSlug("s".to_owned()),
    })
    .unwrap_err()
    .to_string();
    assert!(
        refusal.contains("can only flatten structs and maps"),
        "Got: {refusal}"
    );
}

/// So the merge refuses it too, rather than closing the object around the fields that are left.
#[test]
#[cfg(feature = "jsonschema")]
#[should_panic(
    expected = "`FlatOverBrand`: `#[serde(flatten)]` of `FlatSlug` is not written as an object"
)]
fn test_flattening_a_string_newtype_is_refused_by_the_merge() {
    assert!(FlatOverBrand::json_schema().is_object());
}

/// The remedy the refusal names is one the author can act on.
#[test]
#[cfg(feature = "jsonschema")]
#[should_panic(expected = "write the field as a named member so the value gets a key of its own")]
fn test_the_flatten_merge_refusal_names_the_remedy() {
    assert!(FlatOverBrand::json_schema().is_object());
}

/// What serde writes for a flattened untagged enum: the struct's own fields, and beside them the
/// members of whichever union member matched. One key set per member, and no key naming the field.
#[test]
fn test_flattening_an_untagged_enum_writes_the_matched_members_keys() {
    assert_eq!(
        serde_json::to_value(FlatOverUntagged {
            own: "o".to_owned(),
            either: FlatEither::First(FlatFirst { a: "x".to_owned() }),
        })
        .unwrap(),
        serde_json::json!({ "own": "o", "a": "x" })
    );
    assert_eq!(
        serde_json::to_value(FlatOverUntagged {
            own: "o".to_owned(),
            either: FlatEither::Second(FlatSecond { b: true }),
        })
        .unwrap(),
        serde_json::json!({ "own": "o", "b": true })
    );
}

/// So the merged schema is the union of the merges: the base multiplied over every member of the
/// union, each branch closed around exactly the keys that member writes.
#[test]
#[cfg(feature = "jsonschema")]
fn test_flattening_an_untagged_enum_multiplies_the_base_over_its_members() {
    assert_eq!(
        serde_json::to_string(&FlatOverUntagged::json_schema()).unwrap(),
        r#"{"type":"object","oneOf":[{"type":"object","properties":{"own":{"type":"string"},"a":{"type":"string"}},"required":["own","a"],"additionalProperties":false},{"type":"object","properties":{"own":{"type":"string"},"b":{"type":"boolean"}},"required":["own","b"],"additionalProperties":false}]}"#
    );
}

/// And every payload serde writes is accepted by it. Before the base multiplied out, the document
/// closed around `own` alone and rejected both.
#[test]
#[cfg(feature = "jsonschema")]
fn test_the_untagged_flatten_schema_accepts_every_payload_serde_writes() {
    let schema = FlatOverUntagged::json_schema();
    for payload in [
        serde_json::json!({ "own": "o", "a": "x" }),
        serde_json::json!({ "own": "o", "b": true }),
    ] {
        assert!(
            closed_document_accepts(&schema, &payload),
            "{payload} is rejected by {schema}"
        );
    }
    assert!(
        !closed_document_accepts(
            &serde_json::json!({
                "type": "object",
                "properties": { "own": { "type": "string" } },
                "required": ["own"],
                "additionalProperties": false
            }),
            &serde_json::json!({ "own": "o", "a": "x" })
        ),
        "the document closed around the base alone accepts a key it does not name"
    );
}

/// A union member serde writes as a string is a member serde cannot flatten at all.
#[test]
fn test_flattening_an_untagged_enum_over_a_string_member_is_unserializable() {
    let refusal = serde_json::to_value(FlatOverScalarUntagged {
        own: "o".to_owned(),
        either: FlatScalarEither::Text("t".to_owned()),
    })
    .unwrap_err()
    .to_string();
    assert!(
        refusal.contains("can only flatten structs and maps"),
        "Got: {refusal}"
    );
}

/// So the merge refuses the whole union, naming the branch that cannot join the object.
#[test]
#[cfg(feature = "jsonschema")]
#[should_panic(
    expected = "`FlatOverScalarUntagged`: `#[serde(flatten)]` of `FlatScalarEither` writes a union member that is not an object — its branch 2 describes a `string`"
)]
fn test_a_string_member_of_a_flattened_untagged_enum_is_refused_by_the_merge() {
    assert!(FlatOverScalarUntagged::json_schema().is_object());
}

/// The remedy that refusal names is the same one every flattened non-object gets.
#[test]
#[cfg(feature = "jsonschema")]
#[should_panic(expected = "write the field as a named member so the value gets a key of its own")]
fn test_the_untagged_branch_refusal_names_the_remedy() {
    assert!(FlatOverScalarUntagged::json_schema().is_object());
}

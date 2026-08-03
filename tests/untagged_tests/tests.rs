use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tixschema::model_schema;

// ISO-8601 date string. Branded newtype carries the regex pattern.
#[model_schema(pattern = r"^\d{4}-\d{2}-\d{2}$")]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DateString(pub String);

// The migration target: an entry whose flattened variant carries `Vec<DateValue>`.
#[model_schema()]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DataElementSampleValueEntry {
    data_element_id: String,
    #[serde(flatten)]
    variant: DataElementSampleValueVariant,
}

#[model_schema()]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "dataType")]
enum DataElementSampleValueVariant {
    Date {
        #[serde(rename = "sampleValues")]
        sample_values: Vec<DateValue>,
    },
}

// A date sample value: an ISO date string OR an epoch number (TupleSingle members).
#[model_schema()]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
enum DateValue {
    N(i64),
    S(DateString),
}

// Untagged enum with named-struct variants.
#[model_schema()]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum NamedUnion {
    A { x: String },
    B { y: i64 },
}

// Untagged enum whose struct variant carries an `Option` that keeps `None` off the wire, matching
// the absent form the union member renders.
#[model_schema()]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
enum CompliantUnion {
    Count(i64),
    Note {
        id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        note: Option<String>,
    },
}

// An untagged newtype variant's content has no key to drop: it is the whole serialized value, so
// a `None` there reaches the wire as a bare `null`. The non-`Option` member beside it carries none.
#[model_schema()]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
enum SlotUnion {
    Maybe(Option<i64>),
    Plain(String),
}

// A plain enum's members are what a map keyed by it writes its object's keys from.
#[model_schema()]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
enum Bucket {
    Large,
    Small,
}

// The map keys a variant's member may carry: one the registry enumerates, one open by nature.
#[model_schema()]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum KeyedUnion {
    Counts { counts: HashMap<Bucket, u32> },
    Labels { labels: HashMap<String, String> },
}

#[test]
fn test_untagged_entry_constructible() {
    let entry = DataElementSampleValueEntry {
        data_element_id: String::new(),
        variant: DataElementSampleValueVariant::Date {
            sample_values: Vec::new(),
        },
    };
    assert!(entry.data_element_id.is_empty());
}

// ========================================================================
// TypeScript
// ========================================================================

#[test]
#[cfg(feature = "typescript")]
fn test_tuple_single_union_typescript() {
    let ts = DateValue::ts_definition();
    assert!(
        ts.contains("export type DateValue = number | DateString;"),
        "Got:\n{ts}"
    );
}

#[test]
#[cfg(feature = "typescript")]
fn test_named_union_typescript() {
    let ts = NamedUnion::ts_definition();
    assert!(
        ts.contains("export type NamedUnion = { x: string } | { y: number };"),
        "Got:\n{ts}"
    );
}

#[test]
#[cfg(feature = "typescript")]
fn test_compliant_union_typescript() {
    let ts = CompliantUnion::ts_definition();
    assert!(
        ts.contains("number | { id: string; note: string | undefined };"),
        "Got:\n{ts}"
    );
}

#[test]
#[cfg(feature = "typescript")]
fn test_flatten_end_to_end_typescript() {
    let ts = DataElementSampleValueEntry::ts_definition();
    assert!(
        ts.contains("} & DataElementSampleValueVariant;"),
        "Got:\n{ts}"
    );
    let variant_ts = DataElementSampleValueVariant::ts_definition();
    assert!(
        variant_ts.contains("sampleValues: Array<DateValue>;"),
        "Got:\n{variant_ts}"
    );
}

// ========================================================================
// Zod
// ========================================================================

#[test]
#[cfg(feature = "zod")]
fn test_tuple_single_union_zod() {
    let zod = DateValue::zod_schema();
    assert!(
        zod.contains("z.union([z.number().int(), DateString$Schema])"),
        "Got:\n{zod}"
    );
    assert!(zod.contains("DateValue$Schema"), "Got:\n{zod}");
    // The `ZodType<...> = ...$RawSchema` framing only appears when typescript is also enabled.
    #[cfg(feature = "typescript")]
    assert!(
        zod.contains("export const DateValue$Schema: ZodType<DateValue> = DateValue$RawSchema;"),
        "Got:\n{zod}"
    );
}

#[test]
#[cfg(feature = "zod")]
fn test_date_string_branded_pattern_zod() {
    let zod = DateString::zod_schema();
    assert!(
        zod.contains(r".check(z.regex(/^\d{4}-\d{2}-\d{2}$/))"),
        "Got:\n{zod}"
    );
}

#[test]
#[cfg(feature = "zod")]
fn test_named_union_zod() {
    let zod = NamedUnion::zod_schema();
    assert!(zod.contains("z.union(["), "Got:\n{zod}");
    assert!(
        zod.contains("z.strictObject({ x: z.string(), })"),
        "Got:\n{zod}"
    );
    assert!(
        zod.contains("z.strictObject({ y: z.number().int(), })"),
        "Got:\n{zod}"
    );
}

#[test]
#[cfg(feature = "zod")]
fn test_compliant_union_zod() {
    let zod = CompliantUnion::zod_schema();
    assert!(
        zod.contains(
            "z.strictObject({ id: z.string(), \
             note: z.union([z.string(), z.undefined()]).prefault(undefined), })"
        ),
        "Got:\n{zod}"
    );
    assert!(!zod.contains("z.nullable"), "Got:\n{zod}");
}

#[test]
#[cfg(feature = "zod")]
fn test_flatten_end_to_end_zod() {
    let zod = DataElementSampleValueVariant::zod_schema();
    assert!(
        zod.contains("sampleValues: z.array(DateValue$Schema)"),
        "Got:\n{zod}"
    );
}

// ========================================================================
// JSON Schema
// ========================================================================

#[test]
#[cfg(feature = "jsonschema")]
fn test_tuple_single_union_json_schema() {
    let schema = DateValue::json_schema();
    let any_of = schema["anyOf"].as_array().unwrap();
    assert_eq!(any_of.len(), 2);
    assert_eq!(any_of[0]["type"], "integer");
    assert_eq!(any_of[1]["type"], "string");
    assert_eq!(any_of[1]["pattern"], r"^\d{4}-\d{2}-\d{2}$");
}

#[test]
#[cfg(feature = "jsonschema")]
fn test_named_union_json_schema() {
    let schema = NamedUnion::json_schema();
    let any_of = schema["anyOf"].as_array().unwrap();
    assert_eq!(any_of.len(), 2);
    for branch in any_of {
        assert_eq!(branch["type"], "object");
        assert_eq!(branch["additionalProperties"], false);
        assert!(branch["properties"].is_object());
        assert!(branch["required"].is_array());
    }
}

#[test]
#[cfg(feature = "jsonschema")]
fn test_compliant_union_json_schema() {
    let schema = CompliantUnion::json_schema();
    let any_of = schema["anyOf"].as_array().unwrap();
    let required = any_of[1]["required"].as_array().unwrap();
    assert!(
        required.contains(&serde_json::json!("id")),
        "Got:\n{schema}"
    );
    assert!(
        !required.contains(&serde_json::json!("note")),
        "Got:\n{schema}"
    );
}

#[test]
#[cfg(feature = "jsonschema")]
fn test_flatten_end_to_end_json_schema() {
    // A single-variant flattened entry collapses to a flat object (no `oneOf`); the untagged
    // `Vec<DateValue>` still renders its `anyOf` under `sampleValues.items`.
    let schema = DataElementSampleValueEntry::json_schema();
    let items = &schema["properties"]["sampleValues"]["items"];
    let any_of = items["anyOf"].as_array().unwrap();
    assert_eq!(any_of.len(), 2);
    assert_eq!(any_of[0]["type"], "integer");
    assert_eq!(any_of[1]["type"], "string");
    assert_eq!(any_of[1]["pattern"], r"^\d{4}-\d{2}-\d{2}$");
}

// ========================================================================
// Serde round-trip
// ========================================================================

#[test]
fn test_serde_round_trip_string_member() {
    let value = DateValue::S(DateString("2026-06-26".to_owned()));
    let json = serde_json::to_string(&value).unwrap();
    assert_eq!(json, "\"2026-06-26\"");
    let back: DateValue = serde_json::from_str(&json).unwrap();
    assert_eq!(back, value);
}

/// The guard exists so the wire form matches the schema: `None` must leave the key out, and the
/// absent form must parse back through the untagged union.
#[test]
fn test_serde_round_trip_compliant_union_omits_none() {
    let value = CompliantUnion::Note {
        id: "a".to_owned(),
        note: None,
    };
    let json = serde_json::to_string(&value).unwrap();
    assert_eq!(json, r#"{"id":"a"}"#);
    let back: CompliantUnion = serde_json::from_str(&json).unwrap();
    assert_eq!(back, value);
}

#[test]
fn test_serde_round_trip_number_member() {
    let value = DateValue::N(5);
    let json = serde_json::to_string(&value).unwrap();
    assert_eq!(json, "5");
    let back: DateValue = serde_json::from_str(&json).unwrap();
    assert_eq!(back, value);
}

// ========================================================================
// Untagged newtype variant holding an `Option` — the slot the three surfaces read against
// ========================================================================

/// What serde writes for an untagged newtype variant whose content is `None` — the capture the
/// three surface assertions below are read against.
#[test]
fn test_untagged_newtype_option_content_writes_bare_null() {
    assert_eq!(
        serde_json::to_value(SlotUnion::Maybe(None)).unwrap(),
        serde_json::Value::Null,
        "The content is the whole value, so a `None` writes a bare `null`"
    );
}

/// TypeScript describes that content as the slot it is.
#[test]
#[cfg(feature = "typescript")]
fn test_untagged_newtype_option_content_typescript_null_flavor() {
    let ts = SlotUnion::ts_definition();

    assert!(
        ts.contains("export type SlotUnion = number | null | string;"),
        "Maybe's content is the slot the `None` fills with `null`, Plain's carries no null. \
         Got:\n{ts}"
    );
}

/// The Zod schema admits the `null` serde writes. An untagged member cannot be omitted, so an
/// undefined-flavored union there would leave the captured `null` unmatched.
#[test]
#[cfg(feature = "zod")]
fn test_untagged_newtype_option_content_zod_null_flavor() {
    let zod = SlotUnion::zod_schema();

    assert!(
        zod.contains("z.union([z.nullable(z.number().int()), z.string()])"),
        "Got:\n{zod}"
    );
}

/// The JSON schema admits it too: `field_json_schema_value` adds no null wrap on its own, so the
/// member goes through the shared nullable-slot wrap.
#[test]
#[cfg(feature = "jsonschema")]
fn test_untagged_newtype_option_content_json_schema_null_flavor() {
    let schema = SlotUnion::json_schema();
    let any_of = schema["anyOf"].as_array().unwrap();

    assert_eq!(any_of.len(), 2, "Got:\n{schema}");
    assert_eq!(
        any_of[0],
        serde_json::json!({
            "anyOf": [{ "type": "integer" }, { "type": "null" }]
        })
    );
    assert_eq!(any_of[1], serde_json::json!({ "type": "string" }));
}

// ========================================================================
// Untagged member holding a map — the key the guard reads off the written type
// ========================================================================

/// A key that enumerates its members, and an open one, both render the map their written type
/// earns: the guard is a filter over keys the registry rules out, never a rewrite of the ones it
/// admits.
#[test]
#[cfg(feature = "typescript")]
fn test_untagged_map_member_keys_typescript() {
    let ts = KeyedUnion::ts_definition();
    assert!(
        ts.contains(
            "export type KeyedUnion = { counts: Partial<Record<Bucket, number>> } \
             | { labels: Partial<Record<string, string>> };"
        ),
        "Got:\n{ts}"
    );
}

#[test]
#[cfg(feature = "zod")]
fn test_untagged_map_member_keys_zod() {
    let zod = KeyedUnion::zod_schema();
    assert!(
        zod.contains("z.strictObject({ counts: z.record(Bucket$Schema, z.number().int()), })"),
        "Got:\n{zod}"
    );
    assert!(
        zod.contains("z.strictObject({ labels: z.record(z.string(), z.string()), })"),
        "Got:\n{zod}"
    );
}

/// A map is one of the inner shapes an untagged member leaves open for v1, so each branch keeps
/// the member as a required key carrying the permissive empty schema.
#[test]
#[cfg(feature = "jsonschema")]
fn test_untagged_map_member_keys_json_schema() {
    let schema = KeyedUnion::json_schema();
    let any_of = schema["anyOf"].as_array().unwrap();
    assert_eq!(any_of.len(), 2, "Got:\n{schema}");
    for (branch, member) in any_of.iter().zip(["counts", "labels"]) {
        assert_eq!(branch["properties"][member], serde_json::json!({}));
        assert_eq!(branch["required"], serde_json::json!([member]));
    }
}

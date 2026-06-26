use serde::{Deserialize, Serialize};
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

#[test]
fn test_serde_round_trip_number_member() {
    let value = DateValue::N(5);
    let json = serde_json::to_string(&value).unwrap();
    assert_eq!(json, "5");
    let back: DateValue = serde_json::from_str(&json).unwrap();
    assert_eq!(back, value);
}

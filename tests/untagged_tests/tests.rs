use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tixschema::model_schema;

#[cfg(all(feature = "jsonschema", feature = "object_id"))]
use mongodb::bson::oid::ObjectId;

// ISO-8601 date string. Branded newtype carries the regex pattern, written with `\d` and reaching
// every surface as the members it stands for.
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
enum KeyedUnion {
    Counts { counts: HashMap<Bucket, u32> },
    Labels { labels: HashMap<String, String> },
}

// A tuple written in an untagged member. Serde writes it as the fixed-arity array a tuple field
// writes, so the member has to describe as that field does.
#[model_schema()]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
enum ShapedUnion {
    Pair { pair: (i64, String) },
}

// The field-position twins: the same members written as struct fields. Field position is the
// rendering an untagged member is held against, so it is written out rather than restated.
#[cfg(feature = "jsonschema")]
#[model_schema()]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct KeyedFields {
    counts: HashMap<Bucket, u32>,
    labels: HashMap<String, String>,
}

#[cfg(feature = "jsonschema")]
#[model_schema()]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ShapedFields {
    pair: (i64, String),
}

// An untagged struct variant carrying a constrained member, beside the same member written in a
// tagged enum. The two must render the same constrained value.
#[cfg(any(feature = "zod", feature = "jsonschema"))]
#[model_schema()]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum ConstrainedUnion {
    Slug {
        #[model_schema_prop(minLength = 2, pattern = "^[a-z]+$")]
        slug: String,
    },
}

#[cfg(any(feature = "zod", feature = "jsonschema"))]
#[model_schema()]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
enum ConstrainedTagged {
    Slug {
        #[model_schema_prop(minLength = 2, pattern = "^[a-z]+$")]
        slug: String,
    },
}

// An untagged member is written by a dispatch of its own, so it is a position an `ObjectId` can be
// spelled in.
#[cfg(all(feature = "jsonschema", feature = "object_id"))]
#[model_schema()]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum OidUnion {
    One { one: ObjectId },
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
        zod.contains(".check(z.regex(/^[0-9]{4}-[0-9]{2}-[0-9]{2}$/))"),
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
    assert_eq!(any_of[1]["pattern"], "^[0-9]{4}-[0-9]{2}-[0-9]{2}$");
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
    assert_eq!(any_of[1]["pattern"], "^[0-9]{4}-[0-9]{2}-[0-9]{2}$");
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

/// What serde writes for an untagged member holding a map — the capture the three surfaces are
/// held against. The enumerated key reaches the wire as the member name it spells; the open key
/// reaches it as itself.
#[test]
fn test_untagged_map_member_wire() {
    let counts = KeyedUnion::Counts {
        counts: HashMap::from([(Bucket::Large, 3_u32)]),
    };
    let labels = KeyedUnion::Labels {
        labels: HashMap::from([("a".to_owned(), "b".to_owned())]),
    };

    let counts_wire = serde_json::to_value(&counts).unwrap();
    let labels_wire = serde_json::to_value(&labels).unwrap();
    assert_eq!(
        counts_wire,
        serde_json::json!({ "counts": { "Large": 3_i32 } })
    );
    assert_eq!(labels_wire, serde_json::json!({ "labels": { "a": "b" } }));

    assert_eq!(
        serde_json::from_value::<KeyedUnion>(counts_wire).unwrap(),
        counts
    );
    assert_eq!(
        serde_json::from_value::<KeyedUnion>(labels_wire).unwrap(),
        labels
    );
}

/// The same capture for a tuple member: serde writes the fixed-arity array, in declaration order.
#[test]
fn test_untagged_tuple_member_wire() {
    let pair = ShapedUnion::Pair {
        pair: (7_i64, "seven".to_owned()),
    };

    let wire = serde_json::to_value(&pair).unwrap();
    assert_eq!(wire, serde_json::json!({ "pair": [7_i64, "seven"] }));
    assert_eq!(serde_json::from_value::<ShapedUnion>(wire).unwrap(), pair);
}

/// A member holding a map describes as the wire it was captured from: the enumerated key spells its
/// properties, the open key its `additionalProperties`. Held against the struct field written from
/// the same type, which is the rendering a member must not diverge from.
#[test]
#[cfg(feature = "jsonschema")]
fn test_untagged_map_member_keys_json_schema() {
    let schema = KeyedUnion::json_schema();
    let fields = KeyedFields::json_schema();
    let any_of = schema["anyOf"].as_array().unwrap();
    assert_eq!(any_of.len(), 2, "Got:\n{schema}");

    assert_eq!(
        any_of[0]["properties"]["counts"],
        serde_json::json!({
            "type": "object",
            "properties": { "Large": { "type": "integer" }, "Small": { "type": "integer" } },
            "additionalProperties": false
        }),
        "Got:\n{schema}"
    );
    assert_eq!(
        any_of[1]["properties"]["labels"],
        serde_json::json!({
            "type": "object",
            "additionalProperties": { "type": "string" }
        }),
        "Got:\n{schema}"
    );

    for (branch, member) in any_of.iter().zip(["counts", "labels"]) {
        assert_eq!(
            branch["properties"][member], fields["properties"][member],
            "the field-position twin must render the same member"
        );
        assert_eq!(branch["required"], serde_json::json!([member]));
    }
}

/// The schema admits the wire the capture recorded: every key serde writes for the enumerated map
/// is one the branch names, and the branch names no key serde cannot write. The object is closed,
/// so a property set that drifted either way would reject the payload the type produces.
#[test]
#[cfg(feature = "jsonschema")]
fn test_untagged_map_member_schema_admits_the_captured_wire() {
    let counts = KeyedUnion::Counts {
        counts: HashMap::from([(Bucket::Large, 1_u32), (Bucket::Small, 2_u32)]),
    };
    let wire = serde_json::to_value(&counts).unwrap();
    let schema = KeyedUnion::json_schema();
    let member = &schema["anyOf"][0]["properties"]["counts"];

    let mut written: Vec<&String> = wire["counts"].as_object().unwrap().keys().collect();
    let mut named: Vec<&String> = member["properties"].as_object().unwrap().keys().collect();
    written.sort_unstable();
    named.sort_unstable();
    assert_eq!(written, named, "Got:\n{schema}");
    assert_eq!(member["additionalProperties"], serde_json::json!(false));
}

/// The same for the tuple member: the array serde writes has the arity the bounds pin and the
/// element types `prefixItems` names, in order.
#[test]
#[cfg(feature = "jsonschema")]
fn test_untagged_tuple_member_schema_admits_the_captured_wire() {
    let pair = ShapedUnion::Pair {
        pair: (7_i64, "seven".to_owned()),
    };
    let wire = serde_json::to_value(&pair).unwrap();
    let schema = ShapedUnion::json_schema();
    let member = &schema["anyOf"][0]["properties"]["pair"];
    let written = wire["pair"].as_array().unwrap();

    assert_eq!(member["minItems"], serde_json::json!(written.len()));
    assert_eq!(member["maxItems"], serde_json::json!(written.len()));

    let named: Vec<&str> = member["prefixItems"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["type"].as_str().unwrap())
        .collect();
    assert_eq!(named, ["integer", "string"], "Got:\n{schema}");
    assert!(written[0].is_i64(), "Got:\n{wire}");
    assert!(written[1].is_string(), "Got:\n{wire}");
}

/// A member holding a tuple describes as the array serde writes, arity bounds and all — the same
/// rendering the struct field written from the same type carries.
#[test]
#[cfg(feature = "jsonschema")]
fn test_untagged_tuple_member_json_schema() {
    let schema = ShapedUnion::json_schema();
    let member = &schema["anyOf"][0]["properties"]["pair"];
    assert_eq!(
        *member,
        serde_json::json!({
            "type": "array",
            "prefixItems": [{ "type": "integer" }, { "type": "string" }],
            "items": false,
            "minItems": 2_i32,
            "maxItems": 2_i32
        }),
        "Got:\n{schema}"
    );
    assert_eq!(
        *member,
        ShapedFields::json_schema()["properties"]["pair"],
        "the field-position twin must render the same member"
    );
}

#[test]
#[cfg(feature = "typescript")]
fn test_untagged_tuple_member_typescript() {
    let ts = ShapedUnion::ts_definition();
    assert!(
        ts.contains("export type ShapedUnion = { pair: [number, string] };"),
        "Got:\n{ts}"
    );
}

#[test]
#[cfg(feature = "zod")]
fn test_untagged_tuple_member_zod() {
    let zod = ShapedUnion::zod_schema();
    assert!(
        zod.contains("z.strictObject({ pair: z.tuple([z.number().int(), z.string()]), })"),
        "Got:\n{zod}"
    );
}

/// The member's constraint reaches Zod in the spelling the tagged twin's does. Before this, the
/// attribute never reached the macro at all: the untagged walk left it on the emitted item and
/// rustc refused it as an attribute that does not exist.
#[test]
#[cfg(feature = "zod")]
fn test_untagged_member_constraint_zod() {
    let expected = "slug: z.string().min(2).check(z.regex(/^[a-z]+$/)),";
    let untagged = ConstrainedUnion::zod_schema();
    assert!(untagged.contains(expected), "Got:\n{untagged}");
    assert!(
        ConstrainedTagged::zod_schema().contains(expected),
        "the tagged twin must render the same member"
    );
}

/// The same constraint in the JSON schema, keyword for keyword with the tagged twin's.
#[test]
#[cfg(feature = "jsonschema")]
fn test_untagged_member_constraint_json_schema() {
    let untagged = ConstrainedUnion::json_schema();
    let member = &untagged["anyOf"][0]["properties"]["slug"];
    assert_eq!(
        *member,
        serde_json::json!({ "type": "string", "minLength": 2_i32, "pattern": "^[a-z]+$" }),
        "Got:\n{untagged}"
    );
    let tagged = ConstrainedTagged::json_schema();
    assert_eq!(
        tagged["oneOf"][0]["properties"]["slug"], *member,
        "the tagged twin must render the same member"
    );
}

/// An untagged member spells the `$oid` object the way every other position spells it — a member is
/// written by its own dispatch, which is one more place the object could have drifted.
#[test]
#[cfg(all(feature = "jsonschema", feature = "object_id"))]
fn test_untagged_objectid_member_spells_the_one_oid_object() {
    let schema = OidUnion::json_schema();
    assert_eq!(
        schema["anyOf"][0]["properties"]["one"],
        serde_json::json!({
            "type": "object",
            "properties": { "$oid": { "type": "string", "pattern": r"^[a-f\d]{24}$" } },
            "required": ["$oid"],
            "additionalProperties": false
        }),
        "Got:\n{schema}"
    );
}

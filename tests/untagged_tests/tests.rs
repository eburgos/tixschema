use alloc::collections::{BTreeSet, VecDeque};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use tixschema::model_schema;

#[cfg(all(feature = "jsonschema", feature = "object_id"))]
use mongodb::bson::oid::ObjectId;

/// The members of `StringArrayUnion`, in the order the union declares them.
#[cfg(feature = "jsonschema")]
const STRING_ARRAY_MEMBERS: [&str; 3] = ["rows", "slugs", "tags"];

/// The members of `WrappedUnion`, in the order the union declares them.
#[cfg(any(feature = "jsonschema", feature = "typescript", feature = "zod"))]
const WRAPPED_MEMBERS: [&str; 3] = ["ids", "ordered", "queued"];

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

// A struct variant's own `rename_all` cases its fields — distinct from the enum's own
// `rename_all`, which cases variant names, never fields. Both variants carry one so the flatten
// holder below closes two renamed keys against each other.
#[model_schema()]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
enum RenameAllUnion {
    #[serde(rename_all = "kebab-case")]
    Fresh { subject_line: String },
    #[serde(rename_all = "kebab-case")]
    Reply { reply_to: String },
}

#[cfg(feature = "typescript")]
#[model_schema()]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RenameAllUnionHolder {
    #[serde(flatten)]
    body: RenameAllUnion,
    own: String,
}

// The identifier-legal control: a rename needing no quoting, proving the fix reaches every rename
// rather than only the ones the quoting rule happens to also cover.
#[cfg(feature = "typescript")]
#[model_schema()]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
enum RenamedIdentifierUnion {
    One {
        #[serde(rename = "replyTo")]
        reply_to: String,
    },
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

// The std wrappers serde writes as a JSON array of their element, written in untagged members:
// each describes as the `Vec` of the same element does, not as a schema module named after the
// wrapper.
#[model_schema()]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
enum WrappedUnion {
    Ids { ids: HashSet<String> },
    Ordered { ordered: BTreeSet<String> },
    Queued { queued: VecDeque<String> },
}

// A `String` under array levels, written in untagged members: bare, bounded, and nested. The value
// a member holds is written straight into the array wrap, so the one value built from statements
// has to reach that wrap the way every other member value does.
#[model_schema()]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
enum StringArrayUnion {
    Rows {
        rows: Vec<Vec<String>>,
    },
    Slugs {
        #[model_schema_prop(minLength = 2, pattern = "^[a-z]+$")]
        slugs: Vec<String>,
    },
    Tags {
        tags: Vec<String>,
    },
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

#[cfg(feature = "jsonschema")]
#[model_schema()]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct WrappedFields {
    ids: HashSet<String>,
    ordered: BTreeSet<String>,
    queued: VecDeque<String>,
}

#[cfg(feature = "jsonschema")]
#[model_schema()]
#[derive(Debug, Clone, Serialize, Deserialize)]
struct StringArrayFields {
    rows: Vec<Vec<String>>,
    #[model_schema_prop(minLength = 2, pattern = "^[a-z]+$")]
    slugs: Vec<String>,
    tags: Vec<String>,
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

// An untagged union whose only member carries a bound: with no other branch to fall to, a value the
// bound rejects is a value the whole union rejects.
#[model_schema()]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
enum SoleConstrainedUnion {
    Slug {
        #[model_schema_prop(minLength = 2, pattern = "^[a-z]+$")]
        slug: String,
    },
}

// A tagged twin of `SoleConstrainedUnion`: the same member under the same bound, written where the
// tag keeps a failing read on the variant it names.
#[model_schema()]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
enum SoleConstrainedTagged {
    Slug {
        #[model_schema_prop(minLength = 2, pattern = "^[a-z]+$")]
        slug: String,
    },
}

// The same member written twice, bound first and bare second: what a failing bound costs is the
// branch, not the read.
#[model_schema()]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
enum CheckedThenLooseUnion {
    Checked {
        #[model_schema_prop(minLength = 2)]
        name: String,
    },
    Loose {
        name: String,
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

/// The untagged lone-slot collapse, declared without a schema because the union refuses it — the
/// wire captured below is the one that refusal has to fit.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(untagged)]
pub enum UntaggedLoneSlotWire {
    Lone(#[serde(skip)] String),
}

/// The same variant declared as the unit it is written as, sharing that one wire.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(untagged)]
pub enum UntaggedUnitWire {
    Lone,
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

/// A variant's own `rename_all` reaches its fields on the untagged path, the same as it does on the
/// struct-field path — quoted here since kebab-case is not identifier-legal.
#[test]
#[cfg(feature = "typescript")]
fn test_rename_all_union_typescript() {
    let ts = RenameAllUnion::ts_definition();
    assert!(
        ts.contains(
            r#"export type RenameAllUnion = { "subject-line": string } | { "reply-to": string };"#
        ),
        "Got:\n{ts}"
    );
}

/// The identifier-legal control: the rename still reaches the surface, quoting or not.
#[test]
#[cfg(feature = "typescript")]
fn test_renamed_identifier_union_typescript() {
    let ts = RenamedIdentifierUnion::ts_definition();
    assert!(
        ts.contains("export type RenamedIdentifierUnion = { replyTo: string };"),
        "Got:\n{ts}"
    );
}

/// The flatten-operand seam and its sibling exclusions, both members renamed: neither key is the
/// Rust ident, on either side of the union.
#[test]
#[cfg(feature = "typescript")]
fn test_rename_all_union_flatten_sibling_exclusions() {
    let ts = RenameAllUnionHolder::ts_definition();
    assert!(
        ts.contains(
            "} & ({ \"subject-line\": string; \"reply-to\"?: never } | { \"reply-to\": string; \"subject-line\"?: never });"
        ),
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
fn test_rename_all_union_zod() {
    let zod = RenameAllUnion::zod_schema();
    assert!(
        zod.contains(r#"z.strictObject({ "reply-to": z.string(), })"#),
        "Got:\n{zod}"
    );
    assert!(
        zod.contains(r#"z.strictObject({ "subject-line": z.string(), })"#),
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
             note: z.union([z.null().transform(() => undefined), z.string(), z.undefined()]).prefault(undefined), })"
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
fn test_rename_all_union_json_schema() {
    let schema = RenameAllUnion::json_schema();
    let any_of = schema["anyOf"].as_array().unwrap();
    let branch = any_of
        .iter()
        .find(|branch| {
            branch["properties"]
                .as_object()
                .unwrap()
                .contains_key("reply-to")
        })
        .unwrap();
    assert_eq!(branch["required"], serde_json::json!(["reply-to"]));
}

/// A closed document — every leaf `additionalProperties: false` — accepts exactly the payload serde
/// writes for a variant-`rename_all` member, and the value round-trips through it.
#[test]
#[cfg(feature = "jsonschema")]
fn test_rename_all_union_round_trips_through_its_closed_schema() {
    let value = RenameAllUnion::Reply {
        reply_to: "x".to_owned(),
    };
    let payload = serde_json::to_value(&value).unwrap();
    assert_eq!(payload, serde_json::json!({ "reply-to": "x" }));

    let schema = RenameAllUnion::json_schema();
    let any_of = schema["anyOf"].as_array().unwrap();
    let branch = any_of
        .iter()
        .find(|branch| {
            branch["properties"]
                .as_object()
                .unwrap()
                .contains_key("reply-to")
        })
        .unwrap();
    let named = branch["properties"].as_object().unwrap();
    let required = branch["required"].as_array().unwrap();
    let written = payload.as_object().unwrap();
    assert!(written.keys().all(|key| named.contains_key(key)));
    assert!(
        required
            .iter()
            .all(|key| written.contains_key(key.as_str().unwrap()))
    );

    let back: RenameAllUnion = serde_json::from_value(payload).unwrap();
    assert_eq!(back, value);
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

    let properties = any_of[1]["properties"].as_object().unwrap();
    assert_eq!(
        properties["note"],
        serde_json::json!({ "anyOf": [{ "type": "string" }, { "type": "null" }] }),
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

#[test]
fn test_serde_round_trip_string_member() {
    let value = DateValue::S(DateString("2026-06-26".to_owned()));
    let json = serde_json::to_string(&value).unwrap();
    assert_eq!(json, "\"2026-06-26\"");
    let back: DateValue = serde_json::from_str(&json).unwrap();
    assert_eq!(back, value);
}

/// The unrenamed member is untouched by the fix: `Fresh` still writes and reads its own field name.
#[test]
fn test_serde_round_trip_unrenamed_untagged_member() {
    let value = NamedUnion::A {
        x: "text".to_owned(),
    };
    let json = serde_json::to_string(&value).unwrap();
    assert_eq!(json, r#"{"x":"text"}"#);
}

/// A variant's own `rename_all` reaches the wire, not just the schema surfaces above.
#[test]
fn test_serde_round_trip_rename_all_union_member() {
    let value = RenameAllUnion::Fresh {
        subject_line: "hi".to_owned(),
    };
    let json = serde_json::to_string(&value).unwrap();
    assert_eq!(json, r#"{"subject-line":"hi"}"#);
    let back: RenameAllUnion = serde_json::from_str(&json).unwrap();
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

/// What serde writes for an untagged member holding a covered sequence wrapper — the capture the
/// three surfaces are held against. Every one of them writes the JSON array a `Vec` of the same
/// element writes, which is the whole reason the wrapper is covered.
#[test]
fn test_untagged_wrapper_member_wire() {
    for value in [
        WrappedUnion::Ids {
            ids: HashSet::from(["north".to_owned()]),
        },
        WrappedUnion::Ordered {
            ordered: BTreeSet::from(["north".to_owned()]),
        },
        WrappedUnion::Queued {
            queued: VecDeque::from(["north".to_owned()]),
        },
    ] {
        let wire = serde_json::to_value(&value).unwrap();
        let (member, written) = wire.as_object().unwrap().iter().next().unwrap();
        assert_eq!(*written, serde_json::json!(["north"]), "for: {member}");
        assert_eq!(
            serde_json::from_value::<WrappedUnion>(wire.clone()).unwrap(),
            value,
            "for: {member}"
        );
    }
}

/// A member holding a covered sequence wrapper describes as the array serde writes — the element's
/// own schema under array wrapping. Held against the struct field written from the same type, the
/// rendering a member must not diverge from.
#[test]
#[cfg(feature = "jsonschema")]
fn test_untagged_wrapper_member_json_schema() {
    let schema = WrappedUnion::json_schema();
    let fields = WrappedFields::json_schema();
    let any_of = schema["anyOf"].as_array().unwrap();
    assert_eq!(any_of.len(), WRAPPED_MEMBERS.len(), "Got:\n{schema}");

    for (branch, member) in any_of.iter().zip(WRAPPED_MEMBERS) {
        assert_eq!(
            branch["properties"][member],
            serde_json::json!({ "type": "array", "items": { "type": "string" } }),
            "Got:\n{schema}"
        );
        assert_eq!(
            branch["properties"][member], fields["properties"][member],
            "the field-position twin must render the same member"
        );
        assert_eq!(branch["required"], serde_json::json!([member]));
    }
}

/// The schema admits the wire the capture recorded: serde writes an array, and each item is what
/// the member's `items` schema names. A member still carrying the wrapper's own name could not have
/// described this payload at all — it named a schema module no expansion declares.
#[test]
#[cfg(feature = "jsonschema")]
fn test_untagged_wrapper_member_schema_admits_the_captured_wire() {
    let queued = WrappedUnion::Queued {
        queued: VecDeque::from(["north".to_owned(), "south".to_owned()]),
    };
    let wire = serde_json::to_value(&queued).unwrap();
    let schema = WrappedUnion::json_schema();
    let member = &schema["anyOf"][2]["properties"]["queued"];
    let written = wire["queued"].as_array().unwrap();

    assert_eq!(member["type"], serde_json::json!("array"), "Got:\n{schema}");
    assert_eq!(member["items"]["type"], serde_json::json!("string"));
    assert!(
        written.iter().all(serde_json::Value::is_string),
        "Got:\n{wire}"
    );
}

#[test]
#[cfg(feature = "typescript")]
fn test_untagged_wrapper_member_typescript() {
    let ts = WrappedUnion::ts_definition();
    for member in WRAPPED_MEMBERS {
        assert!(
            ts.contains(&format!("{{ {member}: Array<string> }}")),
            "Got:\n{ts}"
        );
    }
}

#[test]
#[cfg(feature = "zod")]
fn test_untagged_wrapper_member_zod() {
    let zod = WrappedUnion::zod_schema();
    for member in WRAPPED_MEMBERS {
        assert!(
            zod.contains(&format!(
                "z.strictObject({{ {member}: z.array(z.string()), }})"
            )),
            "Got:\n{zod}"
        );
    }
}

/// What serde writes for an untagged member holding a `String` under array levels — the capture the
/// schema is held against. Bare, bounded and nested all write the array of strings the field of the
/// same written type writes.
#[test]
fn test_untagged_string_array_member_wire() {
    for (value, written) in [
        (
            StringArrayUnion::Rows {
                rows: vec![vec!["north".to_owned()]],
            },
            serde_json::json!({ "rows": [["north"]] }),
        ),
        (
            StringArrayUnion::Slugs {
                slugs: vec!["north".to_owned()],
            },
            serde_json::json!({ "slugs": ["north"] }),
        ),
        (
            StringArrayUnion::Tags {
                tags: vec!["north".to_owned()],
            },
            serde_json::json!({ "tags": ["north"] }),
        ),
    ] {
        let wire = serde_json::to_value(&value).unwrap();
        assert_eq!(wire, written);
        assert_eq!(
            serde_json::from_value::<StringArrayUnion>(wire.clone()).unwrap(),
            value,
            "for: {wire}"
        );
    }
}

/// A member holding a `String` under array levels describes as the array of strings serde writes,
/// matching the struct field written from the same type. This used to be the one value the array
/// wrap could not carry — a Rust block landing where the wrap's `serde_json::json!` reads a JSON
/// object, an expansion that never reached a compiler.
#[test]
#[cfg(feature = "jsonschema")]
fn test_untagged_string_array_member_json_schema() {
    let schema = StringArrayUnion::json_schema();
    let fields = StringArrayFields::json_schema();
    let any_of = schema["anyOf"].as_array().unwrap();
    assert_eq!(any_of.len(), 3, "Got:\n{schema}");

    for (branch, member) in any_of.iter().zip(STRING_ARRAY_MEMBERS) {
        assert_eq!(
            branch["properties"][member], fields["properties"][member],
            "the field-position twin must render `{member}`:\n{schema}"
        );
        assert_eq!(branch["required"], serde_json::json!([member]));
    }

    assert_eq!(
        any_of[2]["properties"]["tags"],
        serde_json::json!({ "type": "array", "items": { "type": "string" } }),
        "Got:\n{schema}"
    );
    assert_eq!(
        any_of[0]["properties"]["rows"],
        serde_json::json!({
            "type": "array",
            "items": { "type": "array", "items": { "type": "string" } }
        }),
        "Got:\n{schema}"
    );
}

/// A bounded `String` under array levels keeps its bounds on the items, where the value they
/// constrain sits — the array itself carries none. Held against the field twin, which is where the
/// same bounds are written today.
#[test]
#[cfg(feature = "jsonschema")]
fn test_untagged_string_array_member_keeps_its_bounds_on_the_items() {
    let schema = StringArrayUnion::json_schema();
    let slugs = &schema["anyOf"][1]["properties"]["slugs"];

    assert_eq!(
        *slugs,
        serde_json::json!({
            "type": "array",
            "items": { "type": "string", "minLength": 2_u32, "pattern": "^[a-z]+$" }
        }),
        "Got:\n{schema}"
    );
    assert_eq!(
        *slugs,
        StringArrayFields::json_schema()["properties"]["slugs"],
        "the field-position twin must render the same bounded items"
    );
}

/// The wrapper spellings of the same element reach the array wrap too, so a `HashSet<String>` in a
/// member describes as the `Vec<String>` beside it does — the headline spelling, held against both
/// the wrapper union and the bare-array one.
#[test]
#[cfg(feature = "jsonschema")]
fn test_untagged_wrapper_of_string_describes_as_the_bare_string_array() {
    let wrapped = WrappedUnion::json_schema();
    let arrayed = StringArrayUnion::json_schema();

    for (branch, member) in wrapped["anyOf"]
        .as_array()
        .unwrap()
        .iter()
        .zip(WRAPPED_MEMBERS)
    {
        assert_eq!(
            branch["properties"][member], arrayed["anyOf"][2]["properties"]["tags"],
            "`{member}` must describe as the bare `Vec<String>` member:\n{wrapped}"
        );
    }
}

#[test]
#[cfg(feature = "typescript")]
fn test_untagged_string_array_member_typescript() {
    let ts = StringArrayUnion::ts_definition();
    for member in ["{ tags: Array<string> }", "{ rows: Array<Array<string>> }"] {
        assert!(ts.contains(member), "Got:\n{ts}");
    }
}

#[test]
#[cfg(feature = "zod")]
fn test_untagged_string_array_member_zod() {
    let zod = StringArrayUnion::zod_schema();
    for member in [
        "z.strictObject({ tags: z.array(z.string()), })",
        "z.strictObject({ rows: z.array(z.array(z.string())), })",
        "z.strictObject({ slugs: z.array(z.string().min(2).check(z.regex(/^[a-z]+$/))), })",
    ] {
        assert!(zod.contains(member), "Got:\n{zod}");
    }
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

/// The member's bound reaches the Rust side too, so a payload both schema surfaces reject stops
/// being one serde reads back without a word.
#[test]
fn test_untagged_member_constraint_is_enforced_on_deserialize() {
    serde_json::from_str::<SoleConstrainedUnion>(r#"{"slug":"a"}"#).unwrap_err();
    serde_json::from_str::<SoleConstrainedUnion>(r#"{"slug":"AB"}"#).unwrap_err();
    assert_eq!(
        serde_json::from_str::<SoleConstrainedUnion>(r#"{"slug":"ab"}"#).unwrap(),
        SoleConstrainedUnion::Slug {
            slug: "ab".to_owned()
        }
    );
}

/// What a bound means in this position: serde tries the variants in order, and a member the bound
/// rejects takes its variant out of the running rather than ending the read — the same thing the
/// union's own schema does under `anyOf` and `z.union`. A violating value lands on the next branch
/// that accepts it, erroring only when none does.
#[test]
fn test_untagged_member_constraint_decides_which_variant_is_read() {
    assert_eq!(
        serde_json::from_str::<CheckedThenLooseUnion>(r#"{"name":"ab"}"#).unwrap(),
        CheckedThenLooseUnion::Checked {
            name: "ab".to_owned()
        }
    );
    assert_eq!(
        serde_json::from_str::<CheckedThenLooseUnion>(r#"{"name":"A"}"#).unwrap(),
        CheckedThenLooseUnion::Loose {
            name: "A".to_owned()
        }
    );
}

/// Serialization is untouched: the hook is a deserializer, and the value it refuses on the way in
/// is one the type can still be built with and written out.
#[test]
fn test_untagged_member_constraint_leaves_serialization_alone() {
    let violating = SoleConstrainedUnion::Slug {
        slug: "A".to_owned(),
    };
    assert_eq!(
        serde_json::to_string(&violating).unwrap(),
        r#"{"slug":"A"}"#
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
            "properties": { "$oid": { "type": "string", "pattern": "^[a-f0-9]{24}$" } },
            "required": ["$oid"],
            "additionalProperties": false
        }),
        "Got:\n{schema}"
    );
}

/// What the read costs the author: serde's derived `Deserialize` for an untagged enum drops each
/// candidate's own error as it moves to the next, so when no variant accepts, the bound's own
/// error is gone and one generic sentence stands in its place. The tagged twin, whose tag names
/// the variant before its members are read, keeps the bound's own words.
#[test]
fn test_untagged_member_bound_failure_reports_serdes_generic_sentence() {
    assert_eq!(
        serde_json::from_str::<SoleConstrainedUnion>(r#"{"slug":"A"}"#)
            .unwrap_err()
            .to_string(),
        "data did not match any variant of untagged enum SoleConstrainedUnion"
    );
    assert_eq!(
        serde_json::from_str::<SoleConstrainedTagged>(r#"{"kind":"Slug","slug":"A"}"#)
            .unwrap_err()
            .to_string(),
        "'slug' is too short: minimum length is 2, got 1"
    );
}

/// The accessor is the one Rust surface that names the bound a union's value violates: the read
/// path hands the bound to serde, which discards the sentence with the candidate it removes, so a
/// value built or read back in Rust has nothing else to ask. It answers in the tagged twin's words.
#[test]
fn test_untagged_union_publishes_validate_for_its_constrained_members() {
    let expected = vec!["'slug' is too short: minimum length is 2, got 1".to_owned()];
    assert_eq!(
        SoleConstrainedUnion::Slug {
            slug: "A".to_owned()
        }
        .validate()
        .unwrap_err(),
        expected
    );
    assert_eq!(
        SoleConstrainedTagged::Slug {
            slug: "A".to_owned()
        }
        .validate()
        .unwrap_err(),
        expected
    );
    SoleConstrainedUnion::Slug {
        slug: "ab".to_owned(),
    }
    .validate()
    .unwrap();
}

/// A union whose variants differ in what they bind: the value's own variant decides which checks
/// run, and a variant carrying no bound has nothing to answer for.
#[test]
fn test_untagged_validate_runs_only_the_held_variants_checks() {
    assert_eq!(
        CheckedThenLooseUnion::Checked {
            name: "A".to_owned()
        }
        .validate()
        .unwrap_err(),
        vec!["'name' is too short: minimum length is 2, got 1".to_owned()]
    );
    assert!(
        CheckedThenLooseUnion::Loose {
            name: "A".to_owned()
        }
        .validate()
        .is_ok(),
        "the bare member is held to no bound"
    );
}

/// The wire the untagged collapse writes and reads: the slot is off the wire in both directions, so
/// serde writes `null` and reads `null` back, and neither the slot's own value nor an array matches.
#[test]
fn test_serde_writes_and_reads_null_for_an_untagged_variant_whose_lone_slot_is_dropped() {
    let written = serde_json::to_value(UntaggedLoneSlotWire::Lone("s".to_owned())).unwrap();
    assert_eq!(written, serde_json::Value::Null);
    assert_eq!(
        serde_json::from_value::<UntaggedLoneSlotWire>(written).unwrap(),
        UntaggedLoneSlotWire::Lone(String::new())
    );
    assert!(
        serde_json::from_str::<UntaggedLoneSlotWire>(r#""s""#).is_err(),
        "the slot's own value must not read back"
    );
    assert!(
        serde_json::from_str::<UntaggedLoneSlotWire>("[]").is_err(),
        "no array spelling reads back"
    );
}

/// The control: a variant declared as a unit writes and reads the same `null`, so the two
/// declarations share one wire in both directions.
#[test]
fn test_serde_writes_and_reads_the_same_null_for_a_declared_untagged_unit_variant() {
    let written = serde_json::to_value(UntaggedUnitWire::Lone).unwrap();
    assert_eq!(written, serde_json::Value::Null);
    assert_eq!(
        serde_json::from_value::<UntaggedUnitWire>(written).unwrap(),
        UntaggedUnitWire::Lone
    );
}

//! The wire is the arbiter, read in both directions before any surface is asserted.
//!
//! Three spellings drop a field out of one direction or both: a bare `skip` drops the key from
//! both read and write, so no surface has a member to describe; `skip_serializing` alone drops
//! the key on the way out but still reads a supplied one, so its member is described under an
//! optional key; `skip_deserializing` alone writes the key in every payload, so its member keeps
//! a required one.
//!
//! Dropping the member is stricter than serde on the read side, deliberately. Recorded against
//! zod 4.4.3 under node v26.2.0: `z.strictObject({ id: z.string() })` rejects
//! `{ id: "1", internal: ["x"] }` with `unrecognized_keys`, while serde accepts that same payload
//! and discards the value — the surfaces describe the payload serde *writes*, and serde writes
//! that key in no payload at all.

use serde::{Deserialize, Serialize};
use tixschema::model_schema;

/// The field the surfaces owe nothing for, beside a member serde does write.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct SkippedField {
    id: String,
    #[serde(skip)]
    internal: Vec<String>,
}

/// The same wire, written as the two halves rather than the word abbreviating them.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct BothHalvesDropped {
    id: String,
    #[serde(skip_serializing, skip_deserializing)]
    internal: Vec<String>,
}

/// Only the write half. The key is never written and a supplied one is still read, so the member
/// stays — under an optional key, which is what the landed omission contract already gives it.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct WriteHalfDropped {
    id: String,
    #[serde(default, skip_serializing)]
    roles: Vec<String>,
}

/// Only the read half. The key is written in every payload and a supplied one is discarded, so the
/// member stays under a required key.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct ReadHalfDropped {
    id: String,
    #[serde(skip_deserializing)]
    roles: Vec<String>,
}

/// The same question asked of a named variant member, whose defs are collected on their own path.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "kind")]
enum SkippedVariantField {
    One {
        id: String,
        #[serde(skip)]
        internal: Vec<String>,
    },
}

/// And of an untagged one, collected on a third path again.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
enum SkippedUntaggedField {
    One {
        id: String,
        #[serde(skip)]
        internal: Vec<String>,
    },
}

fn skipped_field() -> SkippedField {
    SkippedField {
        id: "1".to_owned(),
        internal: vec!["secret".to_owned()],
    }
}

/// serde writes no payload carrying the key and reads the value out of none — the pair every
/// surface expectation below is measured against.
#[test]
fn the_skipped_key_is_written_in_no_payload_and_read_out_of_none() {
    assert_eq!(
        serde_json::to_string(&skipped_field()).unwrap(),
        r#"{"id":"1"}"#
    );

    let supplied = serde_json::from_str::<SkippedField>(r#"{"id":"1","internal":["x"]}"#).unwrap();
    assert_eq!(supplied.internal, Vec::<String>::new());
    let absent = serde_json::from_str::<SkippedField>(r#"{"id":"1"}"#).unwrap();
    assert_eq!(absent.internal, Vec::<String>::new());
}

/// The halves written side by side produce that same pair, so they owe the same surfaces.
#[test]
fn the_two_halves_written_together_are_the_same_wire() {
    assert_eq!(
        serde_json::to_string(&BothHalvesDropped {
            id: "1".to_owned(),
            internal: vec!["secret".to_owned()],
        })
        .unwrap(),
        r#"{"id":"1"}"#
    );

    let supplied =
        serde_json::from_str::<BothHalvesDropped>(r#"{"id":"1","internal":["x"]}"#).unwrap();
    assert_eq!(supplied.internal, Vec::<String>::new());
}

/// The write half alone reads a supplied key, which is what keeps its member on the surfaces.
#[test]
fn the_write_half_alone_still_reads_a_supplied_key() {
    assert_eq!(
        serde_json::to_string(&WriteHalfDropped {
            id: "1".to_owned(),
            roles: vec!["admin".to_owned()],
        })
        .unwrap(),
        r#"{"id":"1"}"#
    );

    let supplied = serde_json::from_str::<WriteHalfDropped>(r#"{"id":"1","roles":["x"]}"#).unwrap();
    assert_eq!(supplied.roles, vec!["x".to_owned()]);
}

/// The read half alone writes the key in every payload, which is what keeps its member required.
#[test]
fn the_read_half_alone_writes_the_key_in_every_payload() {
    assert_eq!(
        serde_json::to_string(&ReadHalfDropped {
            id: "1".to_owned(),
            roles: vec!["admin".to_owned()],
        })
        .unwrap(),
        r#"{"id":"1","roles":["admin"]}"#
    );

    let supplied = serde_json::from_str::<ReadHalfDropped>(r#"{"id":"1","roles":["x"]}"#).unwrap();
    assert_eq!(supplied.roles, Vec::<String>::new());
}

/// No member at all — not an optional one. The name appears nowhere in the emission, the embedded
/// JSON schema in the `JSDoc` block included.
#[test]
#[cfg(feature = "typescript")]
fn typescript_writes_no_member_for_the_field_serde_never_carries() {
    let ts = SkippedField::ts_definition();

    assert!(!ts.contains("internal"), "Got: {ts}");
    assert!(ts.contains("id: string;"), "Got: {ts}");
}

/// The variant paths answer the same way as the struct one.
#[test]
#[cfg(feature = "typescript")]
fn typescript_writes_no_variant_member_for_the_field_serde_never_carries() {
    let tagged = SkippedVariantField::ts_definition();
    assert!(!tagged.contains("internal"), "Got: {tagged}");
    assert!(tagged.contains("id: string"), "Got: {tagged}");

    let untagged = SkippedUntaggedField::ts_definition();
    assert!(!untagged.contains("internal"), "Got: {untagged}");
    assert!(untagged.contains("id: string"), "Got: {untagged}");
}

/// The two halves written apart are one wire, so they are one emission.
#[test]
#[cfg(feature = "typescript")]
fn typescript_answers_the_two_halves_the_way_it_answers_the_word() {
    let ts = BothHalvesDropped::ts_definition();

    assert!(!ts.contains("internal"), "Got: {ts}");
    assert!(ts.contains("id: string;"), "Got: {ts}");
}

/// The members serde does carry keep the spellings the omission contract gave them: an optional
/// key where the write half was dropped, a required one where only the read half was.
#[test]
#[cfg(feature = "typescript")]
fn typescript_leaves_the_half_dropped_members_as_they_stand() {
    let write_half = WriteHalfDropped::ts_definition();
    assert!(
        write_half.contains("roles?: Array<string>;"),
        "Got: {write_half}"
    );

    let read_half = ReadHalfDropped::ts_definition();
    assert!(
        read_half.contains("roles: Array<string>;"),
        "Got: {read_half}"
    );
    assert!(!read_half.contains("roles?"), "Got: {read_half}");
}

/// No key in the object, which is what makes the surrounding `z.strictObject` reject a payload
/// carrying one — stricter than serde, and the module doc says why that is the honest spelling.
#[test]
#[cfg(feature = "zod")]
fn zod_writes_no_key_for_the_field_serde_never_carries() {
    let zod = SkippedField::zod_schema();

    assert!(!zod.contains("internal"), "Got: {zod}");
    assert!(zod.contains("id: z.string(),"), "Got: {zod}");
}

#[test]
#[cfg(feature = "zod")]
fn zod_writes_no_variant_key_for_the_field_serde_never_carries() {
    let tagged = SkippedVariantField::zod_schema();
    assert!(!tagged.contains("internal"), "Got: {tagged}");

    let untagged = SkippedUntaggedField::zod_schema();
    assert!(!untagged.contains("internal"), "Got: {untagged}");
}

/// The half-dropped members keep their Zod spellings exactly.
#[test]
#[cfg(feature = "zod")]
fn zod_leaves_the_half_dropped_members_as_they_stand() {
    let write_half = WriteHalfDropped::zod_schema();
    assert!(
        write_half.contains("roles: z.array(z.string()).optional(),"),
        "Got: {write_half}"
    );

    let read_half = ReadHalfDropped::zod_schema();
    assert!(
        read_half.contains("roles: z.array(z.string()),"),
        "Got: {read_half}"
    );
}

/// Neither described nor required: `properties` has no such entry to begin with.
#[test]
#[cfg(feature = "jsonschema")]
fn the_json_schema_neither_describes_nor_requires_the_field_serde_never_carries() {
    let schema = SkippedField::json_schema();

    assert!(schema["properties"]["internal"].is_null(), "Got: {schema}");
    assert_eq!(
        schema["required"],
        serde_json::json!(["id"]),
        "Got: {schema}"
    );
}

#[test]
#[cfg(feature = "jsonschema")]
fn the_json_schema_drops_the_variant_member_too() {
    let tagged = SkippedVariantField::json_schema();
    // With `serde`, `kind` is read as the internal tag and the member sits beside it. Without
    // `serde`, the tag attribute can't be read at all, so the type falls back to the adjacent
    // form (`src/model_schema.rs:5178-5180`) and the member nests one level down, under `value`.
    #[cfg(feature = "serde")]
    let member = &tagged["oneOf"][0];
    #[cfg(not(feature = "serde"))]
    let member = &tagged["oneOf"][0]["properties"]["value"];

    assert!(member["properties"]["internal"].is_null(), "Got: {tagged}");
    let required = member["required"].as_array().unwrap().clone();
    assert!(
        !required.contains(&serde_json::json!("internal")),
        "Got: {tagged}"
    );
    assert!(required.contains(&serde_json::json!("id")), "Got: {tagged}");

    let untagged = SkippedUntaggedField::json_schema();
    // Same split: with `serde` the variant is untagged and sits at the top of `anyOf`; without
    // it, `untagged` is equally unreadable and the same adjacent fallback applies.
    #[cfg(feature = "serde")]
    let untagged_member = &untagged["anyOf"][0];
    #[cfg(not(feature = "serde"))]
    let untagged_member = &untagged["oneOf"][0]["properties"]["value"];
    assert!(
        untagged_member["properties"]["internal"].is_null(),
        "Got: {untagged}"
    );
}

/// The half-dropped members stay described, one out of `required` and one in it.
#[test]
#[cfg(feature = "jsonschema")]
fn the_json_schema_leaves_the_half_dropped_members_as_they_stand() {
    let write_half = WriteHalfDropped::json_schema();
    assert_eq!(
        write_half["properties"]["roles"]["type"],
        serde_json::json!("array"),
        "Got: {write_half}"
    );
    assert_eq!(
        write_half["required"],
        serde_json::json!(["id"]),
        "Got: {write_half}"
    );

    let read_half = ReadHalfDropped::json_schema();
    assert_eq!(
        read_half["required"],
        serde_json::json!(["id", "roles"]),
        "Got: {read_half}"
    );
}

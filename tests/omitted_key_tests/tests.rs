//! The wire is the arbiter. serde writes two payloads for a field carrying a `skip_serializing_if`
//! — one with the key, one without it — and each surface is held to admitting exactly those two.
//!
//! The payloads are asserted first, in this file, so the surface expectations below are read off
//! them rather than off each other. What serde writes does not turn on the crate's `serde` feature,
//! and neither do the expectations: the attribute is on the declaration in every build, so every
//! build owes the same answer.

use serde::{Deserialize, Serialize};
use tixschema::model_schema;

/// One omitted key beside the members that keep theirs: a plain required field and a sequence,
/// neither of which serde ever drops.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct OmittedKeyFields {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    age: Option<u32>,
    id: String,
    roles: Vec<String>,
}

/// The same omission asked of a field that is not an `Option`. A predicate drops the key for a
/// value that is perfectly present in Rust, so the type under the key is unchanged — only whether
/// the key is written at all.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct PredicateOmittedKey {
    id: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    roles: Vec<String>,
}

fn with_age() -> OmittedKeyFields {
    OmittedKeyFields {
        age: Some(30),
        id: "1".to_owned(),
        roles: vec![],
    }
}

fn without_age() -> OmittedKeyFields {
    OmittedKeyFields {
        age: None,
        id: "1".to_owned(),
        roles: vec![],
    }
}

fn with_roles() -> PredicateOmittedKey {
    PredicateOmittedKey {
        id: "1".to_owned(),
        roles: vec!["admin".to_owned()],
    }
}

fn without_roles() -> PredicateOmittedKey {
    PredicateOmittedKey {
        id: "1".to_owned(),
        roles: vec![],
    }
}

/// The two payloads every surface below is measured against.
#[test]
fn the_omitted_key_is_absent_from_the_payload_serde_writes() {
    assert_eq!(
        serde_json::to_string(&with_age()).unwrap(),
        r#"{"age":30,"id":"1","roles":[]}"#
    );
    assert_eq!(
        serde_json::to_string(&without_age()).unwrap(),
        r#"{"id":"1","roles":[]}"#
    );

    assert_eq!(
        serde_json::from_str::<OmittedKeyFields>(r#"{"id":"1","roles":[]}"#).unwrap(),
        without_age()
    );
}

/// The predicate drops the key for an empty `Vec` and writes it for a full one, and `default` is
/// what reads the dropped key back — the pair the surfaces below have to admit.
#[test]
fn the_predicate_omitted_key_is_absent_from_the_payload_serde_writes() {
    assert_eq!(
        serde_json::to_string(&with_roles()).unwrap(),
        r#"{"id":"1","roles":["admin"]}"#
    );
    assert_eq!(
        serde_json::to_string(&without_roles()).unwrap(),
        r#"{"id":"1"}"#
    );

    assert_eq!(
        serde_json::from_str::<PredicateOmittedKey>(r#"{"id":"1"}"#).unwrap(),
        without_roles()
    );
    assert_eq!(
        serde_json::from_str::<PredicateOmittedKey>(r#"{"id":"1","roles":[]}"#).unwrap(),
        without_roles()
    );
}

/// `age` carries no `ts_optional`, so the attribute that drops its key leaves the spelling alone.
#[test]
#[cfg(feature = "typescript")]
fn typescript_writes_the_option_as_undefined_valued() {
    let ts = OmittedKeyFields::ts_definition();

    assert!(ts.contains("age: number | undefined;"), "Got: {ts}");
    assert!(!ts.contains("age?:"), "Got: {ts}");
    assert!(ts.contains("id: string;"), "Got: {ts}");
    assert!(ts.contains("roles: Array<string>;"), "Got: {ts}");
}

/// The key is optional; the value under it never is. A `Vec` serde declined to write is still a
/// `Vec` when it is written, so the type keeps its own spelling and only the key gains the `?`.
#[test]
#[cfg(feature = "typescript")]
fn typescript_writes_the_predicate_omitted_key_as_optional() {
    let ts = PredicateOmittedKey::ts_definition();

    assert!(ts.contains("roles?: Array<string>;"), "Got: {ts}");
    assert!(!ts.contains("roles: Array<string>"), "Got: {ts}");
    assert!(ts.contains("id: string;"), "Got: {ts}");
}

/// The Zod spelling already admits the payload without the key — a `z.strictObject` rejects an
/// unrecognized key, never a missing one whose schema accepts `undefined` — so it is left as it
/// stands, and this pins that it was not disturbed.
#[test]
#[cfg(feature = "zod")]
fn zod_keeps_the_undefined_union_that_already_admits_the_absent_key() {
    let zod = OmittedKeyFields::zod_schema();

    assert!(
        zod.contains(
            "age: z.union([z.null().transform(() => undefined), z.number().int(), z.undefined()]).prefault(undefined),"
        ),
        "Got: {zod}"
    );
    assert!(zod.contains("id: z.string(),"), "Got: {zod}");
    assert!(zod.contains("roles: z.array(z.string()),"), "Got: {zod}");
}

/// A plain `z.array(...)` member rejects the payload serde writes for an empty `Vec` — recorded
/// against zod 4.4.3 under node v26.2.0, `safeParse({ id: "1" })` failing with `invalid_type` at
/// `roles`. `.optional()` is what admits the absent key while still rejecting `null` and still
/// rejecting an unrecognized key under the surrounding `z.strictObject`.
#[test]
#[cfg(feature = "zod")]
fn zod_marks_the_predicate_omitted_key_optional() {
    let zod = PredicateOmittedKey::zod_schema();

    assert!(
        zod.contains("roles: z.array(z.string()).optional(),"),
        "Got: {zod}"
    );
    assert!(zod.contains("id: z.string(),"), "Got: {zod}");
}

/// The JSON surface says it by leaving the field out of `required` while still describing it.
#[test]
#[cfg(feature = "jsonschema")]
fn the_json_schema_describes_the_omitted_key_without_requiring_it() {
    let schema = OmittedKeyFields::json_schema();

    assert!(schema["properties"]["age"].is_object());
    assert_eq!(
        schema["required"],
        serde_json::json!(["id", "roles"]),
        "Got: {schema}"
    );
}

/// Same for the predicate-dropped key: described, not required.
#[test]
#[cfg(feature = "jsonschema")]
fn the_json_schema_describes_the_predicate_omitted_key_without_requiring_it() {
    let schema = PredicateOmittedKey::json_schema();

    assert_eq!(
        schema["properties"]["roles"]["type"],
        serde_json::json!("array"),
        "Got: {schema}"
    );
    assert_eq!(
        schema["required"],
        serde_json::json!(["id"]),
        "Got: {schema}"
    );
}

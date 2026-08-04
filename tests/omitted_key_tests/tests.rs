//! The wire is the arbiter. serde writes two payloads for a field carrying
//! `skip_serializing_if = "Option::is_none"` — one with the key, one without it — and each surface
//! is held to admitting exactly those two.
//!
//! The payloads are asserted first, in this file, so the surface expectations below are read off
//! them rather than off each other.

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

/// `age: number | undefined` would demand the key the absent-key payload does not carry, so the
/// member is written with an optional key instead. The members serde always writes keep theirs.
#[test]
#[cfg(feature = "typescript")]
fn typescript_writes_the_omitted_key_as_optional() {
    let ts = OmittedKeyFields::ts_definition();

    assert!(ts.contains("age?: number;"), "Got: {ts}");
    assert!(!ts.contains("age: number"), "Got: {ts}");
    assert!(ts.contains("id: string;"), "Got: {ts}");
    assert!(ts.contains("roles: Array<string>;"), "Got: {ts}");
}

/// The Zod spelling already admits the payload without the key — a `z.strictObject` rejects an
/// unrecognized key, never a missing one whose schema accepts `undefined` — so it is left as it
/// stands, and this pins that it was not disturbed.
#[test]
#[cfg(feature = "zod")]
fn zod_keeps_the_undefined_union_that_already_admits_the_absent_key() {
    let zod = OmittedKeyFields::zod_schema();

    assert!(
        zod.contains("age: z.union([z.number().int(), z.undefined()]).prefault(undefined),"),
        "Got: {zod}"
    );
    assert!(zod.contains("id: z.string(),"), "Got: {zod}");
    assert!(zod.contains("roles: z.array(z.string()),"), "Got: {zod}");
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

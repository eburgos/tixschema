use serde::{Deserialize, Serialize};
use tixschema::model_schema;

#[model_schema()]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
enum AuthorKind {
    Agent,
    Human,
}

#[model_schema()]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum CopyKind {
    BodyOnly,
    Noop,
    Verbatim,
}

#[model_schema()]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum FindingKind {
    AgentKeyUnderUserSsh,
    /// Digits never introduce a word break.
    Base64Payload,
    /// An acronym run gets one underscore per capital, not one per run.
    HttpSSHProxy,
}

#[model_schema()]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
enum VerifyErrorKind {
    ContentMismatch,
    #[serde(rename = "verify_failed")]
    VerifyFailed,
}

#[model_schema()]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum SandboxKind {
    #[serde(rename = "AS-IS")]
    AsIs,
    PlainAdd,
}

/// The string serde itself writes for a unit variant.
fn serde_wire_name<T>(variant: &T) -> String
where
    T: Serialize,
{
    serde_json::to_value(variant)
        .unwrap()
        .as_str()
        .unwrap()
        .to_owned()
}

fn serde_wire_names<T>(variants: &[T]) -> Vec<String>
where
    T: Serialize,
{
    variants.iter().map(serde_wire_name).collect()
}

#[test]
fn lowercase_variants_match_serde_wire() {
    assert_eq!(AuthorKind::enum_members(), ["agent", "human"]);
    assert_eq!(
        AuthorKind::enum_members(),
        serde_wire_names(&[AuthorKind::Agent, AuthorKind::Human])
    );
}

#[test]
fn snake_case_variants_match_serde_wire() {
    assert_eq!(CopyKind::enum_members(), ["body_only", "noop", "verbatim"]);
    assert_eq!(
        CopyKind::enum_members(),
        serde_wire_names(&[CopyKind::BodyOnly, CopyKind::Noop, CopyKind::Verbatim])
    );
}

#[test]
fn snake_case_acronyms_and_digits_match_serde_wire() {
    assert_eq!(
        FindingKind::enum_members(),
        [
            "agent_key_under_user_ssh",
            "base64_payload",
            "http_s_s_h_proxy"
        ]
    );
    assert_eq!(
        FindingKind::enum_members(),
        serde_wire_names(&[
            FindingKind::AgentKeyUnderUserSsh,
            FindingKind::Base64Payload,
            FindingKind::HttpSSHProxy,
        ])
    );
}

#[test]
fn per_variant_rename_matches_serde_wire() {
    assert_eq!(
        VerifyErrorKind::enum_members(),
        ["ContentMismatch", "verify_failed"]
    );
    assert_eq!(
        VerifyErrorKind::enum_members(),
        serde_wire_names(&[
            VerifyErrorKind::ContentMismatch,
            VerifyErrorKind::VerifyFailed,
        ])
    );
}

#[test]
fn per_variant_rename_overrides_rename_all() {
    assert_eq!(SandboxKind::enum_members(), ["AS-IS", "plain_add"]);
    assert_eq!(
        SandboxKind::enum_members(),
        serde_wire_names(&[SandboxKind::AsIs, SandboxKind::PlainAdd])
    );
}

#[cfg(feature = "typescript")]
#[test]
fn snake_case_ts_union_matches_serde_wire() {
    let ts = CopyKind::ts_definition();
    assert!(ts.contains("\"body_only\""), "Got:\n{ts}");
    assert!(ts.contains("\"noop\""), "Got:\n{ts}");
    assert!(ts.contains("\"verbatim\""), "Got:\n{ts}");
    assert!(!ts.contains("\"BodyOnly\""), "Got:\n{ts}");
}

#[cfg(feature = "zod")]
#[test]
fn snake_case_zod_enum_matches_serde_wire() {
    let zod = CopyKind::zod_schema();
    assert!(
        zod.contains("z.enum([\"body_only\", \"noop\", \"verbatim\"])"),
        "Got:\n{zod}"
    );
}

#[cfg(feature = "typescript")]
#[test]
fn lowercase_ts_union_unchanged() {
    let ts = AuthorKind::ts_definition();
    assert!(ts.contains("\"agent\""), "Got:\n{ts}");
    assert!(ts.contains("\"human\""), "Got:\n{ts}");
}

#[cfg(feature = "zod")]
#[test]
fn lowercase_zod_enum_unchanged() {
    let zod = AuthorKind::zod_schema();
    assert!(
        zod.contains("z.enum([\"agent\", \"human\"])"),
        "Got:\n{zod}"
    );
}

#[cfg(feature = "jsonschema")]
#[test]
fn snake_case_json_schema_matches_serde_wire() {
    let schema = CopyKind::json_schema();
    let values = schema["enum"].as_array().unwrap();
    assert_eq!(
        values.iter().map(ToString::to_string).collect::<Vec<_>>(),
        ["\"body_only\"", "\"noop\"", "\"verbatim\""]
    );
}

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

// One struct-variant enum per tagged representation, each carrying an enum-level `rename_all` and
// two variants: `Unmarked` has no rename of its own, so its field must stay as declared — the
// container-level rule cases the discriminator alone, never a variant's fields. `Marked` carries
// its own `rename_all`, which serde treats as the container for its own fields and does apply.
#[model_schema()]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum InternallyTaggedRename {
    #[serde(rename_all = "kebab-case")]
    Marked {
        field_two: String,
    },
    Unmarked {
        field_one: String,
    },
}

#[model_schema()]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
enum ExternallyTaggedRename {
    #[serde(rename_all = "kebab-case")]
    Marked {
        field_two: String,
    },
    Unmarked {
        field_one: String,
    },
}

#[model_schema()]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", content = "data", rename_all = "camelCase")]
enum AdjacentlyTaggedRename {
    #[serde(rename_all = "kebab-case")]
    Marked {
        field_two: String,
    },
    Unmarked {
        field_one: String,
    },
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

// Struct-variant field renaming under each of serde's three tagged representations: the
// container-level `rename_all` must not reach a variant's fields, and a variant's own
// `rename_all` must.

#[test]
fn internally_tagged_rename_all_reaches_the_wire_correctly() {
    let unmarked = InternallyTaggedRename::Unmarked {
        field_one: "a".to_owned(),
    };
    let unmarked_json = serde_json::to_value(&unmarked).unwrap();
    assert_eq!(
        unmarked_json,
        serde_json::json!({ "kind": "unmarked", "field_one": "a" })
    );
    assert_eq!(
        serde_json::from_value::<InternallyTaggedRename>(unmarked_json).unwrap(),
        unmarked
    );

    let marked = InternallyTaggedRename::Marked {
        field_two: "b".to_owned(),
    };
    let marked_json = serde_json::to_value(&marked).unwrap();
    assert_eq!(
        marked_json,
        serde_json::json!({ "kind": "marked", "field-two": "b" })
    );
    assert_eq!(
        serde_json::from_value::<InternallyTaggedRename>(marked_json).unwrap(),
        marked
    );
}

#[test]
fn externally_tagged_rename_all_reaches_the_wire_correctly() {
    let unmarked = ExternallyTaggedRename::Unmarked {
        field_one: "a".to_owned(),
    };
    let unmarked_json = serde_json::to_value(&unmarked).unwrap();
    assert_eq!(
        unmarked_json,
        serde_json::json!({ "unmarked": { "field_one": "a" } })
    );
    assert_eq!(
        serde_json::from_value::<ExternallyTaggedRename>(unmarked_json).unwrap(),
        unmarked
    );

    let marked = ExternallyTaggedRename::Marked {
        field_two: "b".to_owned(),
    };
    let marked_json = serde_json::to_value(&marked).unwrap();
    assert_eq!(
        marked_json,
        serde_json::json!({ "marked": { "field-two": "b" } })
    );
    assert_eq!(
        serde_json::from_value::<ExternallyTaggedRename>(marked_json).unwrap(),
        marked
    );
}

#[test]
fn adjacently_tagged_rename_all_reaches_the_wire_correctly() {
    let unmarked = AdjacentlyTaggedRename::Unmarked {
        field_one: "a".to_owned(),
    };
    let unmarked_json = serde_json::to_value(&unmarked).unwrap();
    assert_eq!(
        unmarked_json,
        serde_json::json!({ "kind": "unmarked", "data": { "field_one": "a" } })
    );
    assert_eq!(
        serde_json::from_value::<AdjacentlyTaggedRename>(unmarked_json).unwrap(),
        unmarked
    );

    let marked = AdjacentlyTaggedRename::Marked {
        field_two: "b".to_owned(),
    };
    let marked_json = serde_json::to_value(&marked).unwrap();
    assert_eq!(
        marked_json,
        serde_json::json!({ "kind": "marked", "data": { "field-two": "b" } })
    );
    assert_eq!(
        serde_json::from_value::<AdjacentlyTaggedRename>(marked_json).unwrap(),
        marked
    );
}

#[cfg(feature = "typescript")]
#[test]
fn internally_tagged_rename_all_typescript_matches_serde_wire() {
    let ts = InternallyTaggedRename::ts_definition();
    assert!(ts.contains("kind: \"unmarked\""), "Got:\n{ts}");
    assert!(ts.contains("field_one: string"), "Got:\n{ts}");
    assert!(ts.contains("kind: \"marked\""), "Got:\n{ts}");
    assert!(ts.contains("\"field-two\": string"), "Got:\n{ts}");
}

#[cfg(feature = "typescript")]
#[test]
fn externally_tagged_rename_all_typescript_matches_serde_wire() {
    let ts = ExternallyTaggedRename::ts_definition();
    assert!(ts.contains("\"unmarked\": {"), "Got:\n{ts}");
    assert!(ts.contains("field_one: string"), "Got:\n{ts}");
    assert!(ts.contains("\"marked\": {"), "Got:\n{ts}");
    assert!(ts.contains("\"field-two\": string"), "Got:\n{ts}");
}

#[cfg(feature = "typescript")]
#[test]
fn adjacently_tagged_rename_all_typescript_matches_serde_wire() {
    let ts = AdjacentlyTaggedRename::ts_definition();
    assert!(ts.contains("kind: \"unmarked\""), "Got:\n{ts}");
    assert!(ts.contains("field_one: string"), "Got:\n{ts}");
    assert!(ts.contains("kind: \"marked\""), "Got:\n{ts}");
    assert!(ts.contains("\"field-two\": string"), "Got:\n{ts}");
}

#[cfg(feature = "zod")]
#[test]
fn internally_tagged_rename_all_zod_matches_serde_wire() {
    let zod = InternallyTaggedRename::zod_schema();
    assert!(zod.contains("kind: z.literal(\"unmarked\")"), "Got:\n{zod}");
    assert!(zod.contains("field_one: z.string()"), "Got:\n{zod}");
    assert!(zod.contains("kind: z.literal(\"marked\")"), "Got:\n{zod}");
    assert!(zod.contains("\"field-two\": z.string()"), "Got:\n{zod}");
}

#[cfg(feature = "zod")]
#[test]
fn externally_tagged_rename_all_zod_matches_serde_wire() {
    let zod = ExternallyTaggedRename::zod_schema();
    assert!(
        zod.contains("\"unmarked\": z.strictObject({"),
        "Got:\n{zod}"
    );
    assert!(zod.contains("field_one: z.string()"), "Got:\n{zod}");
    assert!(zod.contains("\"marked\": z.strictObject({"), "Got:\n{zod}");
    assert!(zod.contains("\"field-two\": z.string()"), "Got:\n{zod}");
}

#[cfg(feature = "zod")]
#[test]
fn adjacently_tagged_rename_all_zod_matches_serde_wire() {
    let zod = AdjacentlyTaggedRename::zod_schema();
    assert!(zod.contains("kind: z.literal(\"unmarked\")"), "Got:\n{zod}");
    assert!(zod.contains("field_one: z.string()"), "Got:\n{zod}");
    assert!(zod.contains("kind: z.literal(\"marked\")"), "Got:\n{zod}");
    assert!(zod.contains("\"field-two\": z.string()"), "Got:\n{zod}");
}

/// A closed branch (`additionalProperties: false`) describes exactly the keys serde wrote for
/// `value`: every key on the wire is named in `properties`, and every `required` key is on the
/// wire.
#[cfg(feature = "jsonschema")]
fn assert_branch_accepts<T>(branch: &serde_json::Value, value: &T)
where
    T: serde::Serialize,
{
    let payload = serde_json::to_value(value).unwrap();
    let named = branch["properties"].as_object().unwrap();
    let required = branch["required"].as_array().unwrap();
    let written = payload.as_object().unwrap();
    assert!(
        written.keys().all(|key| named.contains_key(key)),
        "schema does not name a key serde wrote:\nwire: {payload}\nbranch: {branch}"
    );
    assert!(
        required
            .iter()
            .all(|key| written.contains_key(key.as_str().unwrap())),
        "schema requires a key serde did not write:\nwire: {payload}\nbranch: {branch}"
    );
}

#[cfg(feature = "jsonschema")]
#[test]
fn internally_tagged_rename_all_json_schema_matches_serde_wire() {
    let schema = InternallyTaggedRename::json_schema();
    let one_of = schema["oneOf"].as_array().unwrap();

    let unmarked = InternallyTaggedRename::Unmarked {
        field_one: "a".to_owned(),
    };
    let unmarked_branch = one_of
        .iter()
        .find(|branch| branch["properties"]["kind"]["const"] == "unmarked")
        .unwrap();
    assert_branch_accepts(unmarked_branch, &unmarked);

    let marked = InternallyTaggedRename::Marked {
        field_two: "b".to_owned(),
    };
    let marked_branch = one_of
        .iter()
        .find(|branch| branch["properties"]["kind"]["const"] == "marked")
        .unwrap();
    assert_branch_accepts(marked_branch, &marked);
}

#[cfg(feature = "jsonschema")]
#[test]
fn externally_tagged_rename_all_json_schema_matches_serde_wire() {
    let schema = ExternallyTaggedRename::json_schema();
    let one_of = schema["oneOf"].as_array().unwrap();

    let unmarked = ExternallyTaggedRename::Unmarked {
        field_one: "a".to_owned(),
    };
    let unmarked_branch = one_of
        .iter()
        .find(|branch| {
            branch["properties"]
                .as_object()
                .unwrap()
                .contains_key("unmarked")
        })
        .unwrap();
    assert_branch_accepts(unmarked_branch, &unmarked);
    assert!(
        unmarked_branch["properties"]["unmarked"]["properties"]
            .as_object()
            .unwrap()
            .contains_key("field_one")
    );

    let marked = ExternallyTaggedRename::Marked {
        field_two: "b".to_owned(),
    };
    let marked_branch = one_of
        .iter()
        .find(|branch| {
            branch["properties"]
                .as_object()
                .unwrap()
                .contains_key("marked")
        })
        .unwrap();
    assert_branch_accepts(marked_branch, &marked);
    assert!(
        marked_branch["properties"]["marked"]["properties"]
            .as_object()
            .unwrap()
            .contains_key("field-two")
    );
}

// The adjacently-tagged branch below asserts against `properties` directly rather than a
// `data`-nested object: the content-key nesting the wire actually uses is a separate,
// pre-existing gap the JSON Schema surface does not render yet. This test only pins the seam
// this bug is about -- that the field key itself is untouched by the enum's own `rename_all`.
#[cfg(feature = "jsonschema")]
#[test]
fn adjacently_tagged_rename_all_json_schema_matches_serde_wire() {
    let schema = AdjacentlyTaggedRename::json_schema();
    let one_of = schema["oneOf"].as_array().unwrap();
    let unmarked_branch = one_of
        .iter()
        .find(|branch| branch["properties"]["kind"]["const"] == "unmarked")
        .unwrap();
    assert!(
        unmarked_branch["properties"]
            .as_object()
            .unwrap()
            .contains_key("field_one")
    );
    let marked_branch = one_of
        .iter()
        .find(|branch| branch["properties"]["kind"]["const"] == "marked")
        .unwrap();
    assert!(
        marked_branch["properties"]
            .as_object()
            .unwrap()
            .contains_key("field-two")
    );
}

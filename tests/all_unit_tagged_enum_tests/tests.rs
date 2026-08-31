use serde::{Deserialize, Serialize};
use serde_json::Value;
use tixschema::model_schema;

/// A tag with a content key beside it. Serde writes no content key for a unit variant, so the
/// object carries the tag alone here exactly as the internally tagged form does.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "kebab-case", tag = "kind", content = "detail")]
enum AdjacentBalanceError {
    DbError,
    InsufficientBalance,
}

/// The canonical error type of a service: every variant a unit, the whole enum tagged by the key
/// callers narrow on.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "kebab-case", tag = "errorCode")]
enum BalanceError {
    DbError,
    InsufficientBalance,
}

/// The same variants with no tagging key named. Serde writes the bare variant name for this one,
/// which is the string union that stays a string union.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum ExternalBalanceError {
    DbError,
    InsufficientBalance,
}

/// The one object serde writes for a value, read back as its key/value pairs. Every expectation
/// below is spelled from this rather than from what the tagging attribute is expected to mean.
fn wire_object<T>(value: &T) -> serde_json::Map<String, Value>
where
    T: Serialize,
{
    serde_json::to_value(value)
        .unwrap()
        .as_object()
        .unwrap()
        .clone()
}

/// The tag key and the tag value of a wire object carrying nothing but its tag.
fn wire_tag<T>(value: &T) -> (String, String)
where
    T: Serialize,
{
    let object = wire_object(value);
    assert_eq!(object.len(), 1, "a unit variant writes the tag alone");
    let (key, tag) = object.iter().next().unwrap();
    (key.clone(), tag.as_str().unwrap().to_owned())
}

/// A published TypeScript definition with its `JSDoc` lines and blank lines dropped, leaving the
/// type itself to be held against the wire.
#[cfg(feature = "typescript")]
fn ts_shape(definition: &str) -> String {
    definition
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty() && !trimmed.starts_with("/**") && !trimmed.starts_with('*')
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn an_all_unit_internally_tagged_enum_is_written_as_a_tagged_object() {
    assert_eq!(
        serde_json::to_string(&BalanceError::DbError).unwrap(),
        r#"{"errorCode":"db-error"}"#
    );
    assert_eq!(
        serde_json::to_string(&BalanceError::InsufficientBalance).unwrap(),
        r#"{"errorCode":"insufficient-balance"}"#
    );
    assert_eq!(
        wire_tag(&BalanceError::DbError),
        ("errorCode".to_owned(), "db-error".to_owned())
    );
}

#[test]
fn an_all_unit_adjacently_tagged_enum_is_written_as_a_tagged_object() {
    assert_eq!(
        serde_json::to_string(&AdjacentBalanceError::DbError).unwrap(),
        r#"{"kind":"db-error"}"#
    );
    assert_eq!(
        serde_json::to_string(&AdjacentBalanceError::InsufficientBalance).unwrap(),
        r#"{"kind":"insufficient-balance"}"#
    );
    assert_eq!(
        wire_tag(&AdjacentBalanceError::DbError),
        ("kind".to_owned(), "db-error".to_owned())
    );
}

#[test]
fn an_all_unit_enum_naming_no_tag_is_written_as_the_bare_variant_name() {
    assert_eq!(
        serde_json::to_string(&ExternalBalanceError::DbError).unwrap(),
        r#""db-error""#
    );
}

#[cfg(feature = "typescript")]
#[test]
fn the_typescript_union_carries_the_tag_serde_writes() {
    let (tag, db) = wire_tag(&BalanceError::DbError);
    let (_, insufficient) = wire_tag(&BalanceError::InsufficientBalance);

    assert_eq!(
        ts_shape(&BalanceError::ts_definition()),
        format!(
            "export type BalanceError = {{\n  {tag}: \"{db}\";\n}} | {{\n  {tag}: \"{insufficient}\";\n}};"
        )
    );
}

#[cfg(feature = "typescript")]
#[test]
fn the_typescript_union_carries_an_adjacent_tag_too() {
    let (tag, db) = wire_tag(&AdjacentBalanceError::DbError);
    let (_, insufficient) = wire_tag(&AdjacentBalanceError::InsufficientBalance);

    assert_eq!(
        ts_shape(&AdjacentBalanceError::ts_definition()),
        format!(
            "export type AdjacentBalanceError = {{\n  {tag}: \"{db}\";\n}} | {{\n  {tag}: \"{insufficient}\";\n}};"
        )
    );
}

#[cfg(feature = "typescript")]
#[test]
fn an_all_unit_enum_naming_no_tag_still_publishes_the_string_union() {
    assert_eq!(
        ts_shape(&ExternalBalanceError::ts_definition()),
        "export type ExternalBalanceError =\n  | \"db-error\"\n  | \"insufficient-balance\";"
    );
}

#[cfg(feature = "zod")]
#[test]
fn the_zod_schema_discriminates_on_the_tag_serde_writes() {
    let (tag, db) = wire_tag(&BalanceError::DbError);
    let (_, insufficient) = wire_tag(&BalanceError::InsufficientBalance);
    let schema = BalanceError::zod_schema();

    assert!(schema.contains(&format!("z.discriminatedUnion(\"{tag}\", [")));
    assert!(schema.contains(&format!("{tag}: z.literal(\"{db}\"),")));
    assert!(schema.contains(&format!("{tag}: z.literal(\"{insufficient}\"),")));
    assert!(
        !schema.contains("z.enum("),
        "a tagged enum is an object union, not an enumeration of strings"
    );
}

#[cfg(feature = "zod")]
#[test]
fn an_all_unit_enum_naming_no_tag_still_publishes_a_zod_enum() {
    assert!(
        ExternalBalanceError::zod_schema()
            .contains("z.enum([\"db-error\", \"insufficient-balance\"])")
    );
}

#[cfg(feature = "jsonschema")]
#[test]
fn the_json_schema_describes_the_bytes_serde_writes() {
    let schema = BalanceError::json_schema();
    assert_eq!(schema["type"], "object");

    let branches = schema["oneOf"].as_array().unwrap();
    assert_eq!(branches.len(), 2);

    for value in [BalanceError::DbError, BalanceError::InsufficientBalance] {
        let wire = wire_object(&value);
        let (tag, _) = wire_tag(&value);
        let described = branches
            .iter()
            .filter(|branch| branch["properties"][&tag]["const"] == wire[&tag])
            .count();
        assert_eq!(described, 1, "exactly one branch describes {wire:?}");

        let branch = branches
            .iter()
            .find(|candidate| candidate["properties"][&tag]["const"] == wire[&tag])
            .unwrap();
        assert_eq!(
            branch["required"].as_array().unwrap().len(),
            wire.len(),
            "the branch requires exactly the keys the wire carries"
        );
        assert_eq!(branch["required"][0], tag);
        assert_eq!(branch["additionalProperties"], false);
    }
}

#[cfg(feature = "jsonschema")]
#[test]
fn an_all_unit_enum_naming_no_tag_still_publishes_a_string_schema() {
    let schema = ExternalBalanceError::json_schema();
    assert_eq!(schema["type"], "string");
    assert_eq!(schema["enum"][0], "db-error");
}

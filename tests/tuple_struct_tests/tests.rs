use serde::{Deserialize, Serialize};
use tixschema::model_schema;

/// A newtype struct: serde writes the slot's value alone, with nothing around it.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Plain(pub String);

/// A wider tuple struct: serde writes a fixed-arity array.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Pair(pub String, pub u32);

/// A slot cannot be omitted the way an object key can, so a `None` here reaches the wire as
/// `null`.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct MaybeName(pub Option<String>);

/// The wire criterion the newtype surfaces are read against.
#[test]
fn test_serde_writes_a_newtype_struct_as_the_inner_value_alone() {
    assert_eq!(
        serde_json::to_value(Plain("hello".to_owned())).unwrap(),
        serde_json::json!("hello")
    );
}

/// The wire criterion the wider surfaces are read against.
#[test]
fn test_serde_writes_a_wider_tuple_struct_as_a_fixed_array() {
    assert_eq!(
        serde_json::to_value(Pair("hello".to_owned(), 1_u32)).unwrap(),
        serde_json::json!(["hello", 1_u32])
    );
}

/// The wire criterion the nullable surfaces are read against: the slot is always written, so a
/// `None` arrives as `null` rather than as an absent key.
#[test]
fn test_serde_writes_a_none_slot_as_null() {
    assert_eq!(
        serde_json::to_value(MaybeName(None)).unwrap(),
        serde_json::json!(null)
    );
    assert_eq!(
        serde_json::to_value(MaybeName(Some("ada".to_owned()))).unwrap(),
        serde_json::json!("ada")
    );
}

#[cfg(feature = "typescript")]
#[test]
fn test_a_newtype_struct_describes_the_bare_inner_type_in_typescript() {
    let wire = serde_json::to_value(Plain("hello".to_owned())).unwrap();
    assert!(wire.is_string(), "wire: {wire}");

    let ts = Plain::ts_definition();
    assert!(ts.contains("export type Plain = string;"), "Got: {ts}");
    assert!(!ts.contains(": string;"), "Got: {ts}");
}

#[cfg(feature = "zod")]
#[test]
fn test_a_newtype_struct_describes_the_bare_inner_type_in_zod() {
    let wire = serde_json::to_value(Plain("hello".to_owned())).unwrap();
    assert!(wire.is_string(), "wire: {wire}");

    let zod = Plain::zod_schema();
    assert!(zod.contains("Plain$Schema"), "Got: {zod}");
    assert!(zod.contains("= z.string();"), "Got: {zod}");
    assert!(!zod.contains("{ : "), "Got: {zod}");
    assert!(!zod.contains("strictObject"), "Got: {zod}");
}

#[cfg(feature = "jsonschema")]
#[test]
fn test_a_newtype_struct_describes_the_bare_inner_type_in_json_schema() {
    let wire = serde_json::to_value(Plain("hello".to_owned())).unwrap();
    assert!(wire.is_string(), "wire: {wire}");

    assert_eq!(
        Plain::json_schema(),
        serde_json::json!({ "type": "string" })
    );
}

#[cfg(feature = "typescript")]
#[test]
fn test_a_wider_tuple_struct_describes_a_fixed_tuple_in_typescript() {
    let wire = serde_json::to_value(Pair("hello".to_owned(), 1_u32)).unwrap();
    assert!(wire.is_array(), "wire: {wire}");

    let ts = Pair::ts_definition();
    assert!(
        ts.contains("export type Pair = [string, number];"),
        "Got: {ts}"
    );
    assert!(!ts.contains(": string;"), "Got: {ts}");
}

#[cfg(feature = "zod")]
#[test]
fn test_a_wider_tuple_struct_describes_a_fixed_tuple_in_zod() {
    let wire = serde_json::to_value(Pair("hello".to_owned(), 1_u32)).unwrap();
    assert!(wire.is_array(), "wire: {wire}");

    let zod = Pair::zod_schema();
    assert!(zod.contains("Pair$Schema"), "Got: {zod}");
    assert!(
        zod.contains("= z.tuple([z.string(), z.number().int()]);"),
        "Got: {zod}"
    );
    assert!(!zod.contains("{ : "), "Got: {zod}");
    assert!(!zod.contains("strictObject"), "Got: {zod}");
}

/// The `ZodType<...> = ...$RawSchema` framing only appears when typescript is also enabled.
#[cfg(all(feature = "zod", feature = "typescript"))]
#[test]
fn test_a_tuple_struct_zod_schema_is_annotated_with_its_typescript_type() {
    let zod = Pair::zod_schema();
    assert!(
        zod.contains("export const Pair$Schema: ZodType<Pair> = Pair$RawSchema;"),
        "Got: {zod}"
    );
}

#[cfg(feature = "jsonschema")]
#[test]
fn test_a_wider_tuple_struct_describes_a_fixed_tuple_in_json_schema() {
    let wire = serde_json::to_value(Pair("hello".to_owned(), 1_u32)).unwrap();
    assert!(wire.is_array(), "wire: {wire}");

    assert_eq!(
        Pair::json_schema(),
        serde_json::json!({
            "type": "array",
            "prefixItems": [{ "type": "string" }, { "type": "integer" }],
            "items": false,
            "minItems": 2_u32,
            "maxItems": 2_u32
        })
    );
}

#[cfg(feature = "typescript")]
#[test]
fn test_an_optional_slot_describes_the_null_it_writes_in_typescript() {
    let wire = serde_json::to_value(MaybeName(None)).unwrap();
    assert!(wire.is_null(), "wire: {wire}");

    let ts = MaybeName::ts_definition();
    assert!(
        ts.contains("export type MaybeName = string | null;"),
        "Got: {ts}"
    );
}

#[cfg(feature = "zod")]
#[test]
fn test_an_optional_slot_describes_the_null_it_writes_in_zod() {
    let wire = serde_json::to_value(MaybeName(None)).unwrap();
    assert!(wire.is_null(), "wire: {wire}");

    let zod = MaybeName::zod_schema();
    assert!(zod.contains("MaybeName$Schema"), "Got: {zod}");
    assert!(zod.contains("= z.nullable(z.string());"), "Got: {zod}");
}

#[cfg(feature = "jsonschema")]
#[test]
fn test_an_optional_slot_describes_the_null_it_writes_in_json_schema() {
    let wire = serde_json::to_value(MaybeName(None)).unwrap();
    assert!(wire.is_null(), "wire: {wire}");

    assert_eq!(
        MaybeName::json_schema(),
        serde_json::json!({ "anyOf": [{ "type": "string" }, { "type": "null" }] })
    );
}

/// The empty ident of an unnamed slot must not reach any surface as a key.
#[cfg(feature = "typescript")]
#[test]
fn test_typescript_never_names_a_slot_by_its_absent_ident() {
    for ts in [
        Plain::ts_definition(),
        Pair::ts_definition(),
        MaybeName::ts_definition(),
    ] {
        assert!(!ts.contains(": string;"), "Got: {ts}");
        assert!(!ts.contains("  : "), "Got: {ts}");
    }
}

/// [`test_typescript_never_names_a_slot_by_its_absent_ident`] for the Zod surface.
#[cfg(feature = "zod")]
#[test]
fn test_zod_never_names_a_slot_by_its_absent_ident() {
    for zod in [
        Plain::zod_schema(),
        Pair::zod_schema(),
        MaybeName::zod_schema(),
    ] {
        assert!(!zod.contains("{ : "), "Got: {zod}");
        assert!(!zod.contains("  : "), "Got: {zod}");
    }
}

/// [`test_typescript_never_names_a_slot_by_its_absent_ident`] for the JSON-schema surface, where
/// the empty ident used to arrive as a property literally named `""`.
#[cfg(feature = "jsonschema")]
#[test]
fn test_json_schema_never_names_a_slot_by_its_absent_ident() {
    for json in [
        Plain::json_schema(),
        Pair::json_schema(),
        MaybeName::json_schema(),
    ] {
        assert!(!json.to_string().contains("\"\""), "Got: {json}");
    }
}

use serde::{Deserialize, Serialize};
use tixschema::model_schema;

// Each shape names itself so the JSON document hoists it into `$defs`, that key being the one
// place the JSON surface writes a type's own name.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug)]
pub struct SomethingData {
    pub children: Vec<Self>,
    pub label: String,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug)]
pub struct SomethingJson {
    pub children: Vec<Self>,
    pub label: String,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug)]
pub struct Data {
    pub children: Vec<Self>,
    pub label: String,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug)]
pub struct NamesEverySpelling {
    pub bare: Data,
    pub data_spelled: SomethingData,
    pub json_spelled: SomethingJson,
}

#[model_schema(name = "PublishedGauge")]
#[derive(Serialize, Deserialize, Debug)]
pub struct GaugeUnderRustName {
    pub nested: Vec<Self>,
    pub reading: u32,
}

#[model_schema(name = "PublishedGrade")]
#[derive(Serialize, Deserialize, Debug)]
pub enum GradeUnderRustName {
    Group { members: Vec<Self> },
    High,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug)]
pub struct NamesOverriddenItems {
    pub gauge: GaugeUnderRustName,
    pub grade: GradeUnderRustName,
}

// A member key reaches the same seam a type name does, and is spelled from what serde writes.
#[cfg(feature = "serde")]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug)]
pub struct KeyEndingInTheSuffix {
    #[serde(rename = "payloadData")]
    pub payload: String,
}

// An untagged member naming the union itself is read through a thunk rather than written into the
// `const` that declares the name, and the thunk has to reach the union under the name it published.
#[cfg(all(feature = "serde", feature = "zod"))]
#[model_schema(name = "PublishedBranch")]
#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum BranchUnderRustName {
    Nested(Vec<Self>),
    Tip(String),
}

#[cfg(feature = "typescript")]
#[test]
fn typescript_is_published_under_the_whole_ident() {
    for (ts, declared) in [
        (
            SomethingData::ts_definition(),
            "export type SomethingData = {",
        ),
        (
            SomethingJson::ts_definition(),
            "export type SomethingJson = {",
        ),
        (Data::ts_definition(), "export type Data = {"),
    ] {
        assert!(ts.contains(declared), "{declared} missing from: {ts}");
    }
}

#[cfg(feature = "zod")]
#[test]
fn zod_consts_are_published_under_the_whole_ident() {
    for (zod, declared) in [
        (
            SomethingData::zod_schema(),
            "export const SomethingData$Schema",
        ),
        (
            SomethingJson::zod_schema(),
            "export const SomethingJson$Schema",
        ),
        (Data::zod_schema(), "export const Data$Schema"),
    ] {
        assert!(zod.contains(declared), "{declared} missing from: {zod}");
    }
}

#[cfg(feature = "jsonschema")]
#[test]
fn the_json_definition_is_keyed_by_the_whole_ident() {
    for (schema, key) in [
        (SomethingData::json_schema(), "SomethingData"),
        (SomethingJson::json_schema(), "SomethingJson"),
        (Data::json_schema(), "Data"),
    ] {
        assert_eq!(
            schema["$ref"],
            serde_json::json!(format!("#/$defs/{key}")),
            "got: {schema}"
        );
    }
}

/// Nothing is read off the spelling of a name: the shortened one a taken suffix would have left
/// is written by no surface, at the declaration or at a reference to it.
#[cfg(any(feature = "typescript", feature = "zod"))]
#[test]
fn no_surface_writes_the_name_a_taken_suffix_would_leave() {
    #[cfg(feature = "typescript")]
    for ts in [
        SomethingData::ts_definition(),
        NamesEverySpelling::ts_definition(),
    ] {
        assert!(!ts.contains("export type Something ="), "got: {ts}");
        assert!(!ts.contains(": Something;"), "got: {ts}");
    }
    #[cfg(feature = "zod")]
    for zod in [
        SomethingData::zod_schema(),
        NamesEverySpelling::zod_schema(),
    ] {
        assert!(!zod.contains("Something$Schema"), "got: {zod}");
    }
}

#[cfg(feature = "typescript")]
#[test]
fn a_typescript_reference_names_what_the_referenced_item_publishes() {
    let ts = NamesEverySpelling::ts_definition();
    for referenced in [
        "data_spelled: SomethingData;",
        "json_spelled: SomethingJson;",
        "bare: Data;",
    ] {
        assert!(ts.contains(referenced), "{referenced} missing from: {ts}");
    }
}

#[cfg(feature = "zod")]
#[test]
fn a_zod_reference_names_what_the_referenced_item_publishes() {
    let zod = NamesEverySpelling::zod_schema();
    for referenced in [
        "data_spelled: SomethingData$Schema,",
        "json_spelled: SomethingJson$Schema,",
        "bare: Data$Schema,",
    ] {
        assert!(zod.contains(referenced), "{referenced} missing from: {zod}");
    }
}

#[cfg(feature = "jsonschema")]
#[test]
fn a_json_reference_points_at_the_definition_the_referenced_item_hoists() {
    let properties = &NamesEverySpelling::json_schema()["properties"];
    for (member, key) in [
        ("data_spelled", "SomethingData"),
        ("json_spelled", "SomethingJson"),
        ("bare", "Data"),
    ] {
        assert_eq!(
            properties[member],
            serde_json::json!({ "$ref": format!("#/$defs/{key}") })
        );
    }
}

#[cfg(feature = "typescript")]
#[test]
fn an_overridden_struct_and_enum_are_published_and_referenced_under_the_override() {
    assert!(
        GaugeUnderRustName::ts_definition().contains("export type PublishedGauge = {"),
        "got: {}",
        GaugeUnderRustName::ts_definition()
    );
    assert!(
        GradeUnderRustName::ts_definition().contains("export type PublishedGrade ="),
        "got: {}",
        GradeUnderRustName::ts_definition()
    );
    let ts = NamesOverriddenItems::ts_definition();
    for referenced in ["gauge: PublishedGauge;", "grade: PublishedGrade;"] {
        assert!(ts.contains(referenced), "{referenced} missing from: {ts}");
    }
}

#[cfg(feature = "zod")]
#[test]
fn an_overridden_struct_and_enum_publish_and_are_named_by_the_override_in_zod() {
    assert!(
        GaugeUnderRustName::zod_schema().contains("export const PublishedGauge$Schema"),
        "got: {}",
        GaugeUnderRustName::zod_schema()
    );
    assert!(
        GradeUnderRustName::zod_schema().contains("export const PublishedGrade$Schema"),
        "got: {}",
        GradeUnderRustName::zod_schema()
    );
    let zod = NamesOverriddenItems::zod_schema();
    for referenced in [
        "gauge: PublishedGauge$Schema,",
        "grade: PublishedGrade$Schema,",
    ] {
        assert!(zod.contains(referenced), "{referenced} missing from: {zod}");
    }
}

#[cfg(feature = "jsonschema")]
#[test]
fn an_overridden_struct_and_enum_key_their_json_definitions_by_the_override() {
    let properties = &NamesOverriddenItems::json_schema()["properties"];
    for (member, key) in [("gauge", "PublishedGauge"), ("grade", "PublishedGrade")] {
        assert_eq!(
            properties[member],
            serde_json::json!({ "$ref": format!("#/$defs/{key}") })
        );
    }
}

#[cfg(all(feature = "serde", feature = "zod"))]
#[test]
fn a_self_naming_untagged_member_defers_to_the_overridden_name() {
    let zod = BranchUnderRustName::zod_schema();
    assert!(
        zod.contains("z.union([z.lazy(() => z.array(PublishedBranch$Schema)), z.string()])"),
        "got: {zod}"
    );
}

/// A member key reaches the same seam a type name does, so it too is written exactly as serde
/// writes it.
#[cfg(all(feature = "serde", feature = "typescript"))]
#[test]
fn a_typescript_member_key_is_written_exactly_as_serde_writes_it() {
    let ts = KeyEndingInTheSuffix::ts_definition();
    assert!(ts.contains("payloadData: string;"), "got: {ts}");
}

#[cfg(all(feature = "serde", feature = "zod"))]
#[test]
fn a_zod_member_key_is_written_exactly_as_serde_writes_it() {
    let zod = KeyEndingInTheSuffix::zod_schema();
    assert!(zod.contains("payloadData: z.string(),"), "got: {zod}");
}

#[cfg(all(feature = "serde", feature = "jsonschema"))]
#[test]
fn a_json_member_key_is_written_exactly_as_serde_writes_it() {
    let schema = KeyEndingInTheSuffix::json_schema();
    assert_eq!(
        schema["properties"]["payloadData"],
        serde_json::json!({ "type": "string" }),
        "got: {schema}"
    );
}

/// The module an item publishes its schema in is named from the Rust ident, whole: a reference
/// standing above the declaration has nothing else to name it by, and an override is not
/// recoverable from that ident.
#[cfg(feature = "jsonschema")]
#[test]
fn the_schema_module_is_named_from_the_whole_ident() {
    assert_eq!(
        SomethingData::json_schema(),
        something_data_schema::Schema::json_schema()
    );
    assert_eq!(
        GaugeUnderRustName::json_schema(),
        gauge_under_rust_name_schema::Schema::json_schema()
    );
}

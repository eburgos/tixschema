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

// The bare suffix: stripping it would leave nothing to publish under.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug)]
pub struct Data {
    pub children: Vec<Self>,
    pub label: String,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug)]
pub struct SuffixHolder {
    pub bare: Data,
    pub kept: SomethingJson,
    pub stripped: SomethingData,
}

#[cfg(feature = "typescript")]
#[test]
fn typescript_is_published_under_the_name_the_suffix_leaves() {
    let stripped = SomethingData::ts_definition();
    assert!(
        stripped.contains("export type Something = {"),
        "got: {stripped}"
    );
    assert!(!stripped.contains("SomethingData"), "got: {stripped}");

    let kept = SomethingJson::ts_definition();
    assert!(
        kept.contains("export type SomethingJson = {"),
        "got: {kept}"
    );

    let bare = Data::ts_definition();
    assert!(bare.contains("export type Data = {"), "got: {bare}");
}

#[cfg(feature = "zod")]
#[test]
fn zod_consts_are_published_under_the_name_the_suffix_leaves() {
    let stripped = SomethingData::zod_schema();
    assert!(
        stripped.contains("export const Something$Schema"),
        "got: {stripped}"
    );
    assert!(!stripped.contains("SomethingData"), "got: {stripped}");

    let kept = SomethingJson::zod_schema();
    assert!(
        kept.contains("export const SomethingJson$Schema"),
        "got: {kept}"
    );

    let bare = Data::zod_schema();
    assert!(bare.contains("export const Data$Schema"), "got: {bare}");
}

#[cfg(feature = "jsonschema")]
#[test]
fn the_json_definition_is_keyed_by_the_name_the_suffix_leaves() {
    assert_eq!(
        SomethingData::json_schema()["$ref"],
        serde_json::json!("#/$defs/Something")
    );
    assert_eq!(
        SomethingJson::json_schema()["$ref"],
        serde_json::json!("#/$defs/SomethingJson")
    );
    assert_eq!(
        Data::json_schema()["$ref"],
        serde_json::json!("#/$defs/Data")
    );
}

#[cfg(feature = "typescript")]
#[test]
fn a_typescript_reference_names_what_the_referenced_item_publishes() {
    let ts = SuffixHolder::ts_definition();
    assert!(ts.contains("stripped: Something;"), "got: {ts}");
    assert!(ts.contains("kept: SomethingJson;"), "got: {ts}");
    assert!(ts.contains("bare: Data;"), "got: {ts}");
}

#[cfg(feature = "zod")]
#[test]
fn a_zod_reference_names_what_the_referenced_item_publishes() {
    let zod = SuffixHolder::zod_schema();
    assert!(zod.contains("stripped: Something$Schema,"), "got: {zod}");
    assert!(zod.contains("kept: SomethingJson$Schema,"), "got: {zod}");
    assert!(zod.contains("bare: Data$Schema,"), "got: {zod}");
}

#[cfg(feature = "jsonschema")]
#[test]
fn a_json_reference_points_at_the_definition_the_referenced_item_hoists() {
    let document = SuffixHolder::json_schema();
    let properties = &document["properties"];
    assert_eq!(
        properties["stripped"],
        serde_json::json!({ "$ref": "#/$defs/Something" })
    );
    assert_eq!(
        properties["kept"],
        serde_json::json!({ "$ref": "#/$defs/SomethingJson" })
    );
    assert_eq!(
        properties["bare"],
        serde_json::json!({ "$ref": "#/$defs/Data" })
    );
}

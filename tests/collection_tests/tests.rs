use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tixschema::model_schema;

// Test comprehensive HashMap scenarios with various value types
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct ComprehensiveHashMapTest {
    bool_array: HashMap<String, Vec<bool>>,
    bool_value: HashMap<String, bool>,
    f64_array: HashMap<String, Vec<f64>>,
    f64_value: HashMap<String, f64>,
    i64_array: HashMap<String, Vec<i64>>,
    i64_value: HashMap<String, i64>,
    optional_u64: HashMap<String, Option<u64>>,
    optional_u64_array: HashMap<String, Option<Vec<u64>>>,
    string_array: HashMap<String, Vec<String>>,
    string_value: HashMap<String, String>,
    u64_array: HashMap<String, Vec<u64>>,
    u64_value: HashMap<String, u64>,
}

// Test potential edge case with HashMap containing 64-bit integers
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct HashMapWith64Bit {
    i64_map: HashMap<String, i64>,
    id: String,
    mixed_map: HashMap<String, Vec<u64>>,
    u64_map: HashMap<String, u64>,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum MetricSlot {
    Daily,
    Weekly,
}

// An enum-keyed map enumerates its keys, so every member carries the value schema outright
// instead of the open `additionalProperties` a String-keyed map uses.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct EnumKeyedScalarValueMaps {
    bool_value: HashMap<MetricSlot, bool>,
    f64_value: HashMap<MetricSlot, f64>,
    i64_value: HashMap<MetricSlot, i64>,
    string_array: HashMap<MetricSlot, Vec<String>>,
    string_value: HashMap<MetricSlot, String>,
    u64_value: HashMap<MetricSlot, u64>,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct MetricSample {
    label: String,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct EnumKeyedSiblingValueMaps {
    sample_array: HashMap<MetricSlot, Vec<MetricSample>>,
    sample_value: HashMap<MetricSlot, MetricSample>,
}

// A String key enumerates nothing, so one `additionalProperties` schema stands for every member —
// and it is the value type's own, which is what the enum-keyed twin above spells out per key.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct StringKeyedSiblingValueMaps {
    optional_sample: HashMap<String, Option<MetricSample>>,
    sample_array: HashMap<String, Vec<MetricSample>>,
    sample_value: HashMap<String, MetricSample>,
}

/// An alias of a sibling: its schema module is named after the registered export name, not the
/// alias ident, so the member reference has to be resolved through the registry.
#[model_schema()]
type MetricSampleRef = MetricSample;

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct StringKeyedAliasValueMaps {
    sample_array: HashMap<String, Vec<MetricSampleRef>>,
    sample_value: HashMap<String, MetricSampleRef>,
}

// A map entry cannot be dropped the way an object key can, so an `Option` value is spelled `null`
// on the wire rather than omitted — the same twin type on both key paths pins that both agree.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct OptionalMapValues {
    enum_keyed: HashMap<MetricSlot, Option<String>>,
    string_keyed: HashMap<String, Option<String>>,
}

// Test struct with collections
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct UserWithCollections {
    id: String,
    metadata: HashMap<String, String>,
    scores: Vec<u32>,
    tags: Vec<String>,
}

#[cfg(feature = "jsonschema")]
fn assert_hashmap_primitive_type(
    properties: &serde_json::Map<String, serde_json::Value>,
    field_name: &str,
    expected_type: &str,
) {
    assert_eq!(properties[field_name]["type"], "object");
    assert_eq!(
        properties[field_name]["additionalProperties"]["type"],
        expected_type
    );
}

#[cfg(feature = "jsonschema")]
fn assert_hashmap_array_type(
    properties: &serde_json::Map<String, serde_json::Value>,
    field_name: &str,
    expected_item_type: &str,
) {
    assert_eq!(properties[field_name]["type"], "object");
    assert_eq!(
        properties[field_name]["additionalProperties"]["type"],
        "array"
    );
    assert_eq!(
        properties[field_name]["additionalProperties"]["items"]["type"],
        expected_item_type
    );
}

/// The JSON Schema `type` a value carries on the wire, so an emitted schema can be held against
/// what serde actually produces for it.
#[cfg(feature = "jsonschema")]
const fn json_type_name(value: &serde_json::Value) -> &'static str {
    match *value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

#[cfg(feature = "jsonschema")]
fn assert_enum_keyed_map_value(
    properties: &serde_json::Map<String, serde_json::Value>,
    field_name: &str,
    expected_value_schema: &serde_json::Value,
) {
    let field = &properties[field_name];
    assert_eq!(field["type"], "object", "in: {field}");
    assert_eq!(field["additionalProperties"], false, "in: {field}");
    let members = MetricSlot::enum_members();
    assert_eq!(
        field["properties"].as_object().unwrap().len(),
        members.len(),
        "in: {field}"
    );
    for member in members {
        assert_eq!(
            &field["properties"][&member], expected_value_schema,
            "member {member} in: {field}"
        );
    }
}

#[test]
fn test_collection_structs_constructible() {
    let comprehensive = ComprehensiveHashMapTest {
        bool_array: HashMap::new(),
        bool_value: HashMap::new(),
        f64_array: HashMap::new(),
        f64_value: HashMap::new(),
        i64_array: HashMap::new(),
        i64_value: HashMap::new(),
        optional_u64: HashMap::new(),
        optional_u64_array: HashMap::new(),
        string_array: HashMap::new(),
        string_value: HashMap::new(),
        u64_array: HashMap::new(),
        u64_value: HashMap::new(),
    };
    assert!(comprehensive.bool_value.is_empty());
    let with_64bit = HashMapWith64Bit {
        i64_map: HashMap::new(),
        id: String::new(),
        mixed_map: HashMap::new(),
        u64_map: HashMap::new(),
    };
    assert!(with_64bit.id.is_empty());
    let with_collections = UserWithCollections {
        id: String::new(),
        metadata: HashMap::new(),
        scores: Vec::new(),
        tags: Vec::new(),
    };
    assert!(with_collections.id.is_empty());
    let optional_values = OptionalMapValues {
        enum_keyed: HashMap::from([(MetricSlot::Daily, None)]),
        string_keyed: HashMap::from([("k".to_owned(), None)]),
    };
    assert_eq!(optional_values.enum_keyed[&MetricSlot::Daily], None);
}

#[test]
#[cfg(feature = "jsonschema")]
fn test_collections_json_schema() {
    let schema = UserWithCollections::json_schema();

    let properties = schema["properties"].as_object().unwrap();

    // Check array properties
    assert_eq!(properties["tags"]["type"], "array");
    assert_eq!(properties["tags"]["items"]["type"], "string");

    assert_eq!(properties["scores"]["type"], "array");
    assert_eq!(properties["scores"]["items"]["type"], "integer");

    // Check map properties
    assert_eq!(properties["metadata"]["type"], "object");
    assert_eq!(
        properties["metadata"]["additionalProperties"]["type"],
        "string"
    );
}

#[test]
#[cfg(all(feature = "typescript", feature = "zod"))]
fn test_collections_ts_definition() {
    let ts_definition = UserWithCollections::ts_definition();

    // Check TypeScript array types
    assert!(ts_definition.contains("tags: Array<string>;"));
    assert!(ts_definition.contains("scores: Array<number>;"));
    // HashMap becomes Partial<Record<...>> in the generated output
    assert!(ts_definition.contains("metadata: Partial<Record<string, string>>;"));

    // Check Zod schema - now in separate method
    let zod_schema = UserWithCollections::zod_schema();
    assert!(zod_schema.contains("tags: z.array(z.string())"));
    assert!(zod_schema.contains("scores: z.array(z.number().int())"));
    assert!(zod_schema.contains("metadata: z.record(z.string(), z.string())"));
}

#[test]
#[cfg(feature = "jsonschema")]
fn test_comprehensive_hashmap_json_schema() {
    let schema = ComprehensiveHashMapTest::json_schema();
    let properties = schema["properties"].as_object().unwrap();

    // Test simple primitive values
    assert_hashmap_primitive_type(properties, "string_value", "string");
    assert_hashmap_primitive_type(properties, "u64_value", "integer");
    assert_hashmap_primitive_type(properties, "i64_value", "integer");
    assert_hashmap_primitive_type(properties, "f64_value", "number");
    assert_hashmap_primitive_type(properties, "bool_value", "boolean");

    // Test array values
    assert_hashmap_array_type(properties, "string_array", "string");
    assert_hashmap_array_type(properties, "u64_array", "integer");
    assert_hashmap_array_type(properties, "i64_array", "integer");
    assert_hashmap_array_type(properties, "f64_array", "number");
    assert_hashmap_array_type(properties, "bool_array", "boolean");

    // An `Option` value is nullable rather than absent — the array wrap sits inside the union,
    // because the `Option` is the outer one in `Option<Vec<T>>`.
    assert_eq!(
        properties["optional_u64"]["additionalProperties"],
        serde_json::json!({ "anyOf": [{ "type": "integer" }, { "type": "null" }] })
    );
    assert_eq!(
        properties["optional_u64_array"]["additionalProperties"],
        serde_json::json!({
            "anyOf": [
                { "type": "array", "items": { "type": "integer" } },
                { "type": "null" }
            ]
        })
    );
}

#[test]
#[cfg(all(feature = "typescript", feature = "zod"))]
fn test_comprehensive_hashmap_typescript_generation() {
    let ts_definition = ComprehensiveHashMapTest::ts_definition();

    // Test TypeScript type generation for simple values
    assert!(ts_definition.contains("string_value: Partial<Record<string, string>>;"));
    assert!(ts_definition.contains("u64_value: Partial<Record<string, number>>;"));
    assert!(ts_definition.contains("i64_value: Partial<Record<string, number>>;"));
    assert!(ts_definition.contains("f64_value: Partial<Record<string, number>>;"));
    assert!(ts_definition.contains("bool_value: Partial<Record<string, boolean>>;"));

    // Test TypeScript type generation for array values
    assert!(ts_definition.contains("string_array: Partial<Record<string, Array<string>>>;"));
    assert!(ts_definition.contains("u64_array: Partial<Record<string, Array<number>>>;"));
    assert!(ts_definition.contains("i64_array: Partial<Record<string, Array<number>>>;"));
    assert!(ts_definition.contains("f64_array: Partial<Record<string, Array<number>>>;"));
    assert!(ts_definition.contains("bool_array: Partial<Record<string, Array<boolean>>>;"));

    // Test Zod schema generation for simple values - now in separate method
    let zod_schema = ComprehensiveHashMapTest::zod_schema();
    assert!(zod_schema.contains("string_value: z.record(z.string(), z.string())"));
    assert!(zod_schema.contains("u64_value: z.record(z.string(), z.number().int())"));
    assert!(zod_schema.contains("i64_value: z.record(z.string(), z.number().int())"));
    assert!(zod_schema.contains("f64_value: z.record(z.string(), z.number())"));
    assert!(zod_schema.contains("bool_value: z.record(z.string(), z.boolean())"));

    // Test Zod schema generation for array values
    assert!(zod_schema.contains("string_array: z.record(z.string(), z.array(z.string()))"));
    assert!(zod_schema.contains("u64_array: z.record(z.string(), z.array(z.number().int()))"));
    assert!(zod_schema.contains("i64_array: z.record(z.string(), z.array(z.number().int()))"));
    assert!(zod_schema.contains("f64_array: z.record(z.string(), z.array(z.number()))"));
    assert!(zod_schema.contains("bool_array: z.record(z.string(), z.array(z.boolean()))"));

    // An `Option` map value is null-flavored, not undefined-flavored: the entry it sits in cannot
    // be dropped, so serde writes the `None` as `null`.
    assert!(
        ts_definition.contains("optional_u64: Partial<Record<string, number | null>>;"),
        "Got: {ts_definition}"
    );
    assert!(
        ts_definition
            .contains("optional_u64_array: Partial<Record<string, Array<number> | null>>;"),
        "Got: {ts_definition}"
    );
    assert!(
        zod_schema.contains("optional_u64: z.record(z.string(), z.nullable(z.number().int()))"),
        "Got: {zod_schema}"
    );
    assert!(
        zod_schema.contains(
            "optional_u64_array: z.record(z.string(), z.nullable(z.array(z.number().int())))"
        ),
        "Got: {zod_schema}"
    );
}

#[test]
fn test_enum_keyed_scalar_value_maps_constructible() {
    let maps = EnumKeyedScalarValueMaps {
        bool_value: HashMap::new(),
        f64_value: HashMap::new(),
        i64_value: HashMap::new(),
        string_array: HashMap::new(),
        string_value: HashMap::from([
            (MetricSlot::Daily, "d".to_owned()),
            (MetricSlot::Weekly, "w".to_owned()),
        ]),
        u64_value: HashMap::new(),
    };
    assert_eq!(
        maps.string_value.get(&MetricSlot::Daily),
        Some(&"d".to_owned())
    );
    assert_eq!(
        maps.string_value.get(&MetricSlot::Weekly),
        Some(&"w".to_owned())
    );
}

#[test]
#[cfg(feature = "jsonschema")]
fn test_enum_keyed_scalar_value_maps_json_schema() {
    let schema = EnumKeyedScalarValueMaps::json_schema();
    let properties = schema["properties"].as_object().unwrap();

    assert_enum_keyed_map_value(
        properties,
        "string_value",
        &serde_json::json!({ "type": "string" }),
    );
    assert_enum_keyed_map_value(
        properties,
        "u64_value",
        &serde_json::json!({ "type": "integer" }),
    );
    assert_enum_keyed_map_value(
        properties,
        "i64_value",
        &serde_json::json!({ "type": "integer" }),
    );
    assert_enum_keyed_map_value(
        properties,
        "f64_value",
        &serde_json::json!({ "type": "number" }),
    );
    assert_enum_keyed_map_value(
        properties,
        "bool_value",
        &serde_json::json!({ "type": "boolean" }),
    );
    assert_enum_keyed_map_value(
        properties,
        "string_array",
        &serde_json::json!({ "type": "array", "items": { "type": "string" } }),
    );
}

#[test]
#[cfg(all(feature = "typescript", feature = "zod"))]
fn test_enum_keyed_scalar_value_maps_typescript_generation() {
    let ts_definition = EnumKeyedScalarValueMaps::ts_definition();

    assert!(
        ts_definition.contains("string_value: Partial<Record<MetricSlot, string>>;"),
        "Got: {ts_definition}"
    );
    assert!(
        ts_definition.contains("u64_value: Partial<Record<MetricSlot, number>>;"),
        "Got: {ts_definition}"
    );
    assert!(
        ts_definition.contains("bool_value: Partial<Record<MetricSlot, boolean>>;"),
        "Got: {ts_definition}"
    );
    assert!(
        ts_definition.contains("string_array: Partial<Record<MetricSlot, Array<string>>>;"),
        "Got: {ts_definition}"
    );

    let zod_schema = EnumKeyedScalarValueMaps::zod_schema();
    assert!(
        zod_schema.contains("string_value: z.record(MetricSlot$Schema, z.string())"),
        "Got: {zod_schema}"
    );
    assert!(
        zod_schema.contains("u64_value: z.record(MetricSlot$Schema, z.number().int())"),
        "Got: {zod_schema}"
    );
    assert!(
        zod_schema.contains("bool_value: z.record(MetricSlot$Schema, z.boolean())"),
        "Got: {zod_schema}"
    );
    assert!(
        zod_schema.contains("string_array: z.record(MetricSlot$Schema, z.array(z.string()))"),
        "Got: {zod_schema}"
    );
}

#[test]
fn test_enum_keyed_sibling_value_maps_constructible() {
    let maps = EnumKeyedSiblingValueMaps {
        sample_array: HashMap::from([(
            MetricSlot::Daily,
            vec![MetricSample {
                label: "d".to_owned(),
            }],
        )]),
        sample_value: HashMap::new(),
    };
    assert_eq!(maps.sample_array[&MetricSlot::Daily].len(), 1);
}

#[test]
#[cfg(feature = "jsonschema")]
fn test_enum_keyed_sibling_value_maps_json_schema() {
    let sample_schema = MetricSample::json_schema();
    let schema = EnumKeyedSiblingValueMaps::json_schema();
    let properties = schema["properties"].as_object().unwrap();

    assert_enum_keyed_map_value(properties, "sample_value", &sample_schema);
    assert_enum_keyed_map_value(
        properties,
        "sample_array",
        &serde_json::json!({ "type": "array", "items": sample_schema }),
    );
}

/// A `Vec` map value is an array of siblings on the wire, so the member schema has to admit that
/// form and turn away the single sibling object a dropped array wrap would have accepted.
#[test]
#[cfg(feature = "jsonschema")]
fn test_enum_keyed_sibling_array_member_matches_the_serialized_form() {
    let sample = MetricSample {
        label: "d".to_owned(),
    };
    let maps = EnumKeyedSiblingValueMaps {
        sample_array: HashMap::from([(MetricSlot::Daily, vec![sample.clone()])]),
        sample_value: HashMap::new(),
    };
    let payload = serde_json::to_value(&maps).unwrap();
    let member = &EnumKeyedSiblingValueMaps::json_schema()["properties"]["sample_array"]["properties"]
        ["Daily"];

    assert_eq!(
        member["type"],
        json_type_name(&payload["sample_array"]["Daily"]),
        "in: {member}"
    );
    assert_eq!(member["items"], MetricSample::json_schema(), "in: {member}");
    assert_ne!(
        member["type"],
        json_type_name(&serde_json::to_value(&sample).unwrap()),
        "in: {member}"
    );
}

#[test]
#[cfg(all(feature = "typescript", feature = "zod"))]
fn test_enum_keyed_sibling_value_maps_typescript_generation() {
    let ts_definition = EnumKeyedSiblingValueMaps::ts_definition();

    assert!(
        ts_definition.contains("sample_value: Partial<Record<MetricSlot, MetricSample>>;"),
        "Got: {ts_definition}"
    );
    assert!(
        ts_definition.contains("sample_array: Partial<Record<MetricSlot, Array<MetricSample>>>;"),
        "Got: {ts_definition}"
    );

    let zod_schema = EnumKeyedSiblingValueMaps::zod_schema();
    assert!(
        zod_schema.contains("sample_value: z.record(MetricSlot$Schema, MetricSample$Schema)"),
        "Got: {zod_schema}"
    );
    assert!(
        zod_schema
            .contains("sample_array: z.record(MetricSlot$Schema, z.array(MetricSample$Schema))"),
        "Got: {zod_schema}"
    );
}

#[test]
fn test_string_keyed_sibling_value_maps_constructible() {
    let maps = StringKeyedSiblingValueMaps {
        optional_sample: HashMap::from([("m".to_owned(), None)]),
        sample_array: HashMap::from([(
            "d".to_owned(),
            vec![MetricSample {
                label: "d".to_owned(),
            }],
        )]),
        sample_value: HashMap::new(),
    };
    assert_eq!(maps.sample_array["d"].len(), 1);
    assert_eq!(maps.optional_sample["m"], None);
}

/// A `String` key never widens the value: the member is the sibling's own schema, arrayed when the
/// value is a `Vec` and nullable when it is an `Option` — the same schema the enum-key path writes
/// under each key, never the open object that admits every payload alike.
#[test]
#[cfg(feature = "jsonschema")]
fn test_string_keyed_sibling_value_maps_json_schema() {
    let sample_schema = MetricSample::json_schema();
    let schema = StringKeyedSiblingValueMaps::json_schema();
    let properties = schema["properties"].as_object().unwrap();

    for (field_name, expected) in [
        ("sample_value", sample_schema.clone()),
        (
            "sample_array",
            serde_json::json!({ "type": "array", "items": sample_schema }),
        ),
        (
            "optional_sample",
            serde_json::json!({ "anyOf": [sample_schema, { "type": "null" }] }),
        ),
    ] {
        let field = &properties[field_name];
        assert_eq!(field["type"], "object", "in: {field}");
        assert_eq!(field["additionalProperties"], expected, "in: {field}");
    }
}

/// The member schema is held against what serde writes: an array of siblings for the `Vec` field,
/// a single sibling object for the plain one. An open member would have accepted either.
#[test]
#[cfg(feature = "jsonschema")]
fn test_string_keyed_sibling_members_match_the_serialized_form() {
    let sample = MetricSample {
        label: "d".to_owned(),
    };
    let maps = StringKeyedSiblingValueMaps {
        optional_sample: HashMap::from([("m".to_owned(), None)]),
        sample_array: HashMap::from([("d".to_owned(), vec![sample.clone()])]),
        sample_value: HashMap::from([("s".to_owned(), sample)]),
    };
    let payload = serde_json::to_value(&maps).unwrap();
    let schema = StringKeyedSiblingValueMaps::json_schema();

    let arrayed = &schema["properties"]["sample_array"]["additionalProperties"];
    assert_eq!(
        arrayed["type"],
        json_type_name(&payload["sample_array"]["d"]),
        "in: {arrayed}"
    );
    assert_eq!(
        arrayed["items"],
        MetricSample::json_schema(),
        "in: {arrayed}"
    );

    let single = &schema["properties"]["sample_value"]["additionalProperties"];
    assert_eq!(
        single["type"],
        json_type_name(&payload["sample_value"]["s"]),
        "in: {single}"
    );
    assert_eq!(*single, MetricSample::json_schema(), "in: {single}");

    let optional = &schema["properties"]["optional_sample"]["additionalProperties"];
    assert_eq!(
        optional["anyOf"][1]["type"],
        json_type_name(&payload["optional_sample"]["m"]),
        "in: {optional}"
    );
}

#[test]
#[cfg(all(feature = "typescript", feature = "zod"))]
fn test_string_keyed_sibling_value_maps_typescript_generation() {
    let ts_definition = StringKeyedSiblingValueMaps::ts_definition();

    assert!(
        ts_definition.contains("sample_value: Partial<Record<string, MetricSample>>;"),
        "Got: {ts_definition}"
    );
    assert!(
        ts_definition.contains("sample_array: Partial<Record<string, Array<MetricSample>>>;"),
        "Got: {ts_definition}"
    );
    assert!(
        ts_definition.contains("optional_sample: Partial<Record<string, MetricSample | null>>;"),
        "Got: {ts_definition}"
    );

    let zod_schema = StringKeyedSiblingValueMaps::zod_schema();
    assert!(
        zod_schema.contains("sample_value: z.record(z.string(), MetricSample$Schema)"),
        "Got: {zod_schema}"
    );
    assert!(
        zod_schema.contains("sample_array: z.record(z.string(), z.array(MetricSample$Schema))"),
        "Got: {zod_schema}"
    );
    assert!(
        zod_schema
            .contains("optional_sample: z.record(z.string(), z.nullable(MetricSample$Schema))"),
        "Got: {zod_schema}"
    );
}

#[test]
fn test_string_keyed_alias_value_maps_constructible() {
    let maps = StringKeyedAliasValueMaps {
        sample_array: HashMap::new(),
        sample_value: HashMap::from([(
            "s".to_owned(),
            MetricSampleRef {
                label: "s".to_owned(),
            },
        )]),
    };
    assert_eq!(maps.sample_value["s"].label, "s");
}

/// An alias's schema module is named after its registered export name, so the member reference is
/// only resolvable through the registry — deriving it from the alias ident names a module that was
/// never emitted, and the expansion no longer compiles.
#[test]
#[cfg(feature = "jsonschema")]
fn test_string_keyed_alias_value_maps_resolve_the_registered_module() {
    let alias_schema = metric_sample_ref_type_schema::Schema::json_schema();
    let schema = StringKeyedAliasValueMaps::json_schema();
    let properties = schema["properties"].as_object().unwrap();

    assert_eq!(
        properties["sample_value"]["additionalProperties"], alias_schema,
        "in: {schema}"
    );
    assert_eq!(
        properties["sample_array"]["additionalProperties"],
        serde_json::json!({ "type": "array", "items": alias_schema }),
        "in: {schema}"
    );
}

#[test]
#[cfg(all(feature = "typescript", feature = "zod"))]
fn test_string_keyed_alias_value_maps_typescript_generation() {
    let ts_definition = StringKeyedAliasValueMaps::ts_definition();

    assert!(
        ts_definition.contains("sample_value: Partial<Record<string, MetricSampleRefType>>;"),
        "Got: {ts_definition}"
    );
    assert!(
        ts_definition
            .contains("sample_array: Partial<Record<string, Array<MetricSampleRefType>>>;"),
        "Got: {ts_definition}"
    );

    let zod_schema = StringKeyedAliasValueMaps::zod_schema();
    assert!(
        zod_schema.contains("sample_value: z.record(z.string(), MetricSampleRefType$Schema)"),
        "Got: {zod_schema}"
    );
    assert!(
        zod_schema
            .contains("sample_array: z.record(z.string(), z.array(MetricSampleRefType$Schema))"),
        "Got: {zod_schema}"
    );
}

/// A map entry carries its `None` as JSON `null`: unlike an object key, an entry cannot be dropped,
/// so the schema has to admit the null serde writes. Both key paths render the same nullable form,
/// the one a tuple slot already uses for the same reason.
#[test]
#[cfg(feature = "jsonschema")]
fn test_optional_map_values_admit_the_null_serde_writes() {
    let values = OptionalMapValues {
        enum_keyed: HashMap::from([(MetricSlot::Daily, None)]),
        string_keyed: HashMap::from([("k".to_owned(), None)]),
    };
    let payload = serde_json::to_value(&values).unwrap();
    assert_eq!(json_type_name(&payload["enum_keyed"]["Daily"]), "null");
    assert_eq!(json_type_name(&payload["string_keyed"]["k"]), "null");

    let nullable_string = serde_json::json!({
        "anyOf": [{ "type": "string" }, { "type": "null" }]
    });
    let schema = OptionalMapValues::json_schema();
    let properties = schema["properties"].as_object().unwrap();

    assert_enum_keyed_map_value(properties, "enum_keyed", &nullable_string);
    assert_eq!(properties["string_keyed"]["type"], "object");
    assert_eq!(
        properties["string_keyed"]["additionalProperties"], nullable_string,
        "in: {}",
        properties["string_keyed"]
    );
}

/// A map value is null-flavored rather than undefined-flavored, on both key paths: `Partial<Record>`
/// already lets a key be missing, but a key that *is* present carries the `null` serde writes for a
/// `None`, so the value type has to admit it.
#[test]
#[cfg(all(feature = "typescript", feature = "zod"))]
fn test_optional_map_values_are_null_flavored_on_both_key_paths() {
    let ts_definition = OptionalMapValues::ts_definition();
    assert!(
        ts_definition.contains("enum_keyed: Partial<Record<MetricSlot, string | null>>;"),
        "Got: {ts_definition}"
    );
    assert!(
        ts_definition.contains("string_keyed: Partial<Record<string, string | null>>;"),
        "Got: {ts_definition}"
    );

    let zod_schema = OptionalMapValues::zod_schema();
    assert!(
        zod_schema.contains("enum_keyed: z.record(MetricSlot$Schema, z.nullable(z.string()))"),
        "Got: {zod_schema}"
    );
    assert!(
        zod_schema.contains("string_keyed: z.record(z.string(), z.nullable(z.string()))"),
        "Got: {zod_schema}"
    );
}

#[test]
#[cfg(feature = "jsonschema")]
fn test_hashmap_with_64bit_json_schema() {
    let schema = HashMapWith64Bit::json_schema();

    let properties = schema["properties"].as_object().unwrap();

    // Check HashMap with u64 values
    assert_eq!(properties["u64_map"]["type"], "object");
    assert_eq!(
        properties["u64_map"]["additionalProperties"]["type"],
        "integer"
    );

    // Check HashMap with i64 values
    assert_eq!(properties["i64_map"]["type"], "object");
    assert_eq!(
        properties["i64_map"]["additionalProperties"]["type"],
        "integer"
    );

    // Check HashMap with Vec<u64> values
    assert_eq!(properties["mixed_map"]["type"], "object");
    assert_eq!(
        properties["mixed_map"]["additionalProperties"]["type"],
        "array"
    );
    assert_eq!(
        properties["mixed_map"]["additionalProperties"]["items"]["type"],
        "integer"
    );
}

#[test]
#[cfg(all(feature = "typescript", feature = "zod"))]
fn test_hashmap_with_64bit_ts_definition() {
    let ts_definition = HashMapWith64Bit::ts_definition();

    // Check TypeScript HashMap types
    assert!(ts_definition.contains("u64_map: Partial<Record<string, number>>;"));
    assert!(ts_definition.contains("i64_map: Partial<Record<string, number>>;"));
    assert!(ts_definition.contains("mixed_map: Partial<Record<string, Array<number>>>;"));

    // Check Zod schema - now in separate method
    let zod_schema = HashMapWith64Bit::zod_schema();
    assert!(zod_schema.contains("u64_map: z.record(z.string(), z.number().int())"));
    assert!(zod_schema.contains("i64_map: z.record(z.string(), z.number().int())"));
    assert!(zod_schema.contains("mixed_map: z.record(z.string(), z.array(z.number().int()))"));
}

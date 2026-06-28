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

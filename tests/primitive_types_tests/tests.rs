#[cfg(all(test, feature = "serde"))]
use serde::{Deserialize, Serialize};
#[cfg(all(test, feature = "jsonschema"))]
use serde_json::Value;
#[cfg(all(
    test,
    any(feature = "typescript", feature = "jsonschema", feature = "zod")
))]
use std::collections::HashMap;
#[cfg(all(
    test,
    any(
        feature = "typescript",
        feature = "jsonschema",
        feature = "zod",
        feature = "serde"
    )
))]
use tixschema::model_schema;

#[cfg(all(
    test,
    any(
        feature = "typescript",
        feature = "jsonschema",
        feature = "zod",
        feature = "serde"
    )
))]
#[model_schema()]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq)]
struct LargeNumbers {
    array_of_i64: Vec<i64>,
    array_of_u64: Vec<u64>,
    id: String,
    large_signed: i64,
    large_unsigned: u64,
    optional_large_signed: Option<i64>,
    optional_large_unsigned: Option<u64>,
}

#[cfg(all(
    test,
    any(feature = "typescript", feature = "jsonschema", feature = "zod")
))]
// Test edge cases with mixed integer types
#[model_schema()]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq)]
struct MixedIntegers {
    isize_type: isize,
    large_i64: i64,
    large_u64: u64,
    medium_i16: i16,
    medium_u16: u16,
    normal_i32: i32,
    normal_u32: u32,
    size_type: usize,
    small_i8: i8,
    small_u8: u8,
}

#[cfg(all(
    test,
    any(feature = "typescript", feature = "jsonschema", feature = "zod")
))]
// Test edge cases with all primitive integer types in various contexts
#[model_schema()]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq)]
struct PrimitiveTypesShowcase {
    arch_signed: isize,
    arch_unsigned: usize,
    array_f64: Vec<f64>,
    array_i8: Vec<i8>,
    array_u64: Vec<u64>,
    float_double: f64,
    float_single: f32,
    large_signed: i64,
    large_unsigned: u64,
    map_to_f64: HashMap<String, f64>,
    map_to_f64_array: HashMap<String, Vec<f64>>,
    map_to_i8: HashMap<String, i8>,
    map_to_i8_array: HashMap<String, Vec<i8>>,
    map_to_u64: HashMap<String, u64>,
    map_to_u64_array: HashMap<String, Vec<u64>>,
    medium_signed: i32,
    medium_unsigned: u32,
    opt_array_f64: Option<Vec<f64>>,
    opt_array_i8: Option<Vec<i8>>,
    opt_array_u64: Option<Vec<u64>>,
    opt_f64: Option<f64>,
    opt_i8: Option<i8>,
    opt_u64: Option<u64>,
    small_signed: i16,
    small_unsigned: u16,
    tiny_signed: i8,
    tiny_unsigned: u8,
}

#[cfg(feature = "jsonschema")]
fn assert_array_property(
    properties: &serde_json::Map<String, serde_json::Value>,
    field_name: &str,
    item_type: &str,
) {
    assert_eq!(properties[field_name]["type"], "array");
    assert_eq!(properties[field_name]["items"]["type"], item_type);
}

#[cfg(all(feature = "typescript", feature = "zod"))]
fn assert_zod_fields_contain(zod_schema: &str, fields: &[&str], expected_pattern: &str) {
    for field in fields {
        let expected = format!("{field}: {expected_pattern}");
        assert!(zod_schema.contains(&expected));
    }
}

#[cfg(all(feature = "typescript", feature = "zod"))]
fn assert_ts_fields_contain(ts_definition: &str, fields: &[&str], expected_suffix: &str) {
    for field in fields {
        let expected = format!("{field}: {expected_suffix};");
        assert!(ts_definition.contains(&expected));
    }
}

#[cfg(feature = "jsonschema")]
fn assert_hashmap_array_property(
    properties: &serde_json::Map<String, serde_json::Value>,
    field_name: &str,
    item_type: &str,
) {
    assert_eq!(properties[field_name]["type"], "object");
    assert_eq!(
        properties[field_name]["additionalProperties"]["type"],
        "array"
    );
    assert_eq!(
        properties[field_name]["additionalProperties"]["items"]["type"],
        item_type
    );
}

#[cfg(feature = "jsonschema")]
fn assert_hashmap_property(
    properties: &serde_json::Map<String, serde_json::Value>,
    field_name: &str,
    value_type: &str,
) {
    assert_eq!(properties[field_name]["type"], "object");
    assert_eq!(
        properties[field_name]["additionalProperties"]["type"],
        value_type
    );
}

#[cfg(feature = "jsonschema")]
fn assert_property_type(
    properties: &serde_json::Map<String, serde_json::Value>,
    field_name: &str,
    expected_type: &str,
) {
    assert_eq!(properties[field_name]["type"], expected_type);
}

#[test]
#[cfg(feature = "jsonschema")]
fn test_64bit_integers_json_schema() {
    let schema = LargeNumbers::json_schema();

    let properties = schema["properties"].as_object().unwrap();

    // Check that u64 and i64 are properly typed
    assert!(properties.contains_key("large_unsigned"));
    assert!(properties.contains_key("large_signed"));

    // These should be integer type
    assert_eq!(properties["large_unsigned"]["type"], "integer");
    assert_eq!(properties["large_signed"]["type"], "integer");

    // Check optional fields
    assert!(properties.contains_key("optional_large_unsigned"));
    assert!(properties.contains_key("optional_large_signed"));

    // Check arrays
    assert_eq!(properties["array_of_u64"]["type"], "array");
    assert_eq!(properties["array_of_u64"]["items"]["type"], "integer");
    assert_eq!(properties["array_of_i64"]["type"], "array");
    assert_eq!(properties["array_of_i64"]["items"]["type"], "integer");

    // Check required fields
    let required = schema["required"].as_array().unwrap();
    assert!(required.contains(&Value::String("large_unsigned".to_owned())));
    assert!(required.contains(&Value::String("large_signed".to_owned())));
    assert!(required.contains(&Value::String("array_of_u64".to_owned())));
    assert!(required.contains(&Value::String("array_of_i64".to_owned())));

    // Optional fields should NOT be in required
    assert!(!required.contains(&Value::String("optional_large_unsigned".to_owned())));
    assert!(!required.contains(&Value::String("optional_large_signed".to_owned())));
}

#[test]
#[cfg(all(feature = "typescript", feature = "zod"))]
fn test_64bit_integers_ts_definition() {
    let ts_definition = LargeNumbers::ts_definition();

    // Check TypeScript type mapping - should be number
    assert!(ts_definition.contains("large_unsigned: number;"));
    assert!(ts_definition.contains("large_signed: number;"));
    assert!(ts_definition.contains("optional_large_unsigned: number | undefined;"));
    assert!(ts_definition.contains("optional_large_signed: number | undefined;"));
    assert!(ts_definition.contains("array_of_u64: Array<number>;"));
    assert!(ts_definition.contains("array_of_i64: Array<number>;"));

    // Check Zod schema - now in separate method
    let zod_schema = LargeNumbers::zod_schema();
    assert!(zod_schema.contains("large_unsigned: z.number().int()"));
    assert!(zod_schema.contains("large_signed: z.number().int()"));
    assert!(
        zod_schema.contains("optional_large_unsigned: z.union([z.number().int(), z.undefined()])")
    );
    assert!(
        zod_schema.contains("optional_large_signed: z.union([z.number().int(), z.undefined()])")
    );
    assert!(zod_schema.contains("array_of_u64: z.array(z.number().int())"));
    assert!(zod_schema.contains("array_of_i64: z.array(z.number().int())"));
}

#[test]
#[cfg(feature = "jsonschema")]
fn test_mixed_integers_json_schema() {
    let schema = MixedIntegers::json_schema();

    let properties = schema["properties"].as_object().unwrap();

    // All integer types should map to "integer" in JSON schema
    assert_eq!(properties["small_u8"]["type"], "integer");
    assert_eq!(properties["small_i8"]["type"], "integer");
    assert_eq!(properties["medium_u16"]["type"], "integer");
    assert_eq!(properties["medium_i16"]["type"], "integer");
    assert_eq!(properties["normal_u32"]["type"], "integer");
    assert_eq!(properties["normal_i32"]["type"], "integer");
    assert_eq!(properties["large_u64"]["type"], "integer");
    assert_eq!(properties["large_i64"]["type"], "integer");
    assert_eq!(properties["size_type"]["type"], "integer");
    assert_eq!(properties["isize_type"]["type"], "integer");
}

#[test]
#[cfg(all(feature = "typescript", feature = "zod"))]
fn test_mixed_integers_ts_definition() {
    let ts_definition = MixedIntegers::ts_definition();

    // All integer types should map to number in TypeScript
    assert!(ts_definition.contains("small_u8: number;"));
    assert!(ts_definition.contains("small_i8: number;"));
    assert!(ts_definition.contains("medium_u16: number;"));
    assert!(ts_definition.contains("medium_i16: number;"));
    assert!(ts_definition.contains("normal_u32: number;"));
    assert!(ts_definition.contains("normal_i32: number;"));
    assert!(ts_definition.contains("large_u64: number;"));
    assert!(ts_definition.contains("large_i64: number;"));
    assert!(ts_definition.contains("size_type: number;"));
    assert!(ts_definition.contains("isize_type: number;"));

    // All should use z.number().int() in Zod - now in separate method
    let zod_schema = MixedIntegers::zod_schema();
    assert!(zod_schema.contains("small_u8: z.number().int()"));
    assert!(zod_schema.contains("small_i8: z.number().int()"));
    assert!(zod_schema.contains("medium_u16: z.number().int()"));
    assert!(zod_schema.contains("medium_i16: z.number().int()"));
    assert!(zod_schema.contains("normal_u32: z.number().int()"));
    assert!(zod_schema.contains("normal_i32: z.number().int()"));
    assert!(zod_schema.contains("large_u64: z.number().int()"));
    assert!(zod_schema.contains("large_i64: z.number().int()"));
    assert!(zod_schema.contains("size_type: z.number().int()"));
    assert!(zod_schema.contains("isize_type: z.number().int()"));
}

#[test]
#[cfg(feature = "jsonschema")]
fn test_primitive_types_json_schema_details() {
    let schema = PrimitiveTypesShowcase::json_schema();
    let properties = schema["properties"].as_object().unwrap();

    // All integer types should map to "integer" in JSON Schema
    let integer_fields = [
        "tiny_signed",
        "tiny_unsigned",
        "small_signed",
        "small_unsigned",
        "medium_signed",
        "medium_unsigned",
        "large_signed",
        "large_unsigned",
        "arch_signed",
        "arch_unsigned",
    ];
    for field in &integer_fields {
        assert_property_type(properties, field, "integer");
    }

    // Float types should map to "number" in JSON Schema
    assert_property_type(properties, "float_single", "number");
    assert_property_type(properties, "float_double", "number");

    // Optional fields should not be in required array
    let required = schema["required"].as_array().unwrap();
    for opt_field in &["opt_i8", "opt_u64", "opt_f64"] {
        assert!(!required.contains(&serde_json::Value::String((*opt_field).to_owned())));
    }

    // Arrays should have proper structure
    assert_array_property(properties, "array_i8", "integer");
    assert_array_property(properties, "array_u64", "integer");
    assert_array_property(properties, "array_f64", "number");

    // HashMap with primitive values
    assert_hashmap_property(properties, "map_to_i8", "integer");
    assert_hashmap_property(properties, "map_to_u64", "integer");
    assert_hashmap_property(properties, "map_to_f64", "number");

    // HashMap with array values
    assert_hashmap_array_property(properties, "map_to_i8_array", "integer");
    assert_hashmap_array_property(properties, "map_to_u64_array", "integer");
    assert_hashmap_array_property(properties, "map_to_f64_array", "number");
}

#[test]
#[cfg(all(feature = "typescript", feature = "zod"))]
fn test_primitive_types_typescript_generation_details() {
    let ts_definition = PrimitiveTypesShowcase::ts_definition();

    // All integer and float types should map to "number" in TypeScript
    let numeric_fields = [
        "tiny_signed",
        "tiny_unsigned",
        "small_signed",
        "small_unsigned",
        "medium_signed",
        "medium_unsigned",
        "large_signed",
        "large_unsigned",
        "arch_signed",
        "arch_unsigned",
        "float_single",
        "float_double",
    ];
    assert_ts_fields_contain(&ts_definition, &numeric_fields, "number");

    // Optional types should include "| undefined"
    assert_ts_fields_contain(
        &ts_definition,
        &["opt_i8", "opt_u64", "opt_f64"],
        "number | undefined",
    );

    // Arrays should use Array<number> syntax
    assert_ts_fields_contain(
        &ts_definition,
        &["array_i8", "array_u64", "array_f64"],
        "Array<number>",
    );

    // Optional arrays should include "| undefined"
    assert_ts_fields_contain(
        &ts_definition,
        &["opt_array_i8", "opt_array_u64", "opt_array_f64"],
        "Array<number> | undefined",
    );

    // HashMap with primitive values
    assert_ts_fields_contain(
        &ts_definition,
        &["map_to_i8", "map_to_u64", "map_to_f64"],
        "Partial<Record<string, number>>",
    );

    // HashMap with array values
    assert_ts_fields_contain(
        &ts_definition,
        &["map_to_i8_array", "map_to_u64_array", "map_to_f64_array"],
        "Partial<Record<string, Array<number>>>",
    );

    // Check Zod schema - now in separate method
    let zod_schema = PrimitiveTypesShowcase::zod_schema();

    // Integer Zod schemas
    assert_zod_fields_contain(
        &zod_schema,
        &[
            "tiny_signed",
            "tiny_unsigned",
            "large_signed",
            "large_unsigned",
        ],
        "z.number().int()",
    );

    // Float Zod schemas (no .int())
    assert_zod_fields_contain(&zod_schema, &["float_single", "float_double"], "z.number()");

    // Optional Zod schemas
    assert_zod_fields_contain(
        &zod_schema,
        &["opt_i8", "opt_u64"],
        "z.union([z.number().int(), z.undefined()])",
    );
    assert!(zod_schema.contains("opt_f64: z.union([z.number(), z.undefined()])"));

    // Array Zod schemas
    assert_zod_fields_contain(
        &zod_schema,
        &["array_i8", "array_u64"],
        "z.array(z.number().int())",
    );
    assert!(zod_schema.contains("array_f64: z.array(z.number())"));

    // HashMap Zod schemas
    assert_zod_fields_contain(
        &zod_schema,
        &["map_to_i8", "map_to_u64"],
        "z.record(z.string(), z.number().int())",
    );
    assert!(zod_schema.contains("map_to_f64: z.record(z.string(), z.number())"));

    // HashMap with array Zod schemas
    assert_zod_fields_contain(
        &zod_schema,
        &["map_to_i8_array", "map_to_u64_array"],
        "z.record(z.string(), z.array(z.number().int()))",
    );
    assert!(zod_schema.contains("map_to_f64_array: z.record(z.string(), z.array(z.number()))"));
}

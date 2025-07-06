use core_model_macros::model_schema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[cfg(test)]
mod tests {
    use super::*;

    // Test 64-bit integers - potential bug area
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
    struct LargeNumbersJson {
        id: String,
        large_unsigned: u64,
        large_signed: i64,
        optional_large_unsigned: Option<u64>,
        optional_large_signed: Option<i64>,
        array_of_u64: Vec<u64>,
        array_of_i64: Vec<i64>,
    }

    #[test]
    fn test_64bit_integers_json_schema() {
        let schema = LargeNumbersJson::json_schema();
        
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
        assert!(required.contains(&Value::String("large_unsigned".to_string())));
        assert!(required.contains(&Value::String("large_signed".to_string())));
        assert!(required.contains(&Value::String("array_of_u64".to_string())));
        assert!(required.contains(&Value::String("array_of_i64".to_string())));
        
        // Optional fields should NOT be in required
        assert!(!required.contains(&Value::String("optional_large_unsigned".to_string())));
        assert!(!required.contains(&Value::String("optional_large_signed".to_string())));
    }

    #[test]
    fn test_64bit_integers_ts_definition() {
        let ts_definition = LargeNumbersJson::ts_definition();
        
        // Check TypeScript type mapping - should be number
        assert!(ts_definition.contains("large_unsigned: number;"));
        assert!(ts_definition.contains("large_signed: number;"));
        assert!(ts_definition.contains("optional_large_unsigned: number | undefined;"));
        assert!(ts_definition.contains("optional_large_signed: number | undefined;"));
        assert!(ts_definition.contains("array_of_u64: Array<number>;"));
        assert!(ts_definition.contains("array_of_i64: Array<number>;"));
        
        // Check Zod schema - now in separate method
        let zod_schema = LargeNumbersJson::zod_schema();
        assert!(zod_schema.contains("large_unsigned: z.number().int()"));
        assert!(zod_schema.contains("large_signed: z.number().int()"));
        assert!(zod_schema.contains("optional_large_unsigned: z.number().int().or(z.undefined())"));
        assert!(zod_schema.contains("optional_large_signed: z.number().int().or(z.undefined())"));
        assert!(zod_schema.contains("array_of_u64: z.array(z.number().int())"));
        assert!(zod_schema.contains("array_of_i64: z.array(z.number().int())"));
    }

    // Test edge cases with mixed integer types
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
    struct MixedIntegersJson {
        small_u8: u8,
        small_i8: i8,
        medium_u16: u16,
        medium_i16: i16,
        normal_u32: u32,
        normal_i32: i32,
        large_u64: u64,
        large_i64: i64,
        size_type: usize,
        isize_type: isize,
    }

    #[test]
    fn test_mixed_integers_json_schema() {
        let schema = MixedIntegersJson::json_schema();
        
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
    fn test_mixed_integers_ts_definition() {
        let ts_definition = MixedIntegersJson::ts_definition();
        
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
        let zod_schema = MixedIntegersJson::zod_schema();
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

    // Test edge cases with all primitive integer types in various contexts
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
    struct PrimitiveTypesShowcaseJson {
        // Single values
        tiny_signed: i8,
        tiny_unsigned: u8,
        small_signed: i16,
        small_unsigned: u16,
        medium_signed: i32,
        medium_unsigned: u32,
        large_signed: i64,
        large_unsigned: u64,
        arch_signed: isize,
        arch_unsigned: usize,
        float_single: f32,
        float_double: f64,
        
        // Optional values
        opt_i8: Option<i8>,
        opt_u64: Option<u64>,
        opt_f64: Option<f64>,
        
        // Arrays
        array_i8: Vec<i8>,
        array_u64: Vec<u64>,
        array_f64: Vec<f64>,
        
        // Optional arrays
        opt_array_i8: Option<Vec<i8>>,
        opt_array_u64: Option<Vec<u64>>,
        opt_array_f64: Option<Vec<f64>>,
        
        // HashMaps with primitive values
        map_to_i8: HashMap<String, i8>,
        map_to_u64: HashMap<String, u64>,
        map_to_f64: HashMap<String, f64>,
        
        // HashMaps with array values
        map_to_i8_array: HashMap<String, Vec<i8>>,
        map_to_u64_array: HashMap<String, Vec<u64>>,
        map_to_f64_array: HashMap<String, Vec<f64>>,
    }

    #[test]
    fn test_primitive_types_json_schema_details() {
        let schema = PrimitiveTypesShowcaseJson::json_schema();
        
        let properties = schema["properties"].as_object().unwrap();
        
        // All integer types should map to "integer" in JSON Schema
        assert_eq!(properties["tiny_signed"]["type"], "integer");
        assert_eq!(properties["tiny_unsigned"]["type"], "integer");
        assert_eq!(properties["small_signed"]["type"], "integer");
        assert_eq!(properties["small_unsigned"]["type"], "integer");
        assert_eq!(properties["medium_signed"]["type"], "integer");
        assert_eq!(properties["medium_unsigned"]["type"], "integer");
        assert_eq!(properties["large_signed"]["type"], "integer");
        assert_eq!(properties["large_unsigned"]["type"], "integer");
        assert_eq!(properties["arch_signed"]["type"], "integer");
        assert_eq!(properties["arch_unsigned"]["type"], "integer");
        
        // Float types should map to "number" in JSON Schema
        assert_eq!(properties["float_single"]["type"], "number");
        assert_eq!(properties["float_double"]["type"], "number");
        
        // Optional fields should not be in required array
        let required = schema["required"].as_array().unwrap();
        assert!(!required.contains(&serde_json::Value::String("opt_i8".to_string())));
        assert!(!required.contains(&serde_json::Value::String("opt_u64".to_string())));
        assert!(!required.contains(&serde_json::Value::String("opt_f64".to_string())));
        
        // Arrays should have proper structure
        assert_eq!(properties["array_i8"]["type"], "array");
        assert_eq!(properties["array_i8"]["items"]["type"], "integer");
        assert_eq!(properties["array_u64"]["type"], "array");
        assert_eq!(properties["array_u64"]["items"]["type"], "integer");
        assert_eq!(properties["array_f64"]["type"], "array");
        assert_eq!(properties["array_f64"]["items"]["type"], "number");
        
        // HashMap with primitive values
        assert_eq!(properties["map_to_i8"]["type"], "object");
        assert_eq!(properties["map_to_i8"]["additionalProperties"]["type"], "integer");
        assert_eq!(properties["map_to_u64"]["type"], "object");
        assert_eq!(properties["map_to_u64"]["additionalProperties"]["type"], "integer");
        assert_eq!(properties["map_to_f64"]["type"], "object");
        assert_eq!(properties["map_to_f64"]["additionalProperties"]["type"], "number");
        
        // HashMap with array values
        assert_eq!(properties["map_to_i8_array"]["type"], "object");
        assert_eq!(properties["map_to_i8_array"]["additionalProperties"]["type"], "array");
        assert_eq!(properties["map_to_i8_array"]["additionalProperties"]["items"]["type"], "integer");
        
        assert_eq!(properties["map_to_u64_array"]["type"], "object");
        assert_eq!(properties["map_to_u64_array"]["additionalProperties"]["type"], "array");
        assert_eq!(properties["map_to_u64_array"]["additionalProperties"]["items"]["type"], "integer");
        
        assert_eq!(properties["map_to_f64_array"]["type"], "object");
        assert_eq!(properties["map_to_f64_array"]["additionalProperties"]["type"], "array");
        assert_eq!(properties["map_to_f64_array"]["additionalProperties"]["items"]["type"], "number");
    }

    #[test]
    fn test_primitive_types_typescript_generation_details() {
        let ts_definition = PrimitiveTypesShowcaseJson::ts_definition();
        
        // All integer and float types should map to "number" in TypeScript
        assert!(ts_definition.contains("tiny_signed: number;"));
        assert!(ts_definition.contains("tiny_unsigned: number;"));
        assert!(ts_definition.contains("small_signed: number;"));
        assert!(ts_definition.contains("small_unsigned: number;"));
        assert!(ts_definition.contains("medium_signed: number;"));
        assert!(ts_definition.contains("medium_unsigned: number;"));
        assert!(ts_definition.contains("large_signed: number;"));
        assert!(ts_definition.contains("large_unsigned: number;"));
        assert!(ts_definition.contains("arch_signed: number;"));
        assert!(ts_definition.contains("arch_unsigned: number;"));
        assert!(ts_definition.contains("float_single: number;"));
        assert!(ts_definition.contains("float_double: number;"));
        
        // Optional types should include "| undefined"
        assert!(ts_definition.contains("opt_i8: number | undefined;"));
        assert!(ts_definition.contains("opt_u64: number | undefined;"));
        assert!(ts_definition.contains("opt_f64: number | undefined;"));
        
        // Arrays should use Array<number> syntax
        assert!(ts_definition.contains("array_i8: Array<number>;"));
        assert!(ts_definition.contains("array_u64: Array<number>;"));
        assert!(ts_definition.contains("array_f64: Array<number>;"));
        
        // Optional arrays should include "| undefined"
        assert!(ts_definition.contains("opt_array_i8: Array<number> | undefined;"));
        assert!(ts_definition.contains("opt_array_u64: Array<number> | undefined;"));
        assert!(ts_definition.contains("opt_array_f64: Array<number> | undefined;"));
        
        // HashMap with primitive values
        assert!(ts_definition.contains("map_to_i8: Partial<Record<string, number>>;"));
        assert!(ts_definition.contains("map_to_u64: Partial<Record<string, number>>;"));
        assert!(ts_definition.contains("map_to_f64: Partial<Record<string, number>>;"));
        
        // HashMap with array values
        assert!(ts_definition.contains("map_to_i8_array: Partial<Record<string, Array<number>>>;"));
        assert!(ts_definition.contains("map_to_u64_array: Partial<Record<string, Array<number>>>;"));
        assert!(ts_definition.contains("map_to_f64_array: Partial<Record<string, Array<number>>>;"));
        
        // Zod schemas - integers use .int(), floats don't - now in separate method
        let zod_schema = PrimitiveTypesShowcaseJson::zod_schema();
        assert!(zod_schema.contains("tiny_signed: z.number().int()"));
        assert!(zod_schema.contains("tiny_unsigned: z.number().int()"));
        assert!(zod_schema.contains("large_signed: z.number().int()"));
        assert!(zod_schema.contains("large_unsigned: z.number().int()"));
        assert!(zod_schema.contains("float_single: z.number()"));  // No .int() for floats
        assert!(zod_schema.contains("float_double: z.number()"));  // No .int() for floats
        
        // Optional Zod schemas
        assert!(zod_schema.contains("opt_i8: z.number().int().or(z.undefined())"));
        assert!(zod_schema.contains("opt_u64: z.number().int().or(z.undefined())"));
        assert!(zod_schema.contains("opt_f64: z.number().or(z.undefined())"));  // No .int() for float
        
        // Array Zod schemas
        assert!(zod_schema.contains("array_i8: z.array(z.number().int())"));
        assert!(zod_schema.contains("array_u64: z.array(z.number().int())"));
        assert!(zod_schema.contains("array_f64: z.array(z.number())"));  // No .int() for float
        
        // HashMap Zod schemas
        assert!(zod_schema.contains("map_to_i8: z.record(z.string(), z.number().int())"));
        assert!(zod_schema.contains("map_to_u64: z.record(z.string(), z.number().int())"));
        assert!(zod_schema.contains("map_to_f64: z.record(z.string(), z.number())"));  // No .int() for float
        
        // HashMap with array Zod schemas
        assert!(zod_schema.contains("map_to_i8_array: z.record(z.string(), z.array(z.number().int()))"));
        assert!(zod_schema.contains("map_to_u64_array: z.record(z.string(), z.array(z.number().int()))"));
        assert!(zod_schema.contains("map_to_f64_array: z.record(z.string(), z.array(z.number()))"));  // No .int() for float
    }
} 
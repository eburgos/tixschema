use tixschema::model_schema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[cfg(test)]
mod tests {
    use super::*;

    // Test struct with collections
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
    struct UserWithCollections {
        id: String,
        tags: Vec<String>,
        scores: Vec<u32>,
        metadata: HashMap<String, String>,
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
        assert_eq!(properties["metadata"]["additionalProperties"]["type"], "string");
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

    // Test comprehensive HashMap scenarios with various value types
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
    struct ComprehensiveHashMapTestJson {
        // Simple primitives as values
        string_to_string: HashMap<String, String>,
        string_to_u64: HashMap<String, u64>,
        string_to_i64: HashMap<String, i64>,
        string_to_f64: HashMap<String, f64>,
        string_to_bool: HashMap<String, bool>,
        
        // Arrays as values
        string_to_string_array: HashMap<String, Vec<String>>,
        string_to_u64_array: HashMap<String, Vec<u64>>,
        string_to_i64_array: HashMap<String, Vec<i64>>,
        string_to_f64_array: HashMap<String, Vec<f64>>,
        string_to_bool_array: HashMap<String, Vec<bool>>,
        
        // Optional values
        string_to_optional_u64: HashMap<String, Option<u64>>,
        string_to_optional_u64_array: HashMap<String, Option<Vec<u64>>>,
    }

    #[test]
    #[cfg(feature = "jsonschema")]
    fn test_comprehensive_hashmap_json_schema() {
        let schema = ComprehensiveHashMapTestJson::json_schema();
        
        let properties = schema["properties"].as_object().unwrap();
        
        // Test simple primitive values
        assert_eq!(properties["string_to_string"]["type"], "object");
        assert_eq!(properties["string_to_string"]["additionalProperties"]["type"], "string");
        
        assert_eq!(properties["string_to_u64"]["type"], "object");
        assert_eq!(properties["string_to_u64"]["additionalProperties"]["type"], "integer");
        
        assert_eq!(properties["string_to_i64"]["type"], "object");
        assert_eq!(properties["string_to_i64"]["additionalProperties"]["type"], "integer");
        
        assert_eq!(properties["string_to_f64"]["type"], "object");
        assert_eq!(properties["string_to_f64"]["additionalProperties"]["type"], "number");
        
        assert_eq!(properties["string_to_bool"]["type"], "object");
        assert_eq!(properties["string_to_bool"]["additionalProperties"]["type"], "boolean");
        
        // Test array values - these should have "type": "array" with proper "items"
        assert_eq!(properties["string_to_string_array"]["type"], "object");
        assert_eq!(properties["string_to_string_array"]["additionalProperties"]["type"], "array");
        assert_eq!(properties["string_to_string_array"]["additionalProperties"]["items"]["type"], "string");
        
        assert_eq!(properties["string_to_u64_array"]["type"], "object");
        assert_eq!(properties["string_to_u64_array"]["additionalProperties"]["type"], "array");
        assert_eq!(properties["string_to_u64_array"]["additionalProperties"]["items"]["type"], "integer");
        
        assert_eq!(properties["string_to_i64_array"]["type"], "object");
        assert_eq!(properties["string_to_i64_array"]["additionalProperties"]["type"], "array");
        assert_eq!(properties["string_to_i64_array"]["additionalProperties"]["items"]["type"], "integer");
        
        assert_eq!(properties["string_to_f64_array"]["type"], "object");
        assert_eq!(properties["string_to_f64_array"]["additionalProperties"]["type"], "array");
        assert_eq!(properties["string_to_f64_array"]["additionalProperties"]["items"]["type"], "number");
        
        assert_eq!(properties["string_to_bool_array"]["type"], "object");
        assert_eq!(properties["string_to_bool_array"]["additionalProperties"]["type"], "array");
        assert_eq!(properties["string_to_bool_array"]["additionalProperties"]["items"]["type"], "boolean");
    }

    #[test]
    #[cfg(all(feature = "typescript", feature = "zod"))]
    fn test_comprehensive_hashmap_typescript_generation() {
        let ts_definition = ComprehensiveHashMapTestJson::ts_definition();
        
        // Test TypeScript type generation for simple values
        assert!(ts_definition.contains("string_to_string: Partial<Record<string, string>>;"));
        assert!(ts_definition.contains("string_to_u64: Partial<Record<string, number>>;"));
        assert!(ts_definition.contains("string_to_i64: Partial<Record<string, number>>;"));
        assert!(ts_definition.contains("string_to_f64: Partial<Record<string, number>>;"));
        assert!(ts_definition.contains("string_to_bool: Partial<Record<string, boolean>>;"));
        
        // Test TypeScript type generation for array values
        assert!(ts_definition.contains("string_to_string_array: Partial<Record<string, Array<string>>>;"));
        assert!(ts_definition.contains("string_to_u64_array: Partial<Record<string, Array<number>>>;"));
        assert!(ts_definition.contains("string_to_i64_array: Partial<Record<string, Array<number>>>;"));
        assert!(ts_definition.contains("string_to_f64_array: Partial<Record<string, Array<number>>>;"));
        assert!(ts_definition.contains("string_to_bool_array: Partial<Record<string, Array<boolean>>>;"));
        
        // Test Zod schema generation for simple values - now in separate method
        let zod_schema = ComprehensiveHashMapTestJson::zod_schema();
        assert!(zod_schema.contains("string_to_string: z.record(z.string(), z.string())"));
        assert!(zod_schema.contains("string_to_u64: z.record(z.string(), z.number().int())"));
        assert!(zod_schema.contains("string_to_i64: z.record(z.string(), z.number().int())"));
        assert!(zod_schema.contains("string_to_f64: z.record(z.string(), z.number())"));
        assert!(zod_schema.contains("string_to_bool: z.record(z.string(), z.boolean())"));
        
        // Test Zod schema generation for array values  
        assert!(zod_schema.contains("string_to_string_array: z.record(z.string(), z.array(z.string()))"));
        assert!(zod_schema.contains("string_to_u64_array: z.record(z.string(), z.array(z.number().int()))"));
        assert!(zod_schema.contains("string_to_i64_array: z.record(z.string(), z.array(z.number().int()))"));
        assert!(zod_schema.contains("string_to_f64_array: z.record(z.string(), z.array(z.number()))"));
        assert!(zod_schema.contains("string_to_bool_array: z.record(z.string(), z.array(z.boolean()))"));
    }

    // Test potential edge case with HashMap containing 64-bit integers
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
    struct HashMapWith64BitJson {
        id: String,
        u64_map: HashMap<String, u64>,
        i64_map: HashMap<String, i64>,
        mixed_map: HashMap<String, Vec<u64>>,
    }

    #[test]
    #[cfg(feature = "jsonschema")]
    fn test_hashmap_with_64bit_json_schema() {
        let schema = HashMapWith64BitJson::json_schema();
        
        let properties = schema["properties"].as_object().unwrap();
        
        // Check HashMap with u64 values
        assert_eq!(properties["u64_map"]["type"], "object");
        assert_eq!(properties["u64_map"]["additionalProperties"]["type"], "integer");
        
        // Check HashMap with i64 values
        assert_eq!(properties["i64_map"]["type"], "object");
        assert_eq!(properties["i64_map"]["additionalProperties"]["type"], "integer");
        
        // Check HashMap with Vec<u64> values
        assert_eq!(properties["mixed_map"]["type"], "object");
        assert_eq!(properties["mixed_map"]["additionalProperties"]["type"], "array");
        assert_eq!(properties["mixed_map"]["additionalProperties"]["items"]["type"], "integer");
    }

    #[test]
    #[cfg(all(feature = "typescript", feature = "zod"))]
    fn test_hashmap_with_64bit_ts_definition() {
        let ts_definition = HashMapWith64BitJson::ts_definition();
        
        // Check TypeScript HashMap types
        assert!(ts_definition.contains("u64_map: Partial<Record<string, number>>;"));
        assert!(ts_definition.contains("i64_map: Partial<Record<string, number>>;"));
        assert!(ts_definition.contains("mixed_map: Partial<Record<string, Array<number>>>;"));
        
        // Check Zod schema - now in separate method
        let zod_schema = HashMapWith64BitJson::zod_schema();
        assert!(zod_schema.contains("u64_map: z.record(z.string(), z.number().int())"));
        assert!(zod_schema.contains("i64_map: z.record(z.string(), z.number().int())"));
        assert!(zod_schema.contains("mixed_map: z.record(z.string(), z.array(z.number().int()))"));
    }
} 
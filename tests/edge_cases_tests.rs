use core_model_macros::model_schema;
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::HashMap;

#[cfg(test)]
mod tests {
    use super::*;

    // Test nested struct relationships (simplified for testing)
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
    struct AddressJson {
        street: String,
        city: String,
        zip_code: String,
    }

    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
    struct UserWithAddressJson {
        id: String,
        name: String,
        address: AddressJson,
        backup_addresses: Vec<AddressJson>,
    }

    #[test]
    fn test_nested_struct_json_schema() {
        let user_schema = UserWithAddressJson::json_schema();
        let address_schema = AddressJson::json_schema();
        
        let properties = user_schema["properties"].as_object().unwrap();
        
        // Single nested object should reference the nested type
        assert!(properties.contains_key("address"));
        
        // Array of nested objects should be an array with items referencing the nested type
        assert!(properties.contains_key("backup_addresses"));
        assert_eq!(properties["backup_addresses"]["type"], "array");
        
        // Verify Address schema exists and is correct
        assert_eq!(address_schema["type"], "object");
        let address_properties = address_schema["properties"].as_object().unwrap();
        assert!(address_properties.contains_key("street"));
        assert!(address_properties.contains_key("city"));
        assert!(address_properties.contains_key("zip_code"));
    }

    #[test]
    fn test_nested_struct_ts_definition() {
        let user_definition = UserWithAddressJson::ts_definition();
        let address_definition = AddressJson::ts_definition();
        
        // Check that nested types are referenced properly (without Json suffix)
        assert!(user_definition.contains("address: Address;"));
        assert!(user_definition.contains("backup_addresses: Array<Address>;"));
        
        // Verify Address definition exists (without Json suffix in export type)
        assert!(address_definition.contains("export type Address = {"));
        assert!(address_definition.contains("street: string;"));
        assert!(address_definition.contains("city: string;"));
        assert!(address_definition.contains("zip_code: string;"));
        
        // Check Zod schema references (without Json suffix) - now in separate method
        let user_zod_schema = UserWithAddressJson::zod_schema();
        assert!(user_zod_schema.contains("address: Address$Schema"));
        assert!(user_zod_schema.contains("backup_addresses: z.array(Address$Schema)"));
    }

    // Test the specific edge case that was originally failing
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
    struct OriginalBugReproductionJson {
        // This was the original failing case
        problematic_map: HashMap<String, Vec<u64>>,
        
        // Additional cases that might have similar issues
        string_to_vec_i64: HashMap<String, Vec<i64>>,
        string_to_vec_f64: HashMap<String, Vec<f64>>,
        string_to_vec_bool: HashMap<String, Vec<bool>>,
        string_to_vec_string: HashMap<String, Vec<String>>,
        
        // Nested cases
        string_to_optional_vec_u64: HashMap<String, Option<Vec<u64>>>,
    }

    #[test]
    fn test_original_bug_reproduction_json_schema() {
        let schema = OriginalBugReproductionJson::json_schema();
        
        let properties = schema["properties"].as_object().unwrap();
        
        // The original problematic case
        assert_eq!(properties["problematic_map"]["type"], "object");
        assert_eq!(properties["problematic_map"]["additionalProperties"]["type"], "array");
        assert_eq!(properties["problematic_map"]["additionalProperties"]["items"]["type"], "integer");
        
        // Similar cases
        assert_eq!(properties["string_to_vec_i64"]["additionalProperties"]["type"], "array");
        assert_eq!(properties["string_to_vec_i64"]["additionalProperties"]["items"]["type"], "integer");
        
        assert_eq!(properties["string_to_vec_f64"]["additionalProperties"]["type"], "array");
        assert_eq!(properties["string_to_vec_f64"]["additionalProperties"]["items"]["type"], "number");
        
        assert_eq!(properties["string_to_vec_bool"]["additionalProperties"]["type"], "array");
        assert_eq!(properties["string_to_vec_bool"]["additionalProperties"]["items"]["type"], "boolean");
        
        assert_eq!(properties["string_to_vec_string"]["additionalProperties"]["type"], "array");
        assert_eq!(properties["string_to_vec_string"]["additionalProperties"]["items"]["type"], "string");
    }

    #[test]
    fn test_original_bug_reproduction_typescript() {
        let ts_definition = OriginalBugReproductionJson::ts_definition();
        
        // TypeScript should use Array<T> syntax for the HashMap values
        assert!(ts_definition.contains("problematic_map: Partial<Record<string, Array<number>>>;"));
        assert!(ts_definition.contains("string_to_vec_i64: Partial<Record<string, Array<number>>>;"));
        assert!(ts_definition.contains("string_to_vec_f64: Partial<Record<string, Array<number>>>;"));
        assert!(ts_definition.contains("string_to_vec_bool: Partial<Record<string, Array<boolean>>>;"));
        assert!(ts_definition.contains("string_to_vec_string: Partial<Record<string, Array<string>>>;"));
        
        // Zod schemas should use z.array(...) for the HashMap values - now in separate method
        let zod_schema = OriginalBugReproductionJson::zod_schema();
        assert!(zod_schema.contains("problematic_map: z.record(z.string(), z.array(z.number().int()))"));
        assert!(zod_schema.contains("string_to_vec_i64: z.record(z.string(), z.array(z.number().int()))"));
        assert!(zod_schema.contains("string_to_vec_f64: z.record(z.string(), z.array(z.number()))"));
        assert!(zod_schema.contains("string_to_vec_bool: z.record(z.string(), z.array(z.boolean()))"));
        assert!(zod_schema.contains("string_to_vec_string: z.record(z.string(), z.array(z.string()))"));
    }

    // Let's start with just one complex case to debug
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
    struct SimpleComplexTestJson {
        // Start with just triple-nested to see what fails
        nested_map_of_arrays: HashMap<String, Vec<HashMap<String, u64>>>,
    }

    // Now let's try the really complex case
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
    struct ReallyComplexTestJson {
        // The quadruple nested case that was causing issues
        quadruple_nested: HashMap<String, Vec<HashMap<String, Vec<HashMap<String, u64>>>>>,
        
        // Another challenging case with optional nested structures
        optional_nested: Option<HashMap<String, Vec<Option<HashMap<String, Option<Vec<i64>>>>>>>,
    }

    #[test]
    fn test_complex_nested_maps_json_schema() {
        let schema = SimpleComplexTestJson::json_schema();
        let properties = schema["properties"].as_object().unwrap();
        
        // Test the triple-nested structure: HashMap<String, Vec<HashMap<String, u64>>>
        // Expected: object -> array -> object -> integer
        assert_eq!(properties["nested_map_of_arrays"]["type"], "object");
        
        let additional_props = properties["nested_map_of_arrays"]["additionalProperties"].as_object().unwrap();
        assert_eq!(additional_props["type"], "array");
        
        let items = additional_props["items"].as_object().unwrap();
        assert_eq!(items["type"], "object");
        
        let inner_additional_props = items["additionalProperties"].as_object().unwrap();
        assert_eq!(inner_additional_props["type"], "integer");
    }

    #[test]  
    fn test_complex_nested_maps_typescript() {
        let ts_definition = SimpleComplexTestJson::ts_definition();
        
        // TypeScript should use the correct nested structure
        assert!(ts_definition.contains("nested_map_of_arrays: Partial<Record<string, Array<Partial<Record<string, number>>>>>;"));
        
        // Zod schema should use the correct nested structure - now in separate method
        let zod_schema = SimpleComplexTestJson::zod_schema();
        assert!(zod_schema.contains("nested_map_of_arrays: z.record(z.string(), z.array(z.record(z.string(), z.number().int()))),"));
    }

    #[test]
    fn test_quadruple_nested_maps_compilation() {
        // If this compiles without panic, it's a huge success!
        let schema = ReallyComplexTestJson::json_schema();
        let ts_definition = ReallyComplexTestJson::ts_definition();
        
        // Check that the schema contains our fields
        let properties = schema["properties"].as_object().unwrap();
        assert!(properties.contains_key("quadruple_nested"));
        assert!(properties.contains_key("optional_nested"));
        
        // Basic structure checks
        assert_eq!(properties["quadruple_nested"]["type"], "object");
        
        // Check TypeScript contains our fields (exact types may be complex)
        assert!(ts_definition.contains("quadruple_nested"));
        assert!(ts_definition.contains("optional_nested"));
    }


} 
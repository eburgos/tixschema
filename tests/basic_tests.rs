use core_model_macros::model_schema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[cfg(test)]
mod tests {
    use super::*;

    // Test simple struct with basic field types
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
    struct BasicUser {
        id: String,
        name: String,
        age: u32,
        height: f32,
        is_active: bool,
    }

    #[test]
    fn test_basic_struct_json_schema() {
        let schema = BasicUser::json_schema();
        
        // Check that it's an object
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["additionalProperties"], false);
        
        // Check properties exist
        let properties = schema["properties"].as_object().unwrap();
        assert!(properties.contains_key("id"));
        assert!(properties.contains_key("name"));
        assert!(properties.contains_key("age"));
        assert!(properties.contains_key("height"));
        assert!(properties.contains_key("is_active"));
        
        // Check property types
        assert_eq!(properties["id"]["type"], "string");
        assert_eq!(properties["name"]["type"], "string");
        assert_eq!(properties["age"]["type"], "integer");
        assert_eq!(properties["height"]["type"], "number");
        assert_eq!(properties["is_active"]["type"], "boolean");
        
        // Check required fields
        let required = schema["required"].as_array().unwrap();
        assert_eq!(required.len(), 5);
        assert!(required.contains(&Value::String("id".to_string())));
        assert!(required.contains(&Value::String("name".to_string())));
        assert!(required.contains(&Value::String("age".to_string())));
        assert!(required.contains(&Value::String("height".to_string())));
        assert!(required.contains(&Value::String("is_active".to_string())));
    }

    #[test]
    fn test_basic_struct_ts_definition() {
        let ts_definition = BasicUser::ts_definition();
        
        // Check that it contains TypeScript type definition
        assert!(ts_definition.contains("export type BasicUser = {"));
        assert!(ts_definition.contains("id: string;"));
        assert!(ts_definition.contains("name: string;"));
        assert!(ts_definition.contains("age: number;"));
        assert!(ts_definition.contains("height: number;"));
        assert!(ts_definition.contains("is_active: boolean;"));
        
        // Check that it contains Zod schema
        assert!(ts_definition.contains("export const BasicUser$Schema"));
        assert!(ts_definition.contains("z.strictObject({"));
        assert!(ts_definition.contains("id: z.string()"));
        assert!(ts_definition.contains("name: z.string()"));
        assert!(ts_definition.contains("age: z.number().int()"));
        assert!(ts_definition.contains("height: z.number()"));
        assert!(ts_definition.contains("is_active: z.boolean()"));
    }

    // Test struct with optional fields
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
    struct UserWithOptionals {
        id: String,
        name: String,
        email: Option<String>,
        age: Option<u32>,
        nickname: Option<String>,
    }

    #[test]
    fn test_optional_fields_json_schema() {
        let schema = UserWithOptionals::json_schema();
        
        // Check properties exist
        let properties = schema["properties"].as_object().unwrap();
        assert!(properties.contains_key("id"));
        assert!(properties.contains_key("name"));
        assert!(properties.contains_key("email"));
        assert!(properties.contains_key("age"));
        assert!(properties.contains_key("nickname"));
        
        // Check required fields (only non-optional ones)
        let required = schema["required"].as_array().unwrap();
        assert_eq!(required.len(), 2);
        assert!(required.contains(&Value::String("id".to_string())));
        assert!(required.contains(&Value::String("name".to_string())));
        assert!(!required.contains(&Value::String("email".to_string())));
        assert!(!required.contains(&Value::String("age".to_string())));
        assert!(!required.contains(&Value::String("nickname".to_string())));
    }

    #[test]
    fn test_optional_fields_ts_definition() {
        let ts_definition = UserWithOptionals::ts_definition();
        
        // Check that optional fields are properly typed
        assert!(ts_definition.contains("id: string;"));
        assert!(ts_definition.contains("name: string;"));
        assert!(ts_definition.contains("email: string | undefined;"));
        assert!(ts_definition.contains("age: number | undefined;"));
        assert!(ts_definition.contains("nickname: string | undefined;"));
        
        // Check Zod schema has optional fields
        assert!(ts_definition.contains("email: z.string().optional()"));
        assert!(ts_definition.contains("age: z.number().int().optional()"));
        assert!(ts_definition.contains("nickname: z.string().optional()"));
    }

    // Test empty struct
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
    struct EmptyStruct {}

    #[test]
    fn test_empty_struct_json_schema() {
        let schema = EmptyStruct::json_schema();
        
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["additionalProperties"], false);
        
        let properties = schema["properties"].as_object().unwrap();
        assert!(properties.is_empty());
        
        let required = schema["required"].as_array().unwrap();
        assert!(required.is_empty());
    }

    #[test]
    fn test_empty_struct_ts_definition() {
        let ts_definition = EmptyStruct::ts_definition();
        
        // Should generate Record<string, never> for empty structs
        assert!(ts_definition.contains("export type EmptyStruct = Record<string, never>;"));
    }
} 
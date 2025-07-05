use core_model_macros::model_schema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

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
    fn test_collections_ts_definition() {
        let ts_definition = UserWithCollections::ts_definition();
        
        // Check TypeScript array types
        assert!(ts_definition.contains("tags: Array<string>;"));
        assert!(ts_definition.contains("scores: Array<number>;"));
        // HashMap becomes Partial<Record<...>> in the generated output
        assert!(ts_definition.contains("metadata: Partial<Record<string, string>>;"));
        
        // Check Zod schema
        assert!(ts_definition.contains("tags: z.array(z.string())"));
        assert!(ts_definition.contains("scores: z.array(z.number().int())"));
        assert!(ts_definition.contains("metadata: z.record(z.string(), z.string())"));
    }

    // Test struct with serde attributes
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
    #[serde(rename_all = "camelCase")]
    struct UserWithSerde {
        user_id: String,
        first_name: String,
        last_name: String,
        #[serde(rename = "emailAddress")]
        email: String,
        created_at: String,
        is_verified: bool,
    }

    #[test]
    fn test_serde_attributes_json_schema() {
        let schema = UserWithSerde::json_schema();
        
        let properties = schema["properties"].as_object().unwrap();
        
        // Check that field names are converted to camelCase
        assert!(properties.contains_key("userId"));
        assert!(properties.contains_key("firstName"));
        assert!(properties.contains_key("lastName"));
        assert!(properties.contains_key("emailAddress")); // Custom rename
        assert!(properties.contains_key("createdAt"));
        assert!(properties.contains_key("isVerified"));
        
        // Check that snake_case names are NOT present
        assert!(!properties.contains_key("user_id"));
        assert!(!properties.contains_key("first_name"));
        assert!(!properties.contains_key("last_name"));
        assert!(!properties.contains_key("email"));
        assert!(!properties.contains_key("created_at"));
        assert!(!properties.contains_key("is_verified"));
    }

    #[test]
    fn test_serde_attributes_ts_definition() {
        let ts_definition = UserWithSerde::ts_definition();
        
        // Check that field names are converted in TypeScript
        assert!(ts_definition.contains("userId: string;"));
        assert!(ts_definition.contains("firstName: string;"));
        assert!(ts_definition.contains("lastName: string;"));
        assert!(ts_definition.contains("emailAddress: string;"));
        assert!(ts_definition.contains("createdAt: string;"));
        assert!(ts_definition.contains("isVerified: boolean;"));
        
        // Check Zod schema
        assert!(ts_definition.contains("userId: z.string()"));
        assert!(ts_definition.contains("firstName: z.string()"));
        assert!(ts_definition.contains("lastName: z.string()"));
        assert!(ts_definition.contains("emailAddress: z.string()"));
        assert!(ts_definition.contains("createdAt: z.string()"));
        assert!(ts_definition.contains("isVerified: z.boolean()"));
    }

    // Test plain enum
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
    #[serde(rename_all = "lowercase")]
    enum UserStatus {
        Active,
        Inactive,
        Pending,
        Suspended,
    }

    #[test]
    fn test_plain_enum_json_schema() {
        let schema = UserStatus::json_schema();
        
        assert_eq!(schema["type"], "string");
        
        let enum_values = schema["enum"].as_array().unwrap();
        assert_eq!(enum_values.len(), 4);
        assert!(enum_values.contains(&Value::String("active".to_string())));
        assert!(enum_values.contains(&Value::String("inactive".to_string())));
        assert!(enum_values.contains(&Value::String("pending".to_string())));
        assert!(enum_values.contains(&Value::String("suspended".to_string())));
    }

    #[test]
    fn test_plain_enum_ts_definition() {
        let ts_definition = UserStatus::ts_definition();
        
        // Check TypeScript union type
        assert!(ts_definition.contains("export type UserStatus = "));
        assert!(ts_definition.contains("\"active\" | \"inactive\" | \"pending\" | \"suspended\""));
        
        // Check Zod schema
        assert!(ts_definition.contains("export const UserStatus$Schema"));
        assert!(ts_definition.contains("z.enum([\"active\", \"inactive\", \"pending\", \"suspended\"])"));
    }

    #[test]
    fn test_plain_enum_members() {
        let members = UserStatus::enum_members();
        assert_eq!(members.len(), 4);
        assert!(members.contains(&"active".to_string()));
        assert!(members.contains(&"inactive".to_string()));
        assert!(members.contains(&"pending".to_string()));
        assert!(members.contains(&"suspended".to_string()));
    }

    // Test discriminated union (tagged enum)
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
    #[serde(tag = "type", rename_all = "camelCase")]
    enum PaymentMethod {
        CreditCard {
            card_number: String,
            expiry_date: String,
            cvv: String,
        },
        BankTransfer {
            account_number: String,
            routing_number: String,
        },
        PayPal {
            email: String,
        },
    }

    #[test]
    fn test_discriminated_union_json_schema() {
        let schema = PaymentMethod::json_schema();
        
        assert_eq!(schema["type"], "object");
        assert!(schema.get("oneOf").is_some());
        
        let one_of = schema["oneOf"].as_array().unwrap();
        assert_eq!(one_of.len(), 3);
        
        // Check that each variant has the discriminator field
        for variant in one_of {
            let properties = variant["properties"].as_object().unwrap();
            assert!(properties.contains_key("type"));
            assert_eq!(properties["type"]["type"], "string");
            assert!(properties["type"].get("const").is_some());
        }
    }

    #[test]
    fn test_discriminated_union_ts_definition() {
        let ts_definition = PaymentMethod::ts_definition();
        
        // Check that it contains discriminated union syntax
        assert!(ts_definition.contains("export type PaymentMethod = "));
        assert!(ts_definition.contains("type: \"creditCard\""));
        assert!(ts_definition.contains("type: \"bankTransfer\""));
        assert!(ts_definition.contains("type: \"payPal\""));
        
        // Check field names are converted to camelCase
        assert!(ts_definition.contains("cardNumber: string;"));
        assert!(ts_definition.contains("expiryDate: string;"));
        assert!(ts_definition.contains("accountNumber: string;"));
        assert!(ts_definition.contains("routingNumber: string;"));
        
        // Check Zod discriminated union
        assert!(ts_definition.contains("z.discriminatedUnion(\"type\""));
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
        
        // Check Zod schema references (without Json suffix)
        assert!(user_definition.contains("address: Address$Schema"));
        assert!(user_definition.contains("backup_addresses: z.array(Address$Schema)"));
        
        // Verify Address definition exists (without Json suffix in export type)
        assert!(address_definition.contains("export type Address = {"));
        assert!(address_definition.contains("street: string;"));
        assert!(address_definition.contains("city: string;"));
        assert!(address_definition.contains("zip_code: string;"));
    }
} 
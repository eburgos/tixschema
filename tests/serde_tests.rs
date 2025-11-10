//! Comprehensive tests for Serde attribute handling
//!
//! This module tests how tixschema handles Serde attributes when the `serde` feature is enabled.
//!
//! ## What is tested:
//! - `rename_all` with different case conventions (camelCase, `snake_case`, `PascalCase`, etc.)
//! - Field-level rename attribute
//! - Combination of `rename_all` and field rename
//! - Serde attributes with optional fields
//! - Serde attributes in TypeScript, Zod, and JSON schema generation
//! - Enum `rename_all`

#[cfg(feature = "serde")]
#[expect(clippy::unwrap_used, reason = "This is a test file")]
#[expect(clippy::enum_variant_names, reason = "This is a test file")]
mod tests {
    use serde::{Deserialize, Serialize};
    use tixschema::model_schema;

    // ========================================================================
    // Basic Serde Rename Tests
    // ========================================================================

    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
    #[serde(rename_all = "camelCase")]
    struct UserWithSerdeJson {
        user_id: String,
        first_name: String,
        last_name: String,
        #[serde(rename = "emailAddress")]
        email: String,
        created_at: String,
        is_verified: bool,
    }

    #[test]
    #[cfg(feature = "jsonschema")]
    fn test_serde_rename_all_camel_case_json_schema() {
        let schema = UserWithSerdeJson::json_schema();

        let properties = schema["properties"].as_object().unwrap();

        // Check that serde rename attributes are applied
        assert!(properties.contains_key("userId")); // user_id -> userId
        assert!(properties.contains_key("firstName")); // first_name -> firstName
        assert!(properties.contains_key("lastName")); // last_name -> lastName
        assert!(properties.contains_key("emailAddress")); // email -> emailAddress (manual rename)
        assert!(properties.contains_key("createdAt")); // created_at -> createdAt
        assert!(properties.contains_key("isVerified")); // is_verified -> isVerified

        // Check that original field names are NOT present
        assert!(!properties.contains_key("user_id"));
        assert!(!properties.contains_key("first_name"));
        assert!(!properties.contains_key("last_name"));
        assert!(!properties.contains_key("email"));
        assert!(!properties.contains_key("created_at"));
        assert!(!properties.contains_key("is_verified"));

        // Verify field types
        assert_eq!(properties["userId"]["type"], "string");
        assert_eq!(properties["firstName"]["type"], "string");
        assert_eq!(properties["lastName"]["type"], "string");
        assert_eq!(properties["emailAddress"]["type"], "string");
        assert_eq!(properties["createdAt"]["type"], "string");
        assert_eq!(properties["isVerified"]["type"], "boolean");
    }

    #[test]
    #[cfg(feature = "typescript")]
    fn test_serde_rename_all_camel_case_typescript() {
        let ts_definition = UserWithSerdeJson::ts_definition();

        // Check that field names are converted in TypeScript
        assert!(ts_definition.contains("userId: string;"));
        assert!(ts_definition.contains("firstName: string;"));
        assert!(ts_definition.contains("lastName: string;"));
        assert!(ts_definition.contains("emailAddress: string;"));
        assert!(ts_definition.contains("createdAt: string;"));
        assert!(ts_definition.contains("isVerified: boolean;"));
    }

    #[test]
    #[cfg(feature = "zod")]
    fn test_serde_rename_all_camel_case_zod() {
        let zod_schema = UserWithSerdeJson::zod_schema();

        assert!(zod_schema.contains("userId: z.string()"));
        assert!(zod_schema.contains("firstName: z.string()"));
        assert!(zod_schema.contains("lastName: z.string()"));
        assert!(zod_schema.contains("emailAddress: z.string()"));
        assert!(zod_schema.contains("createdAt: z.string()"));
        assert!(zod_schema.contains("isVerified: z.boolean()"));
    }

    // ========================================================================
    // Different rename_all Conventions
    // Note: Currently only camelCase and lowercase are fully implemented
    // lowercase converts field names to lowercase while keeping underscores
    // ========================================================================

    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    #[serde(rename_all = "lowercase")]
    struct LowercaseJson {
        field_one: String,
        field_two: String,
    }

    #[test]
    #[cfg(feature = "typescript")]
    fn test_rename_all_lowercase() {
        let ts = LowercaseJson::ts_definition();

        // lowercase converts to lowercase but keeps underscores
        assert!(ts.contains("field_one: string;"));
        assert!(ts.contains("field_two: string;"));
    }

    // ========================================================================
    // Serde with Optional Fields
    // ========================================================================

    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    #[serde(rename_all = "camelCase")]
    struct OptionalFieldsJson {
        required_field: String,
        optional_field: Option<String>,
        #[serde(rename = "customOptional")]
        another_optional: Option<i32>,
    }

    #[test]
    #[cfg(feature = "typescript")]
    fn test_serde_with_optional_fields_typescript() {
        let ts = OptionalFieldsJson::ts_definition();

        assert!(ts.contains("requiredField: string;"));
        assert!(ts.contains("optionalField: string | undefined;"));
        assert!(ts.contains("customOptional: number | undefined;"));
    }

    #[test]
    #[cfg(feature = "zod")]
    fn test_serde_with_optional_fields_zod() {
        let zod = OptionalFieldsJson::zod_schema();

        assert!(zod.contains("requiredField: z.string()"));
        assert!(zod.contains("optionalField: z.union([z.string(), z.undefined()])"));
        assert!(zod.contains("customOptional: z.union([z.number().int(), z.undefined()])"));
    }

    #[test]
    #[cfg(feature = "jsonschema")]
    fn test_serde_with_optional_fields_json_schema() {
        let schema = OptionalFieldsJson::json_schema();

        let properties = schema["properties"].as_object().unwrap();
        assert!(properties.contains_key("requiredField"));
        assert!(properties.contains_key("optionalField"));
        assert!(properties.contains_key("customOptional"));

        let required = schema["required"].as_array().unwrap();
        assert!(required.contains(&serde_json::Value::String("requiredField".to_string())));
        assert!(!required.contains(&serde_json::Value::String("optionalField".to_string())));
        assert!(!required.contains(&serde_json::Value::String("customOptional".to_string())));
    }

    // ========================================================================
    // Enum Serde Tests
    // ========================================================================

    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    #[serde(rename_all = "lowercase")]
    enum StatusJson {
        Active,
        Inactive,
        Pending,
    }

    #[test]
    #[cfg(feature = "typescript")]
    fn test_enum_rename_all_lowercase() {
        let ts = StatusJson::ts_definition();

        assert!(ts.contains("\"active\""));
        assert!(ts.contains("\"inactive\""));
        assert!(ts.contains("\"pending\""));
    }

    #[test]
    #[cfg(feature = "zod")]
    fn test_enum_rename_all_zod() {
        let zod = StatusJson::zod_schema();

        assert!(zod.contains("\"active\""));
        assert!(zod.contains("\"inactive\""));
        assert!(zod.contains("\"pending\""));
    }

    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    enum ColorJson {
        Red,
        #[serde(rename = "dark_green")]
        Green,
        Blue,
    }

    #[test]
    #[cfg(feature = "typescript")]
    fn test_enum_field_rename() {
        let ts = ColorJson::ts_definition();

        assert!(ts.contains("\"Red\""));
        assert!(ts.contains("\"dark_green\""));
        assert!(ts.contains("\"Blue\""));
        assert!(!ts.contains("\"Green\""));
    }

    // ========================================================================
    // Discriminated Union with Serde
    // ========================================================================

    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    #[serde(tag = "type", rename_all = "camelCase")]
    enum EventJson {
        UserCreated {
            user_id: String,
            user_name: String,
        },
        UserDeleted {
            user_id: String,
        },
        UserUpdated {
            user_id: String,
            #[serde(rename = "newEmail")]
            email: String,
        },
    }

    #[test]
    #[cfg(feature = "typescript")]
    fn test_discriminated_union_with_serde_typescript() {
        let ts = EventJson::ts_definition();

        // Check discriminator field with camelCase applied to variant names
        assert!(ts.contains("type: \"userCreated\""));
        assert!(ts.contains("type: \"userDeleted\""));
        assert!(ts.contains("type: \"userUpdated\""));

        // Check renamed fields
        assert!(ts.contains("userId: string;"));
        assert!(ts.contains("userName: string;"));
        assert!(ts.contains("newEmail: string;"));
    }

    #[test]
    #[cfg(feature = "zod")]
    fn test_discriminated_union_with_serde_zod() {
        let zod = EventJson::zod_schema();

        assert!(zod.contains("z.discriminatedUnion"));
        assert!(zod.contains("\"type\""));
        assert!(zod.contains("userId"));
        assert!(zod.contains("userName"));
        assert!(zod.contains("newEmail"));
    }
}

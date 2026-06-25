use serde::{Deserialize, Serialize};
use tixschema::model_schema;

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
enum Color {
    Blue,
    #[serde(rename = "dark_green")]
    Green,
    Red,
}

// ========================================================================
// Discriminated Union with Serde
// ========================================================================

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "camelCase")]
enum Event {
    #[serde(rename = "userCreated")]
    Created {
        user_id: String,
        user_name: String,
    },
    #[serde(rename = "userDeleted")]
    Deleted {
        user_id: String,
    },
    #[serde(rename = "userUpdated")]
    Updated {
        #[serde(rename = "newEmail")]
        email: String,
        user_id: String,
    },
}

// ========================================================================
// Different rename_all Conventions
// Note: Currently only camelCase and lowercase are fully implemented
// lowercase converts field names to lowercase while keeping underscores
// ========================================================================

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "lowercase")]
struct Lowercase {
    field_one: String,
    field_two: String,
}

// ========================================================================
// Serde with Optional Fields
// ========================================================================

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct OptionalFields {
    #[serde(rename = "customOptional")]
    another_optional: Option<i32>,
    optional_field: Option<String>,
    required_field: String,
}

// ========================================================================
// Enum Serde Tests
// ========================================================================

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "lowercase")]
enum Status {
    Active,
    Inactive,
    Pending,
}

// ========================================================================
// Basic Serde Rename Tests
// ========================================================================

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
struct UserWithSerde {
    created_at: String,
    #[serde(rename = "emailAddress")]
    email: String,
    first_name: String,
    is_verified: bool,
    last_name: String,
    user_id: String,
}

#[test]
#[cfg(feature = "jsonschema")]
fn test_serde_rename_all_camel_case_json_schema() {
    let schema = UserWithSerde::json_schema();

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
    let ts_definition = UserWithSerde::ts_definition();

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
    let zod_schema = UserWithSerde::zod_schema();

    assert!(zod_schema.contains("userId: z.string()"));
    assert!(zod_schema.contains("firstName: z.string()"));
    assert!(zod_schema.contains("lastName: z.string()"));
    assert!(zod_schema.contains("emailAddress: z.string()"));
    assert!(zod_schema.contains("createdAt: z.string()"));
    assert!(zod_schema.contains("isVerified: z.boolean()"));
}

#[test]
#[cfg(feature = "typescript")]
fn test_rename_all_lowercase() {
    let ts = Lowercase::ts_definition();

    // lowercase converts to lowercase but keeps underscores
    assert!(ts.contains("field_one: string;"));
    assert!(ts.contains("field_two: string;"));
}

#[test]
#[cfg(feature = "typescript")]
fn test_serde_with_optional_fields_typescript() {
    let ts = OptionalFields::ts_definition();

    assert!(ts.contains("requiredField: string;"));
    assert!(ts.contains("optionalField: string | undefined;"));
    assert!(ts.contains("customOptional: number | undefined;"));
}

#[test]
#[cfg(feature = "zod")]
fn test_serde_with_optional_fields_zod() {
    let zod = OptionalFields::zod_schema();

    assert!(zod.contains("requiredField: z.string()"));
    assert!(zod.contains("optionalField: z.union([z.string(), z.undefined()])"));
    assert!(zod.contains("customOptional: z.union([z.number().int(), z.undefined()])"));
}

#[test]
#[cfg(feature = "jsonschema")]
fn test_serde_with_optional_fields_json_schema() {
    let schema = OptionalFields::json_schema();

    let properties = schema["properties"].as_object().unwrap();
    assert!(properties.contains_key("requiredField"));
    assert!(properties.contains_key("optionalField"));
    assert!(properties.contains_key("customOptional"));

    let required = schema["required"].as_array().unwrap();
    assert!(required.contains(&serde_json::Value::String("requiredField".to_owned())));
    assert!(!required.contains(&serde_json::Value::String("optionalField".to_owned())));
    assert!(!required.contains(&serde_json::Value::String("customOptional".to_owned())));
}

#[test]
#[cfg(feature = "typescript")]
fn test_enum_rename_all_lowercase() {
    let ts = Status::ts_definition();

    assert!(ts.contains("\"active\""));
    assert!(ts.contains("\"inactive\""));
    assert!(ts.contains("\"pending\""));
}

#[test]
#[cfg(feature = "zod")]
fn test_enum_rename_all_zod() {
    let zod = Status::zod_schema();

    assert!(zod.contains("\"active\""));
    assert!(zod.contains("\"inactive\""));
    assert!(zod.contains("\"pending\""));
}

#[test]
#[cfg(feature = "typescript")]
fn test_enum_field_rename() {
    let ts = Color::ts_definition();

    assert!(ts.contains("\"Red\""));
    assert!(ts.contains("\"dark_green\""));
    assert!(ts.contains("\"Blue\""));
    assert!(!ts.contains("\"Green\""));
}

#[test]
#[cfg(feature = "typescript")]
fn test_discriminated_union_with_serde_typescript() {
    let ts = Event::ts_definition();

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
    let zod = Event::zod_schema();

    assert!(zod.contains("z.discriminatedUnion"));
    assert!(zod.contains("\"type\""));
    assert!(zod.contains("userId"));
    assert!(zod.contains("userName"));
    assert!(zod.contains("newEmail"));
}

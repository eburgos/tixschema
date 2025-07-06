use tixschema::model_schema;
use serde::{Deserialize, Serialize};

#[cfg(test)]
mod tests {
    use super::*;

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
    #[cfg(all(feature = "jsonschema", feature = "serde"))]
    fn test_serde_attributes_json_schema() {
        let schema = UserWithSerde::json_schema();
        
        let properties = schema["properties"].as_object().unwrap();
        
        // Check that serde rename attributes are applied
        assert!(properties.contains_key("userId"));  // user_id -> userId
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
    #[cfg(all(feature = "typescript", feature = "serde", feature = "zod"))]
    fn test_serde_attributes_ts_definition() {
        let ts_definition = UserWithSerde::ts_definition();
        
        // Check that field names are converted in TypeScript
        assert!(ts_definition.contains("userId: string;"));
        assert!(ts_definition.contains("firstName: string;"));
        assert!(ts_definition.contains("lastName: string;"));
        assert!(ts_definition.contains("emailAddress: string;"));
        assert!(ts_definition.contains("createdAt: string;"));
        assert!(ts_definition.contains("isVerified: boolean;"));
        
        // Check Zod schema - now in separate method
        let zod_schema = UserWithSerde::zod_schema();
        assert!(zod_schema.contains("userId: z.string()"));
        assert!(zod_schema.contains("firstName: z.string()"));
        assert!(zod_schema.contains("lastName: z.string()"));
        assert!(zod_schema.contains("emailAddress: z.string()"));
        assert!(zod_schema.contains("createdAt: z.string()"));
        assert!(zod_schema.contains("isVerified: z.boolean()"));
    }
} 
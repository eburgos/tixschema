#[cfg(test)]
mod tests {
    #[cfg(all(test, feature = "serde"))]
    use serde::{Deserialize, Serialize};
    #[cfg(all(test, any(feature = "jsonschema")))]
    use serde_json::Value;
    #[cfg(all(
        test,
        any(feature = "typescript", feature = "jsonschema", feature = "zod", feature = "serde")
    ))]
    use tixschema::{model_schema, model_schema_prop};

    // Test the example from the user request
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
    struct AccountContextJson {
        pub aud: String,
        pub exp: i64,
        pub iat: i64,
        #[model_schema_prop(as = String, literal = "ProDoctivity")]
        pub iss: String,
        pub nbf: i64,
        #[model_schema_prop(as = String, minLength = 1)]
        pub sub: String,
        #[model_schema_prop(as = String, minLength = 1)]
        pub jti: String,
    }

    #[test]
    #[cfg(feature = "typescript")]
    fn test_string_literal_typescript() {
        let ts_definition = AccountContextJson::ts_definition();
        
        // Check that the literal field generates the correct TypeScript type
        assert!(ts_definition.contains("iss: \"ProDoctivity\";"));
        
        // Check that other fields are still normal string types
        assert!(ts_definition.contains("aud: string;"));
        assert!(ts_definition.contains("sub: string;"));
        assert!(ts_definition.contains("jti: string;"));
        
        // Check that numeric fields are still numbers
        assert!(ts_definition.contains("exp: number;"));
        assert!(ts_definition.contains("iat: number;"));
        assert!(ts_definition.contains("nbf: number;"));
        
        // Check that minLength fields have documentation
        assert!(ts_definition.contains("Minimum length: 1"));
    }

    #[test]
    #[cfg(feature = "zod")]
    fn test_string_literal_zod() {
        let zod_schema = AccountContextJson::zod_schema();
        
        // Check that the literal field generates the correct Zod schema
        assert!(zod_schema.contains("iss: z.literal(\"ProDoctivity\")"));
        
        // Check that other fields are still normal string schemas
        assert!(zod_schema.contains("aud: z.string()"));
        
        // Check that minLength fields have the correct validation
        assert!(zod_schema.contains("sub: z.string().min(1)"));
        assert!(zod_schema.contains("jti: z.string().min(1)"));
        
        // Check that numeric fields use correct Zod types
        assert!(zod_schema.contains("exp: z.number().int()"));
        assert!(zod_schema.contains("iat: z.number().int()"));
        assert!(zod_schema.contains("nbf: z.number().int()"));
    }

    #[test]
    #[cfg(feature = "jsonschema")]
    fn test_string_literal_json_schema() {
        let schema = AccountContextJson::json_schema();
        let properties = schema["properties"].as_object().unwrap();
        
        // Check that the literal field has the correct JSON schema
        let iss_prop = &properties["iss"];
        assert_eq!(iss_prop["type"], "string");
        assert_eq!(iss_prop["const"], "ProDoctivity");
        
        // Check that other string fields are normal strings without const
        let aud_prop = &properties["aud"];
        assert_eq!(aud_prop["type"], "string");
        assert!(aud_prop.get("const").is_none());
        assert!(aud_prop.get("minLength").is_none());
        
        // Check that minLength fields have the correct validation
        let sub_prop = &properties["sub"];
        assert_eq!(sub_prop["type"], "string");
        assert!(sub_prop.get("const").is_none());
        assert_eq!(sub_prop["minLength"], 1);
        
        let jti_prop = &properties["jti"];
        assert_eq!(jti_prop["type"], "string");
        assert!(jti_prop.get("const").is_none());
        assert_eq!(jti_prop["minLength"], 1);
    }

    // Test multiple literal values in one struct
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
    struct MultipleLiteralsJson {
        pub id: String,
        #[model_schema_prop(literal = "fixed_type")]
        pub type_field: String,
        #[model_schema_prop(literal = "v1.0")]
        pub version: String,
        pub name: String,
    }

    #[test]
    #[cfg(feature = "typescript")]
    fn test_multiple_literals_typescript() {
        let ts_definition = MultipleLiteralsJson::ts_definition();
        
        // Check multiple literals
        assert!(ts_definition.contains("type_field: \"fixed_type\";"));
        assert!(ts_definition.contains("version: \"v1.0\";"));
        
        // Check normal fields
        assert!(ts_definition.contains("id: string;"));
        assert!(ts_definition.contains("name: string;"));
    }

    #[test]
    #[cfg(feature = "zod")]
    fn test_multiple_literals_zod() {
        let zod_schema = MultipleLiteralsJson::zod_schema();
        
        // Check multiple literals
        assert!(zod_schema.contains("type_field: z.literal(\"fixed_type\")"));
        assert!(zod_schema.contains("version: z.literal(\"v1.0\")"));
        
        // Check normal fields
        assert!(zod_schema.contains("id: z.string()"));
        assert!(zod_schema.contains("name: z.string()"));
    }

    // Test optional literal fields
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
    struct OptionalLiteralJson {
        pub id: String,
        #[model_schema_prop(literal = "optional_literal")]
        pub optional_type: Option<String>,
    }

    #[test]
    #[cfg(feature = "typescript")]
    fn test_optional_literal_typescript() {
        let ts_definition = OptionalLiteralJson::ts_definition();
        
        // Check that optional literal works correctly
        assert!(ts_definition.contains("optional_type: \"optional_literal\" | undefined;"));
        assert!(ts_definition.contains("id: string;"));
    }

    #[test]
    #[cfg(feature = "zod")]
    fn test_optional_literal_zod() {
        let zod_schema = OptionalLiteralJson::zod_schema();
        
        // Check that optional literal works correctly
        assert!(zod_schema.contains("optional_type: z.literal(\"optional_literal\").or(z.undefined())"));
        assert!(zod_schema.contains("id: z.string()"));
    }

    // Test array of literals
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
    struct ArrayLiteralJson {
        pub id: String,
        #[model_schema_prop(literal = "array_item")]
        pub literal_array: Vec<String>,
    }

    #[test]
    #[cfg(feature = "typescript")]
    fn test_array_literal_typescript() {
        let ts_definition = ArrayLiteralJson::ts_definition();
        
        // Check that array of literals works correctly
        assert!(ts_definition.contains("literal_array: Array<\"array_item\">;"));
        assert!(ts_definition.contains("id: string;"));
    }

    #[test]
    #[cfg(feature = "zod")]
    fn test_array_literal_zod() {
        let zod_schema = ArrayLiteralJson::zod_schema();
        
        // Check that array of literals works correctly
        assert!(zod_schema.contains("literal_array: z.array(z.literal(\"array_item\"))"));
        assert!(zod_schema.contains("id: z.string()"));
    }

    #[test]
    #[cfg(feature = "jsonschema")]
    fn test_array_literal_json_schema() {
        let schema = ArrayLiteralJson::json_schema();
        let properties = schema["properties"].as_object().unwrap();
        
        // Check that array of literals has correct JSON schema
        let literal_array_prop = &properties["literal_array"];
        assert_eq!(literal_array_prop["type"], "array");
        assert_eq!(literal_array_prop["items"]["type"], "string");
        assert_eq!(literal_array_prop["items"]["const"], "array_item");
    }

    // Test struct with comprehensive minLength configurations
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
    struct MinLengthTestJson {
        #[model_schema_prop(as = String, minLength = 1)]
        pub name: String,
        #[model_schema_prop(as = String, minLength = 5)]
        pub username: String,
        #[model_schema_prop(as = String, minLength = 10)]
        pub password: String,
        // Regular string without minLength
        pub description: String,
        // Optional string with minLength
        #[model_schema_prop(as = String, minLength = 3)]
        pub nickname: Option<String>,
        // Array of strings with minLength on the items
        #[model_schema_prop(as = String, minLength = 2)]
        pub tags: Vec<String>,
    }

    #[test]
    #[cfg(feature = "typescript")]
    fn test_min_length_typescript() {
        let ts_definition = MinLengthTestJson::ts_definition();
        
        // Check that all fields have correct TypeScript types
        assert!(ts_definition.contains("name: string;"));
        assert!(ts_definition.contains("username: string;"));
        assert!(ts_definition.contains("password: string;"));
        assert!(ts_definition.contains("description: string;"));
        assert!(ts_definition.contains("nickname: string | undefined;"));
        assert!(ts_definition.contains("tags: Array<string>;"));
        
        // Check that minLength documentation is present
        assert!(ts_definition.contains("Minimum length: 1"));
        assert!(ts_definition.contains("Minimum length: 5"));
        assert!(ts_definition.contains("Minimum length: 10"));
        assert!(ts_definition.contains("Minimum length: 3"));
        assert!(ts_definition.contains("Minimum length: 2"));
    }

    #[test]
    #[cfg(feature = "zod")]
    fn test_min_length_zod() {
        let zod_schema = MinLengthTestJson::zod_schema();
        
        // Check that minLength fields have correct validation
        assert!(zod_schema.contains("name: z.string().min(1)"));
        assert!(zod_schema.contains("username: z.string().min(5)"));
        assert!(zod_schema.contains("password: z.string().min(10)"));
        assert!(zod_schema.contains("tags: z.array(z.string().min(2))"));
        
        // Check that regular string field doesn't have minLength
        assert!(zod_schema.contains("description: z.string(),"));
        
        // Check that optional string with minLength works correctly
        assert!(zod_schema.contains("nickname: z.string().min(3).or(z.undefined())"));
    }

    #[test]
    #[cfg(feature = "jsonschema")]
    fn test_min_length_json_schema() {
        let schema = MinLengthTestJson::json_schema();
        let properties = schema["properties"].as_object().unwrap();
        
        // Check that minLength fields have the correct JSON schema
        let name_prop = &properties["name"];
        assert_eq!(name_prop["type"], "string");
        assert_eq!(name_prop["minLength"], 1);
        
        let username_prop = &properties["username"];
        assert_eq!(username_prop["type"], "string");
        assert_eq!(username_prop["minLength"], 5);
        
        let password_prop = &properties["password"];
        assert_eq!(password_prop["type"], "string");
        assert_eq!(password_prop["minLength"], 10);
        
        let nickname_prop = &properties["nickname"];
        assert_eq!(nickname_prop["type"], "string");
        assert_eq!(nickname_prop["minLength"], 3);
        
        // Check that regular string field doesn't have minLength
        let description_prop = &properties["description"];
        assert_eq!(description_prop["type"], "string");
        assert!(description_prop.get("minLength").is_none());
        
        // Check that array field has minLength on items
        let tags_prop = &properties["tags"];
        assert_eq!(tags_prop["type"], "array");
        assert_eq!(tags_prop["items"]["type"], "string");
        assert_eq!(tags_prop["items"]["minLength"], 2);
    }

    // Test combining literal and minLength (should prioritize literal)
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
    struct CombinedTestJson {
        #[model_schema_prop(as = String, literal = "fixed", minLength = 10)]
        pub fixed_field: String,
        #[model_schema_prop(as = String, minLength = 1)]
        pub normal_field: String,
    }

    #[test]
    #[cfg(feature = "typescript")]
    fn test_combined_literal_minlength_typescript() {
        let ts_definition = CombinedTestJson::ts_definition();
        
        // Literal should take precedence - should be a literal type, not a string with minLength
        assert!(ts_definition.contains("fixed_field: \"fixed\";"));
        assert!(ts_definition.contains("normal_field: string;"));
        
        // Should still have minLength documentation for the normal field
        assert!(ts_definition.contains("Minimum length: 1"));
    }

    #[test]
    #[cfg(feature = "zod")]
    fn test_combined_literal_minlength_zod() {
        let zod_schema = CombinedTestJson::zod_schema();
        
        // Literal should take precedence - should be a literal, not a string with minLength
        assert!(zod_schema.contains("fixed_field: z.literal(\"fixed\")"));
        assert!(zod_schema.contains("normal_field: z.string().min(1)"));
    }

    #[test]
    #[cfg(feature = "jsonschema")]
    fn test_combined_literal_minlength_json_schema() {
        let schema = CombinedTestJson::json_schema();
        let properties = schema["properties"].as_object().unwrap();
        
        // Literal should take precedence
        let fixed_prop = &properties["fixed_field"];
        assert_eq!(fixed_prop["type"], "string");
        assert_eq!(fixed_prop["const"], "fixed");
        assert!(fixed_prop.get("minLength").is_none()); // Should not have minLength when literal
        
        // Normal field should have minLength
        let normal_prop = &properties["normal_field"];
        assert_eq!(normal_prop["type"], "string");
        assert_eq!(normal_prop["minLength"], 1);
        assert!(normal_prop.get("const").is_none());
    }
} 
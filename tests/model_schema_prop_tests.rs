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
        pub sub: String,
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
    }

    #[test]
    #[cfg(feature = "zod")]
    fn test_string_literal_zod() {
        let zod_schema = AccountContextJson::zod_schema();
        
        // Check that the literal field generates the correct Zod schema
        assert!(zod_schema.contains("iss: z.literal(\"ProDoctivity\")"));
        
        // Check that other fields are still normal string schemas
        assert!(zod_schema.contains("aud: z.string()"));
        assert!(zod_schema.contains("sub: z.string()"));
        assert!(zod_schema.contains("jti: z.string()"));
        
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
        
        let sub_prop = &properties["sub"];
        assert_eq!(sub_prop["type"], "string");
        assert!(sub_prop.get("const").is_none());
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
} 
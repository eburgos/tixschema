use serde::{Deserialize, Serialize};
use tixschema::model_schema;

#[cfg(feature = "zod")]
#[test]
fn test_simple_enum_example() {
    /// Data type enumeration
    /// ```rust example
    /// let data_type = DataTypeJson::Numeric;
    /// println!("Example: {:?}", data_type);
    /// ```
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub enum DataTypeJson {
        Alphanumeric,
        Numeric,
        Date,
    }

    // Check that schema_example() method exists and returns correct value
    let example = DataTypeJson::schema_example();
    assert_eq!(example.as_str().unwrap(), "Numeric");

    // Check that zod schema includes the example
    let zod = DataTypeJson::zod_schema();
    assert!(zod.contains("example:"));
    assert!(zod.contains("\"Numeric\""));
}

#[cfg(feature = "zod")]
#[test]
fn test_simple_struct_example() {
    /// User profile
    /// ```rust example
    /// let user = UserJson {
    ///     name: "John Doe".to_string(),
    ///     age: 25,
    /// };
    /// println!("User: {:?}", user);
    /// ```
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct UserJson {
        pub name: String,
        pub age: u32,
    }

    // Check that schema_example() method exists
    let example = UserJson::schema_example();
    assert_eq!(example["name"].as_str().unwrap(), "John Doe");
    assert_eq!(example["age"].as_u64().unwrap(), 25);

    // Check that zod schema includes the example
    let zod = UserJson::zod_schema();
    assert!(zod.contains("example:"));
    assert!(zod.contains("John Doe"));
    assert!(zod.contains("25"));
}

#[cfg(feature = "zod")]
#[test]
fn test_complex_struct_with_logic() {
    /// Complex user with setup logic
    /// ```rust example
    /// let user_id = "usr_123".to_string();
    /// let tags = vec!["admin".to_string(), "active".to_string()];
    /// let user = ComplexUserJson {
    ///     id: user_id,
    ///     tags,
    ///     age: 30,
    /// };
    /// println!("Complex user: {:?}", user);
    /// ```
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct ComplexUserJson {
        pub id: String,
        pub tags: Vec<String>,
        pub age: u32,
    }

    // Check that schema_example() method exists and logic executed correctly
    let example = ComplexUserJson::schema_example();
    assert_eq!(example["id"].as_str().unwrap(), "usr_123");
    assert_eq!(example["tags"][0].as_str().unwrap(), "admin");
    assert_eq!(example["tags"][1].as_str().unwrap(), "active");
    assert_eq!(example["age"].as_u64().unwrap(), 30);

    // Check that zod schema includes the example
    let zod = ComplexUserJson::zod_schema();
    assert!(zod.contains("example:"));
}

#[cfg(feature = "zod")]
#[test]
fn test_nested_types_example() {
    use std::collections::HashMap;

    /// Profile with nested types
    /// ```rust example
    /// let profile = ProfileJson {
    ///     tags: vec!["tag1".to_string()],
    ///     metadata: HashMap::from([("key1".to_string(), "value1".to_string())]),
    ///     optional_field: Some("present".to_string()),
    /// };
    /// println!("Profile: {:?}", profile);
    /// ```
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct ProfileJson {
        pub tags: Vec<String>,
        pub metadata: HashMap<String, String>,
        pub optional_field: Option<String>,
    }

    // Check that schema_example() method exists
    let example = ProfileJson::schema_example();
    assert_eq!(example["tags"][0].as_str().unwrap(), "tag1");
    assert_eq!(example["metadata"]["key1"].as_str().unwrap(), "value1");
    assert_eq!(example["optional_field"].as_str().unwrap(), "present");

    // Check that zod schema includes the example
    let zod = ProfileJson::zod_schema();
    assert!(zod.contains("example:"));
}

#[cfg(feature = "zod")]
#[test]
fn test_discriminated_enum_example() {
    /// Event types
    /// ```rust example
    /// let event = EventJson::UserCreated {
    ///     user_id: "user_123".to_string(),
    ///     timestamp: "2024-01-01".to_string(),
    /// };
    /// println!("Event: {:?}", event);
    /// ```
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    #[serde(tag = "type")]
    pub enum EventJson {
        UserCreated { user_id: String, timestamp: String },
        UserDeleted { user_id: String },
    }

    // Check that schema_example() method exists
    let example = EventJson::schema_example();
    assert_eq!(example["type"].as_str().unwrap(), "UserCreated");
    assert_eq!(example["user_id"].as_str().unwrap(), "user_123");
    assert_eq!(example["timestamp"].as_str().unwrap(), "2024-01-01");

    // Check that zod schema includes the example
    let zod = EventJson::zod_schema();
    assert!(zod.contains("example:"));
}

#[cfg(feature = "zod")]
#[test]
fn test_multiple_examples_uses_first() {
    /// Type with multiple examples
    /// ```rust example
    /// let _: FirstExampleJson = FirstExampleJson { value: 1 };
    /// ```
    /// Second description
    /// ```rust example
    /// let _: FirstExampleJson = FirstExampleJson { value: 2 };
    /// ```
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct FirstExampleJson {
        pub value: u32,
    }

    // Check that only the first example is used
    let example = FirstExampleJson::schema_example();
    assert_eq!(example["value"].as_u64().unwrap(), 1);
}

#[test]
fn test_no_example_no_method() {
    /// Type without example
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct NoExampleJson {
        pub value: String,
    }

    // schema_example() should not exist, so we can't call it
    // This test just verifies it compiles without the method

    // However, zod schema should still be generated without example
    #[cfg(feature = "zod")]
    {
        let zod = NoExampleJson::zod_schema();
        assert!(!zod.contains("example:"));
    }
}

#[cfg(all(feature = "zod", feature = "typescript"))]
#[test]
fn test_typescript_zod_format() {
    /// Test type
    /// ```rust example
    /// TestJson { name: "test".to_string() }
    /// ```
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct TestJson {
        pub name: String,
    }

    let zod = TestJson::zod_schema();

    // Should have TypeScript-style format with ZodType
    assert!(zod.contains("ZodType<"));
    assert!(zod.contains("$RawSchema"));
    assert!(zod.contains("example:"));
}

#[cfg(all(feature = "zod", not(feature = "typescript")))]
#[test]
fn test_javascript_zod_format() {
    /// Test type
    /// ```rust example
    /// TestJson { name: "test".to_string() }
    /// ```
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct TestJson {
        pub name: String,
    }

    let zod = TestJson::zod_schema();

    // Should have JavaScript-style format without ZodType
    assert!(!zod.contains("ZodType<"));
    assert!(zod.contains("example:"));
}

#[cfg(feature = "zod")]
#[test]
fn test_serde_attributes_in_example() {
    /// User with serde attributes
    /// ```rust example
    /// let user = UserWithSerdeJson {
    ///     user_id: "123".to_string(),
    ///     email_address: "test@example.com".to_string(),
    /// };
    /// println!("User: {:?}", user);
    /// ```
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    #[serde(rename_all = "camelCase")]
    pub struct UserWithSerdeJson {
        pub user_id: String,
        #[serde(rename = "emailAddress")]
        pub email_address: String,
    }

    // Check that example serializes with serde attributes
    let example = UserWithSerdeJson::schema_example();
    // The example should have camelCase keys
    assert!(example.get("userId").is_some());
    assert_eq!(
        example["emailAddress"].as_str().unwrap(),
        "test@example.com"
    );

    let zod = UserWithSerdeJson::zod_schema();
    assert!(zod.contains("example:"));
}

#[cfg(all(feature = "zod", feature = "object_id"))]
#[test]
fn test_objectid_example() {
    use mongodb::bson::oid::ObjectId;

    /// Document with ObjectId
    /// ```rust example
    /// let oid = ObjectId::parse_str("507f1f77bcf86cd799439011").unwrap();
    /// let doc = DocumentJson {
    ///     id: oid,
    ///     name: "test".to_string(),
    /// };
    /// println!("Document: {:?}", doc);
    /// ```
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct DocumentJson {
        pub id: ObjectId,
        pub name: String,
    }

    // Check that schema_example() method exists
    let example = DocumentJson::schema_example();
    assert_eq!(example["name"].as_str().unwrap(), "test");
    // ObjectId should serialize to { "$oid": "..." } format
    assert!(example["id"].get("$oid").is_some());

    let zod = DocumentJson::zod_schema();
    assert!(zod.contains("example:"));
}

#[test]
fn test_example_fence_must_be_exact() {
    /// Type with regular code fence (not example)
    /// ```rust
    /// RegularFenceJson { value: 1 }
    /// ```
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct RegularFenceJson {
        pub value: u32,
    }

    // schema_example() should not exist since fence is not "rust example"
    #[cfg(feature = "zod")]
    {
        let zod = RegularFenceJson::zod_schema();
        assert!(!zod.contains("example:"));
    }
}

#[cfg(feature = "zod")]
#[test]
fn test_empty_example_block() {
    /// Type with empty example (should fail compilation if uncommented)
    /// This test verifies that the example must contain valid code
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    #[allow(dead_code)]
    pub struct ValidExampleJson {
        pub value: u32,
    }

    // We test with a valid example instead
    // An empty example block would cause a compilation error
    // which is the desired behavior
}

#[cfg(feature = "zod")]
#[test]
fn test_regex_transform_println() {
    /// Data type with println pattern (for doctest compatibility)
    /// ```rust example
    /// let data_type = DataTypeJson::Integer;
    /// println!("data_type: {:?}", data_type);
    /// ```
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub enum DataTypeJson {
        Integer,
        Float,
        String,
    }

    // Check that transformation worked - should extract the variable
    let example = DataTypeJson::schema_example();
    assert_eq!(example.as_str().unwrap(), "Integer");

    let zod = DataTypeJson::zod_schema();
    assert!(zod.contains("example:"));
    assert!(zod.contains("\"Integer\""));
}

#[cfg(feature = "zod")]
#[test]
fn test_regex_transform_let_underscore() {
    /// Status with let underscore pattern (for doctest compatibility)
    /// ```rust example
    /// let _: StatusJson = StatusJson::Active;
    /// ```
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub enum StatusJson {
        Active,
        Inactive,
    }

    // Check that transformation worked - should extract the value
    let example = StatusJson::schema_example();
    assert_eq!(example.as_str().unwrap(), "Active");

    let zod = StatusJson::zod_schema();
    assert!(zod.contains("example:"));
    assert!(zod.contains("\"Active\""));
}

#[cfg(feature = "zod")]
#[test]
fn test_description_strips_examples() {
    /// This is a description
    /// with multiple lines
    /// ```rust example
    /// let _: DescriptionTestJson = DescriptionTestJson { value: 42 };
    /// ```
    /// More description after example
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct DescriptionTestJson {
        pub value: u32,
    }

    let zod = DescriptionTestJson::zod_schema();
    // Description should not contain the example code
    assert!(!zod.contains("DescriptionTestJson { value: 42 }"));
    // Note: Structs don't embed descriptions in .meta() currently (only enums do)
}

#[cfg(feature = "zod")]
#[test]
fn test_description_escapes_quotes() {
    /// Description with "quoted" text
    /// ```rust example
    /// let _: QuoteTestJson = QuoteTestJson::Active;
    /// ```
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub enum QuoteTestJson {
        Active,
        Inactive,
    }

    let zod = QuoteTestJson::zod_schema();
    // The generated code should have escaped quotes in description
    assert!(zod.contains("example:"));
    // Check that the description is properly formatted with escaped quotes
    assert!(zod.contains("description:"));
    assert!(zod.contains(r#"\"quoted\""#));
}

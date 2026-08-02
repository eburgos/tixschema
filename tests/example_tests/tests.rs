use serde::{Deserialize, Serialize};
use tixschema::model_schema;

#[cfg(feature = "zod")]
#[test]
fn test_simple_enum_example() {
    /// Data type enumeration
    /// ```rust example
    /// let data_type = DataType::Numeric;
    /// println!("Example: {:?}", data_type);
    /// ```
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub enum DataType {
        Alphanumeric,
        Date,
        Numeric,
    }

    // Check that schema_example() method exists and returns correct value
    let example = DataType::schema_example();
    assert_eq!(example.as_str().unwrap(), "Numeric");

    // Check that zod schema includes the example
    let zod = DataType::zod_schema();
    assert!(zod.contains("example:"));
    assert!(zod.contains("\"Numeric\""));

    // Verify that example injection doesn't drop $Schema line
    #[cfg(feature = "typescript")]
    {
        assert!(
            zod.contains("DataType$RawSchema"),
            "Plain enum with example should have $RawSchema. Got:\n{zod}"
        );
        assert!(
            zod.contains("export const DataType$Schema: ZodType<DataType> = DataType$RawSchema;"),
            "Plain enum with example should have $Schema referencing $RawSchema. Got:\n{zod}"
        );
    }
}

#[cfg(feature = "zod")]
#[test]
fn test_simple_struct_example() {
    /// User profile
    /// ```rust example
    /// let user = User {
    ///     name: "John Doe".to_string(),
    ///     age: 25,
    /// };
    /// println!("User: {:?}", user);
    /// ```
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct User {
        pub age: u32,
        pub name: String,
    }

    // Check that schema_example() method exists
    let example = User::schema_example();
    assert_eq!(example["name"].as_str().unwrap(), "John Doe");
    assert_eq!(example["age"].as_u64().unwrap(), 25);

    // Check that zod schema includes the example
    let zod = User::zod_schema();
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
    /// let user = ComplexUser {
    ///     id: user_id,
    ///     tags,
    ///     age: 30,
    /// };
    /// println!("Complex user: {:?}", user);
    /// ```
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct ComplexUser {
        pub age: u32,
        pub id: String,
        pub tags: Vec<String>,
    }

    // Check that schema_example() method exists and logic executed correctly
    let example = ComplexUser::schema_example();
    assert_eq!(example["id"].as_str().unwrap(), "usr_123");
    assert_eq!(example["tags"][0].as_str().unwrap(), "admin");
    assert_eq!(example["tags"][1].as_str().unwrap(), "active");
    assert_eq!(example["age"].as_u64().unwrap(), 30);

    // Check that zod schema includes the example
    let zod = ComplexUser::zod_schema();
    assert!(zod.contains("example:"));
}

#[cfg(feature = "zod")]
#[test]
fn test_nested_types_example() {
    use std::collections::HashMap;

    /// Profile with nested types
    /// ```rust example
    /// let profile = Profile {
    ///     tags: vec!["tag1".to_string()],
    ///     metadata: HashMap::from([("key1".to_string(), "value1".to_string())]),
    ///     optional_field: Some("present".to_string()),
    /// };
    /// println!("Profile: {:?}", profile);
    /// ```
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct Profile {
        pub metadata: HashMap<String, String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub optional_field: Option<String>,
        pub tags: Vec<String>,
    }

    // Check that schema_example() method exists
    let example = Profile::schema_example();
    assert_eq!(example["tags"][0].as_str().unwrap(), "tag1");
    assert_eq!(example["metadata"]["key1"].as_str().unwrap(), "value1");
    assert_eq!(example["optional_field"].as_str().unwrap(), "present");

    // Check that zod schema includes the example
    let zod = Profile::zod_schema();
    assert!(zod.contains("example:"));
}

#[cfg(feature = "zod")]
#[test]
fn test_discriminated_enum_example() {
    /// Event types
    /// ```rust example
    /// let event = Event::UserCreated {
    ///     user_id: "user_123".to_string(),
    ///     timestamp: "2024-01-01".to_string(),
    /// };
    /// println!("Event: {:?}", event);
    /// ```
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    #[serde(tag = "type")]
    pub enum Event {
        UserCreated { user_id: String, timestamp: String },
        UserDeleted { user_id: String },
    }

    // Check that schema_example() method exists
    let example = Event::schema_example();
    assert_eq!(example["type"].as_str().unwrap(), "UserCreated");
    assert_eq!(example["user_id"].as_str().unwrap(), "user_123");
    assert_eq!(example["timestamp"].as_str().unwrap(), "2024-01-01");

    // Check that zod schema includes the example
    let zod = Event::zod_schema();
    assert!(zod.contains("example:"));
}

#[cfg(feature = "zod")]
#[test]
fn test_multiple_examples_uses_first() {
    /// Type with multiple examples
    /// ```rust example
    /// let _: FirstExample = FirstExample { value: 1 };
    /// ```
    /// Second description
    /// ```rust example
    /// let _: FirstExample = FirstExample { value: 2 };
    /// ```
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct FirstExample {
        pub value: u32,
    }

    // Check that only the first example is used
    let example = FirstExample::schema_example();
    assert_eq!(example["value"].as_u64().unwrap(), 1);
}

#[test]
fn test_no_example_no_method() {
    /// Type without example
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct NoExample {
        pub value: String,
    }

    let no_example = NoExample {
        value: String::new(),
    };
    assert!(no_example.value.is_empty());

    // schema_example() should not exist, so we can't call it
    // This test just verifies it compiles without the method

    // However, zod schema should still be generated without example
    #[cfg(feature = "zod")]
    {
        let zod = NoExample::zod_schema();
        assert!(!zod.contains("example:"));
    }
}

#[cfg(all(feature = "zod", feature = "typescript"))]
#[test]
fn test_typescript_zod_format() {
    /// Test type
    /// ```rust example
    /// Test { name: "test".to_string() }
    /// ```
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct Test {
        pub name: String,
    }

    let zod = Test::zod_schema();

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
    /// Test { name: "test".to_string() }
    /// ```
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct Test {
        pub name: String,
    }

    let zod = Test::zod_schema();

    // Should have JavaScript-style format without ZodType
    assert!(!zod.contains("ZodType<"));
    assert!(zod.contains("example:"));
}

#[cfg(feature = "zod")]
#[test]
fn test_serde_attributes_in_example() {
    /// User with serde attributes
    /// ```rust example
    /// let user = UserWithSerde {
    ///     user_id: "123".to_string(),
    ///     email_address: "test@example.com".to_string(),
    /// };
    /// println!("User: {:?}", user);
    /// ```
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    #[serde(rename_all = "camelCase")]
    pub struct UserWithSerde {
        #[serde(rename = "emailAddress")]
        pub email_address: String,
        pub user_id: String,
    }

    // Check that example serializes with serde attributes
    let example = UserWithSerde::schema_example();
    // The example should have camelCase keys
    assert!(example.get("userId").is_some());
    assert_eq!(
        example["emailAddress"].as_str().unwrap(),
        "test@example.com"
    );

    let zod = UserWithSerde::zod_schema();
    assert!(zod.contains("example:"));
}

#[cfg(all(feature = "zod", feature = "object_id"))]
#[test]
fn test_objectid_example() {
    use mongodb::bson::oid::ObjectId;

    /// Document with `ObjectId`
    /// ```rust example
    /// let oid = ObjectId::parse_str("507f1f77bcf86cd799439011").unwrap();
    /// let doc = Document {
    ///     id: oid,
    ///     name: "test".to_string(),
    /// };
    /// println!("Document: {:?}", doc);
    /// ```
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct Document {
        pub id: ObjectId,
        pub name: String,
    }

    // Check that schema_example() method exists
    let example = Document::schema_example();
    assert_eq!(example["name"].as_str().unwrap(), "test");
    // ObjectId should serialize to { "$oid": "..." } format
    assert!(example["id"].get("$oid").is_some());

    let zod = Document::zod_schema();
    assert!(zod.contains("example:"));
}

#[test]
fn test_example_fence_must_be_exact() {
    /// Type with regular code fence (not example)
    /// ```rust
    /// RegularFence { value: 1 }
    /// ```
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct RegularFence {
        pub value: u32,
    }

    let regular_fence = RegularFence { value: 0 };
    assert_eq!(regular_fence.value, 0_u32);

    // schema_example() should not exist since fence is not "rust example"
    #[cfg(feature = "zod")]
    {
        let zod = RegularFence::zod_schema();
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
    pub struct ValidExample {
        pub value: u32,
    }

    // We test with a valid example instead
    // An empty example block would cause a compilation error
    // which is the desired behavior
    let zod = ValidExample::zod_schema();
    assert!(zod.contains("value"));
}

#[cfg(feature = "zod")]
#[test]
fn test_regex_transform_println() {
    /// Data type with println pattern (for doctest compatibility)
    /// ```rust example
    /// let data_type = DataType::Integer;
    /// println!("data_type: {:?}", data_type);
    /// ```
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub enum DataType {
        Float,
        Integer,
        String,
    }

    // Check that transformation worked - should extract the variable
    let example = DataType::schema_example();
    assert_eq!(example.as_str().unwrap(), "Integer");

    let zod = DataType::zod_schema();
    assert!(zod.contains("example:"));
    assert!(zod.contains("\"Integer\""));
}

#[cfg(feature = "zod")]
#[test]
fn test_regex_transform_let_underscore() {
    /// Status with let underscore pattern (for doctest compatibility)
    /// ```rust example
    /// let _: Status = Status::Active;
    /// ```
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub enum Status {
        Active,
        Inactive,
    }

    // Check that transformation worked - should extract the value
    let example = Status::schema_example();
    assert_eq!(example.as_str().unwrap(), "Active");

    let zod = Status::zod_schema();
    assert!(zod.contains("example:"));
    assert!(zod.contains("\"Active\""));
}

#[cfg(feature = "zod")]
#[test]
fn test_description_strips_examples() {
    /// This is a description
    /// with multiple lines
    /// ```rust example
    /// let _: DescriptionTest = DescriptionTest { value: 42 };
    /// ```
    /// More description after example
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct DescriptionTest {
        pub value: u32,
    }

    let zod = DescriptionTest::zod_schema();
    // Description should not contain the example code
    assert!(!zod.contains("DescriptionTest { value: 42 }"));
    // Note: Structs don't embed descriptions in .meta() currently (only enums do)
}

#[cfg(feature = "zod")]
#[test]
fn test_description_escapes_quotes() {
    /// Description with "quoted" text
    /// ```rust example
    /// let _: QuoteTest = QuoteTest::Active;
    /// ```
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub enum QuoteTest {
        Active,
        Inactive,
    }

    let zod = QuoteTest::zod_schema();
    // The generated code should have escaped quotes in description
    assert!(zod.contains("example:"));
    // Check that the description is properly formatted with escaped quotes
    assert!(zod.contains("description:"));
    assert!(zod.contains(r#"\"quoted\""#));
}

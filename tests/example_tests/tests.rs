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

    let example = DataType::schema_example();
    assert_eq!(example.as_str().unwrap(), "Numeric");

    let zod = DataType::zod_schema();
    assert!(zod.contains("example:"));
    assert!(zod.contains("\"Numeric\""));

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

    let example = User::schema_example();
    assert_eq!(example["name"].as_str().unwrap(), "John Doe");
    assert_eq!(example["age"].as_u64().unwrap(), 25);

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

    let example = ComplexUser::schema_example();
    assert_eq!(example["id"].as_str().unwrap(), "usr_123");
    assert_eq!(example["tags"][0].as_str().unwrap(), "admin");
    assert_eq!(example["tags"][1].as_str().unwrap(), "active");
    assert_eq!(example["age"].as_u64().unwrap(), 30);

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

    let example = Profile::schema_example();
    assert_eq!(example["tags"][0].as_str().unwrap(), "tag1");
    assert_eq!(example["metadata"]["key1"].as_str().unwrap(), "value1");
    assert_eq!(example["optional_field"].as_str().unwrap(), "present");

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

    let example = Event::schema_example();
    assert_eq!(example["type"].as_str().unwrap(), "UserCreated");
    assert_eq!(example["user_id"].as_str().unwrap(), "user_123");
    assert_eq!(example["timestamp"].as_str().unwrap(), "2024-01-01");

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

    let example = UserWithSerde::schema_example();
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

    let example = Document::schema_example();
    assert_eq!(example["name"].as_str().unwrap(), "test");
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
    assert!(zod.contains("example:"));
    assert!(zod.contains("description:"));
    assert!(zod.contains(r#"\"quoted\""#));
}

/// A doc example on a generic item is Rust the expansion has to compile, and a parameter names no
/// type to compile it at, so every parameter the item declares is instantiated at the filling
/// `default_types` declares for it — here `String`, which is also what an undeclared parameter
/// falls back to, so this item is annotated exactly as it was before the filling was read.
#[cfg(feature = "zod")]
#[test]
fn a_generic_struct_renders_its_example_at_its_declared_filling() {
    /// A generic type carrying an example block.
    ///
    /// ```rust example
    /// Boxed { value: "x".to_owned() }
    /// ```
    #[model_schema(default_types(ValueType = String))]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct Boxed<ValueType> {
        pub value: ValueType,
    }

    assert_eq!(
        Boxed::<String>::schema_example(),
        serde_json::json!({ "value": "x" })
    );
}

/// The arity is whatever the item declares, so a two-parameter item takes two arguments and not
/// one, each read off the entry that names it.
#[cfg(feature = "zod")]
#[test]
fn a_generic_enum_renders_its_example_at_its_declared_fillings() {
    /// A generic enum carrying an example block.
    ///
    /// ```rust example
    /// Tagged::Held { held: "x".to_owned(), tag: "t".to_owned() }
    /// ```
    #[model_schema(default_types(HeldType = String, TagType = String))]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub enum Tagged<HeldType, TagType> {
        Held { held: HeldType, tag: TagType },
    }

    assert_eq!(
        Tagged::<String, String>::schema_example(),
        serde_json::json!({ "Held": { "held": "x", "tag": "t" } })
    );
}

/// `default_types` is the one argument that names a concrete type per parameter, so the example is
/// annotated at what the author already declared. A parameter bounded by a trait `String` does not
/// satisfy then compiles, and the value the example builds is the one the filling admits rather
/// than one an annotation picked over it contradicts.
#[cfg(feature = "zod")]
#[test]
fn a_bounded_parameter_renders_its_example_at_its_declared_filling() {
    /// A generic type whose parameter carries a bound `String` does not satisfy.
    ///
    /// ```rust example
    /// Counted { count: 7u32 }
    /// ```
    #[model_schema(default_types(CountType = u32))]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct Counted<CountType: Copy> {
        pub count: CountType,
    }

    assert_eq!(
        Counted::<u32>::schema_example(),
        serde_json::json!({ "count": 7_u32 })
    );
}

/// Every enum shape reads the filling the same way, the seam that annotates the value being the one
/// the struct path reaches too — so a parameter filled at something other than `String` renders the
/// value that filling builds here as well.
#[cfg(feature = "zod")]
#[test]
fn a_generic_enum_renders_its_example_at_a_filling_that_is_not_a_string() {
    /// A generic enum carrying an example block.
    ///
    /// ```rust example
    /// Measured::Held { held: 7u32 }
    /// ```
    #[model_schema(default_types(HeldType = u32))]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub enum Measured<HeldType> {
        Held { held: HeldType },
    }

    assert_eq!(
        Measured::<u32>::schema_example(),
        serde_json::json!({ "Held": { "held": 7_u32 } })
    );
}

/// A parameter no entry names keeps the `String` fallback: the convention holds wherever nothing was
/// declared to replace it. Only a build without `jsonschema` reaches it — with that feature on, a
/// filling is required for every parameter before an example is ever built.
#[cfg(all(feature = "zod", not(feature = "jsonschema")))]
#[test]
fn an_unfilled_parameter_keeps_the_string_instantiation() {
    /// A generic type declaring no filling.
    ///
    /// ```rust example
    /// Unfilled { value: "x".to_owned() }
    /// ```
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct Unfilled<ValueType> {
        pub value: ValueType,
    }

    assert_eq!(
        Unfilled::<String>::schema_example(),
        serde_json::json!({ "value": "x" })
    );
}

/// A filling written as `String` is exactly what an unfilled parameter falls back to, so the two
/// items write the same schema down to the byte once their names are read as one — reading the
/// declaration leaves every item the convention already got right untouched. Names of equal length
/// so nothing but the name itself differs.
#[cfg(all(feature = "zod", not(feature = "jsonschema")))]
#[test]
fn a_string_filling_and_no_filling_write_the_same_schema() {
    /// A generic type declaring its filling.
    ///
    /// ```rust example
    /// Written { value: "x".to_owned() }
    /// ```
    #[model_schema(default_types(ValueType = String))]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct Written<ValueType> {
        pub value: ValueType,
    }

    /// A generic type declaring its filling.
    ///
    /// ```rust example
    /// Omitted { value: "x".to_owned() }
    /// ```
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct Omitted<ValueType> {
        pub value: ValueType,
    }

    assert_eq!(
        Written::<String>::zod_schema().replace("Written", "Item"),
        Omitted::<String>::zod_schema().replace("Omitted", "Item")
    );
}

/// A generic item publishes a factory, whose last `;` closes its arrow function — so the example
/// anchors on the statement binding the schema the factory caches instead. That statement is the
/// one place on the value every call returns that also keeps two calls with the same arguments the
/// same schema: attaching at `return schema` would hand back an instance the cache never stored.
#[cfg(feature = "zod")]
#[test]
fn a_factory_carries_its_example_on_the_schema_it_memoizes() {
    /// A generic type carrying an example block.
    ///
    /// ```rust example
    /// Held { value: "x".to_owned() }
    /// ```
    #[model_schema(default_types(ValueType = String))]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct Held<ValueType> {
        pub value: ValueType,
    }

    let zod = Held::<String>::zod_schema();
    assert!(
        zod.contains(
            "  const schema = buildHeld$Schema(valueType).meta({\n    example: \
             {\"value\":\"x\"}\n  });\n"
        ),
        "Got: {zod}"
    );
    assert!(!zod.contains("}.meta("), "Got: {zod}");
    assert!(
        zod.contains("  valueType[Held$SchemaMemo] = schema;\n  return schema;\n};"),
        "Got: {zod}"
    );
}

/// A type that declares no parameter publishes the annotated `const`, whose own `;` still closes
/// the value the example belongs on.
#[cfg(feature = "zod")]
#[test]
fn a_plain_const_still_carries_its_example_on_the_exported_binding() {
    /// A type carrying an example block.
    ///
    /// ```rust example
    /// Kept { value: "x".to_owned() }
    /// ```
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct Kept {
        pub value: String,
    }

    let zod = Kept::zod_schema();
    #[cfg(feature = "typescript")]
    assert!(
        zod.contains(
            "export const Kept$Schema: ZodType<Kept> = Kept$RawSchema.meta({\n  example: \
             {\"value\":\"x\"}\n});"
        ),
        "Got: {zod}"
    );
    #[cfg(not(feature = "typescript"))]
    assert!(
        zod.contains(".meta({\n  example: {\"value\":\"x\"}\n});"),
        "Got: {zod}"
    );
}

/// A lifetime takes no filling and needs none: it elides in the value's annotation, so an item
/// binding one is annotated exactly as an item binding nothing and its example is built as
/// written.
#[cfg(feature = "zod")]
#[test]
fn a_lifetime_item_renders_its_example_with_the_lifetime_elided() {
    /// A borrowing type carrying an example block.
    ///
    /// ```rust example
    /// Labelled { label: "x" }
    /// ```
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct Labelled<'label> {
        pub label: &'label str,
    }

    assert_eq!(
        Labelled::schema_example(),
        serde_json::json!({ "label": "x" })
    );
}

/// An example on a const-declaring item is only unwritable where an example is written out at all,
/// and `zod` is the only surface that writes one. Without it the block sits unread exactly as it
/// does on every other item here, so the item expands and its own contract is untouched.
#[cfg(not(feature = "zod"))]
#[test]
fn a_const_declaring_item_keeps_its_example_where_no_example_is_read() {
    /// A const-bearing type carrying an example block.
    ///
    /// ```rust example
    /// Slotted { label: "x".to_owned() }
    /// ```
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct Slotted<const WIDTH: usize> {
        pub label: String,
    }

    let slotted = Slotted::<3> {
        label: "x".to_owned(),
    };
    assert_eq!(slotted.label, "x");
}

#[cfg(all(test, feature = "serde"))]
use serde::{Deserialize, Serialize};
#[cfg(all(
    test,
    any(
        feature = "typescript",
        feature = "jsonschema",
        feature = "zod",
        feature = "serde"
    )
))]
use tixschema::model_schema;

// Test the example from the user request.
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
struct AccountContext {
    pub aud: String,
    pub exp: i64,
    pub iat: i64,
    #[model_schema_prop(as = String, literal = "Tixena")]
    pub iss: String,
    #[model_schema_prop(as = String, minLength = 1)]
    pub jti: String,
    pub nbf: i64,
    #[model_schema_prop(as = String, minLength = 1)]
    pub sub: String,
}

// Test array of literals.
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
struct ArrayLiteral {
    pub id: String,
    #[model_schema_prop(literal = "array_item")]
    pub literal_array: Vec<String>,
}

// Test combining literal and minLength (should prioritize literal).
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
struct CombinedTest {
    #[model_schema_prop(as = String, literal = "fixed", minLength = 10)]
    pub fixed_field: String,
    #[model_schema_prop(as = String, minLength = 1)]
    pub normal_field: String,
}

// Test struct with comprehensive minLength configurations.
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
struct MinLengthTest {
    // Regular string without minLength
    pub description: String,
    #[model_schema_prop(as = String, minLength = 1)]
    pub name: String,
    // Optional string with minLength
    #[model_schema_prop(as = String, minLength = 3)]
    pub nickname: Option<String>,
    #[model_schema_prop(as = String, minLength = 10)]
    pub password: String,
    // Array of strings with minLength on the items
    #[model_schema_prop(as = String, minLength = 2)]
    pub tags: Vec<String>,
    #[model_schema_prop(as = String, minLength = 5)]
    pub username: String,
}

// Test multiple literal values in one struct.
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
struct MultipleLiterals {
    pub id: String,
    pub name: String,
    #[model_schema_prop(literal = "fixed_type")]
    pub type_field: String,
    #[model_schema_prop(literal = "v1.0")]
    pub version: String,
}

// Test optional literal fields.
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
struct OptionalLiteral {
    pub id: String,
    #[model_schema_prop(literal = "optional_literal")]
    pub optional_type: Option<String>,
}

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
struct Inner {
    pub value: String,
}

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
struct TsOptionalStruct {
    #[model_schema_prop(ts_optional)]
    pub f: Option<Inner>,
    pub g: Option<Inner>,
    // `as` is currently a no-op override; this field guards their coexistence.
    #[model_schema_prop(as = Inner, ts_optional)]
    pub h: Option<Inner>,
}

#[cfg(all(
    test,
    any(feature = "typescript", feature = "jsonschema", feature = "zod")
))]
#[model_schema()]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", serde(tag = "type"))]
enum TsOptionalVariant {
    FilterPart {
        #[model_schema_prop(ts_optional)]
        filter: Option<Inner>,
    },
}

#[test]
#[cfg(feature = "typescript")]
fn test_string_literal_typescript() {
    let ts_definition = AccountContext::ts_definition();

    // Check that the literal field generates the correct TypeScript type
    assert!(ts_definition.contains("iss: \"Tixena\";"));

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
    let zod_schema = AccountContext::zod_schema();

    // Check that the literal field generates the correct Zod schema
    assert!(zod_schema.contains("iss: z.literal(\"Tixena\")"));

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
    let schema = AccountContext::json_schema();
    let properties = schema["properties"].as_object().unwrap();

    // Check that the literal field has the correct JSON schema
    let iss_prop = &properties["iss"];
    assert_eq!(iss_prop["type"], "string");
    assert_eq!(iss_prop["const"], "Tixena");

    // Check that other string fields are normal strings without const
    let aud_prop = &properties["aud"];
    assert_eq!(aud_prop["type"], "string");
    assert!(aud_prop.get("const").is_none());
    assert!(aud_prop.get("minLength").is_none());

    // Check that minLength fields have the correct validation
    let sub_prop = &properties["sub"];
    assert_eq!(sub_prop["type"], "string");
    assert!(sub_prop.get("const").is_none());
    assert_eq!(sub_prop["minLength"], 1_i32);

    let jti_prop = &properties["jti"];
    assert_eq!(jti_prop["type"], "string");
    assert!(jti_prop.get("const").is_none());
    assert_eq!(jti_prop["minLength"], 1_i32);
}

#[test]
#[cfg(feature = "typescript")]
fn test_multiple_literals_typescript() {
    let ts_definition = MultipleLiterals::ts_definition();

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
    let zod_schema = MultipleLiterals::zod_schema();

    // Check multiple literals
    assert!(zod_schema.contains("type_field: z.literal(\"fixed_type\")"));
    assert!(zod_schema.contains("version: z.literal(\"v1.0\")"));

    // Check normal fields
    assert!(zod_schema.contains("id: z.string()"));
    assert!(zod_schema.contains("name: z.string()"));
}

#[test]
#[cfg(feature = "typescript")]
fn test_optional_literal_typescript() {
    let ts_definition = OptionalLiteral::ts_definition();

    // Check that optional literal works correctly
    assert!(ts_definition.contains("optional_type: \"optional_literal\" | undefined;"));
    assert!(ts_definition.contains("id: string;"));
}

#[test]
#[cfg(feature = "zod")]
fn test_optional_literal_zod() {
    let zod_schema = OptionalLiteral::zod_schema();

    // Check that optional literal works correctly
    assert!(
        zod_schema
            .contains("optional_type: z.union([z.literal(\"optional_literal\"), z.undefined()])")
    );
    assert!(zod_schema.contains("id: z.string()"));
}

#[test]
#[cfg(feature = "typescript")]
fn test_array_literal_typescript() {
    let ts_definition = ArrayLiteral::ts_definition();

    // Check that array of literals works correctly
    assert!(ts_definition.contains("literal_array: Array<\"array_item\">;"));
    assert!(ts_definition.contains("id: string;"));
}

#[test]
#[cfg(feature = "zod")]
fn test_array_literal_zod() {
    let zod_schema = ArrayLiteral::zod_schema();

    // Check that array of literals works correctly
    assert!(zod_schema.contains("literal_array: z.array(z.literal(\"array_item\"))"));
    assert!(zod_schema.contains("id: z.string()"));
}

#[test]
#[cfg(feature = "jsonschema")]
fn test_array_literal_json_schema() {
    let schema = ArrayLiteral::json_schema();
    let properties = schema["properties"].as_object().unwrap();

    // Check that array of literals has correct JSON schema
    let literal_array_prop = &properties["literal_array"];
    assert_eq!(literal_array_prop["type"], "array");
    assert_eq!(literal_array_prop["items"]["type"], "string");
    assert_eq!(literal_array_prop["items"]["const"], "array_item");
}

#[test]
#[cfg(feature = "typescript")]
fn test_min_length_typescript() {
    let ts_definition = MinLengthTest::ts_definition();

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
    let zod_schema = MinLengthTest::zod_schema();

    // Check that minLength fields have correct validation
    assert!(zod_schema.contains("name: z.string().min(1)"));
    assert!(zod_schema.contains("username: z.string().min(5)"));
    assert!(zod_schema.contains("password: z.string().min(10)"));
    assert!(zod_schema.contains("tags: z.array(z.string().min(2))"));

    // Check that regular string field doesn't have minLength
    assert!(zod_schema.contains("description: z.string(),"));

    // Check that optional string with minLength works correctly
    assert!(zod_schema.contains("nickname: z.union([z.string().min(3), z.undefined()])"));
}

#[test]
#[cfg(feature = "jsonschema")]
fn test_min_length_json_schema() {
    let schema = MinLengthTest::json_schema();
    let properties = schema["properties"].as_object().unwrap();

    // Check that minLength fields have the correct JSON schema
    let name_prop = &properties["name"];
    assert_eq!(name_prop["type"], "string");
    assert_eq!(name_prop["minLength"], 1_i32);

    let username_prop = &properties["username"];
    assert_eq!(username_prop["type"], "string");
    assert_eq!(username_prop["minLength"], 5_i32);

    let password_prop = &properties["password"];
    assert_eq!(password_prop["type"], "string");
    assert_eq!(password_prop["minLength"], 10_i32);

    let nickname_prop = &properties["nickname"];
    assert_eq!(nickname_prop["type"], "string");
    assert_eq!(nickname_prop["minLength"], 3_i32);

    // Check that regular string field doesn't have minLength
    let description_prop = &properties["description"];
    assert_eq!(description_prop["type"], "string");
    assert!(description_prop.get("minLength").is_none());

    // Check that array field has minLength on items
    let tags_prop = &properties["tags"];
    assert_eq!(tags_prop["type"], "array");
    assert_eq!(tags_prop["items"]["type"], "string");
    assert_eq!(tags_prop["items"]["minLength"], 2_i32);
}

#[test]
#[cfg(feature = "typescript")]
fn test_combined_literal_minlength_typescript() {
    let ts_definition = CombinedTest::ts_definition();

    // Literal should take precedence - should be a literal type, not a string with minLength
    assert!(ts_definition.contains("fixed_field: \"fixed\";"));
    assert!(ts_definition.contains("normal_field: string;"));

    // Should still have minLength documentation for the normal field
    assert!(ts_definition.contains("Minimum length: 1"));
}

#[test]
#[cfg(feature = "zod")]
fn test_combined_literal_minlength_zod() {
    let zod_schema = CombinedTest::zod_schema();

    // Literal should take precedence - should be a literal, not a string with minLength
    assert!(zod_schema.contains("fixed_field: z.literal(\"fixed\")"));
    assert!(zod_schema.contains("normal_field: z.string().min(1)"));
}

#[test]
#[cfg(feature = "jsonschema")]
fn test_combined_literal_minlength_json_schema() {
    let schema = CombinedTest::json_schema();
    let properties = schema["properties"].as_object().unwrap();

    // Literal should take precedence
    let fixed_prop = &properties["fixed_field"];
    assert_eq!(fixed_prop["type"], "string");
    assert_eq!(fixed_prop["const"], "fixed");
    assert!(fixed_prop.get("minLength").is_none()); // Should not have minLength when literal

    // Normal field should have minLength
    let normal_prop = &properties["normal_field"];
    assert_eq!(normal_prop["type"], "string");
    assert_eq!(normal_prop["minLength"], 1_i32);
    assert!(normal_prop.get("const").is_none());
}

#[test]
#[cfg(feature = "typescript")]
fn test_ts_optional_struct_typescript() {
    let ts = TsOptionalStruct::ts_definition();

    assert!(ts.contains("f?: Inner;"), "expected `f?: Inner;` in:\n{ts}");
    assert!(
        !ts.contains("f: Inner | undefined"),
        "did not expect required-key form for `f` in:\n{ts}"
    );

    assert!(
        ts.contains("g: Inner | undefined;"),
        "expected control `g: Inner | undefined;` in:\n{ts}"
    );

    assert!(ts.contains("h?: Inner;"), "expected `h?: Inner;` in:\n{ts}");
}

#[test]
#[cfg(feature = "zod")]
fn test_ts_optional_struct_zod_unchanged() {
    let zod = TsOptionalStruct::zod_schema();

    assert!(
        zod.contains("f: z.union([Inner$Schema, z.undefined()]).prefault(undefined)"),
        "expected unchanged Zod for `f` in:\n{zod}"
    );
    assert!(
        zod.contains("g: z.union([Inner$Schema, z.undefined()]).prefault(undefined)"),
        "expected unchanged Zod for `g` in:\n{zod}"
    );
}

#[test]
#[cfg(feature = "jsonschema")]
fn test_ts_optional_struct_json_schema_unchanged() {
    let schema = TsOptionalStruct::json_schema();
    let required_arr = schema["required"].as_array().unwrap();
    let required: Vec<&str> = required_arr.iter().filter_map(|v| v.as_str()).collect();

    assert!(
        !required.contains(&"f"),
        "`f` should not be required: {required:?}"
    );
    assert!(
        !required.contains(&"g"),
        "`g` should not be required: {required:?}"
    );
}

#[test]
#[cfg(feature = "typescript")]
fn test_ts_optional_variant_typescript() {
    let ts = TsOptionalVariant::ts_definition();

    // Covers the named-variant render path (write_named_variant_fields).
    assert!(
        ts.contains("filter?: Inner;"),
        "expected `filter?: Inner;` in variant TS:\n{ts}"
    );
    assert!(
        !ts.contains("filter: Inner | undefined"),
        "did not expect required-key form for variant `filter` in:\n{ts}"
    );
}

#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
use serde::{Deserialize, Serialize};
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
use tixschema::model_schema;

// Pattern tests

#[cfg(feature = "zod")]
#[test]
fn test_pattern_zod_output() {
    #[model_schema()]
    #[derive(Serialize, Deserialize)]
    pub struct PatternTest {
        #[model_schema_prop(pattern = "^[0-9a-fA-F]{24}$")]
        pub data_element_id: String,
    }

    let schema = PatternTest::zod_schema();
    assert!(
        schema.contains(".check(z.regex(/^[0-9a-fA-F]{24}$/))"),
        "Schema: {schema}"
    );
}

#[cfg(feature = "jsonschema")]
#[test]
fn test_pattern_json_schema() {
    #[model_schema()]
    #[derive(Serialize, Deserialize)]
    pub struct PatternTest {
        #[model_schema_prop(pattern = "^[0-9a-fA-F]{24}$")]
        pub data_element_id: String,
    }

    let schema = PatternTest::json_schema();
    let schema_str = serde_json::to_string(&schema).unwrap();
    assert!(
        schema_str.contains("\"pattern\":\"^[0-9a-fA-F]{24}$\""),
        "Schema: {schema_str}"
    );
}

#[cfg(feature = "typescript")]
#[test]
fn test_pattern_ts_type_unaffected() {
    #[model_schema()]
    #[derive(Serialize, Deserialize)]
    pub struct PatternTsTest {
        #[model_schema_prop(pattern = "^[0-9a-fA-F]{24}$")]
        pub data_element_id: String,
    }

    let ts = PatternTsTest::ts_definition();
    // TypeScript type should just be string, no regex info in the type body
    assert!(ts.contains("data_element_id: string"), "TS: {ts}");
    assert!(!ts.contains("regex"), "TS should not contain regex: {ts}");
    // The type body itself (after the JSDoc comment) should not have pattern syntax
    // Note: the JSON schema section of the JSDoc may include "pattern" as schema metadata,
    // but the actual TypeScript type definition should be plain `string`.
    let type_body_start = ts.find("export type").unwrap_or(0);
    let type_body = &ts[type_body_start..];
    assert!(
        !type_body.contains(".regex") && !type_body.contains("z.regex"),
        "TypeScript type body should not contain regex syntax: {type_body}"
    );
}

#[cfg(all(feature = "zod", feature = "serde"))]
#[test]
fn test_pattern_enum_variant_field() {
    #[model_schema()]
    #[derive(Serialize, Deserialize)]
    #[serde(tag = "type")]
    pub enum PatternEnum {
        Variant {
            #[model_schema_prop(pattern = "^[0-9a-fA-F]{24}$")]
            id: String,
        },
    }

    let schema = PatternEnum::zod_schema();
    assert!(
        schema.contains(".check(z.regex(/^[0-9a-fA-F]{24}$/))"),
        "Schema: {schema}"
    );
}

#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn test_pattern_rust_validation_valid() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    pub struct ValidationTest {
        #[model_schema_prop(pattern = "^[0-9a-fA-F]{24}$")]
        pub id: String,
    }

    let valid = r#"{"id": "507f1f77bcf86cd799439011"}"#;
    let result: Result<ValidationTest, _> = serde_json::from_str(valid);
    assert!(
        result.is_ok(),
        "Valid hex ID should deserialize successfully"
    );
}

#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn test_pattern_rust_validation_invalid() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    pub struct ValidationInvalid {
        #[model_schema_prop(pattern = "^[0-9a-fA-F]{24}$")]
        pub id: String,
    }

    let invalid = r#"{"id": "not-a-hex-id"}"#;
    let result: Result<ValidationInvalid, _> = serde_json::from_str(invalid);
    assert!(result.is_err(), "Invalid hex ID should fail to deserialize");
    let err_str = result.unwrap_err().to_string();
    assert!(
        err_str.contains("does not match pattern"),
        "Error: {err_str}"
    );
}

// Preprocess tests

#[cfg(feature = "zod")]
#[test]
fn test_preprocess_single_fn_zod() {
    #[model_schema()]
    #[derive(Serialize, Deserialize)]
    pub struct PreprocessSingle {
        #[model_schema_prop(preprocess = ["epoch_to_date"])]
        pub date_value: String,
    }

    let schema = PreprocessSingle::zod_schema();
    assert!(
        schema.contains("z.preprocess(epoch_to_date,"),
        "Schema: {schema}"
    );
}

#[cfg(feature = "zod")]
#[test]
fn test_preprocess_multiple_fns_zod() {
    #[model_schema()]
    #[derive(Serialize, Deserialize)]
    pub struct PreprocessMultiple {
        #[model_schema_prop(preprocess = ["epoch_to_date", "trim"])]
        pub date_value: String,
    }

    let schema = PreprocessMultiple::zod_schema();
    assert!(
        schema.contains("z.preprocess(epoch_to_date, z.preprocess(trim,"),
        "Schema: {schema}"
    );
}

#[cfg(feature = "typescript")]
#[test]
fn test_preprocess_ts_type_unaffected() {
    #[model_schema()]
    #[derive(Serialize, Deserialize)]
    pub struct PreprocessTs {
        #[model_schema_prop(preprocess = ["epoch_to_date"])]
        pub date_value: String,
    }

    let ts = PreprocessTs::ts_definition();
    assert!(ts.contains("date_value: string"), "TS: {ts}");
    assert!(
        !ts.contains("preprocess"),
        "TS should not contain preprocess: {ts}"
    );
}

#[cfg(feature = "jsonschema")]
#[test]
fn test_preprocess_json_schema_unaffected() {
    #[model_schema()]
    #[derive(Serialize, Deserialize)]
    pub struct PreprocessJsonSchema {
        #[model_schema_prop(preprocess = ["epoch_to_date"])]
        pub date_value: String,
    }

    let schema = PreprocessJsonSchema::json_schema();
    // JSON schema should be same as without preprocess - just string type
    let properties = schema["properties"].as_object().unwrap();
    let date_schema = &properties["date_value"];
    assert_eq!(date_schema["type"], "string");
    assert!(date_schema.get("preprocess").is_none());
}

// Combined test
#[cfg(feature = "zod")]
#[test]
fn test_pattern_and_preprocess_same_field() {
    #[model_schema()]
    #[derive(Serialize, Deserialize)]
    pub struct PatternAndPreprocess {
        #[model_schema_prop(pattern = "^[0-9a-fA-F]{24}$", preprocess = ["trim"])]
        pub id: String,
    }

    let schema = PatternAndPreprocess::zod_schema();
    // Should have preprocess wrapping the string with pattern check
    assert!(schema.contains("z.preprocess(trim,"), "Schema: {schema}");
    assert!(
        schema.contains(".check(z.regex(/^[0-9a-fA-F]{24}$/))"),
        "Schema: {schema}"
    );
}

// ==================== Pattern with optional field ====================

#[cfg(feature = "zod")]
#[test]
fn test_pattern_optional_field_zod() {
    #[model_schema()]
    #[derive(Serialize, Deserialize)]
    pub struct PatternOptional {
        #[model_schema_prop(pattern = "^[0-9]+$")]
        #[serde(skip_serializing_if = "Option::is_none")]
        pub code: Option<String>,
    }

    let schema = PatternOptional::zod_schema();
    // Optional field with pattern: z.union([z.string().check(z.regex(...)), z.undefined()])
    assert!(
        schema.contains("z.union([z.string().check(z.regex(/^[0-9]+$/)), z.undefined()])"),
        "Expected optional pattern union in Zod schema: {schema}"
    );
}

// ==================== Pattern combined with minLength/maxLength ====================

#[cfg(feature = "zod")]
#[test]
fn test_pattern_with_min_max_length_zod() {
    #[model_schema()]
    #[derive(Serialize, Deserialize)]
    pub struct PatternMinMaxLen {
        #[model_schema_prop(minLength = 3, maxLength = 20, pattern = "^[a-z]+$")]
        pub username: String,
    }

    let schema = PatternMinMaxLen::zod_schema();
    assert!(
        schema.contains(".min(3)"),
        "Expected .min(3) in Zod schema: {schema}"
    );
    assert!(
        schema.contains(".max(20)"),
        "Expected .max(20) in Zod schema: {schema}"
    );
    assert!(
        schema.contains(".check(z.regex(/^[a-z]+$/))"),
        "Expected .check(z.regex(...)) in Zod schema: {schema}"
    );
}

#[cfg(feature = "jsonschema")]
#[test]
fn test_pattern_with_min_length_json_schema() {
    #[model_schema()]
    #[derive(Serialize, Deserialize)]
    pub struct PatternMinLenJsonSchema {
        #[model_schema_prop(minLength = 3, pattern = "^[a-z]+$")]
        pub username: String,
    }

    let schema = PatternMinLenJsonSchema::json_schema();
    let schema_str = serde_json::to_string(&schema).unwrap();
    assert!(
        schema_str.contains("\"minLength\":3"),
        "Expected minLength in JSON schema: {schema_str}"
    );
    assert!(
        schema_str.contains("\"pattern\":\"^[a-z]+$\""),
        "Expected pattern in JSON schema: {schema_str}"
    );
}

// ==================== Preprocess with optional field ====================

#[cfg(feature = "zod")]
#[test]
fn test_preprocess_optional_field_zod() {
    #[model_schema()]
    #[derive(Serialize, Deserialize)]
    pub struct PreprocessOptional {
        #[model_schema_prop(preprocess = ["trim"])]
        #[serde(skip_serializing_if = "Option::is_none")]
        pub nickname: Option<String>,
    }

    let schema = PreprocessOptional::zod_schema();
    // Preprocess wraps the inner schema, then union with undefined wraps that
    assert!(
        schema.contains("z.union([z.preprocess(trim, z.string()), z.undefined()])"),
        "Expected preprocess inside optional union in Zod schema: {schema}"
    );
}

// ==================== Preprocess with multiple fields ====================

#[cfg(feature = "zod")]
#[test]
fn test_preprocess_different_fields() {
    #[model_schema()]
    #[derive(Serialize, Deserialize)]
    pub struct PreprocessDiffFields {
        #[model_schema_prop(preprocess = ["to_lowercase"])]
        pub email: String,
        #[model_schema_prop(preprocess = ["trim"])]
        pub name: String,
    }

    let schema = PreprocessDiffFields::zod_schema();
    assert!(
        schema.contains("z.preprocess(trim, z.string())"),
        "Expected trim preprocess for name field: {schema}"
    );
    assert!(
        schema.contains("z.preprocess(to_lowercase, z.string())"),
        "Expected to_lowercase preprocess for email field: {schema}"
    );
}

// ==================== Pattern with special regex characters ====================

#[cfg(feature = "zod")]
#[test]
fn test_pattern_special_regex_chars() {
    #[model_schema()]
    #[derive(Serialize, Deserialize)]
    pub struct PatternSpecialRegex {
        #[model_schema_prop(pattern = r"^\d{3}\.\d{3}\.\d{3}-\d{2}$")]
        pub cpf: String,
    }

    let schema = PatternSpecialRegex::zod_schema();
    assert!(
        schema.contains(r".check(z.regex(/^\d{3}\.\d{3}\.\d{3}-\d{2}$/))"),
        "Expected special regex chars passed through in Zod schema: {schema}"
    );
}

// ==================== Preprocess ordering with three functions ====================

#[cfg(feature = "zod")]
#[test]
fn test_preprocess_ordering_three_fns() {
    #[model_schema()]
    #[derive(Serialize, Deserialize)]
    pub struct PreprocessThreeFns {
        #[model_schema_prop(preprocess = ["a", "b", "c"])]
        pub value: String,
    }

    let schema = PreprocessThreeFns::zod_schema();
    // Nesting order: z.preprocess(a, z.preprocess(b, z.preprocess(c, z.string())))
    assert!(
        schema.contains("z.preprocess(a, z.preprocess(b, z.preprocess(c, z.string())))"),
        "Expected correct nesting order a(b(c(inner))): {schema}"
    );
}

// ==================== Pattern on enum variant combined with minLength ====================

#[cfg(all(feature = "zod", feature = "serde"))]
#[test]
fn test_pattern_enum_variant_with_min_length() {
    #[model_schema()]
    #[derive(Serialize, Deserialize)]
    #[serde(tag = "type")]
    pub enum PatternEnumMinLen {
        Entry {
            #[model_schema_prop(pattern = "^[a-z]+$", minLength = 2)]
            slug: String,
        },
    }

    let schema = PatternEnumMinLen::zod_schema();
    assert!(
        schema.contains(".min(2)"),
        "Expected .min(2) in Zod enum schema: {schema}"
    );
    assert!(
        schema.contains(".check(z.regex(/^[a-z]+$/))"),
        "Expected .check(z.regex(...)) in Zod enum schema: {schema}"
    );
}

// ==================== Serde validation for pattern with enum variant ====================

#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn test_pattern_enum_variant_serde_validation() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    #[serde(tag = "type")]
    pub enum PatternEnumSerde {
        Item {
            #[model_schema_prop(pattern = "^[a-z_]+$")]
            key: String,
        },
    }

    // Valid: lowercase with underscores
    let valid = r#"{"type": "Item", "key": "hello_world"}"#;
    let result: Result<PatternEnumSerde, _> = serde_json::from_str(valid);
    assert!(
        result.is_ok(),
        "Valid key should deserialize: {:?}",
        result.err()
    );

    // Invalid: contains uppercase and digits
    let invalid = r#"{"type": "Item", "key": "Hello123"}"#;
    let invalid_result: Result<PatternEnumSerde, _> = serde_json::from_str(invalid);
    assert!(
        invalid_result.is_err(),
        "Invalid key should fail to deserialize"
    );
    let err_str = invalid_result.unwrap_err().to_string();
    assert!(
        err_str.contains("does not match pattern"),
        "Error should mention pattern mismatch: {err_str}"
    );
}

// ==================== Forward slash inside the Zod regex literal ====================

// The Zod surface splices a pattern into a JS regex literal, where `/` is the delimiter: an
// unescaped one closes the literal early and the emitted TypeScript stops parsing. That escaping
// belongs to the splice alone — JSON Schema and the Rust-side validator carry the pattern as a
// plain string and must see it byte for byte as written.

#[cfg(feature = "zod")]
#[test]
fn test_a_slash_in_a_field_pattern_escapes_the_zod_regex_delimiter() {
    #[model_schema()]
    #[derive(Serialize, Deserialize)]
    pub struct SlashField {
        #[model_schema_prop(pattern = "^/[a-z]+$")]
        pub name: String,
    }

    let schema = SlashField::zod_schema();
    assert!(
        schema.contains(r".check(z.regex(/^\/[a-z]+$/))"),
        "Expected the slash escaped inside the regex literal: {schema}"
    );
}

#[cfg(all(feature = "zod", feature = "serde"))]
#[test]
fn test_a_slash_in_a_brand_pattern_escapes_the_zod_regex_delimiter() {
    #[model_schema(pattern = "^/[a-z]+$")]
    #[derive(Serialize, Deserialize, Debug)]
    #[serde(transparent)]
    pub struct SlashBrand(pub String);

    let schema = SlashBrand::zod_schema();
    assert!(
        schema.contains(r".check(z.regex(/^\/[a-z]+$/))"),
        "Expected the slash escaped inside the regex literal: {schema}"
    );
}

#[cfg(feature = "zod")]
#[test]
fn test_an_already_escaped_slash_is_not_escaped_twice() {
    #[model_schema()]
    #[derive(Serialize, Deserialize)]
    pub struct EscapedSlashField {
        #[model_schema_prop(pattern = r"^\/[a-z]+$")]
        pub name: String,
    }

    let schema = EscapedSlashField::zod_schema();
    assert!(
        schema.contains(r".check(z.regex(/^\/[a-z]+$/))"),
        "Expected the existing escape carried through untouched: {schema}"
    );
    assert!(
        !schema.contains(r"\\/"),
        "An escaped slash must not gain a second backslash: {schema}"
    );
}

#[cfg(feature = "zod")]
#[test]
fn test_a_backslash_escape_before_a_slash_is_read_as_one_unit() {
    #[model_schema()]
    #[derive(Serialize, Deserialize)]
    pub struct LiteralBackslashField {
        // `\\` is a literal backslash, so the `/` after it is unescaped and needs its own escape.
        #[model_schema_prop(pattern = r"^\\/[a-z]+$")]
        pub name: String,
    }

    let schema = LiteralBackslashField::zod_schema();
    assert!(
        schema.contains(r".check(z.regex(/^\\\/[a-z]+$/))"),
        "Expected the slash after a literal backslash escaped: {schema}"
    );
}

#[cfg(feature = "jsonschema")]
#[test]
fn test_a_slash_pattern_reaches_json_schema_unescaped() {
    #[model_schema()]
    #[derive(Serialize, Deserialize)]
    pub struct SlashJsonSchema {
        #[model_schema_prop(pattern = "^/[a-z]+$")]
        pub name: String,
    }

    let schema_str = serde_json::to_string(&SlashJsonSchema::json_schema()).unwrap();
    assert!(
        schema_str.contains(r#""pattern":"^/[a-z]+$""#),
        "JSON Schema must carry the pattern as written: {schema_str}"
    );
}

#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn test_the_escaped_zod_pattern_matches_the_value_set_the_validator_enforces() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    pub struct SlashValidated {
        #[model_schema_prop(pattern = "^/[a-z]+$")]
        pub name: String,
    }

    serde_json::from_str::<SlashValidated>(r#"{"name": "/etc"}"#).unwrap();
    for rejected in [
        r#"{"name": "etc"}"#,
        r#"{"name": "/ETC"}"#,
        r#"{"name": "//etc"}"#,
    ] {
        let err = serde_json::from_str::<SlashValidated>(rejected).unwrap_err();
        assert!(
            err.to_string().contains("does not match pattern"),
            "Expected a pattern rejection for {rejected}: {err}"
        );
    }
}

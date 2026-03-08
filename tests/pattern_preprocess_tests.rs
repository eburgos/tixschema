use serde::{Deserialize, Serialize};
use tixschema::model_schema;

// Pattern tests

#[cfg(feature = "zod")]
#[test]
fn test_pattern_zod_output() {
    #[model_schema()]
    #[derive(Serialize, Deserialize)]
    pub struct PatternTestJson {
        #[model_schema_prop(pattern = "^[0-9a-fA-F]{24}$")]
        pub data_element_id: String,
    }

    let schema = PatternTestJson::zod_schema();
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
    pub struct PatternJsonTestJson {
        #[model_schema_prop(pattern = "^[0-9a-fA-F]{24}$")]
        pub data_element_id: String,
    }

    let schema = PatternJsonTestJson::json_schema();
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
    pub struct PatternTsTestJson {
        #[model_schema_prop(pattern = "^[0-9a-fA-F]{24}$")]
        pub data_element_id: String,
    }

    let ts = PatternTsTestJson::ts_definition();
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
    pub enum PatternEnumJson {
        Variant {
            #[model_schema_prop(pattern = "^[0-9a-fA-F]{24}$")]
            id: String,
        },
    }

    let schema = PatternEnumJson::zod_schema();
    assert!(
        schema.contains(".check(z.regex(/^[0-9a-fA-F]{24}$/))"),
        "Schema: {schema}"
    );
}

#[cfg(all(feature = "serde", any(feature = "typescript", feature = "zod", feature = "jsonschema")))]
#[test]
fn test_pattern_rust_validation_valid() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    pub struct ValidationTestJson {
        #[model_schema_prop(pattern = "^[0-9a-fA-F]{24}$")]
        pub id: String,
    }

    let valid = r#"{"id": "507f1f77bcf86cd799439011"}"#;
    let result: Result<ValidationTestJson, _> = serde_json::from_str(valid);
    assert!(result.is_ok(), "Valid hex ID should deserialize successfully");
}

#[cfg(all(feature = "serde", any(feature = "typescript", feature = "zod", feature = "jsonschema")))]
#[test]
fn test_pattern_rust_validation_invalid() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    pub struct ValidationInvalidJson {
        #[model_schema_prop(pattern = "^[0-9a-fA-F]{24}$")]
        pub id: String,
    }

    let invalid = r#"{"id": "not-a-hex-id"}"#;
    let result: Result<ValidationInvalidJson, _> = serde_json::from_str(invalid);
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
    pub struct PreprocessSingleJson {
        #[model_schema_prop(preprocess = ["epoch_to_date"])]
        pub date_value: String,
    }

    let schema = PreprocessSingleJson::zod_schema();
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
    pub struct PreprocessMultipleJson {
        #[model_schema_prop(preprocess = ["epoch_to_date", "trim"])]
        pub date_value: String,
    }

    let schema = PreprocessMultipleJson::zod_schema();
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
    pub struct PreprocessTsJson {
        #[model_schema_prop(preprocess = ["epoch_to_date"])]
        pub date_value: String,
    }

    let ts = PreprocessTsJson::ts_definition();
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
    pub struct PreprocessJsonSchemaJson {
        #[model_schema_prop(preprocess = ["epoch_to_date"])]
        pub date_value: String,
    }

    let schema = PreprocessJsonSchemaJson::json_schema();
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
    pub struct PatternAndPreprocessJson {
        #[model_schema_prop(pattern = "^[0-9a-fA-F]{24}$", preprocess = ["trim"])]
        pub id: String,
    }

    let schema = PatternAndPreprocessJson::zod_schema();
    // Should have preprocess wrapping the string with pattern check
    assert!(schema.contains("z.preprocess(trim,"), "Schema: {schema}");
    assert!(
        schema.contains(".check(z.regex(/^[0-9a-fA-F]{24}$/))"),
        "Schema: {schema}"
    );
}

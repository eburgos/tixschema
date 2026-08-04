#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
use alloc::borrow::Cow;
#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
use alloc::sync::Arc;
#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
use serde::{Deserialize, Serialize};
#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
use tixschema::model_schema;

/// What a type answers when it publishes no inherent `validate()` of its own. An inherent method
/// takes precedence over a trait's, so reaching this one is what says none was published — the same
/// question asked of a constraint-free struct, which has never published one either.
#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
trait UnpublishedValidate {
    fn validate(&self) -> &'static str {
        "no inherent validate()"
    }
}

// ==================== String constraint: maxLength ====================

#[cfg(all(feature = "serde", feature = "zod"))]
#[test]
fn test_max_length_zod() {
    #[model_schema()]
    #[derive(Serialize, Deserialize)]
    pub struct MaxLengthZod {
        #[model_schema_prop(maxLength = 50)]
        pub username: String,
    }

    let schema = MaxLengthZod::zod_schema();
    assert!(
        schema.contains(".max(50)"),
        "Expected .max(50) in Zod schema: {schema}"
    );
}

#[cfg(all(feature = "serde", feature = "zod"))]
#[test]
fn test_min_length_zod() {
    #[model_schema()]
    #[derive(Serialize, Deserialize)]
    pub struct MinLengthZod {
        #[model_schema_prop(minLength = 5)]
        pub username: String,
    }

    let schema = MinLengthZod::zod_schema();
    assert!(
        schema.contains(".min(5)"),
        "Expected .min(5) in Zod schema: {schema}"
    );
}

#[cfg(all(feature = "serde", feature = "zod"))]
#[test]
fn test_min_and_max_length_zod() {
    #[model_schema()]
    #[derive(Serialize, Deserialize)]
    pub struct MinMaxLengthZod {
        #[model_schema_prop(minLength = 5, maxLength = 50)]
        pub username: String,
    }

    let schema = MinMaxLengthZod::zod_schema();
    assert!(
        schema.contains(".min(5)"),
        "Expected .min(5) in Zod schema: {schema}"
    );
    assert!(
        schema.contains(".max(50)"),
        "Expected .max(50) in Zod schema: {schema}"
    );
}

// ==================== String constraint: maxLength — JSON Schema ====================

#[cfg(all(feature = "serde", feature = "jsonschema"))]
#[test]
fn test_max_length_json_schema() {
    #[model_schema()]
    #[derive(Serialize, Deserialize)]
    pub struct MaxLengthJsonSchema {
        #[model_schema_prop(maxLength = 50)]
        pub username: String,
    }

    let schema = MaxLengthJsonSchema::json_schema();
    let schema_str = serde_json::to_string(&schema).unwrap();
    assert!(
        schema_str.contains("\"maxLength\":50"),
        "Expected maxLength:50 in JSON schema: {schema_str}"
    );
}

#[cfg(all(feature = "serde", feature = "jsonschema"))]
#[test]
fn test_min_and_max_length_json_schema() {
    #[model_schema()]
    #[derive(Serialize, Deserialize)]
    pub struct MinMaxLengthJsonSchema {
        #[model_schema_prop(minLength = 5, maxLength = 50)]
        pub username: String,
    }

    let schema = MinMaxLengthJsonSchema::json_schema();
    let schema_str = serde_json::to_string(&schema).unwrap();
    assert!(
        schema_str.contains("\"minLength\":5"),
        "Expected minLength:5 in JSON schema: {schema_str}"
    );
    assert!(
        schema_str.contains("\"maxLength\":50"),
        "Expected maxLength:50 in JSON schema: {schema_str}"
    );
}

// ==================== String constraint: maxLength — Rust validation ====================

#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn test_max_length_rust_valid() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    pub struct MaxLengthValid {
        #[model_schema_prop(maxLength = 10)]
        pub name: String,
    }

    let valid = r#"{"name": "hello"}"#;
    let result: Result<MaxLengthValid, _> = serde_json::from_str(valid);
    assert!(
        result.is_ok(),
        "String within maxLength should succeed: {:?}",
        result.err()
    );
}

#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn test_max_length_rust_invalid() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    pub struct MaxLengthInvalid {
        #[model_schema_prop(maxLength = 5)]
        pub name: String,
    }

    let invalid = r#"{"name": "too long value"}"#;
    let result: Result<MaxLengthInvalid, _> = serde_json::from_str(invalid);
    assert!(result.is_err(), "String exceeding maxLength should fail");
    let err_str = result.unwrap_err().to_string();
    assert!(
        err_str.contains("too long"),
        "Error should mention 'too long': {err_str}"
    );
}

// ==================== String constraint: minLength — Rust validation ====================

#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn test_min_length_rust_valid() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    pub struct MinLengthRustValid {
        #[model_schema_prop(minLength = 3)]
        pub name: String,
    }

    let valid = r#"{"name": "hello"}"#;
    let result: Result<MinLengthRustValid, _> = serde_json::from_str(valid);
    assert!(
        result.is_ok(),
        "String meeting minLength should succeed: {:?}",
        result.err()
    );
}

#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn test_min_length_rust_invalid() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    pub struct MinLengthRustInvalid {
        #[model_schema_prop(minLength = 10)]
        pub name: String,
    }

    let invalid = r#"{"name": "hi"}"#;
    let result: Result<MinLengthRustInvalid, _> = serde_json::from_str(invalid);
    assert!(result.is_err(), "String shorter than minLength should fail");
    let err_str = result.unwrap_err().to_string();
    assert!(
        err_str.contains("too short"),
        "Error should mention 'too short': {err_str}"
    );
}

// ==================== Combined string constraints ====================

#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn test_combined_string_constraints_valid() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    pub struct CombinedStringValid {
        #[model_schema_prop(minLength = 3, maxLength = 20, pattern = "^[a-z]+$")]
        pub id: String,
    }

    let valid = r#"{"id": "hello"}"#;
    let result: Result<CombinedStringValid, _> = serde_json::from_str(valid);
    assert!(
        result.is_ok(),
        "Value meeting all constraints should succeed: {:?}",
        result.err()
    );
}

#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn test_combined_string_constraints_too_short() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    pub struct CombinedStringShort {
        #[model_schema_prop(minLength = 5, maxLength = 20, pattern = "^[a-z]+$")]
        pub id: String,
    }

    let invalid = r#"{"id": "ab"}"#;
    let result: Result<CombinedStringShort, _> = serde_json::from_str(invalid);
    assert!(result.is_err(), "Too short value should fail");
}

#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn test_combined_string_constraints_pattern_fail() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    pub struct CombinedStringPattern {
        #[model_schema_prop(minLength = 3, maxLength = 20, pattern = "^[a-z]+$")]
        pub id: String,
    }

    let invalid = r#"{"id": "Hello123"}"#;
    let result: Result<CombinedStringPattern, _> = serde_json::from_str(invalid);
    assert!(result.is_err(), "Value failing pattern should fail");
}

// ==================== validate() method — string ====================

#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn test_validate_method_ok() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    pub struct ValidateOk {
        #[model_schema_prop(minLength = 3, maxLength = 20)]
        pub name: String,
    }

    let instance = ValidateOk {
        name: "hello".to_owned(),
    };
    let result = instance.validate();
    assert!(
        result.is_ok(),
        "validate() should return Ok for valid data: {:?}",
        result.err()
    );
}

#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn test_validate_method_err() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    pub struct ValidateErr {
        #[model_schema_prop(minLength = 10)]
        pub name: String,
    }

    let instance = ValidateErr {
        name: "hi".to_owned(),
    };
    let result = instance.validate();
    assert!(
        result.is_err(),
        "validate() should return Err for invalid data"
    );
    let errors = result.unwrap_err();
    assert!(!errors.is_empty(), "Errors vector should not be empty");
    assert!(
        errors[0].contains("too short"),
        "Error message should mention 'too short': {errors:?}"
    );
}

#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn test_validate_method_multiple_errors() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    pub struct ValidateMultiErr {
        #[model_schema_prop(maxLength = 3)]
        pub code: String,
        #[model_schema_prop(minLength = 10)]
        pub name: String,
    }

    // name is too short, code is too long
    let instance = ValidateMultiErr {
        name: "hi".to_owned(),
        code: "toolongcode".to_owned(),
    };
    let result = instance.validate();
    assert!(
        result.is_err(),
        "validate() should return Err for invalid data"
    );
    let errors = result.unwrap_err();
    assert_eq!(errors.len(), 2, "Should have 2 errors, got: {errors:?}");
}

// ==================== Numeric constraints: minimum/maximum — Zod ====================

#[cfg(all(feature = "serde", feature = "zod"))]
#[test]
fn test_minimum_zod() {
    #[model_schema()]
    #[derive(Serialize, Deserialize)]
    pub struct MinimumZod {
        #[model_schema_prop(minimum = 1)]
        pub count: i32,
    }

    let schema = MinimumZod::zod_schema();
    assert!(
        schema.contains(".min("),
        "Expected .min() in Zod schema: {schema}"
    );
}

#[cfg(all(feature = "serde", feature = "zod"))]
#[test]
fn test_maximum_zod() {
    #[model_schema()]
    #[derive(Serialize, Deserialize)]
    pub struct MaximumZod {
        #[model_schema_prop(maximum = 100)]
        pub count: i32,
    }

    let schema = MaximumZod::zod_schema();
    assert!(
        schema.contains(".max("),
        "Expected .max() in Zod schema: {schema}"
    );
}

#[cfg(all(feature = "serde", feature = "zod"))]
#[test]
fn test_float_minimum_maximum_zod() {
    #[model_schema()]
    #[derive(Serialize, Deserialize)]
    pub struct FloatMinMaxZod {
        #[model_schema_prop(minimum = 0, maximum = 1)]
        pub ratio: f64,
    }

    let schema = FloatMinMaxZod::zod_schema();
    assert!(
        schema.contains(".min("),
        "Expected .min() in Zod schema for f64: {schema}"
    );
    assert!(
        schema.contains(".max("),
        "Expected .max() in Zod schema for f64: {schema}"
    );
}

// ==================== Numeric constraints: minimum/maximum — JSON Schema ====================

#[cfg(all(feature = "serde", feature = "jsonschema"))]
#[test]
fn test_minimum_json_schema() {
    #[model_schema()]
    #[derive(Serialize, Deserialize)]
    pub struct MinimumJsonSchema {
        #[model_schema_prop(minimum = 1)]
        pub count: i32,
    }

    let schema = MinimumJsonSchema::json_schema();
    let schema_str = serde_json::to_string(&schema).unwrap();
    assert!(
        schema_str.contains("\"minimum\""),
        "Expected 'minimum' in JSON schema: {schema_str}"
    );
}

#[cfg(all(feature = "serde", feature = "jsonschema"))]
#[test]
fn test_maximum_json_schema() {
    #[model_schema()]
    #[derive(Serialize, Deserialize)]
    pub struct MaximumJsonSchema {
        #[model_schema_prop(maximum = 100)]
        pub count: i32,
    }

    let schema = MaximumJsonSchema::json_schema();
    let schema_str = serde_json::to_string(&schema).unwrap();
    assert!(
        schema_str.contains("\"maximum\""),
        "Expected 'maximum' in JSON schema: {schema_str}"
    );
}

// ==================== Numeric constraints: minimum/maximum — Rust validation ====================

#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn test_minimum_rust_valid() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    pub struct MinimumRustValid {
        #[model_schema_prop(minimum = 1)]
        pub count: i32,
    }

    let valid = r#"{"count": 5}"#;
    let result: Result<MinimumRustValid, _> = serde_json::from_str(valid);
    assert!(
        result.is_ok(),
        "Value above minimum should succeed: {:?}",
        result.err()
    );
}

#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn test_minimum_rust_invalid() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    pub struct MinimumRustInvalid {
        #[model_schema_prop(minimum = 10)]
        pub count: i32,
    }

    let invalid = r#"{"count": 3}"#;
    let result: Result<MinimumRustInvalid, _> = serde_json::from_str(invalid);
    assert!(result.is_err(), "Value below minimum should fail");
    let err_str = result.unwrap_err().to_string();
    assert!(
        err_str.contains("too small"),
        "Error should mention 'too small': {err_str}"
    );
}

#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn test_maximum_rust_valid() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    pub struct MaximumRustValid {
        #[model_schema_prop(maximum = 100)]
        pub count: i32,
    }

    let valid = r#"{"count": 50}"#;
    let result: Result<MaximumRustValid, _> = serde_json::from_str(valid);
    assert!(
        result.is_ok(),
        "Value below maximum should succeed: {:?}",
        result.err()
    );
}

#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn test_maximum_rust_invalid() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    pub struct MaximumRustInvalid {
        #[model_schema_prop(maximum = 10)]
        pub count: i32,
    }

    let invalid = r#"{"count": 99}"#;
    let result: Result<MaximumRustInvalid, _> = serde_json::from_str(invalid);
    assert!(result.is_err(), "Value exceeding maximum should fail");
    let err_str = result.unwrap_err().to_string();
    assert!(
        err_str.contains("too large"),
        "Error should mention 'too large': {err_str}"
    );
}

#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn test_minimum_at_boundary() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    pub struct MinBoundary {
        #[model_schema_prop(minimum = 5)]
        pub count: i32,
    }

    // Exactly at minimum should pass
    let valid = r#"{"count": 5}"#;
    let result: Result<MinBoundary, _> = serde_json::from_str(valid);
    assert!(
        result.is_ok(),
        "Value exactly at minimum should succeed: {:?}",
        result.err()
    );
}

#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn test_maximum_at_boundary() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    pub struct MaxBoundary {
        #[model_schema_prop(maximum = 5)]
        pub count: i32,
    }

    // Exactly at maximum should pass
    let valid = r#"{"count": 5}"#;
    let result: Result<MaxBoundary, _> = serde_json::from_str(valid);
    assert!(
        result.is_ok(),
        "Value exactly at maximum should succeed: {:?}",
        result.err()
    );
}

// ==================== validate() method — numeric ====================

#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn test_validate_method_numeric_ok() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    pub struct ValidateNumericOk {
        #[model_schema_prop(minimum = 1, maximum = 100)]
        pub count: i32,
    }

    let instance = ValidateNumericOk { count: 50 };
    let result = instance.validate();
    assert!(
        result.is_ok(),
        "validate() should return Ok for valid numeric data: {:?}",
        result.err()
    );
}

#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn test_validate_method_numeric_err() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    pub struct ValidateNumericErr {
        #[model_schema_prop(minimum = 10, maximum = 100)]
        pub count: i32,
    }

    let instance = ValidateNumericErr { count: 3 };
    let result = instance.validate();
    assert!(
        result.is_err(),
        "validate() should return Err for numeric out of range"
    );
    let errors = result.unwrap_err();
    assert!(!errors.is_empty(), "Errors vector should not be empty");
    assert!(
        errors[0].contains("too small"),
        "Error message should mention 'too small': {errors:?}"
    );
}

// ==================== validate() method — pattern ====================

#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn test_validate_method_pattern_ok() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    pub struct ValidatePatternOk {
        #[model_schema_prop(pattern = "^[a-z]+$")]
        pub slug: String,
    }

    let instance = ValidatePatternOk {
        slug: "hello".to_owned(),
    };
    let result = instance.validate();
    assert!(
        result.is_ok(),
        "validate() should return Ok for value matching pattern: {:?}",
        result.err()
    );
}

#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn test_validate_method_pattern_err() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    pub struct ValidatePatternErr {
        #[model_schema_prop(pattern = "^[a-z]+$")]
        pub slug: String,
    }

    let instance = ValidatePatternErr {
        slug: "ABC123".to_owned(),
    };
    let result = instance.validate();
    assert!(
        result.is_err(),
        "validate() should return Err for value not matching pattern"
    );
    let errors = result.unwrap_err();
    assert!(!errors.is_empty(), "Errors vector should not be empty");
    assert!(
        errors[0].contains("does not match pattern"),
        "Error message should mention 'does not match pattern': {errors:?}"
    );
}

#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn test_validate_method_pattern_and_length() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    pub struct ValidatePatternLength {
        #[model_schema_prop(pattern = "^[a-z]+$", minLength = 3, maxLength = 10)]
        pub tag: String,
    }

    // Valid case
    let valid = ValidatePatternLength {
        tag: "hello".to_owned(),
    };
    assert!(
        valid.validate().is_ok(),
        "Valid value should pass all constraints"
    );

    // Too short — minLength check fires first
    let too_short = ValidatePatternLength {
        tag: "ab".to_owned(),
    };
    let too_short_result = too_short.validate();
    assert!(too_short_result.is_err(), "Too short value should fail");
    let too_short_errors = too_short_result.unwrap_err();
    assert!(
        too_short_errors.iter().any(|e| e.contains("too short")),
        "Should report 'too short' error: {too_short_errors:?}"
    );

    // Pattern failure (uppercase)
    let bad_pattern = ValidatePatternLength {
        tag: "Hello".to_owned(),
    };
    let bad_pattern_result = bad_pattern.validate();
    assert!(
        bad_pattern_result.is_err(),
        "Value not matching pattern should fail"
    );
    let bad_pattern_errors = bad_pattern_result.unwrap_err();
    assert!(
        bad_pattern_errors
            .iter()
            .any(|e| e.contains("does not match pattern")),
        "Should report 'does not match pattern' error: {bad_pattern_errors:?}"
    );
}

// ==================== validate() method — maxLength ====================

#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn test_validate_method_max_length_ok() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    pub struct ValidateMaxLenOk {
        #[model_schema_prop(maxLength = 20)]
        pub label: String,
    }

    let instance = ValidateMaxLenOk {
        label: "short".to_owned(),
    };
    let result = instance.validate();
    assert!(
        result.is_ok(),
        "validate() should return Ok for value within maxLength: {:?}",
        result.err()
    );
}

#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn test_validate_method_max_length_err() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    pub struct ValidateMaxLenErr {
        #[model_schema_prop(maxLength = 5)]
        pub label: String,
    }

    let instance = ValidateMaxLenErr {
        label: "way too long value".to_owned(),
    };
    let result = instance.validate();
    assert!(
        result.is_err(),
        "validate() should return Err for value exceeding maxLength"
    );
    let errors = result.unwrap_err();
    assert!(!errors.is_empty(), "Errors vector should not be empty");
    assert!(
        errors[0].contains("too long"),
        "Error message should mention 'too long': {errors:?}"
    );
}

// ==================== validate() method — mixed string and numeric ====================

#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn test_validate_method_mixed_string_numeric() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    pub struct ValidateMixed {
        #[model_schema_prop(minimum = 10)]
        pub score: i32,
        #[model_schema_prop(minLength = 5)]
        pub title: String,
    }

    // Both fields invalid
    let instance = ValidateMixed {
        title: "hi".to_owned(),
        score: 2,
    };
    let result = instance.validate();
    assert!(
        result.is_err(),
        "validate() should return Err when both fields are invalid"
    );
    let errors = result.unwrap_err();
    assert_eq!(
        errors.len(),
        2,
        "Should have exactly 2 errors, got: {errors:?}"
    );
    assert!(
        errors.iter().any(|e| e.contains("too short")),
        "Should contain string 'too short' error: {errors:?}"
    );
    assert!(
        errors.iter().any(|e| e.contains("too small")),
        "Should contain numeric 'too small' error: {errors:?}"
    );
}

// ==================== Float numeric validation — Rust (serde) ====================

#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn test_minimum_float_rust_valid() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    pub struct MinFloatValid {
        #[model_schema_prop(minimum = 0)]
        pub ratio: f64,
    }

    let valid = r#"{"ratio": 0.5}"#;
    let result: Result<MinFloatValid, _> = serde_json::from_str(valid);
    assert!(
        result.is_ok(),
        "Float above minimum should succeed: {:?}",
        result.err()
    );
}

#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn test_minimum_float_rust_invalid() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    pub struct MinFloatInvalid {
        #[model_schema_prop(minimum = 1)]
        pub ratio: f64,
    }

    let invalid = r#"{"ratio": -0.5}"#;
    let result: Result<MinFloatInvalid, _> = serde_json::from_str(invalid);
    assert!(result.is_err(), "Float below minimum should fail");
    let err_str = result.unwrap_err().to_string();
    assert!(
        err_str.contains("too small"),
        "Error should mention 'too small': {err_str}"
    );
}

#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn test_maximum_float_rust_valid() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    pub struct MaxFloatValid {
        #[model_schema_prop(maximum = 100)]
        pub value: f64,
    }

    let valid = r#"{"value": 50.0}"#;
    let result: Result<MaxFloatValid, _> = serde_json::from_str(valid);
    assert!(
        result.is_ok(),
        "Float below maximum should succeed: {:?}",
        result.err()
    );
}

#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn test_maximum_float_rust_invalid() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    pub struct MaxFloatInvalid {
        #[model_schema_prop(maximum = 10)]
        pub value: f64,
    }

    let invalid = r#"{"value": 15.0}"#;
    let result: Result<MaxFloatInvalid, _> = serde_json::from_str(invalid);
    assert!(result.is_err(), "Float above maximum should fail");
    let err_str = result.unwrap_err().to_string();
    assert!(
        err_str.contains("too large"),
        "Error should mention 'too large': {err_str}"
    );
}

// ==================== validate() method — float ====================

#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn test_validate_method_float_err() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    pub struct ValidateFloatErr {
        #[model_schema_prop(minimum = 0, maximum = 100)]
        pub percentage: f64,
    }

    let instance = ValidateFloatErr { percentage: 150.0 };
    let result = instance.validate();
    assert!(
        result.is_err(),
        "validate() should return Err for float out of range"
    );
    let errors = result.unwrap_err();
    assert!(!errors.is_empty(), "Errors vector should not be empty");
    assert!(
        errors[0].contains("too large"),
        "Error message should mention 'too large': {errors:?}"
    );
}

// ==================== Combined minimum+maximum — Zod ====================

#[cfg(all(feature = "serde", feature = "zod"))]
#[test]
fn test_min_max_numeric_zod() {
    #[model_schema()]
    #[derive(Serialize, Deserialize)]
    pub struct MinMaxNumericZod {
        #[model_schema_prop(minimum = 1, maximum = 100)]
        pub level: i32,
    }

    let schema = MinMaxNumericZod::zod_schema();
    assert!(
        schema.contains(".min("),
        "Expected .min() in Zod schema: {schema}"
    );
    assert!(
        schema.contains(".max("),
        "Expected .max() in Zod schema: {schema}"
    );
}

// ==================== Combined minimum+maximum — JSON Schema ====================

#[cfg(all(feature = "serde", feature = "jsonschema"))]
#[test]
fn test_min_max_numeric_json_schema() {
    #[model_schema()]
    #[derive(Serialize, Deserialize)]
    pub struct MinMaxNumericJsonSchema {
        #[model_schema_prop(minimum = 1, maximum = 100)]
        pub level: i32,
    }

    let schema = MinMaxNumericJsonSchema::json_schema();
    let schema_str = serde_json::to_string(&schema).unwrap();
    assert!(
        schema_str.contains("\"minimum\""),
        "Expected 'minimum' in JSON schema: {schema_str}"
    );
    assert!(
        schema_str.contains("\"maximum\""),
        "Expected 'maximum' in JSON schema: {schema_str}"
    );
}

// ==================== TypeScript unaffected by constraints ====================

#[cfg(all(feature = "serde", feature = "typescript"))]
#[test]
fn test_constraints_dont_affect_typescript() {
    #[model_schema()]
    #[derive(Serialize, Deserialize)]
    pub struct ConstraintsTs {
        #[model_schema_prop(minimum = 0, maximum = 120)]
        pub age: u32,
        #[model_schema_prop(minLength = 3, maxLength = 50, pattern = "^[a-z]+$")]
        pub username: String,
    }

    let ts = ConstraintsTs::ts_definition();
    // Extract just the type body (after "export type ... = {" and before the closing "};")
    // The TypeScript type syntax itself should use plain `string` and `number` — no Zod
    // or JSON Schema constraint methods like `.min()`, `.max()`, `.check()`, `.regex()`.
    assert!(
        ts.contains("export type"),
        "Should contain 'export type': {ts}"
    );
    let type_body_start = ts.find("export type").unwrap();
    let type_section = &ts[type_body_start..];
    assert!(
        !type_section.contains(".min("),
        "TypeScript type should not contain .min(): {type_section}"
    );
    assert!(
        !type_section.contains(".max("),
        "TypeScript type should not contain .max(): {type_section}"
    );
    assert!(
        !type_section.contains(".check("),
        "TypeScript type should not contain .check(): {type_section}"
    );
    assert!(
        !type_section.contains("z.regex"),
        "TypeScript type should not contain z.regex: {type_section}"
    );
    assert!(
        !type_section.contains("z.string"),
        "TypeScript type should not contain z.string: {type_section}"
    );
    assert!(
        !type_section.contains("z.number"),
        "TypeScript type should not contain z.number: {type_section}"
    );
    // Type fields should still be plain `string` and `number`
    assert!(
        type_section.contains("username: string"),
        "TypeScript should have 'username: string': {type_section}"
    );
    assert!(
        type_section.contains("age: number"),
        "TypeScript should have 'age: number': {type_section}"
    );
}

// ==================== Edge cases ====================

#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn test_boundary_min_length_zero() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    pub struct MinLenZero {
        #[model_schema_prop(minLength = 0)]
        pub tag: String,
    }

    // Empty string should pass with minLength = 0
    let instance = MinLenZero { tag: String::new() };
    let validate_result = instance.validate();
    assert!(
        validate_result.is_ok(),
        "Empty string should pass minLength = 0: {:?}",
        validate_result.err()
    );

    // Also test via serde
    let valid = r#"{"tag": ""}"#;
    let serde_result: Result<MinLenZero, _> = serde_json::from_str(valid);
    assert!(
        serde_result.is_ok(),
        "Empty string via serde should pass minLength = 0: {:?}",
        serde_result.err()
    );
}

#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn test_pattern_empty_string_match() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    pub struct PatternEmpty {
        #[model_schema_prop(pattern = "^\\s*$")]
        pub empty_field: String,
    }

    // Empty string should match "^\s*$"
    let valid_instance = PatternEmpty {
        empty_field: String::new(),
    };
    let valid_result = valid_instance.validate();
    assert!(
        valid_result.is_ok(),
        "Empty string should match pattern '^\\s*$': {:?}",
        valid_result.err()
    );

    // Non-empty string should NOT match "^\s*$"
    let invalid_instance = PatternEmpty {
        empty_field: "not empty".to_owned(),
    };
    let invalid_result = invalid_instance.validate();
    assert!(
        invalid_result.is_err(),
        "Non-empty string should not match pattern '^\\s*$'"
    );
    let errors = invalid_result.unwrap_err();
    assert!(
        errors[0].contains("does not match pattern"),
        "Error should mention 'does not match pattern': {errors:?}"
    );
}

// ==================== Constraints under Option / wrappers / sequences ====================

#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn test_validate_optional_string_some_too_short() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    pub struct OptionalConstrained {
        #[model_schema_prop(minLength = 3)]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub nickname: Option<String>,
    }

    let instance = OptionalConstrained {
        nickname: Some("a".to_owned()),
    };
    let result = instance.validate();
    assert!(
        result.is_err(),
        "validate() should reject a Some holding a too-short string"
    );
    let errors = result.unwrap_err();
    assert_eq!(
        errors,
        vec!["'nickname' is too short: minimum length is 3, got 1"],
        "Unexpected errors: {errors:?}"
    );
}

#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn test_validate_optional_string_none_is_ok() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    pub struct OptionalAbsent {
        #[model_schema_prop(minLength = 3)]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub nickname: Option<String>,
    }

    let instance = OptionalAbsent { nickname: None };
    let result = instance.validate();
    assert!(
        result.is_ok(),
        "A None writes no string, so nothing constrains it: {:?}",
        result.err()
    );
}

#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn test_validate_boxed_string_too_short() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    pub struct BoxedConstrained {
        #[model_schema_prop(minLength = 3)]
        pub label: Box<str>,
    }

    let instance = BoxedConstrained { label: "a".into() };
    let result = instance.validate();
    assert!(
        result.is_err(),
        "validate() should reject a too-short string under a Box"
    );
    let errors = result.unwrap_err();
    assert_eq!(
        errors,
        vec!["'label' is too short: minimum length is 3, got 1"],
        "Unexpected errors: {errors:?}"
    );
}

#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn test_validate_vec_string_per_element() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    pub struct VecConstrained {
        #[model_schema_prop(minLength = 3)]
        pub tags: Vec<String>,
    }

    let instance = VecConstrained {
        tags: vec!["ok!".to_owned(), "a".to_owned(), "b".to_owned()],
    };
    let result = instance.validate();
    let errors = result.unwrap_err();
    assert_eq!(
        errors,
        vec![
            "'tags' is too short: minimum length is 3, got 1",
            "'tags' is too short: minimum length is 3, got 1",
        ],
        "Each failing element reports: {errors:?}"
    );
}

#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn test_validate_optional_vec_string() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    pub struct OptionalVecConstrained {
        #[model_schema_prop(minLength = 3)]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub tags: Option<Vec<String>>,
    }

    let present = OptionalVecConstrained {
        tags: Some(vec!["a".to_owned()]),
    };
    let errors = present.validate().unwrap_err();
    assert_eq!(
        errors,
        vec!["'tags' is too short: minimum length is 3, got 1"],
        "Unexpected errors: {errors:?}"
    );

    let absent = OptionalVecConstrained { tags: None };
    assert!(
        absent.validate().is_ok(),
        "A None holds no elements to constrain"
    );
}

#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn test_validate_nested_vec_string() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    pub struct NestedVecConstrained {
        #[model_schema_prop(minLength = 3)]
        pub rows: Vec<Vec<String>>,
    }

    let instance = NestedVecConstrained {
        rows: vec![vec!["ok!".to_owned()], vec!["a".to_owned()]],
    };
    let errors = instance.validate().unwrap_err();
    assert_eq!(
        errors,
        vec!["'rows' is too short: minimum length is 3, got 1"],
        "The constraint lands on the innermost element: {errors:?}"
    );
}

#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn test_validate_optional_numeric_minimum() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    pub struct OptionalNumeric {
        #[model_schema_prop(minimum = 18)]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub age: Option<u32>,
    }

    let too_small = OptionalNumeric { age: Some(5) };
    let errors = too_small.validate().unwrap_err();
    assert_eq!(
        errors,
        vec!["'age' is too small: minimum is 18, got 5"],
        "Unexpected errors: {errors:?}"
    );

    assert!(
        OptionalNumeric { age: None }.validate().is_ok(),
        "A None writes no number to constrain"
    );
}

#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn test_validate_arc_slice_mixed_wrappers() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    pub struct MixedWrappers {
        #[model_schema_prop(maxLength = 2)]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub codes: Option<Arc<[String]>>,
    }

    let instance = MixedWrappers {
        codes: Some(Arc::from(["ok".to_owned(), "toolong".to_owned()])),
    };
    let errors = instance.validate().unwrap_err();
    assert_eq!(
        errors,
        vec!["'codes' is too long: maximum length is 2, got 7"],
        "Unexpected errors: {errors:?}"
    );
}

#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn test_validate_boxed_option_string() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    pub struct BoxedOption {
        #[model_schema_prop(minLength = 3)]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub nickname: Box<Option<String>>,
    }

    let instance = BoxedOption {
        nickname: Box::new(Some("a".to_owned())),
    };
    let errors = instance.validate().unwrap_err();
    assert_eq!(
        errors,
        vec!["'nickname' is too short: minimum length is 3, got 1"],
        "Unexpected errors: {errors:?}"
    );

    assert!(
        BoxedOption {
            nickname: Box::new(None)
        }
        .validate()
        .is_ok(),
        "A None under a Box still writes nothing to constrain"
    );
}

// ==================== Constraints on the wire, under Option / wrappers / sequences ====================

/// The gate every consumer reaches first. A constraint describes the value the field puts on the
/// wire, so a payload carrying a value the constraint rejects is rejected as it is read — at the
/// same place, and with the same message, that `validate()` would answer with.
#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn test_deserialize_optional_string_rejects_a_short_some() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    pub struct WireOptional {
        #[model_schema_prop(minLength = 3)]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub nickname: Option<String>,
    }

    let error = serde_json::from_str::<WireOptional>(r#"{"nickname":"a"}"#).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("'nickname' is too short: minimum length is 3, got 1"),
        "Unexpected error: {error}"
    );

    let accepted = serde_json::from_str::<WireOptional>(r#"{"nickname":"abc"}"#).unwrap();
    assert_eq!(accepted.nickname.as_deref(), Some("abc"));
}

/// A `None` puts no string on the wire, so neither spelling of its absence has anything to reject.
#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn test_deserialize_optional_string_still_admits_an_absent_key() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    pub struct WireAbsent {
        #[serde(skip_serializing_if = "Option::is_none")]
        #[model_schema_prop(minLength = 3)]
        pub nickname: Option<String>,
    }

    assert!(
        serde_json::from_str::<WireAbsent>("{}")
            .unwrap()
            .nickname
            .is_none(),
        "A missing key is what the generated schemas describe as the absent form"
    );
    assert!(
        serde_json::from_str::<WireAbsent>(r#"{"nickname":null}"#)
            .unwrap()
            .nickname
            .is_none(),
        "A null reads as the same None it always did"
    );
}

/// A field that writes its own default keeps it: the hook is given no second one.
#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn test_deserialize_optional_string_keeps_a_written_default() {
    fn preset() -> Option<String> {
        Some("preset".to_owned()).filter(|preset| preset.len() >= 3)
    }

    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    pub struct WirePreset {
        #[serde(default = "preset", skip_serializing_if = "Option::is_none")]
        #[model_schema_prop(minLength = 3)]
        pub nickname: Option<String>,
    }

    assert_eq!(
        serde_json::from_str::<WirePreset>("{}")
            .unwrap()
            .nickname
            .as_deref(),
        Some("preset"),
        "The field's own default is what answers for its missing key"
    );
}

/// A transparent wrapper writes its inner value and nothing else, so the wire the constraint
/// describes is the same one a bare field writes.
#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn test_deserialize_boxed_string_rejects_a_short_value() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    pub struct WireBoxed {
        #[model_schema_prop(minLength = 3)]
        pub label: Box<str>,
    }

    let error = serde_json::from_str::<WireBoxed>(r#"{"label":"a"}"#).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("'label' is too short: minimum length is 3, got 1"),
        "Unexpected error: {error}"
    );
    assert_eq!(
        &*serde_json::from_str::<WireBoxed>(r#"{"label":"abc"}"#)
            .unwrap()
            .label,
        "abc"
    );
}

/// A `Cow` arrives as the `Box` does, its lifetime being no part of what it writes.
#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn test_deserialize_cow_string_rejects_a_short_value() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    pub struct WireCow {
        #[model_schema_prop(minLength = 3)]
        pub label: Cow<'static, str>,
    }

    let error = serde_json::from_str::<WireCow>(r#"{"label":"a"}"#).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("'label' is too short: minimum length is 3, got 1"),
        "Unexpected error: {error}"
    );
    assert_eq!(
        serde_json::from_str::<WireCow>(r#"{"label":"abc"}"#)
            .unwrap()
            .label,
        "abc"
    );
}

/// A sequence writes an array of the constrained value, so one failing element fails the read.
#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn test_deserialize_vec_string_rejects_a_short_element() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    pub struct WireVec {
        #[model_schema_prop(minLength = 3)]
        pub tags: Vec<String>,
    }

    let error = serde_json::from_str::<WireVec>(r#"{"tags":["ok!","a"]}"#).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("'tags' is too short: minimum length is 3, got 1"),
        "Unexpected error: {error}"
    );
    assert_eq!(
        serde_json::from_str::<WireVec>(r#"{"tags":["ok!","two"]}"#)
            .unwrap()
            .tags,
        vec!["ok!".to_owned(), "two".to_owned()]
    );
    assert!(
        serde_json::from_str::<WireVec>(r#"{"tags":[]}"#)
            .unwrap()
            .tags
            .is_empty(),
        "An empty array writes no element to constrain"
    );
}

/// The wrappers compose on the wire in the order they were written.
#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn test_deserialize_optional_vec_string_rejects_a_short_element() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    pub struct WireOptionalVec {
        #[model_schema_prop(minLength = 3)]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub tags: Option<Vec<String>>,
    }

    let error = serde_json::from_str::<WireOptionalVec>(r#"{"tags":["ok!","a"]}"#).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("'tags' is too short: minimum length is 3, got 1"),
        "Unexpected error: {error}"
    );
    assert!(
        serde_json::from_str::<WireOptionalVec>("{}")
            .unwrap()
            .tags
            .is_none(),
        "A missing key is still the absent form the schemas describe"
    );
}

/// A range describes the number on the wire wherever the field wrote it, exactly as a length
/// describes the string.
#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn test_deserialize_optional_numeric_rejects_an_out_of_range_some() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    pub struct WireNumeric {
        #[model_schema_prop(minimum = 18, maximum = 120)]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub age: Option<u32>,
        #[model_schema_prop(minimum = 1)]
        pub counts: Vec<u32>,
    }

    let out_of_range =
        serde_json::from_str::<WireNumeric>(r#"{"age":5,"counts":[1]}"#).unwrap_err();
    assert!(
        out_of_range
            .to_string()
            .contains("'age' is too small: minimum is 18, got 5"),
        "Unexpected error: {out_of_range}"
    );

    let under_element =
        serde_json::from_str::<WireNumeric>(r#"{"age":21,"counts":[1,0]}"#).unwrap_err();
    assert!(
        under_element
            .to_string()
            .contains("'counts' is too small: minimum is 1, got 0"),
        "Unexpected error: {under_element}"
    );

    let accepted = serde_json::from_str::<WireNumeric>(r#"{"age":21,"counts":[1]}"#).unwrap();
    assert_eq!(accepted.age, Some(21));
}

/// What the wire admits and what `validate()` admits are the same set — a value one accepts and the
/// other rejects is the disagreement this covers.
#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn test_deserialize_and_validate_agree_on_every_wrapped_shape() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    pub struct WireAgreement {
        #[model_schema_prop(minLength = 3)]
        pub boxed: Box<str>,
        #[model_schema_prop(minLength = 3)]
        pub cow: Cow<'static, str>,
        #[model_schema_prop(minLength = 3)]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub nested: Option<Vec<String>>,
        #[model_schema_prop(minLength = 3)]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub opt: Option<String>,
        #[model_schema_prop(minLength = 3)]
        pub plain: String,
        #[model_schema_prop(minLength = 3)]
        pub tags: Vec<String>,
    }

    const GOOD: &str =
        r#"{"opt":"aaa","boxed":"bbb","tags":["ccc"],"cow":"ddd","nested":["eee"],"plain":"fff"}"#;

    let accepted = serde_json::from_str::<WireAgreement>(GOOD).unwrap();
    assert!(
        accepted.validate().is_ok(),
        "A payload the wire admits must be one validate() admits"
    );

    for (field, short) in [
        ("opt", r#""a""#),
        ("boxed", r#""b""#),
        ("tags", r#"["c"]"#),
        ("cow", r#""d""#),
        ("nested", r#"["e"]"#),
        ("plain", r#""f""#),
    ] {
        let mut payload: serde_json::Value = serde_json::from_str(GOOD).unwrap();
        payload[field] = serde_json::from_str(short).unwrap();
        let error = serde_json::from_str::<WireAgreement>(&payload.to_string()).unwrap_err();
        assert!(
            error.to_string().contains(&format!(
                "'{field}' is too short: minimum length is 3, got 1"
            )),
            "field {field} was admitted by the wire but is rejected by validate(): {error}"
        );
    }
}

/// A field name is unique only within the variant that declares it, so the helpers a variant's
/// field generates are named for that variant: two variants naming one field carry two constraints.
#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn test_two_variants_naming_one_field_keep_their_own_constraints() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    #[serde(tag = "kind")]
    pub enum Action {
        Delete {
            #[model_schema_prop(minLength = 5)]
            note: String,
        },
        Upload {
            #[model_schema_prop(minLength = 3)]
            note: String,
        },
    }

    let too_short_for_delete =
        serde_json::from_str::<Action>(r#"{"kind":"Delete","note":"abc"}"#).unwrap_err();
    assert!(
        too_short_for_delete
            .to_string()
            .contains("'note' is too short: minimum length is 5, got 3"),
        "Unexpected error: {too_short_for_delete}"
    );
    assert!(
        serde_json::from_str::<Action>(r#"{"kind":"Delete","note":"abcde"}"#).is_ok(),
        "Delete admits its own minimum"
    );

    let too_short_for_upload =
        serde_json::from_str::<Action>(r#"{"kind":"Upload","note":"ab"}"#).unwrap_err();
    assert!(
        too_short_for_upload
            .to_string()
            .contains("'note' is too short: minimum length is 3, got 2"),
        "Unexpected error: {too_short_for_upload}"
    );
    assert!(
        serde_json::from_str::<Action>(r#"{"kind":"Upload","note":"abc"}"#).is_ok(),
        "A value Upload admits is not held to Delete's minimum"
    );
}

// ==================== validate() method — enum members ====================

/// The struct and its single-variant tagged twin carry the same constraint on the same member, so
/// an author who changes the one declaration into the other reads the identical sentence back.
#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn test_tagged_enum_validate_answers_in_the_struct_twins_words() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    pub struct SlugStruct {
        #[model_schema_prop(minLength = 2)]
        pub slug: String,
    }

    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    #[serde(tag = "kind")]
    pub enum SlugTagged {
        One {
            #[model_schema_prop(minLength = 2)]
            slug: String,
        },
    }

    let from_struct = SlugStruct {
        slug: "A".to_owned(),
    }
    .validate()
    .unwrap_err();
    let from_enum = SlugTagged::One {
        slug: "A".to_owned(),
    }
    .validate()
    .unwrap_err();

    assert_eq!(
        from_struct,
        vec!["'slug' is too short: minimum length is 2, got 1".to_owned()]
    );
    assert_eq!(from_enum, from_struct);

    SlugTagged::One {
        slug: "ab".to_owned(),
    }
    .validate()
    .unwrap();
}

/// The tag decides how a value is written, not which of its members carry a bound — so every
/// tagging serde offers publishes the accessor, and all three answer the same violation alike.
#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn test_every_tagged_flavor_publishes_validate_for_its_constrained_members() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    pub enum External {
        One {
            #[model_schema_prop(minLength = 2)]
            slug: String,
        },
    }

    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    #[serde(tag = "kind")]
    pub enum Internal {
        One {
            #[model_schema_prop(minLength = 2)]
            slug: String,
        },
    }

    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    #[serde(tag = "kind", content = "data")]
    pub enum Adjacent {
        One {
            #[model_schema_prop(minLength = 2)]
            slug: String,
        },
    }

    let expected = vec!["'slug' is too short: minimum length is 2, got 1".to_owned()];
    assert_eq!(
        External::One {
            slug: "A".to_owned()
        }
        .validate()
        .unwrap_err(),
        expected
    );
    assert_eq!(
        Internal::One {
            slug: "A".to_owned()
        }
        .validate()
        .unwrap_err(),
        expected
    );
    assert_eq!(
        Adjacent::One {
            slug: "A".to_owned()
        }
        .validate()
        .unwrap_err(),
        expected
    );
}

/// A value holds one variant at a time, so the walk runs that variant's checks and no other's —
/// two variants naming one member are two constraints, and only the held one answers.
#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn test_enum_validate_runs_only_the_held_variants_checks() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    #[serde(tag = "kind")]
    pub enum Action {
        Delete {
            #[model_schema_prop(minLength = 5)]
            note: String,
        },
        Upload {
            #[model_schema_prop(minLength = 3)]
            note: String,
        },
    }

    assert_eq!(
        Action::Delete {
            note: "abc".to_owned()
        }
        .validate()
        .unwrap_err(),
        vec!["'note' is too short: minimum length is 5, got 3".to_owned()]
    );
    assert!(
        Action::Upload {
            note: "abc".to_owned()
        }
        .validate()
        .is_ok(),
        "a value Upload admits is not held to Delete's minimum"
    );
}

/// A variant that carries no bound contributes no check, and one that carries several answers with
/// every violation at once — the struct accessor's own collecting shape, per arm.
#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn test_enum_validate_collects_every_violation_of_the_held_variant() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    #[serde(tag = "kind")]
    pub enum Mixed {
        Bounded {
            #[model_schema_prop(maxLength = 3)]
            code: String,
            #[model_schema_prop(minimum = 10)]
            size: u32,
        },
        Free {
            note: String,
        },
        Nothing,
    }

    assert_eq!(
        Mixed::Bounded {
            code: "toolong".to_owned(),
            size: 1,
        }
        .validate()
        .unwrap_err(),
        vec![
            "'code' is too long: maximum length is 3, got 7".to_owned(),
            "'size' is too small: minimum is 10, got 1".to_owned(),
        ]
    );
    Mixed::Free {
        note: "anything".to_owned(),
    }
    .validate()
    .unwrap();
    Mixed::Nothing.validate().unwrap();
}

/// A member written under wrappers is checked where the constraint lands, exactly as the same field
/// written in a struct is: a `None` writes nothing to check, and a sequence answers per element.
#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn test_enum_validate_reaches_through_a_members_wrappers() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    #[serde(tag = "kind")]
    pub enum Wrapped {
        One {
            #[model_schema_prop(minLength = 2)]
            tags: Vec<String>,
            #[model_schema_prop(minLength = 2)]
            #[serde(skip_serializing_if = "Option::is_none")]
            note: Option<String>,
        },
    }

    assert!(
        Wrapped::One {
            tags: vec!["ab".to_owned()],
            note: None,
        }
        .validate()
        .is_ok(),
        "a None writes nothing for the bound to describe"
    );
    assert_eq!(
        Wrapped::One {
            tags: vec!["ab".to_owned(), "c".to_owned()],
            note: Some("d".to_owned()),
        }
        .validate()
        .unwrap_err(),
        vec![
            "'tags' is too short: minimum length is 2, got 1".to_owned(),
            "'note' is too short: minimum length is 2, got 1".to_owned(),
        ]
    );
}

/// Parity with the struct convention: an enum whose members carry no bound publishes no accessor,
/// which is what a constraint-free struct has always published. The trait's method is reached only
/// because no inherent one shadows it.
#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn test_a_constraint_free_enum_publishes_no_validate_just_as_a_struct_does() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    pub struct FreeStruct {
        pub name: String,
    }

    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    #[serde(tag = "kind")]
    pub enum FreeTagged {
        One { name: String },
        Two,
    }

    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    #[serde(tag = "kind")]
    pub enum BoundTagged {
        One {
            #[model_schema_prop(minLength = 2)]
            name: String,
        },
    }

    impl UnpublishedValidate for FreeStruct {}
    impl UnpublishedValidate for FreeTagged {}

    assert_eq!(
        FreeStruct {
            name: String::new()
        }
        .validate(),
        "no inherent validate()"
    );
    assert_eq!(
        FreeTagged::One {
            name: String::new()
        }
        .validate(),
        "no inherent validate()"
    );
    assert_eq!(FreeTagged::Two.validate(), "no inherent validate()");
    // An enum whose member does carry a bound answers with the accessor's own type, which is what
    // makes the assertions above a statement about publication rather than about the trait: a
    // published `validate()` would shadow the trait's and none of them would even compile.
    assert_eq!(
        BoundTagged::One {
            name: "A".to_owned()
        }
        .validate()
        .unwrap_err(),
        vec!["'name' is too short: minimum length is 2, got 1".to_owned()]
    );
}

/// The arm binds each constrained member under a name of its own, so a member spelled like
/// something the body already reads is still checked rather than taking that name over: `errors` is
/// the accumulator every check pushes into, and `value_0` is the head of the walk a wrapped member
/// is reached through.
#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn test_a_member_named_like_the_walks_own_bindings_is_still_checked() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    #[serde(tag = "kind")]
    pub enum Shadowing {
        One {
            #[model_schema_prop(minLength = 2)]
            errors: String,
            #[model_schema_prop(minLength = 2)]
            value_0: Vec<String>,
        },
    }

    assert_eq!(
        Shadowing::One {
            errors: "A".to_owned(),
            value_0: vec!["B".to_owned()],
        }
        .validate()
        .unwrap_err(),
        vec![
            "'errors' is too short: minimum length is 2, got 1".to_owned(),
            "'value_0' is too short: minimum length is 2, got 1".to_owned(),
        ]
    );
}

/// The match names every variant the enum declares, whatever shape each one was written in, so a
/// value of any of them reaches the accessor and only the constrained one answers.
#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[test]
fn test_every_variant_shape_reaches_the_accessor() {
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug)]
    pub enum EveryShape {
        Bare {
            note: String,
        },
        Named {
            #[model_schema_prop(minLength = 2)]
            slug: String,
        },
        Nothing,
        Pair(String, String),
        Single(String),
    }

    assert_eq!(
        EveryShape::Named {
            slug: "A".to_owned()
        }
        .validate()
        .unwrap_err(),
        vec!["'slug' is too short: minimum length is 2, got 1".to_owned()]
    );
    EveryShape::Bare {
        note: "A".to_owned(),
    }
    .validate()
    .unwrap();
    EveryShape::Single("A".to_owned()).validate().unwrap();
    EveryShape::Pair("A".to_owned(), "B".to_owned())
        .validate()
        .unwrap();
    EveryShape::Nothing.validate().unwrap();
}

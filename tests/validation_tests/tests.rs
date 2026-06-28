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

use serde::{Deserialize, Serialize};
use tixschema::model_schema;

// Tests requiring both zod and typescript features
#[cfg(all(feature = "zod", feature = "typescript"))]
mod zod_ts_tests {
    use super::*;

    #[model_schema()]
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct RoleId<ID_TYPE>(pub ID_TYPE);

    #[model_schema()]
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct CorrelationId(pub String);

    #[test]
    fn test_branded_newtype_ts_definition() {
        let ts = RoleId::<String>::ts_definition();
        assert!(
            ts.contains("export type RoleId<ID_TYPE> = ID_TYPE & $brand<\"RoleId\">"),
            "Got: {ts}"
        );
    }

    #[test]
    fn test_branded_newtype_zod_schema() {
        let zod = RoleId::<String>::zod_schema();
        assert!(zod.contains("z.string().brand"), "Got: {zod}");
        assert!(zod.contains("$ZodBranded<ZodString, \"RoleId\">"), "Got: {zod}");
    }

    #[test]
    fn test_branded_newtype_preserves_generic_param_name() {
        let ts = RoleId::<String>::ts_definition();
        // Should contain ID_TYPE not T or any other name
        assert!(
            ts.contains("ID_TYPE"),
            "Should preserve generic param name. Got: {ts}"
        );
    }

    #[test]
    fn test_branded_newtype_non_generic() {
        let ts = CorrelationId::ts_definition();
        assert!(
            ts.contains("export type CorrelationId = string & $brand<\"CorrelationId\">"),
            "Got: {ts}"
        );
    }

    #[test]
    fn test_branded_newtype_non_generic_zod() {
        let zod = CorrelationId::zod_schema();
        assert!(zod.contains("z.string().brand"), "Got: {zod}");
        assert!(zod.contains("$ZodBranded<ZodString, \"CorrelationId\">"), "Got: {zod}");
    }

    #[test]
    fn test_branded_newtype_non_generic_zod_schema_content() {
        let zod = CorrelationId::zod_schema();
        // Should contain the raw schema definition
        assert!(
            zod.contains("const CorrelationId$RawSchema = z.string().brand"),
            "Should contain raw schema. Got: {zod}"
        );
        // Should contain the exported typed schema
        assert!(
            zod.contains("export const CorrelationId$Schema: $ZodBranded<ZodString, \"CorrelationId\"> = CorrelationId$RawSchema"),
            "Should contain exported schema referencing raw schema. Got: {zod}"
        );
    }

    #[test]
    fn test_branded_newtype_generic_helpers() {
        let ts = RoleId::<String>::ts_definition();
        assert!(
            ts.contains("function isRoleId<T>(_value: T): _value is RoleId<T>"),
            "Should have generic isRoleId guard. Got: {ts}"
        );
        assert!(
            ts.contains("export function createRoleId<T>(value: T): RoleId<T>"),
            "Should have generic createRoleId factory. Got: {ts}"
        );
    }

    #[test]
    fn test_branded_newtype_non_generic_helpers() {
        let ts = CorrelationId::ts_definition();
        assert!(
            ts.contains("function isCorrelationId(_value: string): _value is CorrelationId"),
            "Should have non-generic isCorrelationId guard. Got: {ts}"
        );
        assert!(
            ts.contains("export function createCorrelationId(value: string): CorrelationId"),
            "Should have non-generic createCorrelationId factory. Got: {ts}"
        );
    }

    // Integer inner type branded newtype
    #[model_schema()]
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct SequenceNum(pub u64);

    #[test]
    fn test_branded_newtype_integer_inner() {
        let ts = SequenceNum::ts_definition();
        // Non-generic u64 maps to "number" in TypeScript
        assert!(
            ts.contains("export type SequenceNum = number & $brand<\"SequenceNum\">"),
            "Should have number branded type. Got: {ts}"
        );

        let zod = SequenceNum::zod_schema();
        // Zod should use z.number().int() for u64
        assert!(
            zod.contains("z.number().int()"),
            "Should use z.number().int() for u64. Got: {zod}"
        );
        assert!(
            zod.contains("SequenceNum$RawSchema"),
            "Should contain raw schema. Got: {zod}"
        );
    }

    // Branded newtype with doc comment description
    /// A unique document identifier
    #[model_schema()]
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct DocId(pub String);

    #[test]
    fn test_branded_newtype_zod_has_description() {
        let zod = DocId::zod_schema();
        assert!(
            zod.contains("description:"),
            "Zod schema should contain description. Got:\n{zod}"
        );
        assert!(
            zod.contains("A unique document identifier"),
            "Zod schema should contain doc comment text. Got:\n{zod}"
        );
        // Must still have $Schema line after .meta()
        assert!(
            zod.contains("export const DocId$Schema:"),
            "Zod schema should have $Schema after .meta(). Got:\n{zod}"
        );
    }

    // Generic branded newtype with doc comment and example
    /// Generic document identifier.
    ///
    /// - `DocumentId<String>` for API/HTTP layer
    /// - `DocumentId<ObjectId>` for MongoDB layer
    ///
    /// ```rust example
    /// DocumentId("64de3d95ff45b119e5b53a7e".to_string())
    /// ```
    #[model_schema()]
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(transparent)]
    #[allow(non_camel_case_types)]
    pub struct DocumentId<ID_TYPE>(pub ID_TYPE);

    #[test]
    fn test_branded_newtype_generic_with_example() {
        let zod = DocumentId::<String>::zod_schema();
        assert!(
            zod.contains("description:"),
            "Should contain description. Got:\n{zod}"
        );
        assert!(
            zod.contains("Generic document identifier"),
            "Should contain doc comment text. Got:\n{zod}"
        );
        assert!(
            zod.contains("example:"),
            "Should contain example from doc comment. Got:\n{zod}"
        );
        // Must still have $Schema line
        assert!(
            zod.contains("export const DocumentId$Schema:"),
            "Should have $Schema after example injection. Got:\n{zod}"
        );
    }

    #[test]
    fn test_branded_newtype_generic_schema_example_method() {
        let example = DocumentId::<String>::schema_example();
        assert_eq!(
            example.as_str().unwrap(),
            "64de3d95ff45b119e5b53a7e",
            "schema_example() should return the inner value. Got: {example}"
        );
    }
}

// Tests for branded newtypes with string constraints
#[cfg(all(feature = "zod", feature = "typescript", feature = "serde"))]
mod constrained_branded_tests {
    use super::*;

    #[model_schema(pattern = "^[a-z0-9_]+$", minLength = 3, maxLength = 50)]
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct SlugId(pub String);

    #[test]
    fn test_constrained_branded_zod_has_constraints() {
        let zod = SlugId::zod_schema();
        assert!(
            zod.contains(".min(3)"),
            "Should contain minLength constraint. Got:\n{zod}"
        );
        assert!(
            zod.contains(".max(50)"),
            "Should contain maxLength constraint. Got:\n{zod}"
        );
        assert!(
            zod.contains(".check(z.regex(/^[a-z0-9_]+$/))"),
            "Should contain pattern constraint. Got:\n{zod}"
        );
        assert!(
            zod.contains(".brand"),
            "Should still have brand. Got:\n{zod}"
        );
        assert!(
            zod.contains("export const SlugId$Schema:"),
            "Should have $Schema. Got:\n{zod}"
        );
    }

    #[test]
    fn test_constrained_branded_validate_pass() {
        let valid = SlugId("hello_world".to_string());
        assert!(valid.validate().is_ok());
    }

    #[test]
    fn test_constrained_branded_validate_too_short() {
        let short = SlugId("ab".to_string());
        let result = short.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err()[0].contains("too short"));
    }

    #[test]
    fn test_constrained_branded_validate_too_long() {
        let long = SlugId("a".repeat(51));
        let result = long.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err()[0].contains("too long"));
    }

    #[test]
    fn test_constrained_branded_validate_bad_pattern() {
        let bad = SlugId("UPPERCASE".to_string());
        let result = bad.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err()[0].contains("pattern"));
    }

    #[test]
    fn test_constrained_branded_serde_rejects_invalid() {
        // Serde should reject values that don't match constraints
        let result: Result<SlugId, _> = serde_json::from_str("\"ab\"");
        assert!(result.is_err(), "Should reject too-short value via serde");

        let result: Result<SlugId, _> = serde_json::from_str("\"UPPERCASE\"");
        assert!(result.is_err(), "Should reject bad pattern via serde");
    }

    #[test]
    fn test_constrained_branded_serde_accepts_valid() {
        let result: Result<SlugId, _> = serde_json::from_str("\"hello_world\"");
        assert!(result.is_ok(), "Should accept valid value via serde");
        assert_eq!(result.unwrap(), SlugId("hello_world".to_string()));
    }

    // Pattern-only constraint
    #[model_schema(pattern = "^[0-9a-fA-F]{24}$")]
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct ObjectIdStr(pub String);

    #[test]
    fn test_pattern_only_branded() {
        let zod = ObjectIdStr::zod_schema();
        assert!(
            zod.contains(".check(z.regex(/^[0-9a-fA-F]{24}$/))"),
            "Should contain pattern. Got:\n{zod}"
        );
        // Should not contain min/max
        assert!(!zod.contains(".min("), "Should not have min. Got:\n{zod}");
        assert!(!zod.contains(".max("), "Should not have max. Got:\n{zod}");

        let valid = ObjectIdStr("507f1f77bcf86cd799439011".to_string());
        assert!(valid.validate().is_ok());

        let invalid = ObjectIdStr("not-a-hex-id".to_string());
        assert!(invalid.validate().is_err());
    }

    // Generic branded newtype with constraints — validates via ToString
    #[model_schema(pattern = "^[a-z0-9_]+$", minLength = 3, maxLength = 50)]
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct GenericSlug<T>(pub T);

    #[test]
    fn test_generic_constrained_branded_validate_pass() {
        let valid = GenericSlug("hello_world".to_string());
        assert!(valid.validate().is_ok());
    }

    #[test]
    fn test_generic_constrained_branded_validate_too_short() {
        let short = GenericSlug("ab".to_string());
        let result = short.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err()[0].contains("too short"));
    }

    #[test]
    fn test_generic_constrained_branded_validate_bad_pattern() {
        let bad = GenericSlug("UPPERCASE".to_string());
        let result = bad.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err()[0].contains("pattern"));
    }

    #[test]
    fn test_generic_constrained_branded_serde_rejects_invalid() {
        let result: Result<GenericSlug<String>, _> = serde_json::from_str("\"ab\"");
        assert!(result.is_err(), "Should reject too-short value via serde");
    }

    #[test]
    fn test_generic_constrained_branded_serde_accepts_valid() {
        let result: Result<GenericSlug<String>, _> = serde_json::from_str("\"hello_world\"");
        assert!(result.is_ok(), "Should accept valid value via serde");
    }

    #[test]
    fn test_generic_constrained_branded_zod_has_constraints() {
        let zod = GenericSlug::<String>::zod_schema();
        assert!(zod.contains(".min(3)"), "Got:\n{zod}");
        assert!(zod.contains(".max(50)"), "Got:\n{zod}");
        assert!(
            zod.contains(".check(z.regex(/^[a-z0-9_]+$/))"),
            "Got:\n{zod}"
        );
    }
}

#[cfg(all(feature = "object_id", feature = "serde", feature = "zod"))]
mod constrained_objectid_branded_tests {
    use super::*;
    use mongodb::bson::oid::ObjectId;

    #[model_schema(pattern = "^[0-9a-fA-F]{24}$")]
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct StrictObjectId(pub ObjectId);

    #[test]
    fn test_objectid_branded_validate_pass() {
        let oid = ObjectId::parse_str("507f1f77bcf86cd799439011").unwrap();
        let valid = StrictObjectId(oid);
        assert!(valid.validate().is_ok());
    }

    #[test]
    fn test_objectid_branded_serde_accepts_valid() {
        let json = r#"{"$oid": "507f1f77bcf86cd799439011"}"#;
        let result: Result<StrictObjectId, _> = serde_json::from_str(json);
        assert!(result.is_ok(), "Should accept valid ObjectId via serde");
    }
}

// Branded newtype referenced from a struct
// Note: jsonschema must be OFF because branded newtypes don't generate json_schema(),
// so a struct referencing one cannot compile with jsonschema enabled.
#[cfg(all(feature = "zod", feature = "typescript", not(feature = "jsonschema")))]
mod branded_in_struct_tests {
    use super::*;

    #[model_schema()]
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct TaskId(pub String);

    #[model_schema()]
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct Task {
        pub id: TaskId,
        pub name: String,
    }

    #[test]
    fn test_branded_newtype_used_in_struct() {
        let ts = Task::ts_definition();
        // The struct's TS definition should reference the branded type by its export name
        assert!(
            ts.contains("TaskId"),
            "Struct TS should reference TaskId. Got: {ts}"
        );

        let zod = Task::zod_schema();
        // The struct's Zod schema should reference the branded type's schema
        assert!(
            zod.contains("TaskId$Schema"),
            "Struct Zod should reference TaskId$Schema. Got: {zod}"
        );
    }
}

// zod=OFF, typescript=ON tests
#[cfg(all(feature = "typescript", not(feature = "zod")))]
mod no_zod_tests {
    use super::*;

    #[model_schema()]
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct RoleIdNoZod<ID_TYPE>(pub ID_TYPE);

    #[test]
    fn test_branded_newtype_no_zod_ts_definition() {
        let ts = RoleIdNoZod::<String>::ts_definition();
        assert!(
            ts.contains("declare const __brand_RoleIdNoZod: unique symbol"),
            "Got: {ts}"
        );
        assert!(
            ts.contains(
                "export type RoleIdNoZod<ID_TYPE> = ID_TYPE & { readonly [__brand_RoleIdNoZod]: true }"
            ),
            "Got: {ts}"
        );
    }

    #[test]
    fn test_branded_newtype_no_zod_assert_function() {
        let ts = RoleIdNoZod::<String>::ts_definition();
        assert!(
            ts.contains(
                "export function assertRoleIdNoZod<ID_TYPE>(value: ID_TYPE): asserts value is RoleIdNoZod<ID_TYPE>"
            ),
            "Got: {ts}"
        );
    }

    #[test]
    fn test_branded_newtype_no_zod_generic_helpers() {
        let ts = RoleIdNoZod::<String>::ts_definition();
        assert!(
            ts.contains("function isRoleIdNoZod<T>(_value: T): _value is RoleIdNoZod<T>"),
            "Should have generic isRoleIdNoZod guard. Got: {ts}"
        );
        assert!(
            ts.contains("export function createRoleIdNoZod<T>(value: T): RoleIdNoZod<T>"),
            "Should have generic createRoleIdNoZod factory. Got: {ts}"
        );
    }

    // Non-generic branded newtype without Zod
    #[model_schema()]
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct SessionToken(pub String);

    #[test]
    fn test_branded_newtype_non_generic_no_zod() {
        let ts = SessionToken::ts_definition();
        // Should have unique symbol declaration
        assert!(
            ts.contains("declare const __brand_SessionToken: unique symbol"),
            "Should have unique symbol. Got: {ts}"
        );
        // Should have non-generic type definition
        assert!(
            ts.contains(
                "export type SessionToken = string & { readonly [__brand_SessionToken]: true }"
            ),
            "Should have branded type without generics. Got: {ts}"
        );
        // Should have non-generic assert function
        assert!(
            ts.contains(
                "export function assertSessionToken(value: string): asserts value is SessionToken"
            ),
            "Should have assert function without generics. Got: {ts}"
        );
    }

    #[test]
    fn test_branded_newtype_non_generic_no_zod_helpers() {
        let ts = SessionToken::ts_definition();
        assert!(
            ts.contains("function isSessionToken(_value: string): _value is SessionToken"),
            "Should have non-generic isSessionToken guard. Got: {ts}"
        );
        assert!(
            ts.contains("export function createSessionToken(value: string): SessionToken"),
            "Should have non-generic createSessionToken factory. Got: {ts}"
        );
    }
}

// Serde transparent test (always runs when serde is available)
#[cfg(feature = "serde")]
mod serde_tests {
    use super::*;

    #[model_schema()]
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct WrappedString(pub String);

    #[test]
    fn test_branded_newtype_serde_transparent() {
        // WrappedString("abc") should serialize as "abc"
        let val = WrappedString("abc".to_string());
        let json = serde_json::to_string(&val).unwrap();
        assert_eq!(json, "\"abc\"");

        // "abc" should deserialize as WrappedString("abc")
        let deserialized: WrappedString = serde_json::from_str("\"abc\"").unwrap();
        assert_eq!(deserialized, WrappedString("abc".to_string()));
    }

    #[model_schema()]
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct GenericId<ID_TYPE>(pub ID_TYPE);

    #[test]
    fn test_branded_newtype_generic_serde_roundtrip() {
        // Serialize a generic branded newtype with String inner type
        let original = GenericId::<String>("abc-123".to_string());
        let json = serde_json::to_string(&original).unwrap();
        assert_eq!(json, "\"abc-123\"", "Should serialize transparently");

        // Deserialize back
        let deserialized: GenericId<String> = serde_json::from_str(&json).unwrap();
        assert_eq!(
            deserialized, original,
            "Roundtrip should preserve equality"
        );

        // Also test with a numeric inner type
        let num_original = GenericId::<u64>(42);
        let num_json = serde_json::to_string(&num_original).unwrap();
        assert_eq!(num_json, "42", "Should serialize u64 transparently");

        let num_deserialized: GenericId<u64> = serde_json::from_str(&num_json).unwrap();
        assert_eq!(
            num_deserialized, num_original,
            "Numeric roundtrip should preserve equality"
        );
    }
}

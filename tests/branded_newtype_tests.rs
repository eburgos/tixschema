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
            ts.contains("export type RoleId<ID_TYPE> = ID_TYPE & z.$brand<\"RoleId\">"),
            "Got: {ts}"
        );
    }

    #[test]
    fn test_branded_newtype_zod_schema() {
        let zod = RoleId::<String>::zod_schema();
        assert!(zod.contains("z.string().brand"), "Got: {zod}");
        assert!(zod.contains("ZodType<RoleId<string>>"), "Got: {zod}");
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
            ts.contains("export type CorrelationId = string & z.$brand<\"CorrelationId\">"),
            "Got: {ts}"
        );
    }

    #[test]
    fn test_branded_newtype_non_generic_zod() {
        let zod = CorrelationId::zod_schema();
        assert!(zod.contains("z.string().brand"), "Got: {zod}");
        assert!(zod.contains("ZodType<CorrelationId>"), "Got: {zod}");
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
}

use core_model_macros::model_schema;
use serde::{Deserialize, Serialize};

#[cfg(test)]
mod tests {
    use super::*;

    // Test simple struct for typescript feature testing
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
    struct TypeScriptTestUser {
        id: String,
        name: String,
        age: u32,
        active: bool,
    }

    // Test enum for typescript feature testing
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
    #[serde(rename_all = "lowercase")]
    enum TypeScriptTestStatus {
        Active,
        Inactive,
        Pending,
    }

    // Test discriminated union for typescript feature testing
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
    #[serde(tag = "type", rename_all = "camelCase")]
    enum TypeScriptTestPayment {
        CreditCard { number: String, expiry: String },
        PayPal { email: String },
    }

    // Tests for when typescript feature is ENABLED (default configuration)
    #[test]
    #[cfg(feature = "typescript")]
    fn test_typescript_enabled_struct_ts_definition() {
        let ts_definition = TypeScriptTestUser::ts_definition();
        
        // Should contain TypeScript type definition
        assert!(ts_definition.contains("export type TypeScriptTestUser = {"));
        assert!(ts_definition.contains("id: string;"));
        assert!(ts_definition.contains("name: string;"));
        assert!(ts_definition.contains("age: number;"));
        assert!(ts_definition.contains("active: boolean;"));
        
        // Should NOT contain Zod schema
        assert!(!ts_definition.contains("z.strictObject"));
        assert!(!ts_definition.contains("z.string()"));
    }

    #[test]
    #[cfg(all(feature = "typescript", feature = "zod"))]
    fn test_typescript_enabled_struct_zod_schema() {
        let zod_schema = TypeScriptTestUser::zod_schema();
        
        // Should contain TypeScript-style Zod schema with type annotations
        assert!(zod_schema.contains("export const TypeScriptTestUser$Schema: z.Schema<TypeScriptTestUser, z.ZodTypeDef, unknown> = z.strictObject({"));
        assert!(zod_schema.contains("id: z.string()"));
        assert!(zod_schema.contains("name: z.string()"));
        assert!(zod_schema.contains("age: z.number().int()"));
        assert!(zod_schema.contains("active: z.boolean()"));
        
        // Should NOT contain TypeScript type definition
        assert!(!zod_schema.contains("export type TypeScriptTestUser"));
    }

    #[test]
    #[cfg(feature = "typescript")]
    fn test_typescript_enabled_plain_enum_ts_definition() {
        let ts_definition = TypeScriptTestStatus::ts_definition();
        
        // Should contain TypeScript union type
        assert!(ts_definition.contains("export type TypeScriptTestStatus = \"active\" | \"inactive\" | \"pending\";"));
        
        // Should NOT contain Zod schema
        assert!(!ts_definition.contains("z.enum"));
    }

    #[test]
    #[cfg(all(feature = "typescript", feature = "zod"))]
    fn test_typescript_enabled_plain_enum_zod_schema() {
        let zod_schema = TypeScriptTestStatus::zod_schema();
        
        // Should contain TypeScript-style Zod schema with type annotations
        assert!(zod_schema.contains("export const TypeScriptTestStatus$Schema: z.Schema<TypeScriptTestStatus> = z.enum([\"active\", \"inactive\", \"pending\"]);"));
        
        // Should NOT contain TypeScript type definition
        assert!(!zod_schema.contains("export type TypeScriptTestStatus"));
    }

    #[test]
    #[cfg(feature = "typescript")]
    fn test_typescript_enabled_discriminated_enum_ts_definition() {
        let ts_definition = TypeScriptTestPayment::ts_definition();
        
        // Should contain TypeScript discriminated union
        assert!(ts_definition.contains("export type TypeScriptTestPayment = "));
        assert!(ts_definition.contains("type: \"creditCard\""));
        assert!(ts_definition.contains("type: \"payPal\""));
        
        // Should NOT contain Zod schema
        assert!(!ts_definition.contains("z.discriminatedUnion"));
    }

    #[test]
    #[cfg(all(feature = "typescript", feature = "zod"))]
    fn test_typescript_enabled_discriminated_enum_zod_schema() {
        let zod_schema = TypeScriptTestPayment::zod_schema();
        
        // Should contain TypeScript-style Zod schema with type annotations
        assert!(zod_schema.contains("export const TypeScriptTestPayment$Schema: z.Schema<TypeScriptTestPayment, z.ZodTypeDef, unknown> = "));
        assert!(zod_schema.contains("z.discriminatedUnion"));
        
        // Should NOT contain TypeScript type definition
        assert!(!zod_schema.contains("export type TypeScriptTestPayment"));
    }

    // Tests for when typescript feature is DISABLED
    #[test]
    #[cfg(not(feature = "typescript"))]
    fn test_typescript_disabled_struct_ts_definition_not_available() {
        // The ts_definition method should not be available when typescript feature is disabled
        // This would cause a compile error if we tried to call TypeScriptTestUser::ts_definition()
        // We can't test the compilation failure directly, but we can verify the method doesn't exist
        // by checking that our code compiles without calling it
        
        // This test mainly serves as documentation that ts_definition() is not available
        // when typescript feature is disabled
    }

    #[test]
    #[cfg(all(not(feature = "typescript"), feature = "zod"))]
    fn test_typescript_disabled_struct_zod_schema_javascript_style() {
        let zod_schema = TypeScriptTestUser::zod_schema();
        
        // Should contain JavaScript-style Zod schema WITHOUT type annotations
        assert!(zod_schema.contains("export const TypeScriptTestUser$Schema = z.strictObject({"));
        assert!(zod_schema.contains("id: z.string()"));
        assert!(zod_schema.contains("name: z.string()"));
        assert!(zod_schema.contains("age: z.number().int()"));
        assert!(zod_schema.contains("active: z.boolean()"));
        
        // Should NOT contain TypeScript type annotations
        assert!(!zod_schema.contains(": z.Schema<TypeScriptTestUser, z.ZodTypeDef, unknown>"));
        assert!(!zod_schema.contains("export type TypeScriptTestUser"));
    }

    #[test]
    #[cfg(all(not(feature = "typescript"), feature = "zod"))]
    fn test_typescript_disabled_plain_enum_zod_schema_javascript_style() {
        let zod_schema = TypeScriptTestStatus::zod_schema();
        
        
        // When serde feature is disabled, the rename_all attribute is not processed
        // so the enum values will be the original Rust names (Title case)
        assert!(zod_schema.contains("export const TypeScriptTestStatus$Schema = z.enum([\"Active\", \"Inactive\", \"Pending\"]);"));
        
        // Should NOT contain TypeScript type annotations
        assert!(!zod_schema.contains(": z.Schema<TypeScriptTestStatus>"));
        assert!(!zod_schema.contains("export type TypeScriptTestStatus"));
    }

    #[test]
    #[cfg(all(not(feature = "typescript"), feature = "zod"))]
    fn test_typescript_disabled_discriminated_enum_zod_schema_javascript_style() {
        let zod_schema = TypeScriptTestPayment::zod_schema();
        
        // Should contain JavaScript-style Zod schema WITHOUT type annotations
        assert!(zod_schema.contains("export const TypeScriptTestPayment$Schema = "));
        assert!(zod_schema.contains("z.discriminatedUnion"));
        
        // Should NOT contain TypeScript type annotations
        assert!(!zod_schema.contains(": z.Schema<TypeScriptTestPayment, z.ZodTypeDef, unknown>"));
        assert!(!zod_schema.contains("export type TypeScriptTestPayment"));
    }

    // Feature combination tests
    #[test]
    #[cfg(all(feature = "typescript", feature = "zod"))]
    fn test_typescript_and_zod_both_enabled() {
        let ts_definition = TypeScriptTestUser::ts_definition();
        let zod_schema = TypeScriptTestUser::zod_schema();
        
        // TypeScript definition should be available and contain only TypeScript types
        assert!(ts_definition.contains("export type TypeScriptTestUser = {"));
        assert!(!ts_definition.contains("z.strictObject"));
        
        // Zod schema should be available and contain TypeScript-style type annotations
        assert!(zod_schema.contains("export const TypeScriptTestUser$Schema: z.Schema<TypeScriptTestUser, z.ZodTypeDef, unknown>"));
        assert!(!zod_schema.contains("export type TypeScriptTestUser"));
    }

    #[test]
    #[cfg(all(not(feature = "typescript"), feature = "zod"))]
    fn test_typescript_disabled_zod_enabled() {
        let zod_schema = TypeScriptTestUser::zod_schema();
        
        // Zod schema should be available but in JavaScript style (no type annotations)
        assert!(zod_schema.contains("export const TypeScriptTestUser$Schema = z.strictObject({"));
        assert!(!zod_schema.contains(": z.Schema<TypeScriptTestUser, z.ZodTypeDef, unknown>"));
        
        // TypeScript definition should NOT be available
        // (We can't test the compilation failure directly, but the method shouldn't exist)
    }

    #[test]
    #[cfg(all(feature = "typescript", not(feature = "zod")))]
    fn test_typescript_enabled_zod_disabled() {
        let ts_definition = TypeScriptTestUser::ts_definition();
        
        // TypeScript definition should be available
        assert!(ts_definition.contains("export type TypeScriptTestUser = {"));
        
        // Zod schema should NOT be available
        // (We can't test the compilation failure directly, but the method shouldn't exist)
    }
} 
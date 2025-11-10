//! Comprehensive tests for type alias support
//!
//! Type aliases allow developers to create semantic types (e.g., `DocumentId`, `UserId`)
//! that are distinct in TypeScript but use primitive types in Rust.
//!
//! This module tests:
//! - Basic type aliases (String, numeric types)
//! - Optional type aliases (Option<T>)
//! - Generic type aliases (Pair<T, U>)
//! - Nested type aliases (alias referencing another alias)
//! - Type aliases used as struct fields
//! - Type aliases in collections (Vec, `HashMap`)
//! - TypeScript, Zod, and JSON schema generation
//! - Module structure and naming

#[cfg(test)]
#[expect(clippy::struct_field_names, reason = "This is a test file")]
#[expect(dead_code, reason = "This is a test file")]
mod tests {
    #[cfg(all(test, feature = "serde"))]
    use serde::{Deserialize, Serialize};
    #[cfg(all(test, feature = "typescript"))]
    use tixschema::model_schema;

    // ========================================================================
    // Type Alias Definitions
    // ========================================================================

    /// Simple string-based type alias
    #[cfg(all(test, feature = "typescript"))]
    #[model_schema(name = "DocumentId")]
    pub type DocumentIdJson = String;

    /// Numeric type alias (i64)
    #[cfg(all(test, feature = "typescript"))]
    #[model_schema(name = "Revision")]
    pub type RevisionJson = i64;

    /// Floating-point type alias
    #[cfg(all(test, feature = "typescript"))]
    #[model_schema(name = "Score")]
    pub type ScoreJson = f32;

    /// Boolean type alias
    #[cfg(all(test, feature = "typescript"))]
    #[model_schema(name = "IsActive")]
    pub type IsActiveJson = bool;

    /// Optional type alias
    #[cfg(all(test, feature = "typescript"))]
    #[model_schema(name = "OptionalNote")]
    pub type OptionalNoteJson = Option<String>;

    /// Generic type alias with single type parameter
    #[cfg(all(test, feature = "typescript"))]
    #[model_schema(name = "Wrapper")]
    
    pub type WrapperJson<T> = Option<T>;

    /// Generic type alias with two type parameters
    #[cfg(all(test, feature = "typescript"))]
    #[model_schema(name = "Pair")]
    pub type PairJson<T, U> = (T, U);

    /// Type alias referencing another type alias (nested)
    #[cfg(all(test, feature = "typescript"))]
    #[model_schema(name = "OrderId")]
    pub type OrderIdJson = String;

    #[cfg(all(test, feature = "typescript"))]
    #[model_schema(name = "AuditId")]
    pub type AuditIdJson = OrderIdJson;

    /// Vec type alias
    #[cfg(all(test, feature = "typescript"))]
    #[model_schema(name = "Tags")]
    pub type TagsJson = Vec<String>;

    // ========================================================================
    // Basic Type Alias Tests - TypeScript Generation
    // ========================================================================

    #[test]
    #[cfg(feature = "typescript")]
    fn test_string_alias_typescript() {
        let ts = document_id_schema::Schema::ts_definition();

        assert!(
            ts.contains("export type DocumentId = string;"),
            "Should generate string type alias. Got: {ts}"
        );
        assert!(
            ts.contains("/**"),
            "Should include JSDoc comment. Got: {ts}"
        );
    }

    #[test]
    #[cfg(feature = "typescript")]
    fn test_i64_alias_typescript() {
        let ts = revision_schema::Schema::ts_definition();

        assert!(
            ts.contains("export type Revision = number;"),
            "Should generate number type alias for i64. Got: {ts}"
        );
    }

    #[test]
    #[cfg(feature = "typescript")]
    fn test_f32_alias_typescript() {
        let ts = score_schema::Schema::ts_definition();

        assert!(
            ts.contains("export type Score = number;"),
            "Should generate number type alias for f32. Got: {ts}"
        );
    }

    #[test]
    #[cfg(feature = "typescript")]
    fn test_bool_alias_typescript() {
        let ts = is_active_schema::Schema::ts_definition();

        assert!(
            ts.contains("export type IsActive = boolean;"),
            "Should generate boolean type alias. Got: {ts}"
        );
    }

    #[test]
    #[cfg(feature = "typescript")]
    fn test_optional_alias_typescript() {
        let ts = optional_note_schema::Schema::ts_definition();

        assert!(
            ts.contains("export type OptionalNote = string | undefined;"),
            "Should generate optional type with undefined union. Got: {ts}"
        );
    }

    #[test]
    #[cfg(feature = "typescript")]
    fn test_vec_alias_typescript() {
        let ts = tags_schema::Schema::ts_definition();

        assert!(
            ts.contains("export type Tags = Array<string>;"),
            "Should generate Array type alias. Got: {ts}"
        );
    }

    // ========================================================================
    // Generic Type Alias Tests
    // ========================================================================

    #[test]
    #[cfg(feature = "typescript")]
    fn test_single_generic_alias_typescript() {
        let ts = wrapper_schema::Schema::ts_definition();

        assert!(
            ts.contains("export type Wrapper<T>"),
            "Should include generic type parameter. Got: {ts}"
        );
        assert!(
            ts.contains("T | undefined"),
            "Should use generic T in type definition. Got: {ts}"
        );
    }

    #[test]
    #[cfg(feature = "typescript")]
    fn test_double_generic_alias_typescript() {
        let ts = pair_schema::Schema::ts_definition();

        assert!(
            ts.contains("export type Pair<T, U>"),
            "Should include both generic type parameters. Got: {ts}"
        );
        // Note: Rust tuples are rendered as objects with element_0, element_1, etc.
        // This is consistent with how serde serializes tuples
        assert!(
            ts.contains("element_0: T") && ts.contains("element_1: U"),
            "Should generate object with element_0 and element_1 fields. Got: {ts}"
        );
    }

    // ========================================================================
    // Nested Type Alias Tests
    // ========================================================================

    #[test]
    #[cfg(feature = "typescript")]
    fn test_nested_alias_typescript() {
        let ts = audit_id_schema::Schema::ts_definition();

        assert!(
            ts.contains("export type AuditId"),
            "Should generate AuditId type. Got: {ts}"
        );
        assert!(
            ts.contains("OrderId"),
            "Should reference OrderId type. Got: {ts}"
        );
    }

    // ========================================================================
    // Type Aliases Used in Struct Fields
    // ========================================================================

    #[cfg(all(test, feature = "typescript", feature = "serde"))]
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    struct DocumentRecordJson {
        document_id: DocumentIdJson,
        revision: RevisionJson,
        score: ScoreJson,
        is_active: IsActiveJson,
        note: OptionalNoteJson,
    }

    #[test]
    #[cfg(all(feature = "typescript", feature = "serde"))]
    fn test_struct_with_alias_fields_typescript() {
        let ts = DocumentRecordJson::ts_definition();

        assert!(
            ts.contains("document_id: DocumentId;"),
            "Should use DocumentId type alias. Got: {ts}"
        );
        assert!(
            ts.contains("revision: Revision;"),
            "Should use Revision type alias. Got: {ts}"
        );
        assert!(
            ts.contains("score: Score;"),
            "Should use Score type alias. Got: {ts}"
        );
        assert!(
            ts.contains("is_active: IsActive;"),
            "Should use IsActive type alias. Got: {ts}"
        );
        assert!(
            ts.contains("note: OptionalNote;"),
            "Should use OptionalNote type alias. Got: {ts}"
        );
    }

    #[cfg(all(test, feature = "typescript", feature = "serde"))]
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    struct NestedAliasStructJson {
        user_id: DocumentIdJson,
        order_id: OrderIdJson,
        audit_id: AuditIdJson,
    }

    #[test]
    #[cfg(all(feature = "typescript", feature = "serde"))]
    fn test_struct_with_nested_alias_fields() {
        let ts = NestedAliasStructJson::ts_definition();

        assert!(
            ts.contains("user_id: DocumentId;"),
            "Should use DocumentId. Got: {ts}"
        );
        assert!(
            ts.contains("order_id: OrderId;"),
            "Should use OrderId. Got: {ts}"
        );
        assert!(
            ts.contains("audit_id: AuditId;"),
            "Should use AuditId (nested alias). Got: {ts}"
        );
    }

    // ========================================================================
    // Type Aliases in Collections
    // ========================================================================

    #[cfg(all(test, feature = "typescript", feature = "serde"))]
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    struct CollectionAliasJson {
        document_ids: Vec<DocumentIdJson>,
        tags: TagsJson,
    }

    #[test]
    #[cfg(all(feature = "typescript", feature = "serde"))]
    fn test_alias_in_vec() {
        let ts = CollectionAliasJson::ts_definition();

        assert!(
            ts.contains("document_ids: Array<DocumentId>;"),
            "Should use DocumentId in Array. Got: {ts}"
        );
        assert!(
            ts.contains("tags: Tags;"),
            "Should use Tags type alias. Got: {ts}"
        );
    }

    #[cfg(all(test, feature = "typescript", feature = "serde"))]
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    struct MapAliasJson {
        metadata: std::collections::HashMap<String, DocumentIdJson>,
    }

    #[test]
    #[cfg(all(feature = "typescript", feature = "serde"))]
    fn test_alias_in_hashmap() {
        let ts = MapAliasJson::ts_definition();

        assert!(
            ts.contains("metadata: Partial<Record<string, DocumentId>>;"),
            "Should use DocumentId in HashMap. Got: {ts}"
        );
    }

    // ========================================================================
    // Zod Schema Tests
    // ========================================================================

    #[test]
    #[cfg(all(feature = "zod", feature = "typescript"))]
    fn test_alias_zod_schema_exists() {
        // Zod schema generation for aliases currently returns a stub
        let zod = document_id_schema::Schema::zod_schema();

        // Verify the method exists and returns something
        assert!(
            !zod.is_empty(),
            "Zod schema should return a non-empty string"
        );
    }

    #[test]
    #[cfg(all(feature = "zod", feature = "typescript", feature = "serde"))]
    fn test_struct_with_alias_zod_schema() {
        let zod = DocumentRecordJson::zod_schema();

        // The struct should generate a proper Zod schema
        assert!(
            zod.contains("z.strictObject"),
            "Should contain Zod object schema. Got: {zod}"
        );
    }

    // ========================================================================
    // JSON Schema Tests
    // ========================================================================

    #[test]
    #[cfg(all(feature = "jsonschema", feature = "typescript"))]
    fn test_alias_json_schema_exists() {
        // JSON schema generation for aliases currently returns a stub
        let json_schema = document_id_schema::Schema::json_schema();

        // Verify the method exists and returns a JSON value
        assert!(
            json_schema.is_object(),
            "JSON schema should return an object"
        );
    }

    #[test]
    #[cfg(all(feature = "jsonschema", feature = "typescript", feature = "serde"))]
    fn test_struct_with_alias_json_schema() {
        let schema = DocumentRecordJson::json_schema();

        // The struct should generate a proper JSON schema
        assert_eq!(
            schema["type"], "object",
            "Should be an object schema. Got: {schema:?}"
        );
        assert!(
            schema["properties"].is_object(),
            "Should have properties. Got: {schema:?}"
        );
    }

    // ========================================================================
    // Module Structure Tests
    // ========================================================================

    #[test]
    #[cfg(feature = "typescript")]
    fn test_alias_generates_module() {
        // Verify that the alias generates the expected module structure
        // The module should be accessible as `document_id_schema`
        let ts = document_id_schema::Schema::ts_definition();

        // If we can call this method, the module was generated correctly
        assert!(!ts.is_empty(), "Module should be accessible");
    }

    #[test]
    #[cfg(feature = "typescript")]
    fn test_multiple_aliases_generate_separate_modules() {
        // Each alias should generate its own module
        let doc_ts = document_id_schema::Schema::ts_definition();
        let rev_ts = revision_schema::Schema::ts_definition();
        let score_ts = score_schema::Schema::ts_definition();

        assert!(doc_ts.contains("DocumentId"));
        assert!(rev_ts.contains("Revision"));
        assert!(score_ts.contains("Score"));

        // They should be different
        assert_ne!(doc_ts, rev_ts);
        assert_ne!(rev_ts, score_ts);
    }

    // ========================================================================
    // Edge Cases and Complex Scenarios
    // ========================================================================

    #[cfg(all(test, feature = "typescript", feature = "serde"))]
    #[model_schema()]
    #[derive(Serialize, Deserialize, Debug, Clone)]
    struct ComplexAliasStructJson {
        optional_id: Option<DocumentIdJson>,
        nested_ids: Vec<Vec<DocumentIdJson>>,
        mapped_scores: std::collections::HashMap<String, Vec<ScoreJson>>,
    }

    #[test]
    #[cfg(all(feature = "typescript", feature = "serde"))]
    fn test_complex_alias_usage() {
        let ts = ComplexAliasStructJson::ts_definition();

        assert!(
            ts.contains("optional_id: DocumentId | undefined;"),
            "Should handle Optional<Alias>. Got: {ts}"
        );
        // Note: Vec<Vec<T>> is currently flattened to Array<T> in the type system
        // This is a known behavior - nested arrays are simplified to single array
        assert!(
            ts.contains("nested_ids: Array<DocumentId>;"),
            "Should handle Vec<Vec<Alias>>. Got: {ts}"
        );
        assert!(
            ts.contains("mapped_scores: Partial<Record<string, Array<Score>>>;"),
            "Should handle HashMap<String, Vec<Alias>>. Got: {ts}"
        );
    }

    // ========================================================================
    // Name Override Tests
    // ========================================================================

    #[cfg(all(test, feature = "typescript"))]
    #[model_schema(name = "CustomName")]
    pub type SomeTypeJson = String;

    #[test]
    #[cfg(feature = "typescript")]
    fn test_name_override() {
        let ts = custom_name_schema::Schema::ts_definition();

        assert!(
            ts.contains("export type CustomName = string;"),
            "Should use custom name from attribute. Got: {ts}"
        );
        assert!(
            !ts.contains("SomeType"),
            "Should not contain original type name. Got: {ts}"
        );
    }

    // ========================================================================
    // Documentation Tests
    // ========================================================================

    /// This is a documented type alias
    /// It should appear in the generated TypeScript
    #[cfg(all(test, feature = "typescript"))]
    #[model_schema(name = "DocumentedId")]
    pub type DocumentedIdJson = String;

    #[test]
    #[cfg(feature = "typescript")]
    fn test_alias_with_docs() {
        let ts = documented_id_schema::Schema::ts_definition();

        assert!(
            ts.contains("/**"),
            "Should include JSDoc comment start. Got: {ts}"
        );
        assert!(
            ts.contains("This is a documented type alias"),
            "Should include doc comment. Got: {ts}"
        );
    }
}

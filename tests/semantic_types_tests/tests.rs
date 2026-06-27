#[cfg(all(test, feature = "typescript", feature = "serde"))]
use std::collections::HashMap;

#[cfg(all(test, feature = "serde"))]
use serde::{Deserialize, Serialize};
#[cfg(all(test, feature = "typescript"))]
use tixschema::model_schema;

// ========================================================================
// Type Alias Definitions
// ========================================================================

#[cfg(all(test, feature = "typescript"))]
#[model_schema(name = "AuditId")]
pub type AuditId = OrderId;

/// Simple string-based type alias.
#[cfg(all(test, feature = "typescript"))]
#[model_schema(name = "DocumentId")]
pub type DocumentId = String;

/// This is a documented type alias.
/// It should appear in the generated TypeScript.
#[cfg(all(test, feature = "typescript"))]
#[model_schema(name = "DocumentedId")]
pub type DocumentedId = String;

/// Boolean type alias.
#[cfg(all(test, feature = "typescript"))]
#[model_schema(name = "IsActive")]
pub type IsActive = bool;

/// Optional type alias.
#[cfg(all(test, feature = "typescript"))]
#[model_schema(name = "OptionalNote")]
pub type OptionalNote = Option<String>;

/// Type alias referencing another type alias (nested).
#[cfg(all(test, feature = "typescript"))]
#[model_schema(name = "OrderId")]
pub type OrderId = String;

/// Generic type alias with two type parameters.
#[cfg(all(test, feature = "typescript"))]
#[model_schema(name = "Pair")]
pub type Pair<T, U> = (T, U);

/// Numeric type alias (i64).
#[cfg(all(test, feature = "typescript"))]
#[model_schema(name = "Revision")]
pub type Revision = i64;

/// Floating-point type alias.
#[cfg(all(test, feature = "typescript"))]
#[model_schema(name = "Score")]
pub type Score = f32;

#[cfg(all(test, feature = "typescript"))]
#[model_schema(name = "CustomName")]
pub type SomeType = String;

/// Vec type alias.
#[cfg(all(test, feature = "typescript"))]
#[model_schema(name = "Tags")]
pub type Tags = Vec<String>;

/// Generic type alias with single type parameter.
#[cfg(all(test, feature = "typescript"))]
#[model_schema(name = "Wrapper")]
pub type Wrapper<T> = Option<T>;

#[cfg(all(test, feature = "typescript", feature = "serde"))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
struct CollectionAlias {
    document_ids: Vec<DocumentId>,
    tags: Tags,
}

#[cfg(all(test, feature = "typescript", feature = "serde"))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
struct ComplexAliasStruct {
    mapped_scores: HashMap<String, Vec<Score>>,
    nested_ids: Vec<Vec<DocumentId>>,
    optional_id: Option<DocumentId>,
}

#[cfg(all(test, feature = "typescript", feature = "serde"))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
struct DocumentRecord {
    document_id: DocumentId,
    is_active: IsActive,
    note: OptionalNote,
    revision: Revision,
    score: Score,
}

#[cfg(all(test, feature = "typescript", feature = "serde"))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
struct MapAlias {
    metadata: HashMap<String, DocumentId>,
}

#[cfg(all(test, feature = "typescript", feature = "serde"))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
struct NestedAliasStruct {
    audit_id: AuditId,
    order_id: OrderId,
    #[serde(rename = "user_id")]
    owner: DocumentId,
}

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
    let wrapped: Wrapper<i32> = None;
    assert!(wrapped.is_none());
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
    let pair: Pair<i32, i32> = (1_i32, 2_i32);
    assert_eq!(pair.0 + pair.1, 3_i32);
    let ts = pair_schema::Schema::ts_definition();

    assert!(
        ts.contains("export type Pair<T, U>"),
        "Should include both generic type parameters. Got: {ts}"
    );
    // Rust tuples serialize (via serde) as JSON arrays, so they render as
    // TypeScript tuples `[T, U]` (not objects).
    assert!(
        ts.contains("[T, U]"),
        "Should generate a tuple type [T, U]. Got: {ts}"
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

#[test]
#[cfg(all(feature = "typescript", feature = "serde"))]
fn test_struct_with_alias_fields_typescript() {
    let ts = DocumentRecord::ts_definition();

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

#[test]
#[cfg(all(feature = "typescript", feature = "serde"))]
fn test_struct_with_nested_alias_fields() {
    let ts = NestedAliasStruct::ts_definition();

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

#[test]
#[cfg(all(feature = "typescript", feature = "serde"))]
fn test_alias_in_vec() {
    let ts = CollectionAlias::ts_definition();

    assert!(
        ts.contains("document_ids: Array<DocumentId>;"),
        "Should use DocumentId in Array. Got: {ts}"
    );
    assert!(
        ts.contains("tags: Tags;"),
        "Should use Tags type alias. Got: {ts}"
    );
}

#[test]
#[cfg(all(feature = "typescript", feature = "serde"))]
fn test_alias_in_hashmap() {
    let ts = MapAlias::ts_definition();

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
    let zod = DocumentRecord::zod_schema();

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
    let schema = DocumentRecord::json_schema();

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

#[test]
#[cfg(all(feature = "typescript", feature = "serde"))]
fn test_complex_alias_usage() {
    let ts = ComplexAliasStruct::ts_definition();

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

#[test]
#[cfg(feature = "typescript")]
fn test_name_override() {
    let some: SomeType = String::new();
    assert!(some.is_empty());
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

#[test]
#[cfg(feature = "typescript")]
fn test_alias_with_docs() {
    let documented: DocumentedId = String::new();
    assert!(documented.is_empty());
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

#[cfg(all(test, feature = "typescript", feature = "serde"))]
use std::collections::HashMap;

#[cfg(all(test, feature = "typescript", feature = "serde"))]
use serde::{Deserialize, Serialize};
#[cfg(all(test, feature = "typescript"))]
use tixschema::model_schema;

// ========================================================================
// Type Alias Definitions
// ========================================================================

#[cfg(all(test, feature = "typescript"))]
#[model_schema(name = "AuditId")]
pub type AuditId = OrderId;

/// Tuple type alias mirroring remargin's compact link row: two optional string
/// slots (null-flavored inside a positional tuple), a `Vec<usize>`, and a
/// required string.
#[cfg(all(test, feature = "typescript"))]
#[model_schema(name = "CompactLinkRow")]
pub type CompactLinkRow = (Option<String>, Vec<usize>, String, Option<String>);

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
    #[serde(skip_serializing_if = "Option::is_none")]
    optional_id: Option<DocumentId>,
}

#[cfg(all(test, feature = "zod", feature = "typescript", feature = "serde"))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
struct CompactRowHolder {
    rows: Vec<CompactLinkRow>,
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
    let document_id: DocumentId = String::new();
    assert!(document_id.is_empty());
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
    let revision: Revision = 0;
    assert_eq!(revision, 0);
    let ts = revision_schema::Schema::ts_definition();

    assert!(
        ts.contains("export type Revision = number;"),
        "Should generate number type alias for i64. Got: {ts}"
    );
}

#[test]
#[cfg(feature = "typescript")]
fn test_f32_alias_typescript() {
    let score: Score = 0.0;
    assert!(score.is_finite());
    let ts = score_schema::Schema::ts_definition();

    assert!(
        ts.contains("export type Score = number;"),
        "Should generate number type alias for f32. Got: {ts}"
    );
}

#[test]
#[cfg(feature = "typescript")]
fn test_bool_alias_typescript() {
    let is_active = IsActive::default();
    assert!(!is_active);
    let ts = is_active_schema::Schema::ts_definition();

    assert!(
        ts.contains("export type IsActive = boolean;"),
        "Should generate boolean type alias. Got: {ts}"
    );
}

#[test]
#[cfg(feature = "typescript")]
fn test_optional_alias_typescript() {
    let optional_note: OptionalNote = None;
    assert!(optional_note.is_none());
    let ts = optional_note_schema::Schema::ts_definition();

    assert!(
        ts.contains("export type OptionalNote = string | undefined;"),
        "Should generate optional type with undefined union. Got: {ts}"
    );
}

#[test]
#[cfg(feature = "typescript")]
fn test_vec_alias_typescript() {
    let tags: Tags = Vec::new();
    assert!(tags.is_empty());
    let ts = tags_schema::Schema::ts_definition();

    assert!(
        ts.contains("export type Tags = Array<string>;"),
        "Should generate Array type alias. Got: {ts}"
    );
}

#[test]
#[cfg(feature = "typescript")]
fn test_tuple_alias_typescript() {
    let row: CompactLinkRow = (None, Vec::new(), String::new(), None);
    assert!(row.0.is_none());
    let ts = compact_link_row_schema::Schema::ts_definition();

    assert!(
        ts.contains(
            "export type CompactLinkRow = [string | null, Array<number>, string, string | null];"
        ),
        "Tuple alias should render a null-flavored TS tuple. Got: {ts}"
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
    let order_id = OrderId::default();
    let audit_id: AuditId = order_id;
    assert!(audit_id.is_empty());
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
fn test_scalar_alias_zod_schema() {
    let zod = document_id_schema::Schema::zod_schema();

    assert!(
        zod.contains("const DocumentId$RawSchema = z.string();"),
        "Scalar alias should bind its Zod to $RawSchema. Got: {zod}"
    );
    assert!(
        zod.contains("export const DocumentId$Schema: ZodType<DocumentId> = DocumentId$RawSchema;"),
        "Scalar alias should re-export an annotated $Schema. Got: {zod}"
    );
    assert!(
        !zod.contains("not yet supported"),
        "Alias Zod must no longer be a stub. Got: {zod}"
    );
}

/// Tuple alias emits a valid `$Schema` whose Zod is the null-flavored tuple —
/// the exact shape a struct field `Vec<CompactLinkRow>` references.
#[test]
#[cfg(all(feature = "zod", feature = "typescript"))]
fn test_tuple_alias_zod_schema() {
    let zod = compact_link_row_schema::Schema::zod_schema();

    assert!(
        zod.contains(
            "const CompactLinkRow$RawSchema = z.tuple([z.nullable(z.string()), z.array(z.number().int()), z.string(), z.nullable(z.string())]);"
        ),
        "Tuple alias should render the null-flavored z.tuple. Got: {zod}"
    );
    assert!(
        zod.contains(
            "export const CompactLinkRow$Schema: ZodType<CompactLinkRow> = CompactLinkRow$RawSchema;"
        ),
        "Tuple alias should re-export an annotated $Schema. Got: {zod}"
    );
    assert!(
        !zod.contains("not yet supported"),
        "Alias Zod must no longer be a stub. Got: {zod}"
    );
}

/// A struct field `Vec<CompactLinkRow>` references the alias's now-defined
/// `$Schema` via `z.array(...)` in Zod and `Array<...>` in TS.
#[test]
#[cfg(all(feature = "zod", feature = "typescript", feature = "serde"))]
fn test_vec_of_alias_references_schema() {
    let zod = CompactRowHolder::zod_schema();
    assert!(
        zod.contains("rows: z.array(CompactLinkRow$Schema)"),
        "Vec<CompactLinkRow> should reference the alias $Schema. Got: {zod}"
    );

    let ts = CompactRowHolder::ts_definition();
    assert!(
        ts.contains("rows: Array<CompactLinkRow>;"),
        "Vec<CompactLinkRow> should render Array<CompactLinkRow> in TS. Got: {ts}"
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

/// An alias publishes the schema of the type it names, so a slot filled by the alias validates
/// exactly what the aliased type validates — the scalar mapping being the same one a field written
/// as the target reads.
#[test]
#[cfg(all(feature = "jsonschema", feature = "typescript"))]
fn test_scalar_alias_json_schema() {
    for (alias, schema, expected) in [
        (
            "DocumentId",
            document_id_schema::Schema::json_schema(),
            serde_json::json!({ "type": "string" }),
        ),
        (
            "Revision",
            revision_schema::Schema::json_schema(),
            serde_json::json!({ "type": "integer" }),
        ),
        (
            "Score",
            score_schema::Schema::json_schema(),
            serde_json::json!({ "type": "number" }),
        ),
        (
            "IsActive",
            is_active_schema::Schema::json_schema(),
            serde_json::json!({ "type": "boolean" }),
        ),
    ] {
        assert_eq!(schema, expected, "for {alias}");
    }
}

/// An alias cannot be dropped the way an optional object key can — a slot written as the alias is
/// filled with the `null` serde writes for a `None`, so the schema has to admit it.
#[test]
#[cfg(all(feature = "jsonschema", feature = "typescript"))]
fn test_optional_alias_json_schema() {
    assert_eq!(
        optional_note_schema::Schema::json_schema(),
        serde_json::json!({ "anyOf": [{ "type": "string" }, { "type": "null" }] })
    );
}

#[test]
#[cfg(all(feature = "jsonschema", feature = "typescript"))]
fn test_sequence_alias_json_schema() {
    assert_eq!(
        tags_schema::Schema::json_schema(),
        serde_json::json!({ "type": "array", "items": { "type": "string" } })
    );
}

/// A tuple is the fixed-arity array serde writes it as, at the alias exactly as in field position.
#[test]
#[cfg(all(feature = "jsonschema", feature = "typescript"))]
fn test_tuple_alias_json_schema() {
    assert_eq!(
        compact_link_row_schema::Schema::json_schema(),
        serde_json::json!({
            "type": "array",
            "prefixItems": [
                { "anyOf": [{ "type": "string" }, { "type": "null" }] },
                { "type": "array", "items": { "type": "integer" } },
                { "type": "string" },
                { "anyOf": [{ "type": "string" }, { "type": "null" }] }
            ],
            "items": false,
            "minItems": 4_u64,
            "maxItems": 4_u64
        })
    );
}

/// An alias of an alias carries the target's reference, which resolves through the registry — so
/// the chain lands on the type at the end of it rather than on a link.
#[test]
#[cfg(all(feature = "jsonschema", feature = "typescript"))]
fn test_nested_alias_json_schema() {
    assert_eq!(
        audit_id_schema::Schema::json_schema(),
        order_id_schema::Schema::json_schema()
    );
    assert_eq!(
        audit_id_schema::Schema::json_schema(),
        serde_json::json!({ "type": "string" })
    );
}

/// A type parameter names no type until the alias is instantiated, and every position that
/// references an alias references it uninstantiated — so the parameter admits any value, while the
/// shape around it is still described.
#[test]
#[cfg(all(feature = "jsonschema", feature = "typescript"))]
fn test_generic_alias_json_schema() {
    assert_eq!(
        pair_schema::Schema::json_schema(),
        serde_json::json!({
            "type": "array",
            "prefixItems": [{}, {}],
            "items": false,
            "minItems": 2_u64,
            "maxItems": 2_u64
        })
    );
    assert_eq!(
        wrapper_schema::Schema::json_schema(),
        serde_json::json!({ "anyOf": [{}, { "type": "null" }] })
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

    // A field written as an alias carries the alias module's schema, so what the field validates
    // is what the alias publishes — every slot naming the alias agrees with every other.
    for (field, alias_schema) in [
        ("document_id", document_id_schema::Schema::json_schema()),
        ("is_active", is_active_schema::Schema::json_schema()),
        ("note", optional_note_schema::Schema::json_schema()),
        ("revision", revision_schema::Schema::json_schema()),
        ("score", score_schema::Schema::json_schema()),
    ] {
        assert_eq!(
            schema["properties"][field], alias_schema,
            "for {field} in: {schema}"
        );
    }
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
    // A `Vec<Vec<T>>` writes an array of arrays, so it types as one: a level per level written.
    assert!(
        ts.contains("nested_ids: Array<Array<DocumentId>>;"),
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

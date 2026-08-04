use serde::{Deserialize, Serialize};
use tixschema::model_schema;

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct BasePart {
    owner: String,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
struct DataElementSampleValueEntry {
    data_element_id: String,
    #[serde(flatten)]
    variant: DataElementSampleValueVariant,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "dataType")]
enum DataElementSampleValueVariant {
    Alphanumeric {
        #[serde(rename = "sampleValues")]
        sample_values: Vec<String>,
    },
    Logical {
        #[serde(rename = "sampleValues")]
        sample_values: Vec<bool>,
    },
    Numeric {
        #[serde(rename = "sampleValues")]
        sample_values: Vec<i64>,
    },
}

/// Two bases that flatten each other: neither body exists when the other's merge asks for it, and
/// no finite value inhabits either type.
#[cfg(any(feature = "jsonschema", feature = "zod"))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct CycleFirst {
    first_own: String,
    #[serde(flatten)]
    second: CycleSecond,
}

#[cfg(any(feature = "jsonschema", feature = "zod"))]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct CycleSecond {
    #[serde(flatten)]
    first: Box<CycleFirst>,
    second_own: String,
}

/// The same cycle with a union in the middle: the branch that closes it is the deferred name, and
/// a reference carries none of the members it stands for.
#[cfg(feature = "jsonschema")]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct CycleUnionNode {
    #[serde(flatten)]
    either: CycleUnionEither,
    own: String,
}

#[cfg(feature = "jsonschema")]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
enum CycleUnionEither {
    Only(Box<CycleUnionNode>),
}

/// A cycle spanning both places an intersection operand is written: a struct's `#[serde(flatten)]`
/// base on one side, an internally tagged newtype variant's content on the other. Each names the
/// other's `const`, so neither declaration order puts both above the other.
#[cfg(feature = "zod")]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct CycleVariantContent {
    #[serde(flatten)]
    back: CycleVariantHost,
    own: String,
}

#[cfg(feature = "zod")]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "type")]
enum CycleVariantHost {
    Wrapped(Box<CycleVariantContent>),
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct ExtraPart {
    priority: i64,
}

/// A base that names itself, written only where something asks what it describes as: what a
/// flattened branch that is a reference rather than an object contributes to its container.
#[cfg(feature = "jsonschema")]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct FlatHolder {
    #[serde(flatten)]
    base: FlatNode,
    extra: String,
}

#[cfg(feature = "jsonschema")]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct FlatNode {
    children: Vec<Self>,
    val: String,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct FlattenOnly {
    #[serde(flatten)]
    variant: DataElementSampleValueVariant,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct MultiFlatten {
    #[serde(flatten)]
    base: BasePart,
    #[serde(flatten)]
    extra: ExtraPart,
    id: String,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct NoFlatten {
    id: String,
    name: String,
}

/// A plain enum flattened into a struct. Declared without `#[model_schema()]`: a plain enum writes
/// its own variant name rather than an object, so the crate refuses the declaration and the wire
/// form is only readable from a plain serde type.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
enum FlatHue {
    Red,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct FlatOverEnum {
    own: String,
    #[serde(flatten)]
    tone: FlatHue,
}

/// A newtype over a `String` flattened into a struct: serde writes it as the string it wraps, and
/// the registry cannot tell it apart from a struct — both register as having no enum members. So
/// the declaration compiles and the divergence is left for the merge to catch.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct FlatSlug(String);

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct FlatOverBrand {
    own: String,
    #[serde(flatten)]
    slug: FlatSlug,
}

/// An untagged enum every member of which serde writes as an object, and the struct that flattens
/// it. serde writes whichever member matched into the object the struct is writing, so what the
/// struct writes is one key set per member.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct FlatFirst {
    a: String,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct FlatSecond {
    b: bool,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
enum FlatEither {
    First(FlatFirst),
    Second(FlatSecond),
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct FlatOverUntagged {
    #[serde(flatten)]
    either: FlatEither,
    own: String,
}

/// The same shape with one member serde writes as a string rather than an object. The union names
/// no type of its own, so nothing before the merge can tell the two apart.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
enum FlatScalarEither {
    Obj(FlatFirst),
    Text(String),
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct FlatOverScalarUntagged {
    #[serde(flatten)]
    either: FlatScalarEither,
    own: String,
}

/// A union member that names itself, and the struct that flattens the union. The member describes
/// as a reference into the definitions, and the body it points at is written by the time the merge
/// asks for it.
#[cfg(feature = "jsonschema")]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct FlatSelfNode {
    kids: Vec<Self>,
    leaf: String,
}

#[cfg(feature = "jsonschema")]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
enum FlatSelfEither {
    Node(FlatSelfNode),
}

#[cfg(feature = "jsonschema")]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct FlatOverSelfUntagged {
    #[serde(flatten)]
    either: FlatSelfEither,
    own: String,
}

/// An untagged enum whose members overlap: one member's key set is a subset of the other's, and
/// the difference is a key that member omits when it is absent. serde writes the first member that
/// matches, so the payload it writes for the narrower member is one the wider member admits too.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct OverlapNarrow {
    a: String,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct OverlapWide {
    a: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    b: Option<String>,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
enum OverlapEither {
    Narrow(OverlapNarrow),
    Wide(OverlapWide),
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct FlatOverOverlap {
    #[serde(flatten)]
    either: OverlapEither,
    own: String,
}

/// One object that merges both spellings of a union: a discriminated enum, whose members are
/// exclusive, and the overlapping untagged one, whose members are not.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "kind")]
enum MixedTagged {
    Left { left: String },
    Right { right: bool },
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct FlatOverMixed {
    #[serde(flatten)]
    either: OverlapEither,
    own: String,
    #[serde(flatten)]
    tagged: MixedTagged,
}

/// A union one member of which is itself a union, and the struct that flattens the outer one. serde
/// writes whichever leaf member matched into the object the struct is writing, so the nesting
/// contributes no key of its own and what the struct writes is one key set per leaf.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct NestPlain {
    plain: String,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "kind")]
enum NestTagged {
    Left { left: String },
    Right { right: bool },
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
enum NestEither {
    Plain(NestPlain),
    Tagged(NestTagged),
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct NestHolder {
    #[serde(flatten)]
    either: NestEither,
    own: String,
}

/// The same nesting with one member at every level: a choice of one is no choice, so no wrapper is
/// written and the holder describes as the one key set it writes.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct NestOnly {
    a: String,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
enum NestOnlyInner {
    A(NestOnly),
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
enum NestOnlyOuter {
    Inner(NestOnlyInner),
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct NestOnlyHolder {
    #[serde(flatten)]
    either: NestOnlyOuter,
    own: String,
}

/// Two unions that name each other. The outer one is deferred by the frame that reads it and its
/// body is written by the time the merge asks for it, so the cycle carries no missing body — it is
/// visible only as a name the expansion reaches twice on one path.
#[cfg(feature = "jsonschema")]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct NestCycleHolder {
    #[serde(flatten)]
    either: NestCycleOuter,
    own: String,
}

#[cfg(feature = "jsonschema")]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
enum NestCycleOuter {
    Inner(Box<NestCycleInner>),
}

#[cfg(feature = "jsonschema")]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
enum NestCycleInner {
    Back(Box<NestCycleOuter>),
}

/// A base reached through an `Option`, and the object that flattens it. serde writes the base's
/// members beside the object's own when the field is `Some` and writes the object alone when it is
/// `None` — the declaration guard forces `skip_serializing_if` on the field, so the absent form is
/// the only `None` the crate admits.
///
/// Two required members, so a payload carrying one of them is a base serde never writes.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct OptBase {
    left: String,
    right: bool,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct OptHolder {
    #[serde(flatten, skip_serializing_if = "Option::is_none", default)]
    maybe: Option<OptBase>,
    own: String,
}

/// The same absence over a source that is itself a union: serde writes the matched member's keys or
/// no keys at all, so the choice the `Option` adds sits outside the choice the enum already offered.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct OptUnionHolder {
    #[serde(flatten, skip_serializing_if = "Option::is_none", default)]
    maybe: Option<NestTagged>,
    own: String,
}

/// Whether `payload` is accepted by a document every leaf of which is an object closed by
/// `additionalProperties: false`: a leaf accepts when it names every key the payload carries and
/// requires no key it does not.
///
/// A union is read by the rule its spelling names — `oneOf` accepts on exactly one matching branch
/// and `anyOf` on at least one — so a document that nests one union inside another is read the way
/// a validator reads it, wrapper by wrapper.
#[cfg(feature = "jsonschema")]
fn closed_document_accepts(schema: &serde_json::Value, payload: &serde_json::Value) -> bool {
    if let Some(branches) = schema.get("oneOf") {
        return accepting_branches(branches, payload) == 1;
    }
    if let Some(branches) = schema.get("anyOf") {
        return accepting_branches(branches, payload) >= 1;
    }
    let written = payload.as_object().unwrap();
    let named = schema["properties"].as_object().unwrap();
    let required = schema["required"].as_array().unwrap();
    written.keys().all(|key| named.contains_key(key))
        && required
            .iter()
            .all(|key| written.contains_key(key.as_str().unwrap()))
}

/// How many of a union's branches accept `payload` — what the two spellings disagree about.
#[cfg(feature = "jsonschema")]
fn accepting_branches(branches: &serde_json::Value, payload: &serde_json::Value) -> usize {
    branches
        .as_array()
        .unwrap()
        .iter()
        .filter(|branch| closed_document_accepts(branch, payload))
        .count()
}

#[test]
fn test_flatten_structs_constructible() {
    let base = BasePart {
        owner: String::new(),
    };
    assert!(base.owner.is_empty());
    let extra = ExtraPart { priority: 0 };
    assert_eq!(extra.priority, 0_i64);
    let flatten_only = FlattenOnly {
        variant: DataElementSampleValueVariant::Alphanumeric {
            sample_values: Vec::new(),
        },
    };
    assert!(matches!(
        flatten_only.variant,
        DataElementSampleValueVariant::Alphanumeric { .. }
    ));
    let multi = MultiFlatten {
        base: BasePart {
            owner: String::new(),
        },
        extra: ExtraPart { priority: 0 },
        id: String::new(),
    };
    assert!(multi.id.is_empty());
    let no_flatten = NoFlatten {
        id: String::new(),
        name: String::new(),
    };
    assert!(no_flatten.id.is_empty());
}

// ========================================================================
// TypeScript
// ========================================================================

#[test]
#[cfg(feature = "typescript")]
fn test_flatten_typescript_intersection() {
    let ts = DataElementSampleValueEntry::ts_definition();
    assert!(ts.contains("export type DataElementSampleValueEntry = {"));
    assert!(ts.contains("dataElementId: string;"));
    assert!(ts.contains("} & DataElementSampleValueVariant;"));
    assert!(!ts.contains("variant:"));
}

#[test]
#[cfg(feature = "typescript")]
fn test_flatten_typescript_multiple() {
    let ts = MultiFlatten::ts_definition();
    assert!(ts.contains("id: string;"));
    assert!(ts.contains("} & BasePart & ExtraPart;"));
    assert!(!ts.contains("base:"));
    assert!(!ts.contains("extra:"));
}

#[test]
#[cfg(feature = "typescript")]
fn test_flatten_only_typescript_is_alias() {
    let ts = FlattenOnly::ts_definition();
    assert!(ts.contains("export type FlattenOnly = DataElementSampleValueVariant;"));
    assert!(!ts.contains("Record<string, never>"));
    assert!(!ts.contains("variant:"));
}

#[test]
#[cfg(feature = "typescript")]
fn test_no_flatten_typescript_unchanged() {
    let ts = NoFlatten::ts_definition();
    assert!(ts.contains("export type NoFlatten = {"));
    assert!(ts.contains("id: string;"));
    assert!(ts.contains("name: string;"));
    assert!(!ts.contains(" & "));
}

#[test]
#[cfg(feature = "typescript")]
fn test_flatten_variant_keeps_pascal_discriminator_and_camel_fields() {
    let ts = DataElementSampleValueVariant::ts_definition();
    assert!(ts.contains("dataType: \"Numeric\""));
    assert!(ts.contains("sampleValues:"));
    assert!(!ts.contains("sample_values"));
}

// ========================================================================
// Zod
// ========================================================================

#[test]
#[cfg(feature = "zod")]
fn test_flatten_zod_intersection() {
    let zod = DataElementSampleValueEntry::zod_schema();
    assert!(zod.contains("z.strictObject({"));
    assert!(zod.contains("dataElementId:"));
    assert!(zod.contains("}).and(z.lazy(() => DataElementSampleValueVariant$Schema));"));
    assert!(!zod.contains("variant:"));
}

#[test]
#[cfg(feature = "zod")]
fn test_flatten_zod_multiple_chained() {
    let zod = MultiFlatten::zod_schema();
    assert!(
        zod.contains("}).and(z.lazy(() => BasePart$Schema)).and(z.lazy(() => ExtraPart$Schema));")
    );
}

#[test]
#[cfg(feature = "zod")]
fn test_no_flatten_zod_unchanged() {
    let zod = NoFlatten::zod_schema();
    assert!(zod.contains("z.strictObject({"));
    assert!(!zod.contains(".and("));
}

/// A base's schema is a `const` the emitted module reads, and nothing orders one type's module
/// against another's. A flattened base named straight into the intersection would be read while
/// the const initializer runs, which fails outright for any base declared below the type that
/// flattens it — so the operand is the deferred form and the read happens when something validates.
#[test]
#[cfg(feature = "zod")]
fn test_a_flattened_base_is_never_read_while_the_const_initializes() {
    for zod in [
        DataElementSampleValueEntry::zod_schema(),
        MultiFlatten::zod_schema(),
        FlattenOnly::zod_schema(),
    ] {
        for name in ["DataElementSampleValueVariant", "BasePart", "ExtraPart"] {
            assert!(
                !zod.contains(&format!(".and({name}$Schema)")),
                "`{name}$Schema` is read eagerly in: {zod}"
            );
        }
    }
}

/// Two bases that flatten each other name each other's `const`, and no declaration order puts both
/// above the other. Deferring each read is what makes the pair's modules load at all; the cycle is
/// then reached only by asking the schema to validate, never by importing it.
#[test]
#[cfg(feature = "zod")]
fn test_a_flatten_cycle_defers_both_sides_of_the_pair() {
    let first = CycleFirst::zod_schema();
    let second = CycleSecond::zod_schema();

    assert!(
        first.contains("}).and(z.lazy(() => CycleSecond$Schema));"),
        "expected a deferred base, got: {first}"
    );
    assert!(
        second.contains("}).and(z.lazy(() => CycleFirst$Schema));"),
        "expected a deferred base, got: {second}"
    );
    assert!(
        !first.contains(".and(CycleSecond$Schema)"),
        "`CycleSecond$Schema` is read eagerly in: {first}"
    );
    assert!(
        !second.contains(".and(CycleFirst$Schema)"),
        "`CycleFirst$Schema` is read eagerly in: {second}"
    );
}

/// An intersection operand is written in two places — a struct's flattened base and an internally
/// tagged newtype variant's content — and a cycle can run through both. Deferring one side alone
/// leaves the other reading a name that a module declared below has not bound yet, so both sides
/// carry the same deferral and the pair loads whichever order the modules are assembled in.
#[test]
#[cfg(feature = "zod")]
fn test_a_flatten_cycle_through_a_variants_content_defers_both_sides() {
    let content = CycleVariantContent::zod_schema();
    let host = CycleVariantHost::zod_schema();

    assert!(
        content.contains("}).and(z.lazy(() => CycleVariantHost$Schema));"),
        "expected a deferred base, got: {content}"
    );
    assert!(
        host.contains("}).and(z.lazy(() => CycleVariantContent$Schema))"),
        "expected a deferred content, got: {host}"
    );
    assert!(
        !content.contains(".and(CycleVariantHost$Schema)"),
        "`CycleVariantHost$Schema` is read eagerly in: {content}"
    );
    assert!(
        !host.contains(".and(CycleVariantContent$Schema)"),
        "`CycleVariantContent$Schema` is read eagerly in: {host}"
    );
}

// ========================================================================
// JSON Schema
// ========================================================================

#[test]
#[cfg(feature = "jsonschema")]
fn test_flatten_json_schema_distributes_base_into_variants() {
    let schema = DataElementSampleValueEntry::json_schema();
    let one_of = schema["oneOf"].as_array().unwrap();
    assert_eq!(one_of.len(), 3);
    for branch in one_of {
        assert_eq!(
            branch["additionalProperties"],
            serde_json::Value::Bool(false)
        );
        let props = branch["properties"].as_object().unwrap();
        assert!(props.contains_key("dataElementId"));
        assert!(props.contains_key("dataType"));
        assert!(props.contains_key("sampleValues"));
        let req = branch["required"].as_array().unwrap();
        assert!(req.iter().any(|v| v.as_str() == Some("dataElementId")));
        assert!(req.iter().any(|v| v.as_str() == Some("dataType")));
        assert!(req.iter().any(|v| v.as_str() == Some("sampleValues")));
    }
}

#[test]
#[cfg(feature = "jsonschema")]
fn test_multi_flatten_json_schema_merges_plain_structs() {
    let schema = MultiFlatten::json_schema();
    assert!(schema.get("oneOf").is_none());
    assert_eq!(
        schema["additionalProperties"],
        serde_json::Value::Bool(false)
    );
    let props = schema["properties"].as_object().unwrap();
    assert!(props.contains_key("id"));
    assert!(props.contains_key("owner"));
    assert!(props.contains_key("priority"));
}

#[test]
#[cfg(feature = "jsonschema")]
fn test_flatten_only_json_schema_is_union() {
    let schema = FlattenOnly::json_schema();
    let one_of = schema["oneOf"].as_array().unwrap();
    assert_eq!(one_of.len(), 3);
    for branch in one_of {
        let props = branch["properties"].as_object().unwrap();
        assert!(props.contains_key("dataType"));
        assert!(props.contains_key("sampleValues"));
        assert!(!props.contains_key("dataElementId"));
    }
}

#[test]
#[cfg(feature = "jsonschema")]
fn test_no_flatten_json_schema_closes_additional_properties() {
    let schema = NoFlatten::json_schema();
    assert_eq!(
        schema["additionalProperties"],
        serde_json::Value::Bool(false)
    );
    assert!(schema.get("oneOf").is_none());
}

#[test]
#[cfg(feature = "jsonschema")]
fn test_flatten_recursive_base_contributes_its_fields() {
    let schema = FlatHolder::json_schema();
    assert!(schema.get("oneOf").is_none());
    assert_eq!(
        schema["additionalProperties"],
        serde_json::Value::Bool(false)
    );
    let props = schema["properties"].as_object().unwrap();
    assert!(props.contains_key("children"));
    assert!(props.contains_key("val"));
    assert!(props.contains_key("extra"));
    let req = schema["required"].as_array().unwrap();
    for name in ["children", "val", "extra"] {
        assert!(
            req.iter().any(|v| v.as_str() == Some(name)),
            "{name} missing from {req:?}"
        );
    }
}

/// The base's own self-reference is written from the container's root, so it has to resolve there.
#[test]
#[cfg(feature = "jsonschema")]
fn test_flatten_recursive_base_self_reference_resolves_from_the_container() {
    let schema = FlatHolder::json_schema();
    let reference = schema["properties"]["children"]["items"]["$ref"]
        .as_str()
        .unwrap();
    let pointer = reference.strip_prefix('#').unwrap();
    let resolved = schema.pointer(pointer).unwrap();
    let resolved_props = resolved["properties"].as_object().unwrap();
    assert!(resolved_props.contains_key("children"));
    assert!(resolved_props.contains_key("val"));
}

/// A cycle closed through flatten edges has no body to merge at either end, so it is named rather
/// than described as the closed object over whatever fields happened to be written first.
#[test]
#[cfg(feature = "jsonschema")]
#[should_panic(
    expected = "`CycleSecond`: `#[serde(flatten)]` of `CycleFirst` closes a flatten cycle"
)]
fn test_flatten_cycle_is_rejected_rather_than_described() {
    assert!(CycleFirst::json_schema().is_object());
}

/// The rejection is the cycle's, not the entry point's: asking either end names the edge that
/// closes it.
#[test]
#[cfg(feature = "jsonschema")]
#[should_panic(
    expected = "`CycleFirst`: `#[serde(flatten)]` of `CycleSecond` closes a flatten cycle"
)]
fn test_flatten_cycle_is_rejected_from_either_end() {
    assert!(CycleSecond::json_schema().is_object());
}

/// A cycle that closes through one member of a union is the same cycle — there is no body to merge
/// at the branch either — so the branch is named rather than merged as the reference it is, which
/// contributes nothing and closes the document around the base alone.
#[test]
#[cfg(feature = "jsonschema")]
#[should_panic(
    expected = "`CycleUnionNode`: `#[serde(flatten)]` of `CycleUnionEither` closes a flatten cycle through a union member — its branch 1 is `CycleUnionNode`"
)]
fn test_a_flatten_cycle_through_a_union_member_is_rejected() {
    assert!(CycleUnionNode::json_schema().is_object());
}

/// The remedy is the one a cycle closed through the value itself names.
#[test]
#[cfg(feature = "jsonschema")]
#[should_panic(
    expected = "write the field as a named member so the cycle defers through a reference"
)]
fn test_the_union_member_cycle_refusal_names_the_remedy() {
    assert!(CycleUnionNode::json_schema().is_object());
}

/// Flattening a base that does not name itself writes the document it wrote before, byte for byte.
#[test]
#[cfg(feature = "jsonschema")]
fn test_non_recursive_flatten_documents_are_byte_identical() {
    assert_eq!(
        serde_json::to_string(&DataElementSampleValueEntry::json_schema()).unwrap(),
        r#"{"type":"object","oneOf":[{"type":"object","properties":{"dataElementId":{"type":"string"},"dataType":{"type":"string","const":"Alphanumeric"},"sampleValues":{"type":"array","items":{"type":"string"}}},"required":["dataElementId","dataType","sampleValues"],"additionalProperties":false},{"type":"object","properties":{"dataElementId":{"type":"string"},"dataType":{"type":"string","const":"Logical"},"sampleValues":{"type":"array","items":{"type":"boolean"}}},"required":["dataElementId","dataType","sampleValues"],"additionalProperties":false},{"type":"object","properties":{"dataElementId":{"type":"string"},"dataType":{"type":"string","const":"Numeric"},"sampleValues":{"type":"array","items":{"type":"integer"}}},"required":["dataElementId","dataType","sampleValues"],"additionalProperties":false}]}"#
    );
    assert_eq!(
        serde_json::to_string(&MultiFlatten::json_schema()).unwrap(),
        r#"{"type":"object","properties":{"id":{"type":"string"},"owner":{"type":"string"},"priority":{"type":"integer"}},"required":["id","owner","priority"],"additionalProperties":false}"#
    );
    assert_eq!(
        serde_json::to_string(&FlattenOnly::json_schema()).unwrap(),
        r#"{"type":"object","oneOf":[{"type":"object","properties":{"dataType":{"type":"string","const":"Alphanumeric"},"sampleValues":{"type":"array","items":{"type":"string"}}},"required":["dataType","sampleValues"],"additionalProperties":false},{"type":"object","properties":{"dataType":{"type":"string","const":"Logical"},"sampleValues":{"type":"array","items":{"type":"boolean"}}},"required":["dataType","sampleValues"],"additionalProperties":false},{"type":"object","properties":{"dataType":{"type":"string","const":"Numeric"},"sampleValues":{"type":"array","items":{"type":"integer"}}},"required":["dataType","sampleValues"],"additionalProperties":false}]}"#
    );
    assert_eq!(
        serde_json::to_string(&NoFlatten::json_schema()).unwrap(),
        r#"{"type":"object","additionalProperties":false,"properties":{"id":{"type":"string"},"name":{"type":"string"}},"required":["id","name"]}"#
    );
}

/// And flattening a base that names itself with no union in the middle writes the document it wrote
/// before, byte for byte: the whole-body path reads the deferred name as it always did.
#[test]
#[cfg(feature = "jsonschema")]
fn test_the_deferred_flatten_document_is_byte_identical() {
    assert_eq!(
        serde_json::to_string(&FlatHolder::json_schema()).unwrap(),
        r##"{"$defs":{"FlatNode":{"type":"object","additionalProperties":false,"properties":{"children":{"type":"array","items":{"$ref":"#/$defs/FlatNode"}},"val":{"type":"string"}},"required":["children","val"]}},"type":"object","properties":{"extra":{"type":"string"},"children":{"type":"array","items":{"$ref":"#/$defs/FlatNode"}},"val":{"type":"string"}},"required":["extra","children","val"],"additionalProperties":false}"##
    );
}

// ========================================================================
// Serialization round-trip
// ========================================================================

#[test]
fn test_flatten_serialization_is_flat() {
    let entry = DataElementSampleValueEntry {
        data_element_id: "abc".to_owned(),
        variant: DataElementSampleValueVariant::Numeric {
            sample_values: vec![1_i64, 2_i64, 3_i64],
        },
    };
    let json = serde_json::to_value(&entry).unwrap();
    assert_eq!(json["dataElementId"], "abc");
    assert_eq!(json["dataType"], "Numeric");
    assert_eq!(
        json["sampleValues"],
        serde_json::json!([1_i64, 2_i64, 3_i64])
    );
    assert!(json.get("variant").is_none());

    let back: DataElementSampleValueEntry = serde_json::from_value(json).unwrap();
    assert_eq!(back, entry);
}

#[test]
#[cfg(feature = "jsonschema")]
fn test_flatten_recursive_base_serializes_flat() {
    let holder = FlatHolder {
        base: FlatNode {
            children: vec![FlatNode {
                children: Vec::new(),
                val: "leaf".to_owned(),
            }],
            val: "root".to_owned(),
        },
        extra: "x".to_owned(),
    };
    let json = serde_json::to_value(&holder).unwrap();
    assert_eq!(json["val"], "root");
    assert_eq!(json["extra"], "x");
    assert_eq!(json["children"][0]["val"], "leaf");
    assert!(json.get("base").is_none());

    let back: FlatHolder = serde_json::from_value(json).unwrap();
    assert_eq!(back, holder);
}

/// What serde writes for a flattened plain enum: the enum's own variant name, as a key holding
/// null. No schema closed around the struct's remaining fields names that key, which is why the
/// declaration is refused rather than described.
#[test]
fn test_flattening_a_plain_enum_writes_a_key_the_struct_does_not_name() {
    assert_eq!(
        serde_json::to_value(FlatOverEnum {
            own: "o".to_owned(),
            tone: FlatHue::Red,
        })
        .unwrap(),
        serde_json::json!({ "own": "o", "Red": null })
    );
}

/// And what serde writes for a flattened newtype that reaches the wire as a string — nothing. A
/// name got this one past the declaration guard; the value never reaches the wire at all.
#[test]
fn test_flattening_a_string_newtype_is_unserializable() {
    let refusal = serde_json::to_value(FlatOverBrand {
        own: "o".to_owned(),
        slug: FlatSlug("s".to_owned()),
    })
    .unwrap_err()
    .to_string();
    assert!(
        refusal.contains("can only flatten structs and maps"),
        "Got: {refusal}"
    );
}

/// So the merge refuses it too, rather than closing the object around the fields that are left.
#[test]
#[cfg(feature = "jsonschema")]
#[should_panic(
    expected = "`FlatOverBrand`: `#[serde(flatten)]` of `FlatSlug` is not written as an object"
)]
fn test_flattening_a_string_newtype_is_refused_by_the_merge() {
    assert!(FlatOverBrand::json_schema().is_object());
}

/// The remedy the refusal names is one the author can act on.
#[test]
#[cfg(feature = "jsonschema")]
#[should_panic(expected = "write the field as a named member so the value gets a key of its own")]
fn test_the_flatten_merge_refusal_names_the_remedy() {
    assert!(FlatOverBrand::json_schema().is_object());
}

/// What serde writes for a flattened untagged enum: the struct's own fields, and beside them the
/// members of whichever union member matched. One key set per member, and no key naming the field.
#[test]
fn test_flattening_an_untagged_enum_writes_the_matched_members_keys() {
    assert_eq!(
        serde_json::to_value(FlatOverUntagged {
            own: "o".to_owned(),
            either: FlatEither::First(FlatFirst { a: "x".to_owned() }),
        })
        .unwrap(),
        serde_json::json!({ "own": "o", "a": "x" })
    );
    assert_eq!(
        serde_json::to_value(FlatOverUntagged {
            own: "o".to_owned(),
            either: FlatEither::Second(FlatSecond { b: true }),
        })
        .unwrap(),
        serde_json::json!({ "own": "o", "b": true })
    );
}

/// So the merged schema is the union of the merges: the base multiplied over every member of the
/// union, each branch closed around exactly the keys that member writes, under the spelling the
/// untagged source used.
#[test]
#[cfg(feature = "jsonschema")]
fn test_flattening_an_untagged_enum_multiplies_the_base_over_its_members() {
    assert_eq!(
        serde_json::to_string(&FlatOverUntagged::json_schema()).unwrap(),
        r#"{"type":"object","anyOf":[{"type":"object","properties":{"own":{"type":"string"},"a":{"type":"string"}},"required":["own","a"],"additionalProperties":false},{"type":"object","properties":{"own":{"type":"string"},"b":{"type":"boolean"}},"required":["own","b"],"additionalProperties":false}]}"#
    );
}

/// And every payload serde writes is accepted by it. Before the base multiplied out, the document
/// closed around `own` alone and rejected both.
#[test]
#[cfg(feature = "jsonschema")]
fn test_the_untagged_flatten_schema_accepts_every_payload_serde_writes() {
    let schema = FlatOverUntagged::json_schema();
    for payload in [
        serde_json::json!({ "own": "o", "a": "x" }),
        serde_json::json!({ "own": "o", "b": true }),
    ] {
        assert!(
            closed_document_accepts(&schema, &payload),
            "{payload} is rejected by {schema}"
        );
    }
    assert!(
        !closed_document_accepts(
            &serde_json::json!({
                "type": "object",
                "properties": { "own": { "type": "string" } },
                "required": ["own"],
                "additionalProperties": false
            }),
            &serde_json::json!({ "own": "o", "a": "x" })
        ),
        "the document closed around the base alone accepts a key it does not name"
    );
}

/// A union member serde writes as a string is a member serde cannot flatten at all.
#[test]
fn test_flattening_an_untagged_enum_over_a_string_member_is_unserializable() {
    let refusal = serde_json::to_value(FlatOverScalarUntagged {
        own: "o".to_owned(),
        either: FlatScalarEither::Text("t".to_owned()),
    })
    .unwrap_err()
    .to_string();
    assert!(
        refusal.contains("can only flatten structs and maps"),
        "Got: {refusal}"
    );
}

/// So the merge refuses the whole union, naming the branch that cannot join the object.
#[test]
#[cfg(feature = "jsonschema")]
#[should_panic(
    expected = "`FlatOverScalarUntagged`: `#[serde(flatten)]` of `FlatScalarEither` writes a union member that is not an object — its branch 2 describes a `string`"
)]
fn test_a_string_member_of_a_flattened_untagged_enum_is_refused_by_the_merge() {
    assert!(FlatOverScalarUntagged::json_schema().is_object());
}

/// The remedy that refusal names is the same one every flattened non-object gets.
#[test]
#[cfg(feature = "jsonschema")]
#[should_panic(expected = "write the field as a named member so the value gets a key of its own")]
fn test_the_untagged_branch_refusal_names_the_remedy() {
    assert!(FlatOverScalarUntagged::json_schema().is_object());
}

/// A union member that names itself writes its own keys beside the struct's, the same as any other
/// member: what it describes as says nothing about what it writes.
#[test]
#[cfg(feature = "jsonschema")]
fn test_flattening_a_self_naming_union_member_writes_its_keys() {
    assert_eq!(
        serde_json::to_value(FlatOverSelfUntagged {
            own: "o".to_owned(),
            either: FlatSelfEither::Node(FlatSelfNode {
                kids: Vec::new(),
                leaf: "l".to_owned(),
            }),
        })
        .unwrap(),
        serde_json::json!({ "own": "o", "kids": [], "leaf": "l" })
    );
}

/// So the member merges as the body it names, reference and all: before it was read back it carried
/// no members, and the document closed around the base alone.
#[test]
#[cfg(feature = "jsonschema")]
fn test_a_deferred_union_member_merges_as_the_body_it_names() {
    assert_eq!(
        serde_json::to_string(&FlatOverSelfUntagged::json_schema()).unwrap(),
        r##"{"$defs":{"FlatSelfNode":{"type":"object","additionalProperties":false,"properties":{"kids":{"type":"array","items":{"$ref":"#/$defs/FlatSelfNode"}},"leaf":{"type":"string"}},"required":["kids","leaf"]}},"type":"object","properties":{"own":{"type":"string"},"kids":{"type":"array","items":{"$ref":"#/$defs/FlatSelfNode"}},"leaf":{"type":"string"}},"required":["own","kids","leaf"],"additionalProperties":false}"##
    );
}

/// And the document accepts what serde writes, self-reference resolving from the container's root.
#[test]
#[cfg(feature = "jsonschema")]
fn test_the_deferred_union_member_schema_accepts_the_payload_serde_writes() {
    let schema = FlatOverSelfUntagged::json_schema();
    let payload = serde_json::json!({ "own": "o", "kids": [], "leaf": "l" });
    assert!(
        closed_document_accepts(&schema, &payload),
        "{payload} is rejected by {schema}"
    );
    let reference = schema["properties"]["kids"]["items"]["$ref"]
        .as_str()
        .unwrap();
    let resolved = schema
        .pointer(reference.strip_prefix('#').unwrap())
        .unwrap();
    assert!(
        resolved["properties"]
            .as_object()
            .unwrap()
            .contains_key("leaf")
    );
}

/// What serde writes for a flattened untagged enum whose members overlap: the narrower member's
/// keys, which are a subset of the wider member's. serde takes the first member that matches, so
/// this is the payload it both writes and reads back.
#[test]
fn test_flattening_an_overlapping_untagged_enum_writes_the_narrower_members_keys() {
    let narrow = FlatOverOverlap {
        own: "o".to_owned(),
        either: OverlapEither::Narrow(OverlapNarrow { a: "x".to_owned() }),
    };
    let written = serde_json::to_value(&narrow).unwrap();
    assert_eq!(written, serde_json::json!({ "own": "o", "a": "x" }));
    let back: FlatOverOverlap = serde_json::from_value(written).unwrap();
    assert_eq!(back, narrow);
}

/// And two branches of the merged document admit that payload: the narrow member's branch names
/// exactly its keys, and the wide member's branch names one more that it does not require.
#[test]
#[cfg(feature = "jsonschema")]
fn test_the_overlapping_payload_is_admitted_by_two_branches() {
    let schema = FlatOverOverlap::json_schema();
    assert_eq!(
        accepting_branches(
            &schema["anyOf"],
            &serde_json::json!({ "own": "o", "a": "x" })
        ),
        2,
        "Got: {schema}"
    );
}

/// So the merge keeps the spelling its source used. An untagged enum is first-match-wins, and more
/// than one branch admitting a payload is its normal state, which is what `anyOf` says and `oneOf`
/// denies.
#[test]
#[cfg(feature = "jsonschema")]
fn test_an_overlapping_untagged_flatten_keeps_the_any_of_spelling() {
    assert_eq!(
        serde_json::to_string(&FlatOverOverlap::json_schema()).unwrap(),
        r#"{"type":"object","anyOf":[{"type":"object","properties":{"own":{"type":"string"},"a":{"type":"string"}},"required":["own","a"],"additionalProperties":false},{"type":"object","properties":{"own":{"type":"string"},"a":{"type":"string"},"b":{"type":"string"}},"required":["own","a"],"additionalProperties":false}]}"#
    );
}

/// And the document accepts every payload serde writes for it. Wrapped in `oneOf`, the payload the
/// narrower member writes matched two branches and was rejected.
#[test]
#[cfg(feature = "jsonschema")]
fn test_the_overlapping_untagged_flatten_schema_accepts_every_payload_serde_writes() {
    let schema = FlatOverOverlap::json_schema();
    for payload in [
        serde_json::json!({ "own": "o", "a": "x" }),
        serde_json::json!({ "own": "o", "a": "x", "b": "y" }),
    ] {
        assert!(
            closed_document_accepts(&schema, &payload),
            "{payload} is rejected by {schema}"
        );
    }
}

/// What serde writes for an object that flattens both spellings of a union: the discriminated
/// enum's tag and members, the untagged enum's matched member, and the object's own keys.
#[test]
fn test_flattening_both_spellings_of_a_union_writes_every_members_keys() {
    let mixed = FlatOverMixed {
        own: "o".to_owned(),
        tagged: MixedTagged::Left {
            left: "l".to_owned(),
        },
        either: OverlapEither::Narrow(OverlapNarrow { a: "x".to_owned() }),
    };
    let written = serde_json::to_value(&mixed).unwrap();
    assert_eq!(
        written,
        serde_json::json!({ "kind": "Left", "left": "l", "a": "x", "own": "o" })
    );
    let back: FlatOverMixed = serde_json::from_value(written).unwrap();
    assert_eq!(back, mixed);
}

/// So each source keeps its own wrapper around its own branches, nested in the order the sources
/// were merged: the untagged enum's overlapping members under `anyOf`, and inside each of them the
/// discriminated enum's exclusive members under `oneOf`.
#[test]
#[cfg(feature = "jsonschema")]
fn test_a_mixed_merge_keeps_each_sources_wrapper() {
    let schema = FlatOverMixed::json_schema();
    let untagged = schema["anyOf"].as_array().unwrap();
    assert_eq!(untagged.len(), 2, "Got: {schema}");
    for branch in untagged {
        assert_eq!(
            branch["oneOf"].as_array().unwrap().len(),
            2,
            "Got: {schema}"
        );
    }
}

/// And the document accepts every payload serde writes for it, whichever member of either union
/// matched.
#[test]
#[cfg(feature = "jsonschema")]
fn test_the_mixed_merge_schema_accepts_every_payload_serde_writes() {
    let schema = FlatOverMixed::json_schema();
    for payload in [
        serde_json::json!({ "kind": "Left", "left": "l", "a": "x", "own": "o" }),
        serde_json::json!({ "kind": "Left", "left": "l", "a": "x", "b": "y", "own": "o" }),
        serde_json::json!({ "kind": "Right", "right": true, "a": "x", "own": "o" }),
        serde_json::json!({ "kind": "Right", "right": true, "a": "x", "b": "y", "own": "o" }),
    ] {
        assert!(
            closed_document_accepts(&schema, &payload),
            "{payload} is rejected by {schema}"
        );
    }
}

/// What serde writes for a struct that flattens a union one member of which is itself a union: the
/// struct's own keys, and beside them the keys of whichever leaf member matched. The inner union is
/// a choice, not a value, so it writes no key of its own.
#[test]
fn test_flattening_a_nested_union_writes_the_leaf_members_keys() {
    let nested: [(NestEither, serde_json::Value); 3] = [
        (
            NestEither::Plain(NestPlain {
                plain: "p".to_owned(),
            }),
            serde_json::json!({ "own": "o", "plain": "p" }),
        ),
        (
            NestEither::Tagged(NestTagged::Left {
                left: "l".to_owned(),
            }),
            serde_json::json!({ "own": "o", "kind": "Left", "left": "l" }),
        ),
        (
            NestEither::Tagged(NestTagged::Right { right: true }),
            serde_json::json!({ "own": "o", "kind": "Right", "right": true }),
        ),
    ];
    for (either, expected) in nested {
        let holder = NestHolder {
            own: "o".to_owned(),
            either,
        };
        let written = serde_json::to_value(&holder).unwrap();
        assert_eq!(written, expected);
        let back: NestHolder = serde_json::from_value(written).unwrap();
        assert_eq!(back, holder);
    }
}

/// So the merged schema multiplies the base out over the leaves rather than over the inner union,
/// which carries no members to merge: before the branches expanded, the document named none of the
/// leaves' keys and closed around `own` alone.
#[test]
#[cfg(feature = "jsonschema")]
fn test_the_nested_union_schema_accepts_every_payload_serde_writes() {
    let schema = NestHolder::json_schema();
    for payload in [
        serde_json::json!({ "own": "o", "plain": "p" }),
        serde_json::json!({ "own": "o", "kind": "Left", "left": "l" }),
        serde_json::json!({ "own": "o", "kind": "Right", "right": true }),
    ] {
        assert!(
            closed_document_accepts(&schema, &payload),
            "{payload} is rejected by {schema}"
        );
    }
}

/// And each union keeps the spelling its own source used, however deep it sits: the untagged outer
/// one is first-match-wins under `anyOf`, and the discriminated inner one exclusive under `oneOf`.
#[test]
#[cfg(feature = "jsonschema")]
fn test_a_nested_union_branch_expands_under_its_own_spelling() {
    assert_eq!(
        serde_json::to_string(&NestHolder::json_schema()).unwrap(),
        r#"{"type":"object","anyOf":[{"type":"object","properties":{"own":{"type":"string"},"plain":{"type":"string"}},"required":["own","plain"],"additionalProperties":false},{"type":"object","oneOf":[{"type":"object","properties":{"own":{"type":"string"},"kind":{"type":"string","const":"Left"},"left":{"type":"string"}},"required":["own","kind","left"],"additionalProperties":false},{"type":"object","properties":{"own":{"type":"string"},"kind":{"type":"string","const":"Right"},"right":{"type":"boolean"}},"required":["own","kind","right"],"additionalProperties":false}]}]}"#
    );
}

/// What serde writes when every level of the nesting holds one member: the same one key set the
/// single leaf writes.
#[test]
fn test_flattening_a_single_member_nested_union_writes_the_leafs_keys() {
    let holder = NestOnlyHolder {
        own: "o".to_owned(),
        either: NestOnlyOuter::Inner(NestOnlyInner::A(NestOnly { a: "x".to_owned() })),
    };
    let written = serde_json::to_value(&holder).unwrap();
    assert_eq!(written, serde_json::json!({ "own": "o", "a": "x" }));
    let back: NestOnlyHolder = serde_json::from_value(written).unwrap();
    assert_eq!(back, holder);
}

/// So the document is that one object, wrapped in nothing: a choice of one is no choice at any
/// depth. Before the branches expanded, it closed around `own` and rejected the only payload the
/// type has.
#[test]
#[cfg(feature = "jsonschema")]
fn test_a_single_member_nested_union_collapses_to_the_leafs_object() {
    assert_eq!(
        serde_json::to_string(&NestOnlyHolder::json_schema()).unwrap(),
        r#"{"type":"object","properties":{"own":{"type":"string"},"a":{"type":"string"}},"required":["own","a"],"additionalProperties":false}"#
    );
    assert!(closed_document_accepts(
        &NestOnlyHolder::json_schema(),
        &serde_json::json!({ "own": "o", "a": "x" })
    ));
}

/// A cycle closed through two unions that name each other has a body at every step — the deferred
/// one is filled in before the merge reads it — so it is the expansion path that names it, and the
/// merge names that path rather than descending it forever.
#[test]
#[cfg(feature = "jsonschema")]
#[should_panic(
    expected = "`NestCycleHolder`: `#[serde(flatten)]` of `NestCycleOuter` closes a flatten cycle through nested unions — its branch 1.1 names `NestCycleOuter`, already expanding on the path `NestCycleOuter`"
)]
fn test_a_cycle_through_nested_unions_names_the_path_it_closes() {
    assert!(NestCycleHolder::json_schema().is_object());
}

/// The remedy is the one every flatten cycle names.
#[test]
#[cfg(feature = "jsonschema")]
#[should_panic(
    expected = "write the field as a named member so the cycle defers through a reference"
)]
fn test_the_nested_union_cycle_refusal_names_the_remedy() {
    assert!(NestCycleHolder::json_schema().is_object());
}

/// What serde writes for a base reached through an `Option`: the base's members beside the object's
/// own when the field is `Some`, and the object's own alone when it is `None`. Both read back as the
/// value that wrote them, so both are payloads of the type rather than one form and one accident.
#[test]
fn test_an_optional_flattened_base_writes_its_members_or_nothing() {
    let forms: [(Option<OptBase>, serde_json::Value); 2] = [
        (
            Some(OptBase {
                left: "l".to_owned(),
                right: true,
            }),
            serde_json::json!({ "left": "l", "right": true, "own": "o" }),
        ),
        (None, serde_json::json!({ "own": "o" })),
    ];
    for (maybe, expected) in forms {
        let holder = OptHolder {
            maybe,
            own: "o".to_owned(),
        };
        let written = serde_json::to_value(&holder).unwrap();
        assert_eq!(written, expected);
        let back: OptHolder = serde_json::from_value(written).unwrap();
        assert_eq!(back, holder);
    }
}

/// And when the optional base is a union, the same two forms with the matched member's keys in the
/// present one: the `Option` and the enum are two choices, and only the innermost writes keys.
#[test]
fn test_an_optional_flattened_union_writes_the_matched_members_keys_or_nothing() {
    let forms: [(Option<NestTagged>, serde_json::Value); 3] = [
        (
            Some(NestTagged::Left {
                left: "l".to_owned(),
            }),
            serde_json::json!({ "kind": "Left", "left": "l", "own": "o" }),
        ),
        (
            Some(NestTagged::Right { right: true }),
            serde_json::json!({ "kind": "Right", "right": true, "own": "o" }),
        ),
        (None, serde_json::json!({ "own": "o" })),
    ];
    for (maybe, expected) in forms {
        let holder = OptUnionHolder {
            maybe,
            own: "o".to_owned(),
        };
        let written = serde_json::to_value(&holder).unwrap();
        assert_eq!(written, expected);
        let back: OptUnionHolder = serde_json::from_value(written).unwrap();
        assert_eq!(back, holder);
    }
}

/// So the merged document accepts both: the base's members are what an object writes beside its own
/// or does not write at all, and folding them into one key set required the object to write keys the
/// `None` payload never carries.
#[test]
#[cfg(feature = "jsonschema")]
fn test_the_optional_flatten_schema_accepts_every_payload_serde_writes() {
    let schema = OptHolder::json_schema();
    for payload in [
        serde_json::json!({ "left": "l", "right": true, "own": "o" }),
        serde_json::json!({ "own": "o" }),
    ] {
        assert!(
            closed_document_accepts(&schema, &payload),
            "{payload} is rejected by {schema}"
        );
    }
}

/// And rejects a base written in part. serde writes the base whole or not at all, so a payload
/// carrying some of its required members is one no value of the type produces — which is what the
/// two branches say and dropping the members from `required` would not.
#[test]
#[cfg(feature = "jsonschema")]
fn test_the_optional_flatten_schema_rejects_a_partial_base() {
    let schema = OptHolder::json_schema();
    for payload in [
        serde_json::json!({ "left": "l", "own": "o" }),
        serde_json::json!({ "right": true, "own": "o" }),
    ] {
        assert!(
            !closed_document_accepts(&schema, &payload),
            "{payload} is accepted by {schema}"
        );
    }
}

/// The document that says both: the base's members joined to the object's under one branch, and the
/// object's own alone under another. `anyOf` is what the choice is written with — the branches
/// overlap wherever the base can write no members of its own, which is the same payload the absent
/// branch stands for.
#[test]
#[cfg(feature = "jsonschema")]
fn test_the_optional_flatten_document_offers_the_base_and_its_absence() {
    assert_eq!(
        serde_json::to_string(&OptHolder::json_schema()).unwrap(),
        r#"{"type":"object","anyOf":[{"type":"object","properties":{"own":{"type":"string"},"left":{"type":"string"},"right":{"type":"boolean"}},"required":["own","left","right"],"additionalProperties":false},{"type":"object","properties":{"own":{"type":"string"}},"required":["own"],"additionalProperties":false}]}"#
    );
}

/// And when the optional source is a union, the absence joins its branches from outside: each member
/// keeps the spelling its own enum was written under, and the absent branch is a choice about the
/// whole union rather than a member of it.
#[test]
#[cfg(feature = "jsonschema")]
fn test_an_optional_flattened_union_offers_every_member_and_their_absence() {
    let schema = OptUnionHolder::json_schema();
    for payload in [
        serde_json::json!({ "kind": "Left", "left": "l", "own": "o" }),
        serde_json::json!({ "kind": "Right", "right": true, "own": "o" }),
        serde_json::json!({ "own": "o" }),
    ] {
        assert!(
            closed_document_accepts(&schema, &payload),
            "{payload} is rejected by {schema}"
        );
    }
    assert_eq!(
        serde_json::to_string(&schema).unwrap(),
        r#"{"type":"object","anyOf":[{"type":"object","oneOf":[{"type":"object","properties":{"own":{"type":"string"},"kind":{"type":"string","const":"Left"},"left":{"type":"string"}},"required":["own","kind","left"],"additionalProperties":false},{"type":"object","properties":{"own":{"type":"string"},"kind":{"type":"string","const":"Right"},"right":{"type":"boolean"}},"required":["own","kind","right"],"additionalProperties":false}]},{"type":"object","properties":{"own":{"type":"string"}},"required":["own"],"additionalProperties":false}]}"#
    );
}

/// The same two key sets on the TypeScript surface: the object's own members beside the base's, or
/// the object's own beside none of them. `| undefined` said something else — that the whole value
/// may be missing — and `&` binds tighter than `|`, so it admitted neither payload the object
/// writes for an absent base.
///
/// The absent branch is the base's own keys mapped to `never` rather than `{}`. Both spell the same
/// two payloads for a base written whole, and only the mapped one keeps a base written in part out:
/// `{}` is every object, so a payload carrying one of the base's members passes through it.
#[test]
#[cfg(feature = "typescript")]
fn test_the_optional_flatten_type_offers_the_base_and_its_absence() {
    let ts = OptHolder::ts_definition();
    assert!(
        ts.contains("} & (OptBase | { [K in keyof OptBase]?: never });"),
        "expected the base beside its absence, got: {ts}"
    );
    assert!(
        !ts.contains("| undefined"),
        "the whole value is offered as absent in: {ts}"
    );
}

/// And when the optional source is a union, the absence joins it from outside: the members keep the
/// spelling their own enum was written under, and the branch that carries none of them is written
/// over the keys every member shares.
#[test]
#[cfg(feature = "typescript")]
fn test_an_optional_flattened_union_type_offers_its_members_and_their_absence() {
    let ts = OptUnionHolder::ts_definition();
    assert!(
        ts.contains("} & (NestTagged | { [K in keyof NestTagged]?: never });"),
        "expected the union beside its absence, got: {ts}"
    );
    assert!(
        !ts.contains("| undefined"),
        "the whole value is offered as absent in: {ts}"
    );
}

/// What Zod says about the same base: a choice between the object's own keys merged with the base
/// and the object's own keys alone. The choice is written outside the intersection because that is
/// the only place Zod can read it — an intersection recognizes the keys its operands name, and an
/// operand that is itself a choice leaves each branch answering for the keys the other one carries,
/// which rejects every payload the object writes.
#[test]
#[cfg(feature = "zod")]
fn test_the_optional_flatten_schema_offers_the_base_and_its_absence() {
    let zod = OptHolder::zod_schema();
    assert!(
        zod.contains(
            "z.union([\n  OptHolder$OwnSchema.and(z.lazy(() => OptBase$Schema)),\n  OptHolder$OwnSchema,\n])"
        ),
        "expected the base beside its absence, got: {zod}"
    );
    assert!(
        !zod.contains(".and(z.lazy(() => z.union("),
        "the choice is written inside the intersection in: {zod}"
    );
}

/// And no branch of it admits a base written in part. The base joins a branch whole, under the name
/// its own schema is bound to, or the branch is the object's own keys alone — so a payload carrying
/// some of the base's members belongs to neither, and neither does a bare `undefined`.
///
/// Read off zod 4.4.3: this union accepts `{"left":"l","right":true,"own":"o"}` and `{"own":"o"}`,
/// and rejects `{"left":"l","own":"o"}`, `{"right":true,"own":"o"}`, `undefined`, and any payload
/// carrying a key neither the object nor the base names.
#[test]
#[cfg(feature = "zod")]
fn test_the_optional_flatten_schema_admits_no_partial_base() {
    let zod = OptHolder::zod_schema();
    assert!(
        !zod.contains("z.undefined()") && !zod.contains(".prefault("),
        "the whole value is offered as absent in: {zod}"
    );
    assert!(
        !zod.contains(".optional()"),
        "the base's members are offered one at a time in: {zod}"
    );
    assert_eq!(
        zod.matches("OptBase$Schema").count(),
        1,
        "the base is named more than once in: {zod}"
    );
}

/// What the object's own keys are bound to, so each branch names them rather than repeating them:
/// one strict object, read by both branches of the choice.
#[test]
#[cfg(feature = "zod")]
fn test_the_optional_flatten_schema_binds_the_objects_own_keys_once() {
    let zod = OptHolder::zod_schema();
    assert!(
        zod.contains("const OptHolder$OwnSchema = z.strictObject({\n  own: z.string(),\n\n});"),
        "expected the object's own keys bound once, got: {zod}"
    );
    assert_eq!(
        zod.matches("z.strictObject(").count(),
        1,
        "the object's own keys are written more than once in: {zod}"
    );
}

/// The absence is a question about the `Option` and nothing else, so a base written without one is
/// spelled exactly as it was before there was a second branch to spell — byte for byte, on both
/// surfaces.
#[test]
#[cfg(feature = "typescript")]
fn test_a_non_optional_flatten_type_is_byte_identical() {
    let ts = MultiFlatten::ts_definition();
    let declared = &ts[ts.find("export type").unwrap()..];
    assert_eq!(
        declared,
        "export type MultiFlatten = {\n  /**\n * id\n * \n**/\n  id: string;\n\n} & BasePart & ExtraPart;"
    );
}

#[test]
#[cfg(feature = "zod")]
fn test_a_non_optional_flatten_schema_is_byte_identical() {
    #[cfg(feature = "typescript")]
    const EXPECTED: &str = "const MultiFlatten$RawSchema = z.strictObject({\n  id: z.string(),\n\n}).and(z.lazy(() => BasePart$Schema)).and(z.lazy(() => ExtraPart$Schema));\n\nexport const MultiFlatten$Schema: ZodType<MultiFlatten> = MultiFlatten$RawSchema;";
    #[cfg(not(feature = "typescript"))]
    const EXPECTED: &str = "export const MultiFlatten$Schema = z.strictObject({\n  id: z.string(),\n\n}).and(z.lazy(() => BasePart$Schema)).and(z.lazy(() => ExtraPart$Schema));";

    assert_eq!(MultiFlatten::zod_schema(), EXPECTED);
}

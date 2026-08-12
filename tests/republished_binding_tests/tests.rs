#[cfg(feature = "chrono")]
use chrono::{DateTime, TimeZone as _, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tixschema::model_schema;

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum MetricSlot {
    Daily,
    Weekly,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct PlainStruct {
    label: String,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
#[serde(transparent)]
struct CorrelationId(String);

/// A plain enum derives no `Display`, so the brand over one opts out of the impl.
#[model_schema(no_display)]
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[serde(transparent)]
struct SlotBrand(MetricSlot);

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[serde(transparent)]
struct BoolBrand(bool);

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[serde(transparent)]
struct NumBrand(i32);

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[serde(transparent)]
struct NumBrandOverBrand(NumBrand);

#[model_schema(no_display)]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(transparent)]
struct StructBrand(PlainStruct);

#[model_schema(no_display, default_types(IdType = String))]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(transparent)]
struct GenericBrand<IdType>(IdType);

#[cfg(feature = "chrono")]
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[serde(transparent)]
struct StampBrand(DateTime<Utc>);

#[model_schema()]
type CorrelationRef = CorrelationId;

#[model_schema()]
type SlotBrandRef = SlotBrand;

#[model_schema()]
type BoolBrandRef = BoolBrand;

#[model_schema()]
type NumBrandRef = NumBrand;

#[model_schema()]
type NumBrandOverBrandRef = NumBrandOverBrand;

#[model_schema()]
type StructBrandRef = StructBrand;

#[model_schema()]
type PlainStructRef = PlainStruct;

#[cfg(feature = "chrono")]
#[model_schema()]
type StampBrandRef = StampBrand;

/// An alias of an alias of a brand. The link above it has to forward the brand's own narrowing or
/// this one republishes something already widened.
#[model_schema()]
type ChainedBrandRef = CorrelationRef;

/// An alias written *above* the brand it names: nothing registered can answer for `LateBrand` at
/// this expansion, one macro invocation seeing one item.
#[model_schema()]
type ForwardBrandRef = LateBrand;

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
#[serde(transparent)]
struct LateBrand(String);

#[model_schema()]
type GenericBrandBoundAlias = GenericBrand<String>;

#[model_schema(default_types(IdType = String))]
type GenericBrandAlias<IdType> = GenericBrand<IdType>;

/// serde writes a one-slot tuple struct as the slot's value alone, so this publishes the brand's
/// own binding exactly as an alias of it does — no `#[serde(transparent)]` involved.
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct SlotWrapper(CorrelationId);

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct BoundBrandSlot(GenericBrand<String>);

/// A republished binding carrying an example: the `.meta({ example })` lands on the raw `const`'s
/// value, which `.meta()` returns unchanged.
/// ```rust example
/// ExampledSlot(CorrelationId("corr-1".to_string()))
/// ```
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct ExampledSlot(CorrelationId);

#[model_schema(default_types(IdType = String))]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct BrandSlot<IdType>(GenericBrand<IdType>);

/// The slot is one of the struct's own parameters, not a sibling — the factory composes whatever
/// argument it is handed, and the declared default being a brand changes nothing about that.
#[model_schema(default_types(IdType = CorrelationId))]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct SlotHolder<IdType>(IdType);

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct HoldsBrands {
    direct: CorrelationId,
    keyed: HashMap<CorrelationId, u64>,
    listed: Vec<CorrelationId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    maybe: Option<CorrelationId>,
    paired: (CorrelationId, u64),
    valued: HashMap<String, CorrelationId>,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(transparent)]
struct NamedWrapper {
    inner: CorrelationId,
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
enum BrandOrNumber {
    Brand(CorrelationId),
    Count(i32),
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
enum SoleBrand {
    Brand(CorrelationId),
}

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct FlattenOnly {
    #[serde(flatten)]
    base: PlainStruct,
}

#[model_schema()]
type SlotKey = String;

#[model_schema()]
type BrandList = Vec<CorrelationId>;

#[model_schema()]
type BrandsByName = HashMap<String, CorrelationId>;

#[model_schema()]
type BrandPair = (CorrelationId, u64);

/// The line an alias-of-brand `const` emitted before its annotation was read back off what it
/// published, and the diagnostic a consumer compiling that line got. Recorded verbatim from tsc
/// 5.9.3 under `--strict --target ES2022 --module ESNext --moduleResolution bundler` against zod
/// 4.4.3 on the crate's own emission:
///
/// ```text
/// export const CorrelationRefType$Schema: ZodType<CorrelationRefType> = CorrelationRefType$RawSchema;
///
/// error TS2322: Type '$ZodBranded<ZodString, "CorrelationId">' is not assignable to type
/// 'ZodType<CorrelationId, unknown, $ZodTypeInternals<CorrelationId, unknown>>'.
///   Types of property '_output' are incompatible.
///     Type 'string' is not assignable to type 'CorrelationId'.
///       Type 'string' is not assignable to type '$brand<"CorrelationId">'.
/// ```
///
/// The same TS2322 landed on the one-slot tuple struct, which restates the annotation over the
/// same republished value with no alias anywhere in the shape:
///
/// ```text
/// export const SlotWrapper$Schema: ZodType<SlotWrapper> = SlotWrapper$RawSchema;
///
/// error TS2322: Type '$ZodBranded<ZodString, "CorrelationId">' is not assignable to type
/// 'ZodType<CorrelationId, unknown, $ZodTypeInternals<CorrelationId, unknown>>'.
///   Types of property '_output' are incompatible.
///     Type 'string' is not assignable to type 'CorrelationId'.
///       Type 'string' is not assignable to type '$brand<"CorrelationId">'.
/// ```
///
/// and on the `$SchemaDefault` of an item whose factory republishes a generic brand's factory:
///
/// ```text
/// export const BrandSlot$SchemaDefault: ZodType<BrandSlot<string>> = BrandSlot$SchemaFactory(z.string());
///
/// error TS2322: Type '$ZodBranded<ZodString, "GenericBrand", "out">' is not assignable to type
/// 'ZodType<BrandSlot<string>, unknown, $ZodTypeInternals<BrandSlot<string>, unknown>>'.
///   Types of property '_output' are incompatible.
///     Type 'string' is not assignable to type 'BrandSlot<string>'.
///       Type 'string' is not assignable to type '$brand<"GenericBrand">'.
/// ```
///
/// Every one of those schemas parsed and branded correctly under node: `.brand()` narrows at the
/// value position (`_zod.output`), which is not the `_output` the restated annotation constrained.
/// Only the annotation over the runtime expression was ever wrong.
#[cfg(all(feature = "zod", feature = "typescript"))]
#[test]
fn an_alias_of_a_brand_reads_its_annotation_off_the_binding_it_republishes() {
    for (zod, line) in [
        (
            correlation_ref_schema::Schema::zod_schema(),
            "export const CorrelationRefType$Schema: typeof CorrelationRefType$RawSchema = \
             CorrelationRefType$RawSchema;",
        ),
        (
            slot_brand_ref_schema::Schema::zod_schema(),
            "export const SlotBrandRefType$Schema: typeof SlotBrandRefType$RawSchema = \
             SlotBrandRefType$RawSchema;",
        ),
        (
            bool_brand_ref_schema::Schema::zod_schema(),
            "export const BoolBrandRefType$Schema: typeof BoolBrandRefType$RawSchema = \
             BoolBrandRefType$RawSchema;",
        ),
        (
            num_brand_ref_schema::Schema::zod_schema(),
            "export const NumBrandRefType$Schema: typeof NumBrandRefType$RawSchema = \
             NumBrandRefType$RawSchema;",
        ),
        (
            num_brand_over_brand_ref_schema::Schema::zod_schema(),
            "export const NumBrandOverBrandRefType$Schema: typeof \
             NumBrandOverBrandRefType$RawSchema = NumBrandOverBrandRefType$RawSchema;",
        ),
        (
            struct_brand_ref_schema::Schema::zod_schema(),
            "export const StructBrandRefType$Schema: typeof StructBrandRefType$RawSchema = \
             StructBrandRefType$RawSchema;",
        ),
        (
            plain_struct_ref_schema::Schema::zod_schema(),
            "export const PlainStructRefType$Schema: typeof PlainStructRefType$RawSchema = \
             PlainStructRefType$RawSchema;",
        ),
    ] {
        assert!(zod.contains(line), "want: {line}\ngot: {zod}");
        assert!(!zod.contains("ZodType<"), "got: {zod}");
    }
}

#[cfg(all(feature = "chrono", feature = "zod", feature = "typescript"))]
#[test]
fn an_alias_of_a_datetime_brand_reads_its_annotation_off_the_binding_it_republishes() {
    let zod = stamp_brand_ref_schema::Schema::zod_schema();
    assert!(
        zod.contains(
            "export const StampBrandRefType$Schema: typeof StampBrandRefType$RawSchema = \
             StampBrandRefType$RawSchema;"
        ),
        "got: {zod}"
    );
    assert!(!zod.contains("ZodType<"), "got: {zod}");
}

/// Declaration order decides nothing here: the annotation names a `const` this module writes
/// itself, so the target's own class never has to be looked up.
#[cfg(all(feature = "zod", feature = "typescript"))]
#[test]
fn an_alias_declared_above_its_brand_reads_the_same_annotation() {
    let zod = forward_brand_ref_schema::Schema::zod_schema();
    assert!(
        zod.contains(
            "export const ForwardBrandRefType$Schema: typeof ForwardBrandRefType$RawSchema = \
             ForwardBrandRefType$RawSchema;"
        ),
        "got: {zod}"
    );
    assert!(!zod.contains("ZodType<"), "got: {zod}");
}

/// A chain of any length forwards for free, each link reading the link above it back.
#[cfg(all(feature = "zod", feature = "typescript"))]
#[test]
fn an_alias_of_an_alias_of_a_brand_forwards_the_annotation() {
    let zod = chained_brand_ref_schema::Schema::zod_schema();
    assert!(
        zod.contains("const ChainedBrandRefType$RawSchema = CorrelationRefType$Schema;"),
        "got: {zod}"
    );
    assert!(
        zod.contains(
            "export const ChainedBrandRefType$Schema: typeof ChainedBrandRefType$RawSchema = \
             ChainedBrandRefType$RawSchema;"
        ),
        "got: {zod}"
    );
    assert!(!zod.contains("ZodType<"), "got: {zod}");
}

#[cfg(all(feature = "zod", feature = "typescript"))]
#[test]
fn a_one_slot_tuple_struct_reads_its_annotation_off_the_binding_it_republishes() {
    let zod = SlotWrapper::zod_schema();
    assert!(
        zod.contains("const SlotWrapper$RawSchema = CorrelationId$Schema;"),
        "got: {zod}"
    );
    assert!(
        zod.contains(
            "export const SlotWrapper$Schema: typeof SlotWrapper$RawSchema = \
             SlotWrapper$RawSchema;"
        ),
        "got: {zod}"
    );
    assert!(!zod.contains("ZodType<"), "got: {zod}");
}

#[cfg(all(feature = "zod", feature = "typescript"))]
#[test]
fn a_one_slot_tuple_struct_over_a_bound_generic_brand_reads_the_call_back() {
    let zod = BoundBrandSlot::zod_schema();
    assert!(
        zod.contains("const BoundBrandSlot$RawSchema = GenericBrand$SchemaFactory(z.string());"),
        "got: {zod}"
    );
    assert!(
        zod.contains(
            "export const BoundBrandSlot$Schema: typeof BoundBrandSlot$RawSchema = \
             BoundBrandSlot$RawSchema;"
        ),
        "got: {zod}"
    );
    assert!(!zod.contains("ZodType<"), "got: {zod}");
}

/// The example is appended to the value the raw `const` holds, so the annotation still reads back
/// the type of what was published.
#[cfg(all(feature = "zod", feature = "typescript"))]
#[test]
fn an_example_on_a_republished_binding_lands_on_the_value_the_annotation_reads() {
    let zod = ExampledSlot::zod_schema();
    assert!(
        zod.contains(
            "export const ExampledSlot$Schema: typeof ExampledSlot$RawSchema = \
             ExampledSlot$RawSchema.meta({\n  example: \"corr-1\"\n});"
        ),
        "got: {zod}"
    );
    assert!(!zod.contains("ZodType<"), "got: {zod}");
}

#[cfg(all(feature = "zod", feature = "typescript"))]
#[test]
fn a_non_generic_alias_of_a_generic_brand_reads_the_bound_call_back() {
    let zod = generic_brand_bound_alias_schema::Schema::zod_schema();
    assert!(
        zod.contains(
            "const GenericBrandBoundAliasType$RawSchema = GenericBrand$SchemaFactory(z.string());"
        ),
        "got: {zod}"
    );
    assert!(
        zod.contains(
            "export const GenericBrandBoundAliasType$Schema: typeof \
             GenericBrandBoundAliasType$RawSchema = GenericBrandBoundAliasType$RawSchema;"
        ),
        "got: {zod}"
    );
    assert!(!zod.contains("ZodType<"), "got: {zod}");
}

/// A generic item has no `const` to read back, so its declared default binds the factory call
/// first and the export reads that binding's type — the same two lines one level up.
#[cfg(all(feature = "zod", feature = "typescript"))]
#[test]
fn a_generic_item_republishing_a_factory_binds_its_default_before_annotating_it() {
    for (zod, raw, exported) in [
        (
            generic_brand_alias_schema::Schema::zod_schema(),
            "const GenericBrandAliasType$RawSchemaDefault = \
             GenericBrandAliasType$SchemaFactory(z.string());",
            "export const GenericBrandAliasType$SchemaDefault: typeof \
             GenericBrandAliasType$RawSchemaDefault = GenericBrandAliasType$RawSchemaDefault;",
        ),
        (
            BrandSlot::<String>::zod_schema(),
            "const BrandSlot$RawSchemaDefault = BrandSlot$SchemaFactory(z.string());",
            "export const BrandSlot$SchemaDefault: typeof BrandSlot$RawSchemaDefault = \
             BrandSlot$RawSchemaDefault;",
        ),
    ] {
        assert!(zod.contains(raw), "want: {raw}\ngot: {zod}");
        assert!(zod.contains(exported), "want: {exported}\ngot: {zod}");
        assert!(!zod.contains("ZodType<"), "got: {zod}");
    }
}

/// A wrapped brand is no republish: every one of these builds a new schema around the brand's
/// binding, and the constructor it builds computes its output from what `.brand()` narrowed.
#[cfg(all(feature = "zod", feature = "typescript"))]
#[test]
fn a_composite_over_a_brand_keeps_the_annotation_naming_its_own_type() {
    for (zod, line) in [
        (
            HoldsBrands::zod_schema(),
            "export const HoldsBrands$Schema: ZodType<HoldsBrands> = HoldsBrands$RawSchema;",
        ),
        (
            NamedWrapper::zod_schema(),
            "export const NamedWrapper$Schema: ZodType<NamedWrapper> = NamedWrapper$RawSchema;",
        ),
        (
            BrandOrNumber::zod_schema(),
            "export const BrandOrNumber$Schema: ZodType<BrandOrNumber> = BrandOrNumber$RawSchema;",
        ),
        (
            SoleBrand::zod_schema(),
            "export const SoleBrand$Schema: ZodType<SoleBrand> = SoleBrand$RawSchema;",
        ),
        (
            FlattenOnly::zod_schema(),
            "export const FlattenOnly$Schema: ZodType<FlattenOnly> = FlattenOnly$RawSchema;",
        ),
    ] {
        assert!(zod.contains(line), "want: {line}\ngot: {zod}");
    }
}

/// An alias that builds its own expression keeps stating the type it published beside — the one
/// place the crate checks a rendered expression against its own TypeScript type.
#[cfg(all(feature = "zod", feature = "typescript"))]
#[test]
fn an_alias_of_a_built_expression_keeps_the_annotation_naming_its_own_type() {
    for (zod, line) in [
        (
            slot_key_schema::Schema::zod_schema(),
            "export const SlotKeyType$Schema: ZodType<SlotKeyType> = SlotKeyType$RawSchema;",
        ),
        (
            brand_list_schema::Schema::zod_schema(),
            "export const BrandListType$Schema: ZodType<BrandListType> = BrandListType$RawSchema;",
        ),
        (
            brands_by_name_schema::Schema::zod_schema(),
            "export const BrandsByNameType$Schema: ZodType<BrandsByNameType> = \
             BrandsByNameType$RawSchema;",
        ),
        (
            brand_pair_schema::Schema::zod_schema(),
            "export const BrandPairType$Schema: ZodType<BrandPairType> = BrandPairType$RawSchema;",
        ),
    ] {
        assert!(zod.contains(line), "want: {line}\ngot: {zod}");
    }
}

/// The slot names a parameter, not a sibling, so the default keeps restating the type it fills —
/// the argument the factory is handed is the caller's business, brand or not.
#[cfg(all(feature = "zod", feature = "typescript"))]
#[test]
fn a_generic_slot_over_a_bare_parameter_keeps_the_annotation_naming_its_own_type() {
    let zod = SlotHolder::<CorrelationId>::zod_schema();
    assert!(
        zod.contains(
            "export const SlotHolder$SchemaDefault: ZodType<SlotHolder<CorrelationId>> = \
             SlotHolder$SchemaFactory(z.lazy(() => CorrelationId$Schema));"
        ),
        "got: {zod}"
    );
    assert!(!zod.contains("$RawSchemaDefault"), "got: {zod}");
}

/// A brand's own binding is annotated from its inner's class and never republishes anything.
#[cfg(all(feature = "zod", feature = "typescript"))]
#[test]
fn a_brands_own_binding_keeps_the_annotation_it_already_wrote() {
    let zod = CorrelationId::zod_schema();
    assert!(
        zod.contains(
            "export const CorrelationId$Schema: $ZodBranded<ZodString, \"CorrelationId\"> = \
             CorrelationId$RawSchema;"
        ),
        "got: {zod}"
    );
    let generic = GenericBrand::<String>::zod_schema();
    assert!(
        generic.contains(
            "export const GenericBrand$SchemaDefault: $ZodBranded<ZodString, \"GenericBrand\"> = \
             GenericBrand$SchemaFactory(z.string());"
        ),
        "got: {generic}"
    );
    assert!(!generic.contains("$RawSchemaDefault"), "got: {generic}");
}

/// A build emitting no TypeScript writes every binding as a bare `const`: an annotation is a type,
/// and a JavaScript parser reading one stops at the `:` with no initializer to read.
#[cfg(all(feature = "zod", not(feature = "typescript")))]
#[test]
fn a_javascript_build_annotates_a_republished_binding_no_more_than_any_other() {
    for zod in [
        correlation_ref_schema::Schema::zod_schema(),
        chained_brand_ref_schema::Schema::zod_schema(),
        SlotWrapper::zod_schema(),
        BrandSlot::<String>::zod_schema(),
    ] {
        assert!(zod.contains("export const "), "got: {zod}");
        assert!(!zod.contains("typeof "), "got: {zod}");
        assert!(!zod.contains("$Schema:"), "got: {zod}");
    }
}

/// The TypeScript side is untouched: an alias still publishes the target's own name, and the
/// one-slot tuple struct still publishes the slot's.
#[cfg(feature = "typescript")]
#[test]
fn the_typescript_surface_of_a_republished_binding_is_unchanged() {
    assert!(
        correlation_ref_schema::Schema::ts_definition()
            .contains("export type CorrelationRefType = CorrelationId;"),
        "got: {}",
        correlation_ref_schema::Schema::ts_definition()
    );
    assert!(
        SlotWrapper::ts_definition().contains("export type SlotWrapper = CorrelationId;"),
        "got: {}",
        SlotWrapper::ts_definition()
    );
}

/// What serde actually writes for each of these, read off the wire rather than assumed: a brand
/// and every name over it are invisible, so each republished binding validates the bare value its
/// target validates.
#[test]
fn a_brand_and_the_names_over_it_write_the_bare_value_serde_writes() {
    let branded = CorrelationId("corr-1".to_owned());
    assert_eq!(serde_json::to_string(&branded).unwrap(), "\"corr-1\"");

    let aliased: CorrelationRef = branded.clone();
    assert_eq!(serde_json::to_string(&aliased).unwrap(), "\"corr-1\"");
    let chained: ChainedBrandRef = branded.clone();
    assert_eq!(serde_json::to_string(&chained).unwrap(), "\"corr-1\"");

    let wrapped = SlotWrapper(branded.clone());
    assert_eq!(serde_json::to_string(&wrapped).unwrap(), "\"corr-1\"");
    assert_eq!(
        serde_json::from_str::<SlotWrapper>("\"corr-1\"").unwrap(),
        wrapped
    );

    let exampled = ExampledSlot(branded.clone());
    assert_eq!(serde_json::to_string(&exampled).unwrap(), "\"corr-1\"");
    assert_eq!(
        serde_json::from_str::<ExampledSlot>("\"corr-1\"").unwrap(),
        exampled
    );

    let bound = BoundBrandSlot(GenericBrand("corr-1".to_owned()));
    assert_eq!(serde_json::to_string(&bound).unwrap(), "\"corr-1\"");
    assert_eq!(
        serde_json::from_str::<BoundBrandSlot>("\"corr-1\"").unwrap(),
        bound
    );

    let slotted = BrandSlot::<String>(GenericBrand("corr-1".to_owned()));
    assert_eq!(serde_json::to_string(&slotted).unwrap(), "\"corr-1\"");
    assert_eq!(
        serde_json::from_str::<BrandSlot<String>>("\"corr-1\"").unwrap(),
        slotted
    );

    let held = SlotHolder::<CorrelationId>(branded);
    assert_eq!(serde_json::to_string(&held).unwrap(), "\"corr-1\"");

    let forward: ForwardBrandRef = LateBrand("late-1".to_owned());
    assert_eq!(serde_json::to_string(&forward).unwrap(), "\"late-1\"");
}

/// The brands over the other inners, on the wire, so the aliases above are grounded in what serde
/// wrote for each rather than in a wire form read off the string brand alone.
#[test]
fn every_branded_inner_writes_the_value_its_target_writes() {
    let slotted: SlotBrandRef = SlotBrand(MetricSlot::Weekly);
    assert_eq!(serde_json::to_string(&slotted).unwrap(), "\"Weekly\"");
    let flagged: BoolBrandRef = BoolBrand(true);
    assert_eq!(serde_json::to_string(&flagged).unwrap(), "true");
    let counted: NumBrandRef = NumBrand(-7);
    assert_eq!(serde_json::to_string(&counted).unwrap(), "-7");
    let twice: NumBrandOverBrandRef = NumBrandOverBrand(NumBrand(-7));
    assert_eq!(serde_json::to_string(&twice).unwrap(), "-7");

    let structured = StructBrand(PlainStruct {
        label: "l".to_owned(),
    });
    assert_eq!(
        serde_json::to_string(&structured).unwrap(),
        "{\"label\":\"l\"}"
    );
    assert_eq!(
        serde_json::from_str::<StructBrandRef>("{\"label\":\"l\"}").unwrap(),
        structured
    );
    let aliased_plain: PlainStructRef = PlainStruct {
        label: "l".to_owned(),
    };
    assert_eq!(
        serde_json::to_string(&aliased_plain).unwrap(),
        "{\"label\":\"l\"}"
    );

    let bound: GenericBrandBoundAlias = GenericBrand("g".to_owned());
    assert_eq!(serde_json::to_string(&bound).unwrap(), "\"g\"");
    let parameterized: GenericBrandAlias<String> = GenericBrand("g".to_owned());
    assert_eq!(serde_json::to_string(&parameterized).unwrap(), "\"g\"");
}

/// The aliases that build an expression of their own, on the wire: each resolves straight through
/// to its target, which is why none of them republishes a binding to read an annotation back off.
#[test]
fn an_alias_of_a_built_expression_writes_its_targets_wire_form() {
    let key: SlotKey = "k".to_owned();
    assert_eq!(serde_json::to_string(&key).unwrap(), "\"k\"");
    let listed: BrandList = vec![CorrelationId("l".to_owned())];
    assert_eq!(serde_json::to_string(&listed).unwrap(), "[\"l\"]");
    let named: BrandsByName = HashMap::from([("n".to_owned(), CorrelationId("v".to_owned()))]);
    assert_eq!(serde_json::to_string(&named).unwrap(), "{\"n\":\"v\"}");
    let paired: BrandPair = (CorrelationId("p".to_owned()), 3_u64);
    assert_eq!(serde_json::to_string(&paired).unwrap(), "[\"p\",3]");
}

#[cfg(feature = "chrono")]
#[test]
fn a_datetime_brand_writes_the_timestamp_its_target_writes() {
    let stamped = StampBrand(Utc.with_ymd_and_hms(2020, 1, 2, 3, 4, 5).unwrap());
    assert_eq!(
        serde_json::to_string(&stamped).unwrap(),
        "\"2020-01-02T03:04:05Z\""
    );
    assert_eq!(
        serde_json::from_str::<StampBrandRef>("\"2020-01-02T03:04:05Z\"").unwrap(),
        stamped
    );
}

/// The composites the annotation must not widen past, on the wire: a brand is spent as a bare
/// value in every one of these positions, which is why each still builds a schema of its own.
#[test]
fn a_brand_in_a_composite_is_written_as_the_bare_value_in_every_position() {
    let held = HoldsBrands {
        direct: CorrelationId("a".to_owned()),
        keyed: HashMap::from([(CorrelationId("k".to_owned()), 1_u64)]),
        listed: vec![CorrelationId("l".to_owned())],
        maybe: Some(CorrelationId("m".to_owned())),
        paired: (CorrelationId("p".to_owned()), 2_u64),
        valued: HashMap::from([("v".to_owned(), CorrelationId("w".to_owned()))]),
    };
    let wire = serde_json::to_value(&held).unwrap();
    assert_eq!(wire["direct"], serde_json::json!("a"));
    assert_eq!(wire["keyed"], serde_json::json!({ "k": 1_u64 }));
    assert_eq!(wire["listed"], serde_json::json!(["l"]));
    assert_eq!(wire["maybe"], serde_json::json!("m"));
    assert_eq!(wire["paired"], serde_json::json!(["p", 2_u64]));
    assert_eq!(wire["valued"], serde_json::json!({ "v": "w" }));
    assert_eq!(serde_json::from_value::<HoldsBrands>(wire).unwrap(), held);

    let named = NamedWrapper {
        inner: CorrelationId("n".to_owned()),
    };
    assert_eq!(serde_json::to_string(&named).unwrap(), "\"n\"");
    assert_eq!(
        serde_json::to_string(&BrandOrNumber::Brand(CorrelationId("b".to_owned()))).unwrap(),
        "\"b\""
    );
    assert_eq!(
        serde_json::to_string(&SoleBrand::Brand(CorrelationId("s".to_owned()))).unwrap(),
        "\"s\""
    );
    assert_eq!(
        serde_json::to_string(&FlattenOnly {
            base: PlainStruct {
                label: "f".to_owned()
            }
        })
        .unwrap(),
        "{\"label\":\"f\"}"
    );
}

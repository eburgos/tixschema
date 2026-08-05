extern crate alloc;

use alloc::borrow::ToOwned;
use core::fmt::Write as _;

use proc_macro::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream, Parser as _};
use syn::punctuated::Punctuated;
use syn::{Field, Item, ItemType, Meta, Token, parse_macro_input};

#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
use quote::quote_spanned;

#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
use syn::spanned::Spanned as _;

#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
use syn::Ident;

use crate::{
    field_type::{
        FieldDef, FieldDefType, VariantKind, classify_variant, get_field_def, is_plain_enum,
    },
    utils::{get_field_docs, get_variant_docs, strip_examples_from_docs},
};

#[cfg(feature = "jsonschema")]
use crate::field_type::is_undescribable_primitive;

#[cfg(feature = "typescript")]
use crate::utils::ts_generic_params;
use crate::utils::type_parameters_in_scope;
#[cfg(feature = "serde")]
use crate::utils::written_type;

#[cfg(all(feature = "zod", feature = "typescript"))]
use core::iter::once;

#[cfg(feature = "zod")]
use crate::utils::{
    escape_js_regex_literal, extract_example_from_docs, publishes_zod_factory,
    record_zod_default_arguments, record_zod_factory, zod_default_arguments, zod_factory_argument,
};

#[cfg(all(feature = "zod", feature = "object_id"))]
use crate::features::object_id::get_object_id_zod_schema_with;

// The 24-character hex an `ObjectId`'s `$oid` member holds, as a JSON-schema `pattern`. Read from
// the `ObjectId` feature module, which is where the Zod literal reads it from too, so the two
// surfaces cannot drift into describing the same string different ways.
#[cfg(all(feature = "jsonschema", feature = "object_id"))]
use crate::features::object_id::OBJECT_ID_HEX_PATTERN;

#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
use crate::utils::{
    AliasKind, PublishedShape, ShapeQuestion, constraining_pattern, lookup_alias_info,
    portable_pattern, record_shape_question, record_value_shape, shape_questions_for,
};

#[cfg(feature = "serde")]
use crate::utils::{TrivialPattern, trivial_pattern};

#[cfg(feature = "serde")]
use crate::features::serde::{SerdeFieldMeta, SerdeTypeMeta};
use crate::features::serde::{has_serde_default, parse_serde_key_omission};
// The type is named where a positional slot's own omission is read: the tuple-struct walk, which
// only a describing build performs, and the variant walk, which every build performs.
use crate::features::serde::SerdeKeyOmission;

#[cfg(feature = "serde")]
use crate::field_type::{parse_serde_field_attributes, parse_serde_type_attributes};

#[cfg(any(
    feature = "typescript",
    feature = "zod",
    feature = "jsonschema",
    feature = "serde"
))]
use crate::field_type::is_sequence_wrapper;

#[cfg(feature = "serde")]
use crate::field_type::is_transparent_wrapper;

use crate::features::model_schema_prop::{ModelSchemaPropMeta, parse_model_schema_prop_attributes};

#[cfg(feature = "jsonschema")]
use crate::features::jsonschema::{
    MergedSource, SchemaParameter,
    generate_plain_enum_json_schema_method as generate_plain_enum_json_schema_method_impl,
    generate_struct_json_schema_method as generate_struct_json_schema_method_impl, in_flight_type,
    json_schema_methods,
};
#[cfg(feature = "jsonschema")]
use crate::utils::json_argument_binding;

#[cfg(all(feature = "serde", feature = "jsonschema"))]
use crate::features::jsonschema::{MergeDiagnostic, merged_object_value};

#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
use crate::utils::get_enum_docs;
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
use crate::utils::get_struct_docs;

#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
use crate::utils::{compute_alias_export_name, ident_schema_module_name};

use crate::utils::compute_item_export_name;

#[cfg(any(feature = "typescript", feature = "zod"))]
use crate::utils::get_item_docs;
#[cfg(feature = "typescript")]
use crate::utils::ident_reexport_ts;

#[cfg(feature = "zod")]
use crate::utils::ident_reexport_zod;

#[cfg(feature = "zod")]
use crate::utils::record_zod_union_members;

#[cfg(all(feature = "serde", feature = "zod"))]
use crate::utils::ZodUnionMember;

#[cfg(all(feature = "serde", any(feature = "zod", feature = "typescript")))]
use crate::utils::{FlattenVariant, WireLeaf, record_flatten_variants, record_wire_leaves};

#[cfg(all(feature = "serde", feature = "typescript"))]
use crate::utils::record_ts_union_members;

#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
use crate::utils::register_alias_info;

#[cfg(feature = "serde")]
use crate::utils::to_snake_case;

use crate::rename_rule::resolve_rename_rule;

/// Every argument the `model_schema` parser reads, in the order it tries them, as the
/// unknown-argument rejection names them.
///
/// An argument added to [`apply_arg`] belongs here too: `no_argument_the_parser_reads_is_rejected`
/// walks this list back through the parser and fails on any name here it does not read.
const KNOWN_ARGS: &[&str] = &[
    "name",
    "pattern",
    "minLength",
    "maxLength",
    "no_display",
    "default_types",
];

/// What every plain-enum flatten diagnostic says about its own reach, so an author who fixes the
/// one declaration named there knows what was and was not checked around it.
#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
const FLATTENED_PLAIN_ENUM_SCOPE: &str = "Only a type the registry has already classified reaches this guard; one it has not keeps its \
     TypeScript and Zod intersection, and is refused by the JSON surface when the merge runs.";

/// The one helper every generated module carries above its per-type definitions, and the only
/// assertion anything here writes.
#[cfg(all(feature = "zod", feature = "typescript"))]
const TYPESCRIPT_PREAMBLE: &str = "const createSchemaCache = <Cache extends object>(): Cache => new WeakMap() as unknown as Cache;";

/// The JavaScript flavour of [`TYPESCRIPT_PREAMBLE`]: a build with no `typescript` writes plain
/// JavaScript, which declares no types and so has nothing to assert.
#[cfg(all(feature = "zod", not(feature = "typescript")))]
const TYPESCRIPT_PREAMBLE: &str = "const createSchemaCache = () => new WeakMap();";

/// The indent every member of an emitted object is written at, and so the indent its `JSDoc` block
/// is written at.
const MEMBER_INDENT: &str = "  ";

#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
const WRITTEN_AS_ARRAY: &str = "a JSON array";
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
const WRITTEN_AS_OBJECT: &str = "a JSON object";

/// One variant of a discriminated enum, carrying everything its union member is rendered from.
struct DiscriminatedVariant {
    discriminator_value: String,
    docs: String,
    field_defs: Vec<FieldDef>,
    kind: VariantKind,
}

/// Per-variant data collected from a discriminated enum, plus the collected serde validators, the
/// `compile_error!` tokens for any field-level guard violations, and the `validate()` match arms,
/// empty where no variant carries a constrained member and there is nothing to aggregate.
type DiscriminatedVariantData = (
    Vec<DiscriminatedVariant>,
    Vec<proc_macro2::TokenStream>,
    Vec<proc_macro2::TokenStream>,
    Vec<proc_macro2::TokenStream>,
);

/// Rendered per-variant output for a discriminated enum: TypeScript fragments, Zod fragments
/// (each with its optional-field list), and JSON-schema fragments.
type RenderedVariants = (
    Vec<String>,
    Vec<(String, Vec<String>)>,
    Vec<proc_macro2::TokenStream>,
);

/// What an untagged enum's members contribute to an object that merges it, which only the Zod
/// surface multiplies out and so only it has a member type for. The other tables carry the same
/// slot spelled as what they never read.
#[cfg(all(feature = "serde", feature = "zod"))]
type ZodMergeParts = Vec<ZodUnionMember>;
#[cfg(all(feature = "serde", not(feature = "zod")))]
type ZodMergeParts = Vec<String>;

/// Per-member data collected from an untagged enum: the TypeScript member types, what those members
/// are spelled as where an object merges the enum, the Zod member schemas, what those Zod members
/// contribute to an object that merges the enum, the JSON-schema value tokens, the `compile_error!`
/// tokens for any field-level guard violations, the per-member serde validation functions, and the
/// `validate()` match arms those functions are run from.
#[cfg(feature = "serde")]
type UntaggedMemberData = (
    Vec<String>,
    Vec<String>,
    Vec<String>,
    ZodMergeParts,
    Vec<proc_macro2::TokenStream>,
    Vec<proc_macro2::TokenStream>,
    Vec<proc_macro2::TokenStream>,
    Vec<proc_macro2::TokenStream>,
);

/// Per-field data collected from a struct: the regular field defs, the `#[serde(flatten)]` field
/// defs, the serde validation functions, the `validate()` body fragments, and the
/// `compile_error!` tokens for any field-level guard violations.
type StructFieldData = (
    Vec<FieldDef>,
    Vec<FieldDef>,
    Vec<proc_macro2::TokenStream>,
    Vec<proc_macro2::TokenStream>,
    Vec<proc_macro2::TokenStream>,
);

/// Borrowed pieces needed to assemble the final token stream for a branded newtype.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
struct BrandedNewtypeOutput<'parts> {
    default_types: &'parts [(syn::Ident, syn::Type)],
    delegate_impl_items: &'parts [proc_macro2::TokenStream],
    display_tokens: &'parts proc_macro2::TokenStream,
    generics: &'parts syn::Generics,
    generics_for_ty: &'parts syn::Generics,
    item_struct: &'parts syn::ItemStruct,
    module_ident: &'parts Ident,
    name: &'parts Ident,
    schema_example_tokens: &'parts proc_macro2::TokenStream,
    schema_impl_items: &'parts [proc_macro2::TokenStream],
    /// The type's raw, unsplit `validate()` — [`assemble_branded_output`] runs it through
    /// [`branded_validate_split`] itself, since doing that here rather than at every call site is
    /// the whole point of bundling these fields into one struct.
    validate_method: &'parts proc_macro2::TokenStream,
    validation_tokens: &'parts proc_macro2::TokenStream,
}

/// A constrained brand's generated validation: the two schema-module functions, and the expression
/// through which `validate()` reaches the inner value it hands to the first of them.
#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
struct BrandedValidation {
    checked_inner: proc_macro2::TokenStream,
    deserialize_fn: proc_macro2::TokenStream,
    validate_fn: proc_macro2::TokenStream,
}

/// What a branded newtype's inner writes when what it writes is a composite: an array, a map, a
/// tuple, or a value the parser could not classify at all.
#[cfg(any(feature = "jsonschema", all(feature = "typescript", feature = "zod")))]
enum BrandedComposite {
    Array,
    Map,
    /// A value with no type name to narrow with, which admits anything.
    Opaque,
    Tuple,
}

/// The JSON schema shape a branded newtype's inner field describes.
#[cfg(feature = "jsonschema")]
enum BrandedJsonInner {
    /// The string a chrono type writes, named by the `"format"` keyword that says which instant it
    /// spells — the one field position carries for the same type.
    #[cfg(feature = "chrono")]
    Chrono(&'static str),
    /// The `$oid` object an `ObjectId` writes, whose hex member carries the brand's string
    /// constraints.
    #[cfg(feature = "object_id")]
    ObjectId,
    /// A `"type"` keyword those constraints sit beside.
    Scalar(String),
    /// An inner no one keyword names, carried by the dispatch that describes the same type
    /// wherever else it is written. Boxed so the whole field def does not set the size of an enum
    /// whose other shapes are a type name and a unit.
    Slot(Box<FieldDef>),
}

/// Whether the schema a brand narrows already states a `pattern` of its own.
#[cfg(feature = "jsonschema")]
#[derive(Clone, Copy)]
enum BasePattern {
    /// The base states none, so the brand's is the schema's own `pattern` keyword.
    Absent,
    #[cfg(feature = "object_id")]
    Stated,
}

/// What a field bottoms out in, under the wrappers it was written beneath.
#[cfg(feature = "serde")]
struct ConstrainedShape {
    leaf: ConstraintLeaf,
    /// Every non-`'static` lifetime the field's type spells, deduplicated. A free function that
    /// returns that type has to declare them itself — nothing of the struct's is in scope there.
    lifetimes: Vec<syn::Lifetime>,
    wraps: Vec<ConstraintWrap>,
}

/// The value a constrained field ultimately writes, and how it is spelled.
#[cfg(feature = "serde")]
enum ConstraintLeaf {
    /// The bare Rust numeric type, which is the validator's parameter type.
    Number(&'static str),
    /// A filesystem path, whose checks read its `to_string_lossy` rendering.
    Path,
    Str,
}

/// What the end of a walk does with a violation it found.
///
/// `validate()` answers with every violation in the instance, so its walk collects; a
/// `Deserializer` answers with one error, so the wire walk stops at the first.
#[cfg(feature = "serde")]
#[derive(Clone, Copy)]
enum CheckSink {
    Collect,
    Fail,
}

/// How the body that checks a constrained member reaches the value.
///
/// A struct's field is read off `self`, which is what `validate()` is called on. A variant's member
/// is not reachable that way — the value only names it once the arm that matched the variant has
/// bound it — so the check reads the binding the arm introduced.
#[cfg(feature = "serde")]
#[derive(Clone, Copy)]
enum MemberAccess {
    SelfField,
    VariantBinding,
}

/// A wrapper a constrained field can be written under, outermost first.
///
/// Each one says how to reach one level further in, and nothing else: an `Option` yields what its
/// `Some` holds, a sequence yields each element, and a transparent wrapper — being nothing on the
/// wire — yields only what it derefs to.
#[cfg(feature = "serde")]
#[derive(Clone, Copy)]
enum ConstraintWrap {
    Optional,
    Sequence,
    Transparent,
}

/// Holds the generated validation code for a single field.
#[cfg(feature = "serde")]
struct FieldValidationCode {
    /// Functions to emit into the schema module (static validator + serde deserializer).
    pub module_items: proc_macro2::TokenStream,
    /// Code to contribute to the type-level `validate()` method body.
    pub validate_body: proc_macro2::TokenStream,
}

/// A map member's rendering with the slot wraps still to apply: a `json!` literal fragment, which
/// only a caller writing inside `serde_json::json!` can inline, or a standalone `serde_json::Value`
/// expression, which either caller can take as it stands.
#[cfg(feature = "jsonschema")]
enum MapMemberItem {
    Fragment(proc_macro2::TokenStream),
    Value(proc_macro2::TokenStream),
}

/// Why a value in a slot has no rendering. Each shape carries its own diagnostic, and the field
/// name the message needs belongs to the caller rather than to the dispatch, so the reason travels
/// out and the message is formatted once at the top.
#[cfg(feature = "jsonschema")]
#[derive(Debug)]
enum MapMemberRejection {
    /// A map key with no rendering, whatever reason it has and at whatever depth the value walk
    /// reached it. The reason is the same value the field guard refuses on, so a key is worded
    /// once however the expansion arrives at it.
    Key(MapKeyRejection),
    Tuple,
}

/// What a map key opens: one open member set, the members it enumerates, nothing this expansion can
/// narrow, or no object at all.
///
/// Every position a map is written in reads its key through this one classification, so a key
/// cannot enumerate its members in field position and stay open in a slot.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
enum MapKeyPath<'key> {
    /// A key named by a type path, whose `enum_members()` become the object's keys.
    Enumerated(&'key str),
    /// A key serde writes as a bare string, which enumerates nothing — one schema stands for every
    /// member. `String` is one, and so is a brand the registry proves serde writes as a string.
    Open,
    /// A key with no object for any surface to describe, carrying the reason.
    Refused(MapKeyRejection),
    /// A key this expansion cannot narrow, leaving the members unconstrained.
    Unnarrowed,
}

/// Why a map key a field reaches has no rendering. Every reason carries the name the author can act
/// on, and every one stands on every surface, the key being read off the field all three render
/// from.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[derive(Debug)]
enum MapKeyRejection {
    /// The registry proves the key type carries no `enum_members()`.
    NoEnumMembers(String),
    /// The key is written as an `Option`, named by the inner it holds. A `Some` writes what the
    /// inner writes; a `None` writes nothing a key can be.
    Optional(String),
    /// The key is written under a sequence wrapper, named by the element it holds.
    Sequenced(String),
    /// The key's own type is one serde writes as neither a string nor anything it will stringify.
    Unwritable {
        /// The key as it was written, by its own name where it has one and by its shape where it
        /// has none.
        key_name: String,
        /// What serde writes for it instead, as the message names it.
        written_as: &'static str,
    },
}

/// What a newtype variant's inner value is when the tag is written beside it rather than around
/// it.
#[cfg(feature = "serde")]
enum TaggedContent {
    /// A named type: what it writes are members, and they are the ones that join the tag.
    Flattened,
    /// serde refuses to write it beside the tag, and names the shape the way this names it.
    Refused(&'static str),
    /// serde writes it, but its members are not a set the expansion can name, so no schema closed
    /// around the tag can admit them.
    Unnameable(&'static str),
}

/// One `default_types(IdType = String)` entry as written: the parameter it names, and the type
/// declared for that parameter.
struct DefaultTypeEntry {
    name: syn::Ident,
    ty: syn::Type,
}

#[derive(Default, Clone)]
struct ModelSchemaArgs {
    /// The parser's refusal of the attribute's arguments — one it does not read, or a value it
    /// cannot read — spanned on the tokens that earned it.
    arg_rejection: Option<syn::Error>,
    /// The default type declared for each of the item's own type parameters, in the order they
    /// were written: `default_types(IdType = String, DateType = f64)`. Read by JSON-schema
    /// generation, which has no type parameters of its own and so has to build its document from
    /// one concrete filling.
    default_types: Vec<(syn::Ident, syn::Type)>,
    max_length: Option<usize>,
    min_length: Option<usize>,
    name_override: Option<String>,
    /// Opt-out of the branded newtype `Display` impl (and its inner-type assertion) for brands
    /// whose inner type is a container rather than a scalar.
    no_display: bool,
    /// `pattern` in the spelling every surface reads the same way, or as it was written when it
    /// earned a `pattern_rejection`.
    pattern: Option<String>,
    /// What keeps `pattern` off the surfaces it was written for -- a regex the `regex` crate
    /// cannot parse, a construct a JavaScript regex literal cannot carry, or a shape that admits
    /// every value and so says nothing on any of them -- spanned on the literal it was written as.
    /// Only the brand path splices the argument, and only a schema feature builds that path;
    /// without one, a type-level constraint never reaches a surface at all.
    #[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
    pattern_rejection: Option<syn::Error>,
}

/// One `#[serde(flatten)]` source as a surface writes it: the spelling of what its members are, and
/// whether the object writes them at all.
#[cfg(any(feature = "typescript", feature = "zod"))]
struct MergedOperand {
    absence: SourceAbsence,
    /// What the source contributes to each branch when it names a choice the registry recorded the
    /// branches of — one operand per branch — and empty for every other source, which contributes
    /// `spelling`. Zod only: TypeScript distributes an intersection over a union on its own, so
    /// there the choice is spelled as the one operand it is.
    #[cfg(feature = "zod")]
    branches: Vec<String>,
    spelling: String,
}

/// Which absence a merged source offers beside its members, and none where it always writes them.
#[cfg(any(feature = "typescript", feature = "zod"))]
#[derive(Clone, Copy)]
enum SourceAbsence {
    /// The field's own `Option` says the source writes all of its members or none of them.
    Field,
    /// The source writes its members for every value it holds.
    Never,
    /// The item the source names publishes the absence beside its value, one name away.
    Published,
}

#[cfg(any(feature = "typescript", feature = "zod"))]
impl SourceAbsence {
    /// Which of the two the source offers, for the merge that only has to count the key sets.
    #[cfg(feature = "zod")]
    const fn offered(self) -> bool {
        !matches!(self, Self::Never)
    }

    /// What a flattened source offers, read off the two spellings that reach the same absence: the
    /// field's own `Option`, and the leaves the name it holds recorded. The `Option` is asked first
    /// because it is the outer of the two — a source written `Option<Name>` writes nothing for its
    /// own `None` whatever the name goes on to publish, and the spelling the merge holds is the one
    /// inside the `Option`.
    fn written(fld: &FieldDef) -> Self {
        if fld.is_optional() {
            Self::Field
        } else if flattened_name_offers_absence(fld) {
            Self::Published
        } else {
            Self::Never
        }
    }
}

/// Both answers the registry holds about what a `#[model_schema()]` item publishes, built together
/// from one reading of the declaration so the two cannot come apart.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
struct Surface {
    shape: PublishedShape,
    #[cfg(all(feature = "serde", any(feature = "zod", feature = "typescript")))]
    wire: RecordedWire,
}

/// What a [`Surface`] carries for the merge: the leaves a written type published, or the one
/// keyword a surface read off no written type names.
///
/// The two are one recording — [`Surface::record`] writes either as leaves — and are held apart
/// only so a surface named outright can be built without one, which is what lets the four fixed
/// surfaces be `const`.
#[cfg(all(feature = "serde", any(feature = "zod", feature = "typescript")))]
enum RecordedWire {
    Leaves(Vec<WireLeaf>),
    Named(Option<&'static str>),
}

/// What a tuple struct publishes, which is not always the tuple of its declared slots.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
enum TupleStructShape {
    /// The fixed-arity array serde writes for every other declared arity, holding the slots the
    /// wire carries.
    Array(Vec<FieldDef>),
    /// The bare value serde writes for a struct declaring exactly one slot.
    BareValue(Box<FieldDef>),
}

/// What one untagged variant's member walk produces: the members its surfaces are rendered from,
/// the bindings its constrained members are checked under, and the three enum-wide lists the walk
/// adds to, which the caller joins onto its own in declaration order.
#[cfg(feature = "serde")]
struct UntaggedVariantMembers {
    bound: Vec<(proc_macro2::Ident, proc_macro2::Ident)>,
    checks: Vec<proc_macro2::TokenStream>,
    deferred_attrs: Vec<Vec<syn::Attribute>>,
    field_defs: Vec<FieldDef>,
    guard_errors: Vec<proc_macro2::TokenStream>,
    validation_fns: Vec<proc_macro2::TokenStream>,
}

/// What the item around a field says about it: everything [`process_field`] needs that is not the
/// field itself, bundled so the walk hands one context down instead of six loose parameters.
struct FieldContext<'ctx> {
    /// Whether the container carries `#[serde(default)]`, which supplies a value for every field
    /// under it whose key the payload leaves out.
    container_defaulted: bool,
    rename_all: Option<&'ctx str>,
    schema_module_name: Option<&'ctx str>,
    type_name: &'ctx str,
    type_parameters: &'ctx [String],
    /// The variant this field belongs to, or `None` for a struct's own field.
    variant_ident: Option<&'ctx str>,
}

/// Mutable output buffers shared by the discriminated-enum variant writers. Bundled so each writer
/// takes a single always-mutated `&mut`, keeping its conditionally-written fields (Zod schema, JSON
/// fields) from tripping `needless_pass_by_ref_mut` under feature subsets.
struct VariantParts {
    json_fields: Vec<proc_macro2::TokenStream>,
    optional_fields: Vec<String>,
    schema_code: String,
    type_code: String,
}

/// The two spellings [`zod_default_block`] has on hand for a constrained bare-parameter brand's
/// check chain — see [`branded_zod_string_checks`] and [`branded_zod_base_checks`] for what each
/// renders and why both exist.
#[cfg(feature = "zod")]
struct BrandedDefaultChecks {
    /// Composed inside a deferred argument's `z.lazy(...)` thunk, onto whatever the thunk
    /// resolves to — a sibling's `$Schema` or the fold's `$SchemaDefault` — neither of which is
    /// guaranteed to expose `.min`/`.max`, only the base `.check(...)` every schema carries.
    base: String,
    /// Chained onto an eager argument — `z.string()` and the like — which the checks' own
    /// `ZodString` methods always apply to directly.
    chained: String,
}

/// What [`zod_default_block`] and [`zod_factory_block`] need beyond the item's own name and
/// parameter list, bundled into one reference because the two already carry as many positional
/// parameters as `clippy::too_many_arguments` admits.
#[cfg(feature = "zod")]
struct ZodDefaultInputs<'defaults> {
    #[cfg(feature = "typescript")]
    brand: Option<BrandedAnnotation<'defaults>>,
    constrained: Option<(&'defaults str, &'defaults BrandedDefaultChecks)>,
    default_types: &'defaults [(syn::Ident, syn::Type)],
}

/// What a branded newtype's `$SchemaDefault` is annotated with, read off the shape of the
/// brand's own inner rather than off any one parameter's filling — see
/// [`branded_default_annotation`]. Only [`branded_default_annotation`] reads it, and that
/// function is itself `typescript`-gated (the annotation it writes is a TypeScript-surface
/// concern), so the enum carries the same gate rather than existing unread in a zod-without-
/// typescript build.
#[cfg(all(feature = "zod", feature = "typescript"))]
enum BrandedAnnotation<'defaults> {
    /// The inner is exactly one bare type parameter (`pub struct Name<T>(pub T)`) — the erased
    /// inner itself carries no concrete class to name, so the annotation is read off that one
    /// parameter's own declared default instead.
    BareParameter(&'defaults str),
    /// Every other inner shape — a tuple, an array, a concrete sibling — annotated with the bare
    /// class [`branded_zod_type_name`] already reads off it for the non-generic const, widened
    /// the same way regardless of what fills the brand's parameters.
    Widened(String),
}

/// What [`default_zod_rendering`] found: a self-contained expression left eager, or a name
/// [`deferred_zod_operand`] still has to wrap — kept apart from the final string so
/// [`zod_default_block`] can tell which one it has before deciding where a constrained brand's
/// checks belong: chained directly onto an eager expression, or composed inside the thunk of a
/// deferred one, whose own annotation is not guaranteed to carry `ZodString`'s chain methods.
#[cfg(feature = "zod")]
enum DefaultZodRendering {
    Deferred(String),
    Eager(String),
}

/// The parts [`assemble_schema_output`] joins, bundled into a struct because the standalone
/// default-only `validate()` impl (see [`place_validate_method`]) would otherwise be an eighth
/// positional argument past `clippy::too_many_arguments`' cap.
#[cfg(any(feature = "zod", feature = "typescript", feature = "jsonschema"))]
struct SchemaOutputParts<'parts, T> {
    default_types: &'parts [(syn::Ident, syn::Type)],
    delegate_impl_items: &'parts [proc_macro2::TokenStream],
    generics: &'parts syn::Generics,
    item: &'parts T,
    module_ident: &'parts Ident,
    name: &'parts syn::Ident,
    schema_impl_items: &'parts [proc_macro2::TokenStream],
    /// The type's raw `validate()`, split into its default-only `impl` by
    /// [`assemble_schema_output`] itself — see [`place_validate_method`].
    validate_method: &'parts Option<proc_macro2::TokenStream>,
    validation_fns: &'parts [proc_macro2::TokenStream],
}

#[cfg(feature = "zod")]
impl DefaultZodRendering {
    /// The plain argument an unconstrained default composes: deferred renderings gain their
    /// `z.lazy(...)` wrapper here, at the one place both variants converge back to a single string.
    fn into_argument(self) -> String {
        match self {
            Self::Deferred(schema) => deferred_zod_operand(&schema),
            Self::Eager(schema) => schema,
        }
    }
}

#[cfg(feature = "jsonschema")]
impl MapMemberItem {
    /// The item wrapped for its slot, kept in the form it arrived in.
    fn into_member_schema(self, value: &FieldDef) -> proc_macro2::TokenStream {
        match self {
            Self::Fragment(fragment) => map_member_slot_schema(value, &fragment),
            Self::Value(item_value) => map_member_slot_value(value, item_value),
        }
    }

    /// The item wrapped for its slot as a standalone `serde_json::Value`.
    fn into_member_value(self, value: &FieldDef) -> proc_macro2::TokenStream {
        map_member_slot_value(value, self.into_value())
    }

    /// The item as a standalone `serde_json::Value`, with no slot wrap applied — a fragment lifted
    /// into the `serde_json::json!` a value form already is.
    fn into_value(self) -> proc_macro2::TokenStream {
        match self {
            Self::Fragment(fragment) => quote! { serde_json::json!(#fragment) },
            Self::Value(item_value) => item_value,
        }
    }
}

impl ModelSchemaArgs {
    const fn has_string_constraints(&self) -> bool {
        self.pattern.is_some() || self.min_length.is_some() || self.max_length.is_some()
    }
}

#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
impl Surface {
    /// The fixed-arity JSON array a tuple struct of every arity but one writes, which takes no
    /// string check and which no object can be merged with.
    const fn array() -> Self {
        Self::named(Some("container"), Some("array"))
    }

    /// The bare string a plain unit enum writes its member name as.
    const fn enumerated() -> Self {
        Self::named(Some("enumerated"), Some("string"))
    }

    /// The union an externally tagged enum writes, as the leaves a merge descending it reaches.
    #[cfg(all(feature = "serde", any(feature = "zod", feature = "typescript")))]
    fn externally_tagged(variants: &Punctuated<syn::Variant, Token![,]>) -> Self {
        Self {
            shape: Self::union().shape,
            wire: RecordedWire::Leaves(external_variant_wire_leaves(variants)),
        }
    }

    /// The same union where no merge reads its leaves. `serde` beside a surface that merges is the
    /// pair that flattens one over the other; without both, nothing asks what the variants write and
    /// the union is the union every other enum shape registers.
    #[cfg(all(feature = "serde", not(any(feature = "zod", feature = "typescript"))))]
    const fn externally_tagged(_variants: &Punctuated<syn::Variant, Token![,]>) -> Self {
        Self::union()
    }

    /// A surface neither answer is read off a written type for.
    const fn named(shape: Option<&'static str>, wire: Option<&'static str>) -> Self {
        #[cfg(not(all(feature = "serde", any(feature = "zod", feature = "typescript"))))]
        let _: Option<&'static str> = wire;
        Self {
            shape: PublishedShape::Flat(shape),
            #[cfg(all(feature = "serde", any(feature = "zod", feature = "typescript")))]
            wire: RecordedWire::Named(wire),
        }
    }

    /// The object a struct of named fields writes, which a merge joins its own keys to.
    const fn object() -> Self {
        Self::named(Some("object"), None)
    }

    /// Puts both answers on the entry the name has just registered — taken by value, because a
    /// surface is built for one registration and spent on it.
    fn record(self, rust_ident: &str) {
        record_value_shape(rust_ident, self.shape);
        #[cfg(all(feature = "serde", any(feature = "zod", feature = "typescript")))]
        match self.wire {
            RecordedWire::Leaves(leaves) => record_wire_leaves(rust_ident, &leaves),
            RecordedWire::Named(non_object) => record_wire_leaves(
                rust_ident,
                &[WireLeaf {
                    branch: Vec::new(),
                    non_object,
                }],
            ),
        }
    }

    /// The union an enum writes, whose own members the merge multiplies over where it holds them.
    const fn union() -> Self {
        Self::named(Some("union"), None)
    }

    /// The surface a written type publishes, both answers read off the one def.
    ///
    /// `parameters` is the item's own type-parameter list, in the order it declares them: a target
    /// that *is* one of them publishes a family rather than a shape, and the position is what the
    /// shape answer records for it — see [`PublishedShape`].
    fn written(written: &FieldDef, parameters: &[String]) -> Self {
        Self {
            shape: published_value_shape(written, parameters),
            #[cfg(all(feature = "serde", any(feature = "zod", feature = "typescript")))]
            wire: RecordedWire::Leaves(published_wire_leaves(written)),
        }
    }
}

impl Parse for DefaultTypeEntry {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let name = input.parse()?;
        input.parse::<Token![=]>()?;
        let ty = input.parse()?;
        Ok(Self { name, ty })
    }
}

/// Parses the arguments of a `model_schema` attribute.
///
/// What the parser cannot read is recorded as [`ModelSchemaArgs::arg_rejection`] rather than
/// dropped: these arguments are read here and nowhere else, so one that stops at this parser
/// reaches no emitter, and the item is expanded as though it had been left off — a misspelled
/// `name` yields the unrenamed schema on every surface.
fn parse_model_schema_args(args: proc_macro2::TokenStream) -> ModelSchemaArgs {
    let mut result = ModelSchemaArgs::default();

    if args.is_empty() {
        return result;
    }

    let parser = Punctuated::<Meta, Token![,]>::parse_terminated;
    let read = parser.parse2(args).and_then(|parsed| {
        parsed
            .iter()
            .try_for_each(|meta| apply_arg(&mut result, meta))
    });
    if let Err(rejection) = read {
        result.arg_rejection = Some(rejection);
    }

    result
}

/// Applies one `model_schema` argument to `result`.
///
/// The name is read before the shape, so an argument this parser knows written the wrong way is
/// answered with what it takes rather than reported as unknown.
fn apply_arg(result: &mut ModelSchemaArgs, meta: &Meta) -> syn::Result<()> {
    let path = meta.path();
    if path.is_ident("name") {
        result.name_override = Some(name_arg_value(str_arg(meta, "name")?)?);
    } else if path.is_ident("pattern") {
        record_pattern(result, str_arg(meta, "pattern")?);
    } else if path.is_ident("minLength") {
        result.min_length = Some(length_arg(meta, "minLength")?);
    } else if path.is_ident("maxLength") {
        result.max_length = Some(length_arg(meta, "maxLength")?);
    } else if path.is_ident("no_display") {
        result.no_display = flag_arg(meta, "no_display")?;
    } else if path.is_ident("default_types") {
        result.default_types = default_types_arg(meta)?;
    } else {
        return Err(unknown_arg_rejection(meta));
    }
    Ok(())
}

/// The literal an argument's value was written as.
///
/// Every valued `model_schema` argument reaches a surface as a literal — a name, a regex, a bound
/// — so a value written as anything else is one no argument can carry.
fn arg_literal<'meta>(meta: &'meta Meta, name: &str, takes: &str) -> syn::Result<&'meta syn::Lit> {
    let Meta::NameValue(name_value) = meta else {
        return Err(arg_rejection(meta, name, takes));
    };
    if let syn::Expr::Lit(expr_lit) = &name_value.value {
        Ok(&expr_lit.lit)
    } else {
        Err(arg_rejection(&name_value.value, name, takes))
    }
}

/// The string literal a `name = "…"` argument was written as.
fn str_arg<'meta>(meta: &'meta Meta, name: &str) -> syn::Result<&'meta syn::LitStr> {
    let takes = format!("a string literal, written `{name} = \"…\"`");
    let lit = arg_literal(meta, name, &takes)?;
    if let syn::Lit::Str(lit_str) = lit {
        Ok(lit_str)
    } else {
        Err(arg_rejection(lit, name, &takes))
    }
}

/// The name a `name = "…"` argument was written as, once it is one a name can be spelled from.
///
/// The value is the type's name on every surface and, snake-cased, the Rust module its schema is
/// published from. `Ident::new` builds that module, and it panics on a string that is not an
/// identifier — a panic reports no span and names no argument, so the check is here, where the
/// literal the author wrote is still in hand.
fn name_arg_value(lit: &syn::LitStr) -> syn::Result<String> {
    let value = lit.value();
    let mut chars = value.chars();
    let opens = chars
        .next()
        .is_some_and(|ch| ch.is_ascii_alphabetic() || ch == '_');
    if opens && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_') {
        return Ok(value);
    }
    Err(arg_rejection(
        lit,
        "name",
        "a string literal spelling an identifier: ASCII letters, digits and underscores, not \
         opening with a digit. The value names the type on every surface and, snake-cased, the \
         Rust module its schema is published from, so one an identifier cannot be spelled from is \
         a name no surface can carry",
    ))
}

/// The length a `minLength = N` argument was written as.
///
/// The bound is compared against a `usize` in the generated validator, so one the target type
/// cannot hold is a bound no surface can enforce.
fn length_arg(meta: &Meta, name: &str) -> syn::Result<usize> {
    let takes = format!("an integer literal `usize` can hold, written `{name} = 3`");
    let lit = arg_literal(meta, name, &takes)?;
    if let syn::Lit::Int(lit_int) = lit {
        lit_int.base10_parse()
    } else {
        Err(arg_rejection(lit, name, &takes))
    }
}

/// The pairs a `default_types(…)` argument was written as, in that order.
fn default_types_arg(meta: &Meta) -> syn::Result<Vec<(syn::Ident, syn::Type)>> {
    let takes = "a list of `Parameter = Type` pairs, written `default_types(IdType = String, \
                 DateType = f64)`";
    let Meta::List(list) = meta else {
        return Err(arg_rejection(meta, "default_types", takes));
    };
    let entries =
        list.parse_args_with(Punctuated::<DefaultTypeEntry, Token![,]>::parse_terminated)?;
    if entries.is_empty() {
        return Err(arg_rejection(list, "default_types", takes));
    }
    let mut read: Vec<(syn::Ident, syn::Type)> = Vec::with_capacity(entries.len());
    for entry in entries {
        if let Some((_, declared)) = read.iter().find(|(name, _)| *name == entry.name) {
            return Err(repeated_entry_rejection(&entry.name, declared, &entry.ty));
        }
        read.push((entry.name, entry.ty));
    }
    Ok(read)
}

/// The refusal a second entry for one parameter earns, spanned on the name as the repeat spelled
/// it — the first entry declares what its author meant, and it is the second that leaves the
/// declaration with two answers.
///
/// Both fillings are named: a reader told only that the parameter repeats still has to go back to
/// the list to see which one was dropped.
fn repeated_entry_rejection(
    name: &syn::Ident,
    declared: &syn::Type,
    repeated: &syn::Type,
) -> syn::Error {
    let first = quote!(#declared);
    let second = quote!(#repeated);
    syn::Error::new(
        name.span(),
        format!(
            "`model_schema` argument `default_types` declares `{name}` twice, first as \
             `{first}` and then as `{second}`. A parameter is described from one filling: the \
             JSON-schema document is generated from it, so a second entry would leave which of the \
             two that document describes to whichever way the list happens to be read, and the \
             other silently dropped. Declare `{name}` once, with the type its document should be \
             generated from."
        ),
    )
}

/// Whether a flag argument is set: it stands alone, or takes the boolean literal that says which
/// way to set it.
fn flag_arg(meta: &Meta, name: &str) -> syn::Result<bool> {
    if matches!(*meta, Meta::Path(_)) {
        return Ok(true);
    }
    let takes =
        format!("no value at all, or a boolean literal, written `{name}` or `{name} = false`");
    let lit = arg_literal(meta, name, &takes)?;
    if let syn::Lit::Bool(lit_bool) = lit {
        Ok(lit_bool.value())
    } else {
        Err(arg_rejection(lit, name, &takes))
    }
}

/// Names what an argument takes, spanned on what was written instead.
fn arg_rejection(tokens: impl quote::ToTokens, name: &str, takes: &str) -> syn::Error {
    syn::Error::new_spanned(
        tokens,
        format!("`model_schema` argument `{name}` takes {takes}"),
    )
}

/// Rejects an argument the parser does not read, spanned on the name as written.
fn unknown_arg_rejection(meta: &Meta) -> syn::Error {
    let path = meta.path();
    syn::Error::new_spanned(
        path,
        format!(
            "unknown `model_schema` argument `{}`. This attribute is this crate's own, so an \
             argument it does not read reaches no emitter: the type would be expanded as though \
             the argument had been left off — a misspelled `name` yields the unrenamed schema on \
             every surface. Valid arguments: {}",
            quote!(#path),
            KNOWN_ARGS.join(", ")
        ),
    )
}

/// Records a `pattern` argument, in the spelling the brand path can splice into every surface, or
/// as written alongside what keeps it off them.
///
/// A refused pattern is recorded as written: the guards that answer for what a `pattern` may sit on
/// read that one was given, not what it says.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn record_pattern(result: &mut ModelSchemaArgs, lit_str: &syn::LitStr) {
    match portable_pattern(lit_str).and_then(|portable| constraining_pattern(lit_str, portable)) {
        Ok(pattern) => result.pattern = Some(pattern),
        Err(rejection) => {
            result.pattern_rejection = Some(rejection);
            result.pattern = Some(lit_str.value());
        }
    }
}

/// Without a schema feature no surface reads the argument, so it is kept as written and never
/// judged.
#[cfg(not(any(feature = "typescript", feature = "zod", feature = "jsonschema")))]
fn record_pattern(result: &mut ModelSchemaArgs, lit_str: &syn::LitStr) {
    result.pattern = Some(lit_str.value());
}

/// Executes the `model_schema` macro processing to generate TypeScript and Zod schema definitions.
///
/// This function is the main entry point for the `model_schema` macro and handles both struct and enum types.
pub fn exec_model_schema(args: TokenStream, input: TokenStream) -> TokenStream {
    let parsed_args = parse_model_schema_args(args.into());
    let item = parse_macro_input!(input as Item);
    // An argument the parser refused describes a surface no expansion below can honour, so the
    // item is refused before it is dispatched to its shape.
    if let Some(rejection) = parsed_args.arg_rejection.as_ref()
        && let Some(output) = guard_failure_output(
            &item,
            item_schema_ident(&item),
            &[attr_guard_error(rejection, &item_label(&item))],
        )
    {
        return output;
    }
    // A `default_types` declaration is read against the item's own parameters, which the parser
    // never sees, so both directions are answered here — ahead of every shape, and of the branded
    // split inside the struct path.
    if let Some(output) = guard_failure_output(
        &item,
        item_schema_ident(&item),
        &default_types_guard_errors(&item, &parsed_args),
    ) {
        return output;
    }
    // A doc example is compiled at one instantiation, and a const parameter takes no filling from
    // the convention that names one, so an item writing both is refused here — ahead of every
    // shape, and of the branded split inside the struct path.
    if let Some(output) = guard_failure_output(
        &item,
        item_schema_ident(&item),
        &const_parameter_example_errors(&item),
    ) {
        return output;
    }
    // A const handed to a written type as an argument stands where a type belongs, and no surface
    // that renders an argument list can spell it — refused here, beside the example refusal, and
    // ahead of every shape.
    if let Some(output) = guard_failure_output(
        &item,
        item_schema_ident(&item),
        &const_parameter_argument_errors(&item),
    ) {
        return output;
    }
    // Which Zod binding this item publishes turns on whether it declares type parameters, and a
    // field written at the item's own name reads that answer back the way any other reference
    // does. Recorded here, ahead of every shape, because a self-reference is rendered while the
    // item's own expansion is still running and would otherwise read an answer nobody had given.
    record_own_zod_binding(&item);
    // Whether a filling satisfies the bounds its parameter declares is a question about trait
    // impls, which a proc macro cannot answer — so it is asked here, of the compiler, and read off
    // the item before the shapes take it.
    let filling_bound_checks = default_types_bound_checks(&item, &parsed_args);
    // Held across the dispatch that consumes the item: a constrained brand written above this
    // declaration left its consult for whichever expansion registers the name, and this is that
    // expansion.
    let registering = item_schema_ident(&item).cloned();
    let expanded = if let Item::Struct(item_struct) = item {
        process_struct(item_struct, &parsed_args)
    } else if let Item::Enum(item_enum) = item {
        process_enum(item_enum, &parsed_args)
    } else if let Item::Type(item_type) = item {
        process_type_alias(item_type, &parsed_args)
    } else {
        syn::Error::new_spanned(item, "Unsupported target for model_schema")
            .to_compile_error()
            .into()
    };
    let answered = with_prefixed_tokens(expanded, &deferred_shape_refusals(registering.as_ref()));
    with_prefixed_tokens(answered, &filling_bound_checks)
}

/// Classifies what an alias resolves to, for the registry.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn alias_target_kind(alias_field_def: &FieldDef) -> AliasKind {
    match map_key_path(alias_field_def) {
        MapKeyPath::Open => AliasKind::StringWire,
        MapKeyPath::Unnarrowed => AliasKind::Stringified,
        MapKeyPath::Refused(_) => AliasKind::NoEnumMembers,
        MapKeyPath::Enumerated(target_name) => {
            registered_key_kind(target_name).unwrap_or(AliasKind::Unknown)
        }
    }
}

/// The `compile_error!` tokens an alias whose target reaches a map key with no `enum_members()`
/// earns, or `None` when it reaches none. Spanned on the target, which is where the key was
/// written.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn alias_map_key_guard_error(
    alias: &ItemType,
    export_name: &str,
    alias_field_def: &FieldDef,
) -> Option<proc_macro2::TokenStream> {
    let rejection = map_key_rejection(alias_field_def)?;
    let message = map_key_rejection_message(&format!("type alias `{export_name}`"), &rejection);
    Some(syn::Error::new_spanned(&alias.ty, message).to_compile_error())
}

/// The name of a flattened value's type when the registry proves that type is a plain enum.
#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
fn flattened_plain_enum(inner: &FieldDef) -> Option<&str> {
    let FieldDefType::SiblingType(name, generic_args) = &inner.field_type else {
        return None;
    };
    if !generic_args.is_empty() || inner.is_array() {
        return None;
    }
    (registered_key_kind(name) == Some(AliasKind::EnumMembers)).then_some(name.as_str())
}

/// The alias schema module is referenced by all three schema features — `typescript` and `zod`
/// through the alias's registered export name, `jsonschema` through a Rust path to
/// `#module_ident::Schema::json_schema()`. So the module and its `register_alias_info` call are
/// driven by the union: gating them on `typescript` alone left the jsonschema references
/// dangling. The module's *contents* are chosen per feature while building the tokens.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn process_type_alias(item_type: ItemType, args: &ModelSchemaArgs) -> TokenStream {
    let mut alias = item_type;
    alias
        .attrs
        .retain(|attr| !attr.path().is_ident("model_schema"));

    let rust_ident = alias.ident.clone();
    let rust_ident_str = rust_ident.to_string();
    let export_name = compute_alias_export_name(&rust_ident_str, args.name_override.as_deref());
    let module_name = ident_schema_module_name(&rust_ident_str);
    let module_ident = Ident::new(&module_name, rust_ident.span());

    // Registered only once the target has been classified: the alias's own expansion is the only
    // place that still holds the aliased type's tokens.
    let alias_field_def = get_field_def(export_name.as_str(), &alias.ty, "");
    let kind = alias_target_kind(&alias_field_def);
    register_alias_info(&rust_ident_str, &export_name, &module_name, kind);
    // An alias's schema *is* its target's, so it publishes whatever the target publishes.
    Surface::written(&alias_field_def, &type_parameters_in_scope(&alias.generics))
        .record(&rust_ident_str);
    // An alias *is* the type it names, so a merge that flattens it multiplies over exactly the
    // members the target's own name would have given it.
    #[cfg(feature = "zod")]
    record_zod_union_members(&rust_ident_str, &alias_field_def.zod_union_members());

    // Registered above whatever the outcome, so a type naming a refused alias still resolves to the
    // export name the author wrote and the alias's own diagnostic stays the one they act on.
    if let Some(error) = alias_map_key_guard_error(&alias, &export_name, &alias_field_def)
        && let Some(output) = guard_failure_output(&alias, Some(&alias.ident), &[error])
    {
        return output;
    }

    let ts_method = generate_alias_ts_definition_method(&alias, &export_name, &alias_field_def);
    let json_schema_method =
        generate_alias_json_schema_method(&alias, &export_name, &alias_field_def, args);
    let zod_method = generate_alias_zod_method(
        &alias,
        &export_name,
        &rust_ident_str,
        &alias_field_def,
        &args.default_types,
    );

    let output = quote! {
        #alias

        pub mod #module_ident {
            use super::*;

            #[non_exhaustive]
            pub struct Schema;

            impl Schema {
                #ts_method
                #json_schema_method
                #zod_method
            }
        }
    };

    TokenStream::from(output)
}

#[cfg(not(any(feature = "typescript", feature = "zod", feature = "jsonschema")))]
fn process_type_alias(item_type: ItemType, _args: &ModelSchemaArgs) -> TokenStream {
    let mut alias = item_type;
    alias
        .attrs
        .retain(|attr| !attr.path().is_ident("model_schema"));
    TokenStream::from(quote! { #alias })
}

#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn has_serde_transparent(attrs: &[syn::Attribute]) -> bool {
    for attr in attrs {
        if attr.path().is_ident("serde") {
            let mut found = false;
            let _: syn::Result<()> = attr.parse_nested_meta(|nested| {
                if nested.path.is_ident("transparent") {
                    found = true;
                }
                Ok(())
            });
            if found {
                return true;
            }
        }
    }
    false
}

/// Builds the `JSDoc` comment body (lines prefixed with ` * `) that precedes an item's
/// `export type`, and the one each field and enum variant inside it is emitted under. The item
/// shapes that publish a zod `description` alongside spell both from the same lines in
/// `build_item_docs_and_description`; a member publishes no second surface.
///
/// The no-docs fallback names what is documented as it is exported, not as it is declared in Rust,
/// so a `JSDoc` header never contradicts the line under it.
///
/// A ` ```rust example ` block is dropped before its lines reach the body, the way
/// `item_plain_doc_lines` drops it: the block is Rust source, and nothing reads it as such once it
/// is sitting in a `TypeScript` comment. A consumer after the example reads the Rust docs. What is
/// left reaches the body as written — this is the one surface that publishes docs unflattened.
fn build_jsdoc_body(docs_vec: Option<&[String]>, fallback_name: &str) -> String {
    item_jsdoc_body(&item_lines_or_name(docs_vec, fallback_name, |doc_lines| {
        strip_examples_from_docs(doc_lines)
            .iter()
            .flat_map(|v| v.lines().map(ToOwned::to_owned).collect::<Vec<_>>())
            .collect()
    }))
}

/// The `schema_example()` an enum's type publishes, read off its docs. One seam for all five enum
/// shapes, which differ in how their variants are written and not in what the type beside them
/// exposes.
#[cfg(feature = "zod")]
fn enum_schema_example_method(
    docs_vec: Option<&[String]>,
    name: &syn::Ident,
    generics: &syn::Generics,
    args: &ModelSchemaArgs,
) -> Option<proc_macro2::TokenStream> {
    item_schema_example_method(
        docs_vec.and_then(extract_example_from_docs).as_ref(),
        name,
        generics,
        args,
    )
}

#[cfg(all(
    not(feature = "zod"),
    any(feature = "typescript", feature = "jsonschema")
))]
const fn enum_schema_example_method(
    _docs_vec: Option<&[String]>,
    _name: &syn::Ident,
    _generics: &syn::Generics,
    _args: &ModelSchemaArgs,
) -> Option<proc_macro2::TokenStream> {
    None
}

/// The type a doc example's value is annotated with: the item's own name, with every type parameter
/// it declares instantiated at the type `default_types` declares for it, or at `String` where it
/// declares none.
#[cfg(feature = "zod")]
fn schema_example_value_type(
    name: &syn::Ident,
    generic_params: &[String],
    default_types: &[(syn::Ident, syn::Type)],
) -> proc_macro2::TokenStream {
    if generic_params.is_empty() {
        return quote! { #name };
    }
    let args = generic_params.iter().map(|param| {
        default_types
            .iter()
            .find(|(declared, _)| declared == param.as_str())
            .map_or_else(|| quote! { String }, |(_, ty)| quote! { #ty })
    });
    quote! { #name<#(#args),*> }
}

/// The type-level `schema_example()` a declared item publishes: the value its ` ```rust example `
/// block builds, or `None` where there is nothing to build one from — no example written, or no
/// `zod`, which is the only surface that reads one.
///
/// The `None` answer is the seam's own rather than each caller's, so a struct, a tuple struct and
/// every enum shape reach it the same way instead of repeating the feature pair at the call site.
#[cfg(feature = "zod")]
fn item_schema_example_method(
    example_code: Option<&String>,
    name: &syn::Ident,
    generics: &syn::Generics,
    args: &ModelSchemaArgs,
) -> Option<proc_macro2::TokenStream> {
    let code = example_code?;
    let code_tokens: proc_macro2::TokenStream = code.parse().unwrap();
    let value_ty = schema_example_value_type(
        name,
        &type_parameters_in_scope(generics),
        &args.default_types,
    );
    Some(quote! {
        pub fn schema_example() -> serde_json::Value {
            let value: #value_ty = {
                #code_tokens
            };
            serde_json::to_value(&value).unwrap()
        }
    })
}

#[cfg(all(
    not(feature = "zod"),
    any(feature = "typescript", feature = "jsonschema")
))]
const fn item_schema_example_method(
    _example_code: Option<&String>,
    _name: &syn::Ident,
    _generics: &syn::Generics,
    _args: &ModelSchemaArgs,
) -> Option<proc_macro2::TokenStream> {
    None
}

/// Builds the delegating impl methods (on the type itself) that forward to its schema module.
#[cfg(any(feature = "zod", feature = "typescript", feature = "jsonschema"))]
fn build_struct_delegate_items(
    module_ident: &Ident,
    item_name: &str,
    rust_ident: &str,
    parameters: &[String],
    schema_example_method: Option<&proc_macro2::TokenStream>,
) -> Vec<proc_macro2::TokenStream> {
    // A `schema_example()` method is emitted iff an example was extracted.
    #[cfg(feature = "zod")]
    let has_example = schema_example_method.is_some();
    #[cfg(feature = "zod")]
    let reexport = zod_binding_reexport(rust_ident, item_name, parameters);
    #[cfg(not(feature = "zod"))]
    let _: &_ = &(item_name, rust_ident, parameters);
    #[cfg(not(feature = "zod"))]
    let _: Option<&proc_macro2::TokenStream> = schema_example_method;

    let mut items: Vec<proc_macro2::TokenStream> = Vec::new();

    #[cfg(feature = "jsonschema")]
    items.push(quote! {
        pub fn json_schema() -> serde_json::Value {
            #module_ident::Schema::json_schema()
        }
    });

    #[cfg(feature = "typescript")]
    items.push(quote! {
        pub fn ts_definition() -> String {
            #module_ident::Schema::ts_definition()
        }
    });

    #[cfg(feature = "zod")]
    items.push(if has_example {
        let injected = zod_example_injection(item_name, parameters);
        quote! {
            pub fn zod_schema() -> String {
                let base_schema = #module_ident::Schema::zod_schema();
                let defined = base_schema.strip_suffix(#reexport).unwrap_or(base_schema.as_str());
                let example_json = serde_json::to_string(&Self::schema_example()).unwrap();
                let mut result = #injected;
                result.push_str(#reexport);
                result
            }
        }
    } else {
        quote! {
            pub fn zod_schema() -> String {
                #module_ident::Schema::zod_schema()
            }
        }
    });

    #[cfg(feature = "zod")]
    items.extend(schema_example_method.cloned());
    items
}

/// Assembles the final macro output for a struct or enum: the item itself, its schema module
/// (with the per-field validation functions), the type's delegate impl, and its standalone
/// default-only `validate()` impl when it has one.
#[cfg(any(feature = "zod", feature = "typescript", feature = "jsonschema"))]
fn assemble_schema_output<T>(parts: &SchemaOutputParts<T>) -> TokenStream
where
    T: quote::ToTokens,
{
    let (impl_generics, type_generics, where_clause) = parts.generics.split_for_impl();
    let item = parts.item;
    let module_ident = parts.module_ident;
    let name = parts.name;
    let schema_impl_items = parts.schema_impl_items;
    let validation_fns = parts.validation_fns;
    let delegate_impl_items = parts.delegate_impl_items;
    let (validate_method, default_validate_impl) = place_validate_method(
        parts.validate_method.clone(),
        name,
        parts.generics,
        parts.default_types,
    );

    let output = quote! {
        #item

        pub mod #module_ident {
            use super::*;

            #[non_exhaustive]
            pub struct Schema;

            impl Schema {
                #(#schema_impl_items)*
            }

            #(#validation_fns)*
        }

        impl #impl_generics #name #type_generics #where_clause {
            #(#delegate_impl_items)*
            #validate_method
        }

        #default_validate_impl
    };

    log::trace!("{output}");

    TokenStream::from(output)
}

/// The type-level `validate()` a struct publishes: the aggregate of its per-field validators, or
/// `None` where there is nothing to run — no constrained field, or no `serde` to have generated the
/// validators from.
#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
fn struct_validate_method(
    validate_bodies: &[proc_macro2::TokenStream],
    module_ident: &Ident,
) -> Option<proc_macro2::TokenStream> {
    build_struct_validate_method(validate_bodies, module_ident)
}

#[cfg(all(
    not(feature = "serde"),
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
const fn struct_validate_method(
    _validate_bodies: &[proc_macro2::TokenStream],
    _module_ident: &Ident,
) -> Option<proc_macro2::TokenStream> {
    None
}

/// Builds the type-level `validate()` method that aggregates per-field validators, or `None` when
/// the struct has no constrained fields.
///
/// The generated `validate(&self) -> Result<(), Vec<String>>` calls every per-field
/// `validate_{field}_value` (the same validators serde's `deserialize_with` hooks use), so the
/// same rules apply during deserialization and when validating a programmatically built instance.
#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
fn build_struct_validate_method(
    validate_bodies: &[proc_macro2::TokenStream],
    module_ident: &Ident,
) -> Option<proc_macro2::TokenStream> {
    (!validate_bodies.is_empty()).then(|| {
        quote! {
            /// Validates all constrained fields and returns all validation errors.
            ///
            /// Returns `Ok(())` if all constraints pass, or `Err(Vec<String>)` with all errors.
            pub fn validate(&self) -> Result<(), Vec<String>> {
                use #module_ident::*;
                let mut errors: Vec<String> = Vec::new();
                #(#validate_bodies)*
                if errors.is_empty() { Ok(()) } else { Err(errors) }
            }
        }
    })
}

/// The pattern one variant is matched by in the enum-level `validate()`.
///
/// Only the constrained members are bound, each under the name its check reads; everything else the
/// variant holds is left unread, and a variant that binds every member it has needs no rest pattern
/// to say so.
fn variant_check_pattern(
    variant_ident: &proc_macro2::Ident,
    kind: &VariantKind,
    total_fields: usize,
    bound: &[(proc_macro2::Ident, proc_macro2::Ident)],
) -> proc_macro2::TokenStream {
    if bound.is_empty() {
        return match *kind {
            VariantKind::Unit => quote! { Self::#variant_ident },
            VariantKind::TupleSingle | VariantKind::TupleMultiple => {
                quote! { Self::#variant_ident(..) }
            }
            VariantKind::Named if total_fields == 0 => quote! { Self::#variant_ident {} },
            VariantKind::Named => quote! { Self::#variant_ident { .. } },
        };
    }
    let fields = bound.iter().map(|(field, _)| field);
    let bindings = bound.iter().map(|(_, binding)| binding);
    if bound.len() == total_fields {
        quote! { Self::#variant_ident { #(#fields: #bindings),* } }
    } else {
        quote! { Self::#variant_ident { #(#fields: #bindings,)* .. } }
    }
}

/// The match arms the enum-level `validate()` runs — empty when no variant carries a constrained
/// member, which is the parity a constraint-free struct has: it publishes no `validate()` either.
fn build_member_check_arms(
    per_variant: Vec<(proc_macro2::TokenStream, Vec<proc_macro2::TokenStream>)>,
) -> Vec<proc_macro2::TokenStream> {
    let mut arms: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut unchecked: Vec<proc_macro2::TokenStream> = Vec::new();
    for (pattern, checks) in per_variant {
        if checks.is_empty() {
            unchecked.push(pattern);
        } else {
            arms.push(quote! { #pattern => { #(#checks)* } });
        }
    }
    if arms.is_empty() {
        return Vec::new();
    }
    if !unchecked.is_empty() {
        arms.push(quote! { #(#unchecked)|* => {} });
    }
    arms
}

/// Builds the type-level `validate()` method for an enum, aggregating the checks of whichever
/// variant the value holds, or `None` when no variant carries a constrained member.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn build_enum_validate_method(
    arms: &[proc_macro2::TokenStream],
    module_ident: &Ident,
) -> Option<proc_macro2::TokenStream> {
    (!arms.is_empty()).then(|| {
        quote! {
            /// Validates all constrained fields and returns all validation errors.
            ///
            /// Returns `Ok(())` if all constraints pass, or `Err(Vec<String>)` with all errors.
            pub fn validate(&self) -> Result<(), Vec<String>> {
                use #module_ident::*;
                let mut errors: Vec<String> = Vec::new();
                match self {
                    #(#arms),*
                }
                if errors.is_empty() { Ok(()) } else { Err(errors) }
            }
        }
    })
}

/// Computes the TypeScript types and Zod schemas contributed by a struct's `#[serde(flatten)]`
/// fields, each beside the answer for whether the object writes those members at all (an empty
/// vector for either disabled output feature).
#[cfg(any(feature = "typescript", feature = "zod"))]
fn compute_flatten_outputs(
    flattened_fields: &[FieldDef],
) -> (Vec<MergedOperand>, Vec<MergedOperand>) {
    #[cfg(feature = "typescript")]
    let ts_types = flattened_fields
        .iter()
        .map(|fld| MergedOperand {
            absence: SourceAbsence::written(fld),
            #[cfg(feature = "zod")]
            branches: Vec::new(),
            spelling: flattened_ts_spelling(fld),
        })
        .collect();
    #[cfg(not(feature = "typescript"))]
    let ts_types = Vec::new();

    #[cfg(feature = "zod")]
    let zod_schemas = flattened_fields
        .iter()
        .map(|fld| MergedOperand {
            absence: SourceAbsence::written(fld),
            branches: flattened_zod_branches(fld),
            spelling: fld.zod_merged_schema(),
        })
        .collect();
    #[cfg(not(feature = "zod"))]
    let zod_schemas = Vec::new();

    (ts_types, zod_schemas)
}

/// Whether the registration a flattened source names offers its own absence, which is the second
/// key set the merge owes it.
#[cfg(all(feature = "serde", any(feature = "zod", feature = "typescript")))]
fn flattened_name_offers_absence(fld: &FieldDef) -> bool {
    if fld.is_array() {
        return false;
    }
    let FieldDefType::SiblingType(name, _) = &fld.field_type else {
        return false;
    };
    lookup_alias_info(name).is_some_and(|info| info.wire.iter().any(WireLeaf::is_published_absence))
}

/// No source offers one where nothing reads `#[serde(flatten)]`: without `serde` no field reaches
/// the merge to begin with, and the registry records no wire for a name to publish an absence in.
#[cfg(all(any(feature = "zod", feature = "typescript"), not(feature = "serde")))]
const fn flattened_name_offers_absence(_fld: &FieldDef) -> bool {
    false
}

/// The operands an object joins one per branch for a flattened source, and none for a source that
/// contributes one key set and is joined as the one operand it is.
#[cfg(all(feature = "serde", feature = "zod"))]
fn flattened_zod_branches(fld: &FieldDef) -> Vec<String> {
    let members = fld.zod_union_members();
    if !members.is_empty() {
        return members.into_iter().map(|member| member.spelling).collect();
    }
    fld.flatten_variants()
        .into_iter()
        .map(|variant| variant.zod)
        .collect()
}

/// No source names branches where nothing reads `#[serde(flatten)]`, for the reason no source offers
/// an absence there.
#[cfg(all(feature = "zod", not(feature = "serde")))]
fn flattened_zod_branches(fld: &FieldDef) -> Vec<String> {
    fld.zod_union_members()
        .into_iter()
        .map(|member| member.spelling)
        .collect()
}

/// What one flattened source is written as where the TypeScript merge names it: the parenthesised
/// union of the key sets the choice it names recorded for this position, and the name itself
/// everywhere else.
#[cfg(all(feature = "serde", feature = "typescript"))]
fn flattened_ts_spelling(fld: &FieldDef) -> String {
    let named = fld.typescript_merged_typename();
    let variants: Vec<String> = fld
        .flatten_variants()
        .into_iter()
        .map(|variant| variant.typescript)
        .collect();
    if !variants.is_empty() {
        return if variants.len() > 1 || named_wire_leaves(fld).is_some() {
            format!("({})", variants.join(" | "))
        } else {
            named
        };
    }
    let members = fld.ts_union_members();
    if members.is_empty() {
        named
    } else {
        format!("({})", members.join(" | "))
    }
}

/// No name proves a branch where nothing reads `#[serde(flatten)]`: the registry records no wire for
/// one to prove it in, and no field reaches the merge to begin with.
#[cfg(all(feature = "typescript", not(feature = "serde")))]
fn flattened_ts_spelling(fld: &FieldDef) -> String {
    fld.typescript_merged_typename()
}

/// The schema methods a struct's module publishes — the JSON-schema document, the TypeScript
/// definition, the Zod schema — each built only where its feature is on.
#[cfg(any(feature = "zod", feature = "typescript", feature = "jsonschema"))]
fn struct_schema_impl_items(
    field_defs: Vec<FieldDef>,
    flattened_fields: &[FieldDef],
    item_name: &str,
    rust_ident: &str,
    generics: &syn::Generics,
    args: &ModelSchemaArgs,
    docs: &str,
) -> Vec<proc_macro2::TokenStream> {
    #[cfg(not(feature = "typescript"))]
    let _: &str = docs;
    #[cfg(not(any(feature = "typescript", feature = "zod")))]
    let _: &str = rust_ident;
    #[cfg(not(feature = "jsonschema"))]
    let _: &_ = &args;
    #[cfg(feature = "typescript")]
    let ts_generics = ts_generic_params(generics);
    #[cfg(feature = "zod")]
    let type_parameters = type_parameters_in_scope(generics);
    // bodies: (type_code, schema_code, json_schema_fields, fields_empty)
    let bodies = render_struct_field_bodies(field_defs, Some(item_name));
    // flatten: (ts_types, zod_schemas)
    #[cfg(any(feature = "typescript", feature = "zod"))]
    let flatten = compute_flatten_outputs(flattened_fields);
    vec![
        #[cfg(feature = "jsonschema")]
        generate_json_schema_method(
            &bodies.2,
            &flatten_merged_sources(flattened_fields),
            item_name,
            &schema_parameters(generics, args),
        ),
        #[cfg(feature = "typescript")]
        generate_ts_definition_method(
            docs,
            item_name,
            rust_ident,
            &ts_generics,
            &bodies.0,
            bodies.3,
            &flatten.0,
        ),
        #[cfg(feature = "zod")]
        generate_zod_schema_method(
            item_name,
            rust_ident,
            &type_parameters,
            &args.default_types,
            &bodies.1,
            "",
            &flatten.1,
        ),
    ]
}

/// Renders the per-field TypeScript type code, Zod schema code, and JSON-schema fragments for a
/// struct's (non-flattened) fields. Returns the accumulated code and whether the field set is empty.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn render_struct_field_bodies(
    field_defs: Vec<FieldDef>,
    item_name_opt: Option<&str>,
) -> (String, String, Vec<proc_macro2::TokenStream>, bool) {
    let fields_empty = field_defs.is_empty();
    let mut type_code = String::new();
    let mut schema_code = String::new();
    #[cfg(feature = "jsonschema")]
    let mut json_schema_fields: Vec<proc_macro2::TokenStream> = Vec::new();
    #[cfg(not(feature = "jsonschema"))]
    let json_schema_fields: Vec<proc_macro2::TokenStream> = Vec::new();

    for fld in field_defs {
        schema_code.push_str(&write_field_type_and_schema(
            &mut type_code,
            &fld,
            item_name_opt,
        ));
        #[cfg(feature = "jsonschema")]
        json_schema_fields.push(build_field_schema(&fld));
    }

    (type_code, schema_code, json_schema_fields, fields_empty)
}

/// The schema module a refused item publishes in place of the one it has no description for.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn refused_item_schema_module(ident: &syn::Ident) -> proc_macro2::TokenStream {
    let module_ident = Ident::new(&ident_schema_module_name(&ident.to_string()), ident.span());
    let refusal = format!("`{ident}`: refused by `#[model_schema()]`, so it describes nothing");

    #[cfg(feature = "jsonschema")]
    let json_schema_methods = {
        let in_flight_type = in_flight_type();
        quote! {
            pub fn json_schema() -> serde_json::Value {
                panic!(#refusal)
            }

            pub fn json_schema_within(
                _in_flight: &mut #in_flight_type,
                _hoisted_defs: &mut serde_json::Map<String, serde_json::Value>,
            ) -> serde_json::Value {
                panic!(#refusal)
            }
        }
    };
    #[cfg(not(feature = "jsonschema"))]
    let json_schema_methods = quote! {};

    #[cfg(feature = "typescript")]
    let ts_definition_method = quote! {
        pub fn ts_definition() -> String {
            panic!(#refusal)
        }
    };
    #[cfg(not(feature = "typescript"))]
    let ts_definition_method = quote! {};

    #[cfg(feature = "zod")]
    let zod_schema_method = quote! {
        pub fn zod_schema() -> String {
            panic!(#refusal)
        }
    };
    #[cfg(not(feature = "zod"))]
    let zod_schema_method = quote! {};

    quote! {
        pub mod #module_ident {
            #[non_exhaustive]
            pub struct Schema;

            impl Schema {
                #json_schema_methods
                #ts_definition_method
                #zod_schema_method
            }
        }
    }
}

/// Nothing, in a build with no schema surface: no reference to a module is ever emitted there, so
/// a refused item leaves none dangling.
#[cfg(not(any(feature = "typescript", feature = "zod", feature = "jsonschema")))]
fn refused_item_schema_module(ident: &syn::Ident) -> proc_macro2::TokenStream {
    let _: &syn::Ident = ident;
    quote! {}
}

/// Emits the original item followed by the `compile_error!` tokens of every violated field guard,
/// or `None` when there are none.
fn guard_failure_output<ItemT>(
    item: &ItemT,
    ident: Option<&syn::Ident>,
    guard_errors: &[proc_macro2::TokenStream],
) -> Option<TokenStream>
where
    ItemT: quote::ToTokens,
{
    if guard_errors.is_empty() {
        return None;
    }
    let absorbing_module = ident.map(refused_item_schema_module);
    let output = quote! {
        #item
        #(#guard_errors)*
        #absorbing_module
    };
    log::trace!("{output}");
    Some(TokenStream::from(output))
}

/// Turns a `cfg_attr` rejection into `compile_error!` tokens naming the item it was found on,
/// keeping the attribute's span so the diagnostic still points at the offending line.
#[cfg(feature = "serde")]
fn cfg_attr_guard_error(rejection: &syn::Error, item: &str) -> proc_macro2::TokenStream {
    syn::Error::new(
        rejection.span(),
        format!("model_schema: {item}: {rejection}"),
    )
    .to_compile_error()
}

/// Turns a rejected `pattern` into `compile_error!` tokens naming what carries it, keeping the
/// literal's span so the diagnostic points at the pattern as written.
fn pattern_guard_error(rejection: &syn::Error, subject: &str) -> proc_macro2::TokenStream {
    syn::Error::new(
        rejection.span(),
        format!("model_schema: {subject}: {rejection}"),
    )
    .to_compile_error()
}

/// Turns this crate's own attribute parser's refusal — of a `model_schema` argument or of a
/// `model_schema_prop` key — into `compile_error!` tokens naming what carries it, keeping the
/// refusal's span so the diagnostic points at the argument, key or value as written.
fn attr_guard_error(rejection: &syn::Error, subject: &str) -> proc_macro2::TokenStream {
    syn::Error::new(
        rejection.span(),
        format!("model_schema: {subject}: {rejection}"),
    )
    .to_compile_error()
}

/// Names a field in a guard message; tuple slots have no ident to name.
fn field_label(raw_field_ident: &str) -> String {
    if raw_field_ident.is_empty() {
        "tuple field".to_owned()
    } else {
        format!("field `{raw_field_ident}`")
    }
}

/// Names the item a type-level guard message is about; a shape this macro does not expand has no
/// ident worth naming.
fn item_label(item: &Item) -> String {
    if let Item::Struct(item_struct) = item {
        format!("type `{}`", item_struct.ident)
    } else if let Item::Enum(item_enum) = item {
        format!("type `{}`", item_enum.ident)
    } else if let Item::Type(item_type) = item {
        format!("type `{}`", item_type.ident)
    } else {
        "item".to_owned()
    }
}

/// The Rust ident a refused item publishes its schema module under — one of the three shapes this
/// macro expands, which are the three a reference can name. Anything else names no module, so a
/// refusal there leaves nothing dangling.
const fn item_schema_ident(item: &Item) -> Option<&syn::Ident> {
    if let Item::Struct(item_struct) = item {
        Some(&item_struct.ident)
    } else if let Item::Enum(item_enum) = item {
        Some(&item_enum.ident)
    } else if let Item::Type(item_type) = item {
        Some(&item_type.ident)
    } else {
        None
    }
}

/// Records which of the two Zod bindings an item publishes, ahead of the shape it is dispatched to.
#[cfg(feature = "zod")]
fn record_own_zod_binding(item: &Item) {
    let Some(generics) = item_generics(item) else {
        return;
    };
    let Some(name) = item_schema_ident(item) else {
        return;
    };
    record_zod_factory(
        &name.to_string(),
        !type_parameters_in_scope(generics).is_empty(),
    );
}

/// Nothing, in a build that publishes no Zod binding at all.
#[cfg(not(feature = "zod"))]
const fn record_own_zod_binding(_item: &Item) {}

/// The parameters an item declares — the three shapes `model_schema` expands each bind their own;
/// anything else binds none this expansion can read.
const fn item_generics(item: &Item) -> Option<&syn::Generics> {
    if let Item::Struct(item_struct) = item {
        Some(&item_struct.generics)
    } else if let Item::Enum(item_enum) = item {
        Some(&item_enum.generics)
    } else if let Item::Type(item_type) = item {
        Some(&item_type.generics)
    } else {
        None
    }
}

/// The `compile_error!` tokens a `default_types` declaration earns against the item it was written
/// on, checked in both directions: an entry naming nothing the item declares, and — where a JSON
/// document is generated — a parameter the declaration left out.
fn default_types_guard_errors(
    item: &Item,
    args: &ModelSchemaArgs,
) -> Vec<proc_macro2::TokenStream> {
    let Some(generics) = item_generics(item) else {
        return Vec::new();
    };
    let declared: Vec<&syn::Ident> = generics
        .params
        .iter()
        .filter_map(|param| match param {
            syn::GenericParam::Type(type_param) => Some(&type_param.ident),
            syn::GenericParam::Const(_) | syn::GenericParam::Lifetime(_) => None,
        })
        .collect();

    #[cfg(feature = "jsonschema")]
    let mut rejections = entries_naming_no_parameter(args, &declared);
    #[cfg(not(feature = "jsonschema"))]
    let rejections = entries_naming_no_parameter(args, &declared);
    #[cfg(feature = "jsonschema")]
    rejections.extend(parameters_left_without_a_default(args, &declared));
    #[cfg(feature = "jsonschema")]
    rejections.extend(fillings_no_document_can_be_built_from(args));

    rejections
        .iter()
        .map(|rejection| attr_guard_error(rejection, &item_label(item)))
        .collect()
}

/// The refusal every `default_types` entry that names no type parameter of the item earns, spanned
/// on the entry as written.
fn entries_naming_no_parameter(
    args: &ModelSchemaArgs,
    declared: &[&syn::Ident],
) -> Vec<syn::Error> {
    args.default_types
        .iter()
        .filter(|(name, _)| !declared.contains(&name))
        .map(|(name, _)| syn::Error::new_spanned(name, undeclared_entry_message(name, declared)))
        .collect()
}

/// Why an entry naming nothing is refused, in the two spellings the item earns: one that declares
/// parameters names them back, and one that declares none says so.
fn undeclared_entry_message(name: &syn::Ident, declared: &[&syn::Ident]) -> String {
    if declared.is_empty() {
        return format!(
            "`default_types` entry `{name}` names a type parameter, but this item declares none. A \
             default type is the concrete filling a parameter is described from, so an item with \
             no type parameter has nothing for one to fill."
        );
    }
    let names = declared
        .iter()
        .map(|declared_name| format!("`{declared_name}`"))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "`default_types` entry `{name}` names no type parameter of this item. An entry declares \
         the filling for the parameter it names, so one that names none fills nothing while the \
         parameter it was meant for is left with no default at all. Type parameters of this item: \
         {names}."
    )
}

/// The refusal every type parameter the declaration left out earns, spanned on the parameter.
#[cfg(feature = "jsonschema")]
fn parameters_left_without_a_default(
    args: &ModelSchemaArgs,
    declared: &[&syn::Ident],
) -> Vec<syn::Error> {
    declared
        .iter()
        .filter(|name| {
            !args
                .default_types
                .iter()
                .any(|(written, _)| written == **name)
        })
        .map(|name| syn::Error::new_spanned(name, missing_default_message(name, declared)))
        .collect()
}

/// Why a parameter with no default is refused: what the default is for, what the feature has to do
/// with it, and the declaration to write for this item's own parameters.
#[cfg(feature = "jsonschema")]
fn missing_default_message(name: &syn::Ident, declared: &[&syn::Ident]) -> String {
    let sample = declared
        .iter()
        .map(|parameter| format!("{parameter} = String"))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "`#[model_schema]` requires a default type for every type parameter, and `{name}` has \
         none. The default type is what the JSON-schema document is generated from: JSON Schema \
         has no type parameters, so without a declared default the macro would have to guess — and \
         a wrong guess produces a document that silently rejects valid payloads. This is required \
         because the `jsonschema` feature is enabled. Declare one for every parameter of this item, \
         each with the type its document should be generated from: \
         `#[model_schema(default_types({sample}))]`."
    )
}

/// The refusal every `default_types` entry earns whose filling names a value no document can be
/// built from, spanned on the filling as written.
#[cfg(feature = "jsonschema")]
fn fillings_no_document_can_be_built_from(args: &ModelSchemaArgs) -> Vec<syn::Error> {
    args.default_types
        .iter()
        .filter_map(|(name, filling)| {
            let written = bare_type_name(filling)?;
            is_undescribable_primitive(&written).then(|| {
                syn::Error::new_spanned(filling, undescribable_filling_message(name, &written))
            })
        })
        .collect()
}

/// The name a type is written as when it is written as a bare name and nothing else — no
/// qualifier, no path, no arguments — or `None` for every other spelling.
///
/// Only that shape can be checked against a list of reserved names: `some::path::char` names
/// whatever that path resolves to, and a name carrying arguments is not a primitive at all.
#[cfg(feature = "jsonschema")]
fn bare_type_name(filling: &syn::Type) -> Option<String> {
    let syn::Type::Path(type_path) = filling else {
        return None;
    };
    if type_path.qself.is_some() {
        return None;
    }
    let [segment] = type_path.path.segments.iter().collect::<Vec<_>>()[..] else {
        return None;
    };
    matches!(segment.arguments, syn::PathArguments::None).then(|| segment.ident.to_string())
}

/// Why a filling the dispatch has no arm for is refused: what the filling is read for, what the
/// emission would otherwise name, what the feature has to do with it, and the way out.
#[cfg(feature = "jsonschema")]
fn undescribable_filling_message(name: &syn::Ident, written: &str) -> String {
    format!(
        "`default_types` entry `{name}` is filled at `{written}`, which `#[model_schema]` cannot \
         build a JSON-schema document from. The filling is what the parameter's document is \
         generated from, and it is rendered through the same dispatch a field's type is — a \
         dispatch that has no arm for `{written}` and so takes it for another `#[model_schema]` \
         item, emitting a call into a `{written}_schema` module that nothing publishes. This is \
         refused because the `jsonschema` feature is enabled, the document being the only thing \
         the filling is read for. Fill the parameter at a type the macro describes — a primitive it \
         maps, or a `#[model_schema]` item — or model `{written}` as a newtype over one that \
         carries what the wire holds."
    )
}

/// The compile-time checks a `default_types` declaration earns against the bounds its parameters
/// declare — one per bounded parameter, and one more carrying jointly whatever bound reads a
/// neighbour — emitted beside the expansion rather than in place of it.
fn default_types_bound_checks(
    item: &Item,
    args: &ModelSchemaArgs,
) -> Vec<proc_macro2::TokenStream> {
    if matches!(item, Item::Type(_)) {
        return Vec::new();
    }
    let Some(generics) = item_generics(item) else {
        return Vec::new();
    };
    let mut checks: Vec<proc_macro2::TokenStream> = args
        .default_types
        .iter()
        .filter_map(|(name, filling)| filling_bound_check(generics, name, filling))
        .collect();
    checks.extend(joint_bound_check(generics, args));
    checks
}

/// The check one filling earns against the bounds its parameter declares alone, or `None` where it
/// declares none such.
fn filling_bound_check(
    generics: &syn::Generics,
    name: &syn::Ident,
    filling: &syn::Type,
) -> Option<proc_macro2::TokenStream> {
    let bounds: Vec<&syn::TypeParamBound> = declared_bounds(generics, name)
        .into_iter()
        .filter(|bound| !mentions_another_parameter(bound, generics, name))
        .collect();
    if bounds.is_empty() {
        return None;
    }
    Some(quote! {
        const _: fn() = || {
            fn default_type_filling<#name: #(#bounds)+*>() {}
            default_type_filling::<#filling>();
        };
    })
}

/// The one check carrying every bound the per-filling pass left behind for reading a neighbour, or
/// `None` where it left none behind or where the item cannot be spelled jointly.
fn joint_bound_check(
    generics: &syn::Generics,
    args: &ModelSchemaArgs,
) -> Option<proc_macro2::TokenStream> {
    let consts: Vec<String> = generics
        .params
        .iter()
        .filter_map(|param| match param {
            syn::GenericParam::Const(const_param) => Some(const_param.ident.to_string()),
            syn::GenericParam::Type(_) | syn::GenericParam::Lifetime(_) => None,
        })
        .collect();
    let mut predicates: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut declared: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut fillings: Vec<&syn::Type> = Vec::new();
    for param in &generics.params {
        match param {
            syn::GenericParam::Lifetime(lifetime_param) => {
                declared.push(quote! { #lifetime_param });
            }
            syn::GenericParam::Const(_) => {}
            syn::GenericParam::Type(type_param) => {
                let name = &type_param.ident;
                let filling = args
                    .default_types
                    .iter()
                    .find(|(declared_name, _)| declared_name == name)
                    .map(|(_, filling)| filling)?;
                declared.push(quote! { #name });
                fillings.push(filling);
                let joint: Vec<&syn::TypeParamBound> = declared_bounds(generics, name)
                    .into_iter()
                    .filter(|bound| {
                        mentions_another_parameter(bound, generics, name)
                            && !reads_any_name(quote::ToTokens::to_token_stream(bound), &consts)
                    })
                    .collect();
                if !joint.is_empty() {
                    predicates.push(quote! { #name: #(#joint)+* });
                }
            }
        }
    }
    if predicates.is_empty() {
        return None;
    }
    Some(quote! {
        const _: fn() = || {
            fn default_type_fillings<#(#declared),*>()
            where
                #(#predicates),*
            {
            }
            default_type_fillings::<#(#fillings),*>();
        };
    })
}

/// Everything a parameter is bounded by, in both places a declaration can put it: beside the
/// parameter, and in the item's `where` clause.
fn declared_bounds<'generics>(
    generics: &'generics syn::Generics,
    name: &syn::Ident,
) -> Vec<&'generics syn::TypeParamBound> {
    let beside_the_parameter = generics.params.iter().filter_map(|param| match param {
        syn::GenericParam::Type(type_param) if type_param.ident == *name => {
            Some(&type_param.bounds)
        }
        syn::GenericParam::Type(_)
        | syn::GenericParam::Const(_)
        | syn::GenericParam::Lifetime(_) => None,
    });
    let in_the_where_clause = generics
        .where_clause
        .iter()
        .flat_map(|where_clause| &where_clause.predicates)
        .filter_map(|predicate| {
            let syn::WherePredicate::Type(predicate_type) = predicate else {
                return None;
            };
            (named_parameter(&predicate_type.bounded_ty) == Some(name))
                .then_some(&predicate_type.bounds)
        });
    beside_the_parameter
        .chain(in_the_where_clause)
        .flatten()
        .collect()
}

/// The parameter a `where` predicate bounds, when it bounds a bare one; a predicate written about
/// `Vec<T>` or `T::Item` bounds that type, not `T`.
fn named_parameter(bounded_ty: &syn::Type) -> Option<&syn::Ident> {
    let syn::Type::Path(type_path) = bounded_ty else {
        return None;
    };
    if type_path.qself.is_some() {
        return None;
    }
    type_path.path.get_ident()
}

/// Whether a bound reads any generic parameter of the item other than the one it bounds.
///
/// Asked of the tokens rather than the parsed bound because the answer only ever withholds a check:
/// a path segment that happens to spell a parameter's name is read as the parameter, which costs
/// the item nothing it had before.
fn mentions_another_parameter(
    bound: &syn::TypeParamBound,
    generics: &syn::Generics,
    name: &syn::Ident,
) -> bool {
    let own = name.to_string();
    let neighbours: Vec<String> = generics
        .params
        .iter()
        .map(|param| match param {
            syn::GenericParam::Type(type_param) => type_param.ident.to_string(),
            syn::GenericParam::Const(const_param) => const_param.ident.to_string(),
            syn::GenericParam::Lifetime(lifetime_param) => {
                lifetime_param.lifetime.ident.to_string()
            }
        })
        .filter(|declared| *declared != own)
        .collect();
    reads_any_name(quote::ToTokens::to_token_stream(bound), &neighbours)
}

/// Whether a token stream spells any of the given names, at any nesting depth.
fn reads_any_name(tokens: proc_macro2::TokenStream, names: &[String]) -> bool {
    tokens.into_iter().any(|tree| match tree {
        proc_macro2::TokenTree::Ident(ident) => names.contains(&ident.to_string()),
        proc_macro2::TokenTree::Group(group) => reads_any_name(group.stream(), names),
        proc_macro2::TokenTree::Punct(_) | proc_macro2::TokenTree::Literal(_) => false,
    })
}

/// Emits the expansion with the tokens it earned beside it standing ahead of it, or the expansion
/// untouched where it earned none.
fn with_prefixed_tokens(expanded: TokenStream, prefix: &[proc_macro2::TokenStream]) -> TokenStream {
    if prefix.is_empty() {
        return expanded;
    }
    let item_and_surface: proc_macro2::TokenStream = expanded.into();
    quote! {
        #(#prefix)*
        #item_and_surface
    }
    .into()
}

/// The `compile_error!` tokens an item earns for carrying a ` ```rust example ` block while
/// declaring a const parameter, or none where it carries no example and none where it declares no
/// const.
///
/// A doc example is Rust the expansion has to compile, so its value is annotated with the item's
/// own name at one instantiation — every parameter filled in, a type parameter taking the type
/// `default_types` declares for it or `String` where it declares none. A const takes no filling
/// from that convention: both spellings name a type, and a const is a value. Nor can one be read
/// off the item, since `default_types` declares types and no value is the one every
/// const-parameterised example is written at, so one picked here would render the example at a
/// length the author never wrote. So the example is refused where it was written, rather than
/// expanded into an annotation that fails `E0107` before anything runs.
///
/// Answered at the one seam every expanded shape is dispatched from, so a struct and an enum cannot
/// come to answer differently, and a branded newtype is answered before the struct path splits it
/// off. An alias reaches here too and is deliberately left out: it publishes no `schema_example()`
/// at all, so its example is already unread and a const on it costs nothing.
///
/// Only a build that reads an example owes the refusal, which is why this is `zod`-gated: without
/// it no `schema_example()` is emitted and an example on a const-declaring item is exactly as
/// unread as an example on any other item.
#[cfg(feature = "zod")]
fn const_parameter_example_errors(item: &Item) -> Vec<proc_macro2::TokenStream> {
    let (generics, attrs) = if let Item::Struct(item_struct) = item {
        (&item_struct.generics, &item_struct.attrs)
    } else if let Item::Enum(item_enum) = item {
        (&item_enum.generics, &item_enum.attrs)
    } else {
        return Vec::new();
    };
    let consts: Vec<&syn::Ident> = generics
        .params
        .iter()
        .filter_map(|param| match param {
            syn::GenericParam::Const(const_param) => Some(&const_param.ident),
            syn::GenericParam::Type(_) | syn::GenericParam::Lifetime(_) => None,
        })
        .collect();
    let Some(first) = consts.first() else {
        return Vec::new();
    };
    if get_item_docs(attrs)
        .and_then(|docs| extract_example_from_docs(&docs))
        .is_none()
    {
        return Vec::new();
    }
    vec![attr_guard_error(
        &syn::Error::new_spanned(first, const_parameter_example_message(&consts)),
        &item_label(item),
    )]
}

/// Nothing, in a build that reads no example: the method the refusal is owed for is never built,
/// so the block sits unread the way it does on every other item here.
#[cfg(not(feature = "zod"))]
const fn const_parameter_example_errors(_item: &Item) -> Vec<proc_macro2::TokenStream> {
    Vec::new()
}

/// Why a doc example on a const-declaring item is refused: what the example has to be, why the
/// convention that fills a type parameter reaches no const, what the feature has to do with it, and
/// the two ways out.
#[cfg(feature = "zod")]
fn const_parameter_example_message(consts: &[&syn::Ident]) -> String {
    let names = consts
        .iter()
        .map(|name| format!("`{name}`"))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "`#[model_schema]` cannot build a `schema_example()` for an item that declares a const \
         parameter, and this item declares {names}. The example is Rust compiled at one \
         instantiation, so its value is annotated with this item's own name and every parameter \
         filled in: a type parameter is filled at the type `default_types` declares for it, or at \
         `String` where it declares none, and a const takes no filling from that convention — both \
         spellings name a type, a const is a value, and no value is the one every \
         const-parameterised example is written at, so one chosen here would render the example at \
         a length this item's author never wrote. This is refused because the `zod` feature is \
         enabled, `zod` being the only surface that reads an example. Remove the ` ```rust example \
         ` block, or the const parameter, whichever this item can do without."
    )
}

/// The `compile_error!` tokens an item earns for handing one of its own const parameters to a
/// generic type it writes, or none where no type written on it does that.
#[cfg(any(feature = "typescript", feature = "jsonschema"))]
fn const_parameter_argument_errors(item: &Item) -> Vec<proc_macro2::TokenStream> {
    let Some(generics) = item_generics(item) else {
        return Vec::new();
    };
    let consts: Vec<String> = generics
        .params
        .iter()
        .filter_map(|param| match param {
            syn::GenericParam::Const(const_param) => Some(const_param.ident.to_string()),
            syn::GenericParam::Type(_) | syn::GenericParam::Lifetime(_) => None,
        })
        .collect();
    if consts.is_empty() {
        return Vec::new();
    }
    let mut found: Vec<syn::Ident> = Vec::new();
    for written in item_written_types(item) {
        collect_const_arguments(written, &consts, &mut found);
    }
    found
        .iter()
        .map(|argument| {
            attr_guard_error(
                &syn::Error::new_spanned(
                    argument,
                    const_parameter_argument_message(&argument.to_string()),
                ),
                &item_label(item),
            )
        })
        .collect()
}

/// Every type an item writes out: a struct's field types, an enum's variant field types, an alias's
/// target. The types whose spelling reaches a surface, which is where an argument is written.
#[cfg(any(feature = "typescript", feature = "jsonschema"))]
fn item_written_types(item: &Item) -> Vec<&syn::Type> {
    if let Item::Struct(item_struct) = item {
        item_struct.fields.iter().map(|field| &field.ty).collect()
    } else if let Item::Enum(item_enum) = item {
        item_enum
            .variants
            .iter()
            .flat_map(|variant| variant.fields.iter().map(|field| &field.ty))
            .collect()
    } else if let Item::Type(item_type) = item {
        vec![item_type.ty.as_ref()]
    } else {
        Vec::new()
    }
}

/// Every place one of `consts` is handed to a written type as an argument, collected as the ident
/// it was written at so the refusal points there.
#[cfg(any(feature = "typescript", feature = "jsonschema"))]
fn collect_const_arguments(written: &syn::Type, consts: &[String], found: &mut Vec<syn::Ident>) {
    match written {
        syn::Type::Path(type_path) => {
            for segment in &type_path.path.segments {
                let syn::PathArguments::AngleBracketed(angled) = &segment.arguments else {
                    continue;
                };
                for argument in &angled.args {
                    match argument {
                        syn::GenericArgument::Type(inner) => {
                            if let Some(ident) = bare_ident(inner)
                                && consts.contains(&ident.to_string())
                            {
                                found.push(ident);
                            } else {
                                collect_const_arguments(inner, consts, found);
                            }
                        }
                        syn::GenericArgument::Const(syn::Expr::Path(path)) => {
                            if let Some(ident) = path.path.get_ident()
                                && consts.contains(&ident.to_string())
                            {
                                found.push(ident.clone());
                            }
                        }
                        // A const written as anything but a bare name is an expression, not one of
                        // the item's parameters standing alone; a lifetime and an associated
                        // binding name no const at all.
                        syn::GenericArgument::Const(_)
                        | syn::GenericArgument::Lifetime(_)
                        | syn::GenericArgument::AssocType(_)
                        | syn::GenericArgument::AssocConst(_)
                        | syn::GenericArgument::Constraint(_)
                        | _ => {}
                    }
                }
            }
        }
        syn::Type::Array(array) => collect_const_arguments(&array.elem, consts, found),
        syn::Type::Slice(slice) => collect_const_arguments(&slice.elem, consts, found),
        syn::Type::Reference(reference) => collect_const_arguments(&reference.elem, consts, found),
        syn::Type::Paren(paren) => collect_const_arguments(&paren.elem, consts, found),
        syn::Type::Group(group) => collect_const_arguments(&group.elem, consts, found),
        syn::Type::Tuple(tuple) => {
            for element in &tuple.elems {
                collect_const_arguments(element, consts, found);
            }
        }
        // None of these writes an argument list a const could stand in: a function pointer, an
        // `impl Trait`, an inferred or never type, a trait object, a raw pointer, and the two
        // spellings `syn` hands back unparsed.
        syn::Type::FnPtr(_)
        | syn::Type::ImplTrait(_)
        | syn::Type::Infer(_)
        | syn::Type::Macro(_)
        | syn::Type::Never(_)
        | syn::Type::Ptr(_)
        | syn::Type::TraitObject(_)
        | syn::Type::Verbatim(_)
        | _ => {}
    }
}

/// The ident a type is written as when it is written as a bare name and nothing else, or `None`.
#[cfg(any(feature = "typescript", feature = "jsonschema"))]
fn bare_ident(written: &syn::Type) -> Option<syn::Ident> {
    let syn::Type::Path(type_path) = written else {
        return None;
    };
    if type_path.qself.is_some() {
        return None;
    }
    type_path.path.get_ident().cloned()
}

/// Why a const handed to a written type as an argument is refused: where the const does render,
/// where this one stands instead, what each surface would emit, what the features have to do with
/// it, and the way out.
#[cfg(any(feature = "typescript", feature = "jsonschema"))]
fn const_parameter_argument_message(name: &str) -> String {
    let surfaces = if cfg!(feature = "typescript") && cfg!(feature = "jsonschema") {
        "the `typescript` and `jsonschema` features are enabled, the two surfaces that render an \
         argument list"
    } else if cfg!(feature = "typescript") {
        "the `typescript` feature is enabled, a surface that renders an argument list"
    } else {
        "the `jsonschema` feature is enabled, a surface that renders an argument list"
    };
    format!(
        "`#[model_schema]` cannot render the const parameter `{name}` as a type argument. An \
         argument list is read as a list of types — that is what every surface writes one from — so \
         `{name}` standing in one is taken for a type, and no surface can spell it: the JSON \
         document would call into a `{}_schema` module nothing publishes, and the TypeScript \
         declaration would write `{name}` where nothing binds it, a const being dropped from the \
         declaration it was written on. This is refused because {surfaces}. A const does render as \
         an array length — `[T; {name}]`, which describes as an unbounded array — so hold the value \
         that way, or fill the argument with a type.",
        name.to_lowercase()
    )
}

/// Nothing, in a build that renders no argument list: Zod names the schema the *item* published and
/// a const-declaring item publishes the one schema it has, so the const never stands where a type
/// is read.
#[cfg(not(any(feature = "typescript", feature = "jsonschema")))]
const fn const_parameter_argument_errors(_item: &Item) -> Vec<proc_macro2::TokenStream> {
    Vec::new()
}

/// The `compile_error!` tokens for every `cfg_attr`-wrapped serde attribute an enum carries: on the
/// type (tagging, variant casing) and on each variant (`rename`).
///
/// Collected before the enum is dispatched to its shape, since all three shapes read those same
/// attributes and each would otherwise render a contract the wrapper had quietly emptied.
#[cfg(feature = "serde")]
fn enum_cfg_attr_guard_errors(
    item_enum: &syn::ItemEnum,
    type_meta: &SerdeTypeMeta,
) -> Vec<proc_macro2::TokenStream> {
    let name = &item_enum.ident;
    type_meta
        .cfg_attr_rejection
        .as_ref()
        .map(|rejection| cfg_attr_guard_error(rejection, &format!("type `{name}`")))
        .into_iter()
        .chain(item_enum.variants.iter().filter_map(|variant| {
            parse_serde_field_attributes(&variant.attrs)
                .cfg_attr_rejection
                .as_ref()
                .map(|rejection| {
                    cfg_attr_guard_error(rejection, &format!("variant `{}`", variant.ident))
                })
        }))
        .collect()
}

/// The `compile_error!` tokens for a branded newtype whose inner type is `Option`, or `None` for
/// every other inner type.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn branded_option_inner_error(
    name: &Ident,
    inner_field: &Field,
) -> Option<proc_macro2::TokenStream> {
    get_field_def("_inner", &inner_field.ty, "")
        .is_optional()
        .then(|| {
            syn::Error::new_spanned(
                inner_field,
                format!(
                    "model_schema: branded newtype `{name}` wraps an `Option`, which has no \
                     representable schema: #[serde(transparent)] writes a `None` to the wire as \
                     `null` (skip_serializing_if cannot suppress it — a transparent newtype has \
                     no key to omit), while the generated brand renders the inner type alone. \
                     Brand the inner type and make the use site optional instead: `Option<{name}>`."
                ),
            )
            .to_compile_error()
        })
}

/// The `compile_error!` tokens for a branded newtype that applies `pattern`, `minLength`, or
/// `maxLength` to an inner type whose schema is not a string, or `None` when the inner can carry
/// them.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn branded_constraint_inner_error(
    generics: &syn::Generics,
    name: &Ident,
    inner_field: &Field,
    args: &ModelSchemaArgs,
) -> Option<proc_macro2::TokenStream> {
    if !args.has_string_constraints() {
        return None;
    }
    let inner = branded_inner_value_surface(generics, inner_field);
    if let FieldDefType::SiblingType(inner_name, _) = &inner.field_type
        && let Some(parameter) = inner.argument_parameter_name()
    {
        let message = format!(
            "model_schema: branded newtype `{name}` applies string constraints (pattern, \
             minLength, maxLength) to `{inner_name}`, whose schema this crate builds from the \
             type parameter `{parameter}` — so the checks land on whatever the instantiation \
             supplies for it, and one declaration covers every instantiation: Zod appends \
             `.min`/`.max` to a schema the call site decides the shape of, JSON Schema writes \
             them beside the `{{}}` a parameter describes as, where they go inert, and \
             `validate()` measures the inner's `Display` rendering — three surfaces, three \
             answers. Brand a string-typed inner, or drop the constraints."
        );
        return Some(syn::Error::new_spanned(inner_field, message).to_compile_error());
    }
    if !inner.is_array()
        && let FieldDefType::TypeParam(parameter) = &inner.field_type
    {
        let default = declared_default_field(parameter, &args.default_types);
        // `None` here means string-shaped — including the `String` fallback for an entry the
        // declaration left out — so the bare parameter is admitted exactly where a concrete
        // argument of the same shape would be.
        return non_string_inner_shape(&default).map(|_| {
            let default_ty = declared_default_type_name(parameter, &args.default_types);
            let message = declared_default_constraint_message(name, parameter, &default_ty);
            syn::Error::new_spanned(inner_field, message).to_compile_error()
        });
    }
    let message = match (&inner.field_type, non_string_inner_shape(&inner)) {
        (FieldDefType::SiblingType(inner_name, _), Some(shape)) => {
            named_inner_constraint_message(&name.to_string(), inner_name, shape)
        }
        (_, Some(shape)) => format!(
            "model_schema: branded newtype `{name}` applies string constraints (pattern, \
             minLength, maxLength) to a {shape} inner type, which cannot carry them: Zod reads \
             `.min`/`.max` as bounds on the value itself and has no regex check for a non-string \
             schema, JSON Schema ignores `minLength`/`maxLength`/`pattern` outside `\"type\": \
             \"string\"`, and `validate()` measures the inner's `Display` rendering — three \
             surfaces, three answers. Brand a string-typed inner, or drop the constraints."
        ),
        (_, None) => return None,
    };
    Some(syn::Error::new_spanned(inner_field, message).to_compile_error())
}

/// The Rust spelling of `parameter`'s declared default, for the refusal that names it — `String`
/// for an entry the declaration left out, the same fallback [`declared_default_field`] resolves to
/// a `FieldDef` from. Only reached once that fallback has already proven *not* string-shaped, so
/// the fallback branch here is unreachable in practice; it is total anyway because a refusal
/// message is not the place to let a lookup miss go unspelled.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn declared_default_type_name(
    parameter: &str,
    default_types: &[(syn::Ident, syn::Type)],
) -> String {
    default_types
        .iter()
        .find(|(declared, _)| declared == parameter)
        .map_or_else(|| "String".to_owned(), |(_, ty)| quote! { #ty }.to_string())
}

/// Why a constrained brand's bare-parameter inner is refused when its *declared default* — not the
/// parameter — turns out not to be string-shaped: the default is what `$SchemaDefault` composes
/// the checks onto, so a default the checks cannot measure is refused in its place.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn declared_default_constraint_message(name: &Ident, parameter: &str, default_ty: &str) -> String {
    format!(
        "model_schema: branded newtype `{name}` applies string constraints (pattern, minLength, \
         maxLength), but its declared default for `{parameter}` is `{default_ty}`, which cannot \
         carry them: Zod reads `.min`/`.max` as bounds on the value itself and has no regex check \
         for a non-string schema, JSON Schema ignores `minLength`/`maxLength`/`pattern` outside \
         `\"type\": \"string\"`, and `validate()` measures the inner's `Display` rendering. \
         Declare a string-typed default, or drop the constraints."
    )
}

/// `parameter`'s declared default as a `syn::Type`, for substituting a concrete Rust type rather
/// than spelling one for a diagnostic. Falls back to `String` for an entry the declaration left
/// out — the same fallback [`declared_default_type_name`] and [`declared_default_field`] resolve
/// to, so all three readers of an absent entry agree.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn declared_default_syn_type(
    parameter: &str,
    default_types: &[(syn::Ident, syn::Type)],
) -> syn::Type {
    default_types
        .iter()
        .find(|(declared, _)| declared == parameter)
        .map_or_else(|| syn::parse_quote!(String), |(_, ty)| ty.clone())
}

/// The `impl` header and self-type a type's *declared-default* `validate()` is written against:
/// every type parameter is replaced by [`declared_default_syn_type`], while a lifetime or const
/// parameter carries through unchanged — `default_types` only ever fills a type parameter, so
/// there is nothing declared for the other two to substitute.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn default_instantiation(
    name: &Ident,
    generics: &syn::Generics,
    default_types: &[(syn::Ident, syn::Type)],
) -> (syn::Generics, proc_macro2::TokenStream) {
    let mut impl_generics = syn::Generics::default();
    let mut arguments: Vec<proc_macro2::TokenStream> = Vec::new();
    for param in &generics.params {
        match param {
            syn::GenericParam::Type(type_param) => {
                let default =
                    declared_default_syn_type(&type_param.ident.to_string(), default_types);
                arguments.push(quote! { #default });
            }
            syn::GenericParam::Lifetime(lifetime_param) => {
                impl_generics
                    .params
                    .push(syn::GenericParam::Lifetime(lifetime_param.clone()));
                let lifetime = &lifetime_param.lifetime;
                arguments.push(quote! { #lifetime });
            }
            syn::GenericParam::Const(const_param) => {
                impl_generics
                    .params
                    .push(syn::GenericParam::Const(const_param.clone()));
                let ident = &const_param.ident;
                arguments.push(quote! { #ident });
            }
        }
    }
    let self_ty = if arguments.is_empty() {
        quote! { #name }
    } else {
        quote! { #name<#(#arguments),*> }
    };
    (impl_generics, self_ty)
}

/// Splits a generated `validate()` away from a type's other delegate methods when the type has
/// parameters: the checks a `validate()` runs belong to the declared default (see
/// [`default_instantiation`]), not to every instantiation, so a generic type's `validate()` moves
/// into its own `impl Name<DeclaredDefault> { .. }` block rather than sitting on the same
/// `impl<T> Name<T>` the schema delegates (`ts_definition`, `zod_schema`, …) still share — those
/// do not depend on the constraints, so they are unaffected.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn place_validate_method(
    validate_method: Option<proc_macro2::TokenStream>,
    name: &Ident,
    generics: &syn::Generics,
    default_types: &[(syn::Ident, syn::Type)],
) -> (Option<proc_macro2::TokenStream>, proc_macro2::TokenStream) {
    let Some(method) = validate_method else {
        return (None, quote! {});
    };
    if type_parameters_in_scope(generics).is_empty() {
        return (Some(method), quote! {});
    }
    let (default_generics, self_ty) = default_instantiation(name, generics, default_types);
    let (impl_generics, _, _) = default_generics.split_for_impl();
    (
        None,
        quote! {
            impl #impl_generics #self_ty {
                #method
            }
        },
    )
}

/// A branded newtype's `validate()`, split the same way [`assemble_schema_output`] splits a
/// struct's or enum's: the piece that stays on the brand's own `impl` block (empty once the type is
/// generic — see [`place_validate_method`]), and the standalone default-only `impl` block to append
/// after it. Takes the raw (possibly empty) `TokenStream` `inject_branded_serde_attrs` returns
/// rather than an `Option`, since that is what the brand path already has on hand.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn branded_validate_split(
    raw_validate_method: proc_macro2::TokenStream,
    name: &Ident,
    generics: &syn::Generics,
    default_types: &[(syn::Ident, syn::Type)],
) -> (proc_macro2::TokenStream, proc_macro2::TokenStream) {
    let (for_branded_impl, default_validate_impl) = place_validate_method(
        (!raw_validate_method.is_empty()).then_some(raw_validate_method),
        name,
        generics,
        default_types,
    );
    (for_branded_impl.unwrap_or_default(), default_validate_impl)
}

/// The consult a constrained brand leaves unanswered, for the expansion that registers the name to
/// answer, or `None` for every brand the registry could answer at its own expansion.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn deferred_shape_question(
    item_struct: &syn::ItemStruct,
    args: &ModelSchemaArgs,
) -> Option<ShapeQuestion> {
    if !args.has_string_constraints() {
        return None;
    }
    let inner =
        branded_inner_value_surface(&item_struct.generics, item_struct.fields.iter().next()?);
    if inner.is_array() || sequence_wrapper_element(&inner).is_some() {
        return None;
    }
    let FieldDefType::SiblingType(inner_name, arguments) = &inner.field_type else {
        return None;
    };
    if lookup_alias_info(inner_name).is_some() {
        return None;
    }
    Some(ShapeQuestion {
        argument_shapes: arguments.iter().map(non_string_inner_shape).collect(),
        brand: item_struct.ident.to_string(),
        inner: inner_name.clone(),
    })
}

/// What a name a brand asked about publishes, once the registration answering it has landed —
/// `None` where that is a shape a string check lands on, and where the reference wrote no argument
/// at the position the registration published.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn answered_shape(
    published: PublishedShape,
    argument_shapes: &[Option<&'static str>],
) -> Option<&'static str> {
    match published {
        PublishedShape::Flat(shape) => shape,
        PublishedShape::Parameter(position) => *argument_shapes.get(position)?,
    }
}

/// The `compile_error!` tokens an item owes the constrained brands that asked about its name before
/// it existed, spanned on the item's own name — the only tokens this expansion holds.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn deferred_shape_refusals(registering: Option<&Ident>) -> Vec<proc_macro2::TokenStream> {
    let Some(name) = registering else {
        return Vec::new();
    };
    let rust_ident = name.to_string();
    let questions = shape_questions_for(&rust_ident);
    // Asked before the record is read, so the expansion of an item no brand named — which is most
    // of them — never pays for the lookup.
    if questions.is_empty() {
        return Vec::new();
    }
    let Some(info) = lookup_alias_info(&rust_ident) else {
        return Vec::new();
    };
    questions
        .into_iter()
        .filter_map(|question| {
            let shape = answered_shape(info.value_shape, &question.argument_shapes)?;
            let message = named_inner_constraint_message(&question.brand, &rust_ident, shape);
            Some(syn::Error::new_spanned(name, message).to_compile_error())
        })
        .collect()
}

/// The same, where no surface reads a published shape and so no brand ever asked.
#[cfg(not(any(feature = "typescript", feature = "zod", feature = "jsonschema")))]
const fn deferred_shape_refusals(
    _registering: Option<&syn::Ident>,
) -> Vec<proc_macro2::TokenStream> {
    Vec::new()
}

/// How a constrained brand over a name the registry proves publishes no string is refused, in the
/// one wording both orders of the two declarations reach it by.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn named_inner_constraint_message(brand: &str, inner: &str, shape: &str) -> String {
    format!(
        "model_schema: branded newtype `{brand}` applies string constraints (pattern, minLength, \
         maxLength) to `{inner}`, which this crate writes as the {shape} value the checks are then \
         appended to — and that binding carries no string for them to measure: Zod either reads \
         `.min`/`.max` as a bound on something else or has no such check on it at all, JSON Schema \
         ignores `minLength`/`maxLength`/`pattern` outside `\"type\": \"string\"`, and `validate()` \
         measures the inner's `Display` rendering — three surfaces, three answers. Brand a \
         string-typed inner, or drop the constraints."
    )
}

/// Names the shape an inner type resolves to that has no string for the constraints to measure, or
/// `None` when it writes exactly one and so can carry them.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn non_string_inner_shape(inner: &FieldDef) -> Option<&'static str> {
    if inner.is_array() || sequence_wrapper_element(inner).is_some() {
        return Some("container");
    }
    match &inner.field_type {
        FieldDefType::SiblingType(inner_name, arguments) => {
            instantiated_value_shape(inner_name, arguments)
        }
        FieldDefType::Map(..) | FieldDefType::Tuple(..) => Some("container"),
        FieldDefType::Boolean => Some("boolean"),
        FieldDefType::U8
        | FieldDefType::U16
        | FieldDefType::U32
        | FieldDefType::U64
        | FieldDefType::I8
        | FieldDefType::I16
        | FieldDefType::I32
        | FieldDefType::I64
        | FieldDefType::Usize
        | FieldDefType::Isize
        | FieldDefType::F32
        | FieldDefType::F64 => Some("numeric"),
        FieldDefType::TypeParam(_) | FieldDefType::Unknown => Some("opaque"),
        // A `char` writes the one-character string every other string-shaped arm here does, and
        // `validate()` reaches it the same way it reaches a numeric or boolean inner: through
        // `Display`.
        FieldDefType::Char | FieldDefType::String | FieldDefType::StringLiteral(_) => None,
        #[cfg(feature = "object_id")]
        FieldDefType::ObjectId => None,
        #[cfg(feature = "chrono")]
        FieldDefType::NaiveDate
        | FieldDefType::NaiveTime
        | FieldDefType::NaiveDateTime
        | FieldDefType::DateTime => None,
    }
}

/// What a registered name filled with `arguments` publishes as a value, and `None` for a name the
/// registry has no answer for — one declared below the type reading it, or one this crate never
/// expands at all.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn instantiated_value_shape(rust_ident: &str, arguments: &[FieldDef]) -> Option<&'static str> {
    match lookup_alias_info(rust_ident)?.value_shape {
        PublishedShape::Flat(shape) => shape,
        PublishedShape::Parameter(position) => non_string_inner_shape(arguments.get(position)?),
    }
}

/// The JSON type keyword a registered name's wire describes as, when the registry proves that wire
/// is no object, and `None` for every name it cannot prove one way or the other.
#[cfg(all(feature = "serde", any(feature = "zod", feature = "typescript")))]
fn registered_non_object_wire(rust_ident: &str) -> Option<&'static str> {
    match lookup_alias_info(rust_ident)?.wire.as_slice() {
        [only] => only.non_object,
        _ => None,
    }
}

/// The JSON type keyword a value one keyword describes writes, and `None` for every type that
/// writes a document no single keyword names — a composite, a name, an `ObjectId`'s `$oid` object,
/// an opaque value.
#[cfg(any(
    feature = "jsonschema",
    all(feature = "serde", any(feature = "zod", feature = "typescript"))
))]
const fn scalar_json_type_keyword(field_type: &FieldDefType) -> Option<&'static str> {
    match *field_type {
        FieldDefType::Boolean => Some("boolean"),
        FieldDefType::F32 | FieldDefType::F64 => Some("number"),
        FieldDefType::I8
        | FieldDefType::I16
        | FieldDefType::I32
        | FieldDefType::I64
        | FieldDefType::Isize
        | FieldDefType::U8
        | FieldDefType::U16
        | FieldDefType::U32
        | FieldDefType::U64
        | FieldDefType::Usize => Some("integer"),
        FieldDefType::Char | FieldDefType::String | FieldDefType::StringLiteral(_) => {
            Some("string")
        }
        #[cfg(feature = "chrono")]
        FieldDefType::DateTime
        | FieldDefType::NaiveDate
        | FieldDefType::NaiveDateTime
        | FieldDefType::NaiveTime => Some("string"),
        #[cfg(feature = "object_id")]
        FieldDefType::ObjectId => None,
        FieldDefType::Map(..)
        | FieldDefType::SiblingType(..)
        | FieldDefType::Tuple(..)
        | FieldDefType::TypeParam(_)
        | FieldDefType::Unknown => None,
    }
}

/// What the value surface a `#[model_schema()]` item publishes under a name is, as the
/// constrained-brand guard reads shapes — the answer [`crate::utils::record_value_shape`] stores
/// for every name a brand can then reach through.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn published_value_shape(written: &FieldDef, parameters: &[String]) -> PublishedShape {
    let mut target = written.clone();
    target.erase_type_parameters(parameters);
    if target.is_optional() {
        return PublishedShape::Flat(Some("nullable"));
    }
    published_parameter_position(&target, parameters).map_or_else(
        || PublishedShape::Flat(non_string_inner_shape(&target)),
        PublishedShape::Parameter,
    )
}

/// The position of the item's own parameter a written target *is*, and `None` for every target that
/// fixes a shape of its own around one.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn published_parameter_position(target: &FieldDef, parameters: &[String]) -> Option<usize> {
    if target.is_array() || sequence_wrapper_element(target).is_some() {
        return None;
    }
    let name = target.parameter_shape_name()?;
    parameters.iter().position(|declared| declared == name)
}

/// The `compile_error!` tokens for every `cfg_attr`-wrapped serde attribute a branded newtype
/// carries: on the type and on its inner slot, which is positional and so has no name to print.
#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
fn branded_cfg_attr_guard_errors(
    item_struct: &syn::ItemStruct,
    inner_field: &Field,
) -> Vec<proc_macro2::TokenStream> {
    let name = &item_struct.ident;
    parse_serde_type_attributes(&item_struct.attrs)
        .cfg_attr_rejection
        .as_ref()
        .map(|rejection| cfg_attr_guard_error(rejection, &format!("type `{name}`")))
        .into_iter()
        .chain(
            parse_serde_field_attributes(&inner_field.attrs)
                .cfg_attr_rejection
                .as_ref()
                .map(|rejection| cfg_attr_guard_error(rejection, &field_label(""))),
        )
        .collect()
}

#[cfg(all(
    not(feature = "serde"),
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
const fn branded_cfg_attr_guard_errors(
    _item_struct: &syn::ItemStruct,
    _inner_field: &Field,
) -> Vec<proc_macro2::TokenStream> {
    Vec::new()
}

/// The `compile_error!` tokens for a brand whose `pattern` argument a guard refuses, or `None`
/// when it carries none or it clears them.
///
/// The brand is the only shape that reads a type-level `pattern`, and it splices the string into
/// the Rust validator, the Zod literal and the JSON schema alike; every other shape is refused the
/// argument before a surface is built.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn branded_pattern_error(name: &Ident, args: &ModelSchemaArgs) -> Option<proc_macro2::TokenStream> {
    args.pattern_rejection
        .as_ref()
        .map(|rejection| pattern_guard_error(rejection, &format!("type `{name}`")))
}

/// The inner type as the two validating surfaces receive it: with the brand's own type parameters
/// already classified as parameters, which is what leaves each surface writing the filling it
/// reaches one through rather than a reference to a type of that name.
///
/// The guard and the registration read the inner through this one call, so what a brand is refused
/// for and what it records for the next brand to read cannot come apart.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn branded_inner_value_surface(generics: &syn::Generics, inner_field: &Field) -> FieldDef {
    surface_field_def(generics, &get_field_def("_inner", &inner_field.ty, ""))
}

/// What the registry records for a brand.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn branded_alias_kind(inner_field: &Field) -> AliasKind {
    let inner = get_field_def("", &inner_field.ty, "");
    match map_key_path(&inner) {
        MapKeyPath::Open => AliasKind::StringWire,
        MapKeyPath::Unnarrowed => AliasKind::Stringified,
        MapKeyPath::Refused(_) => AliasKind::NoEnumMembers,
        MapKeyPath::Enumerated(inner_name) => match registered_key_kind(inner_name) {
            Some(AliasKind::EnumMembers) => AliasKind::StringWire,
            Some(AliasKind::Stringified) => AliasKind::Stringified,
            _ => AliasKind::NoEnumMembers,
        },
    }
}

/// The `compile_error!` tokens for a branded newtype whose inner reaches a map key no surface can
/// write, or `None` when it reaches none.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn branded_map_key_error(name: &Ident, inner_field: &Field) -> Option<proc_macro2::TokenStream> {
    let inner = get_field_def("", &inner_field.ty, "");
    let rejection = map_key_rejection(&inner)?;
    let message = map_key_rejection_message(&format!("type `{name}`"), &rejection);
    Some(syn::Error::new_spanned(&inner_field.ty, message).to_compile_error())
}

/// The `compile_error!` tokens for every guard a branded newtype violates: a `cfg_attr`-wrapped
/// serde attribute on the type or on its inner slot, an `Option` inner type, string constraints
/// over an inner type that cannot carry them, a `pattern` that is not a regex, and a map key the
/// inner reaches that no surface can write.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn branded_guard_errors(
    item_struct: &syn::ItemStruct,
    args: &ModelSchemaArgs,
) -> Vec<proc_macro2::TokenStream> {
    let inner_field = item_struct.fields.iter().next().unwrap();
    branded_cfg_attr_guard_errors(item_struct, inner_field)
        .into_iter()
        .chain(branded_option_inner_error(&item_struct.ident, inner_field))
        .chain(branded_constraint_inner_error(
            &item_struct.generics,
            &item_struct.ident,
            inner_field,
            args,
        ))
        .chain(branded_pattern_error(&item_struct.ident, args))
        .chain(branded_map_key_error(&item_struct.ident, inner_field))
        .collect()
}

/// The `compile_error!` tokens for a `#[serde(flatten)]` field the registry proves is a plain enum,
/// or `None` for every other field.
///
/// Read from the declaration rather than from the collected field so the diagnostic keeps the span
/// of the type it rejects.
#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
fn flattened_field_guard_error(
    field: &syn::Field,
    type_name: &str,
) -> Option<proc_macro2::TokenStream> {
    let inner = get_field_def("_flattened", &field.ty, "");
    let inner_name = flattened_plain_enum(&inner)?;
    let field_name = field_label(
        &field
            .ident
            .as_ref()
            .map_or_else(String::new, ToString::to_string),
    );
    Some(
        syn::Error::new_spanned(
            &field.ty,
            format!(
                "model_schema: {field_name} of `{type_name}` carries `#[serde(flatten)]` over \
                 `{inner_name}`, which serde does not write as an object: a plain enum writes its \
                 own variant name, so it contributes no members to the object being written — \
                 serde writes that name as a key holding null, which a schema closed around the \
                 remaining fields rejects. Write the field as a named member so the value gets a \
                 key of its own, or flatten a type serde writes as an object. \
                 {FLATTENED_PLAIN_ENUM_SCOPE}"
            ),
        )
        .to_compile_error(),
    )
}

/// The attributes the emitted field keeps from the declaration it was written with: everything but
/// this crate's own `model_schema_prop`, which is inert to every derive, so a copy left on the
/// emitted item is one rustc reports as an attribute that does not exist, pointing at the author's
/// line with a diagnostic naming the wrong macro.
fn declaration_attrs(field: &Field) -> Vec<syn::Attribute> {
    field
        .attrs
        .iter()
        .filter(|attr| !attr.path().is_ident("model_schema_prop"))
        .cloned()
        .collect()
}

/// Writes the serde attributes generation held back onto the fields they were generated for.
fn apply_deferred_field_attrs<'field>(
    fields: impl Iterator<Item = &'field mut Field>,
    deferred: Vec<Vec<syn::Attribute>>,
) {
    for (field, attrs) in fields.zip(deferred) {
        field.attrs.extend(attrs);
    }
}

/// The `compile_error!` tokens for a `#[serde(flatten)]` field whose recorded wire proves serde
/// does not write an object where the merge needs one, or `None` for every other field.
#[cfg(all(feature = "serde", feature = "zod"))]
fn flatten_edge_guard_error(
    field: &syn::Field,
    type_name: &str,
) -> Option<proc_macro2::TokenStream> {
    let inner = get_field_def("_flattened", &field.ty, "");
    let FieldDefType::SiblingType(inner_name, _) = &inner.field_type else {
        return None;
    };
    let refused = inner
        .zod_union_members()
        .into_iter()
        .find_map(|member| member.non_object.map(|named| (member.branch_path(), named)))
        .or_else(|| flattened_name_refused_branch(&inner));
    let message = if let Some((branch, named)) = refused {
        format!(
            "model_schema: `{type_name}`: `#[serde(flatten)]` of `{inner_name}` writes a \
             union member that is not an object — its branch {branch} describes a `{named}`, \
             which has no members to merge, and what serde writes for that member does not \
             join the object being written; write the field as a named member so the value \
             gets a key of its own."
        )
    } else {
        let named = flattened_name_non_object_wire(&inner)?;
        format!(
            "model_schema: `{type_name}`: `#[serde(flatten)]` of `{inner_name}` is not written \
             as an object — its schema describes a `{named}`, which has no members to merge, \
             and what serde writes for it does not join the object being written; write the \
             field as a named member so the value gets a key of its own."
        )
    };
    Some(syn::Error::new_spanned(&field.ty, message).to_compile_error())
}

/// The JSON type keyword the item a flattened field names publishes, where the name is the whole of
/// what the field writes and the registry proves that wire is no object — and `None` everywhere the
/// declaration is left as it stands.
#[cfg(all(feature = "serde", feature = "zod"))]
fn flattened_name_non_object_wire(inner: &FieldDef) -> Option<&'static str> {
    if inner.is_array() || flattened_plain_enum(inner).is_some() {
        return None;
    }
    let FieldDefType::SiblingType(name, _) = &inner.field_type else {
        return None;
    };
    registered_non_object_wire(name)
}

/// The branch a flattened field's own name proves is no object, beside the JSON type keyword that
/// branch describes as — and `None` for every name whose leaves prove no such branch.
#[cfg(all(feature = "serde", feature = "zod"))]
fn flattened_name_refused_branch(inner: &FieldDef) -> Option<(String, &'static str)> {
    if inner.is_array() || flattened_plain_enum(inner).is_some() {
        return None;
    }
    let FieldDefType::SiblingType(name, _) = &inner.field_type else {
        return None;
    };
    let leaves = lookup_alias_info(name)?.wire;
    let (absences, values): (Vec<&WireLeaf>, Vec<&WireLeaf>) =
        leaves.iter().partition(|leaf| leaf.is_published_absence());
    if absences.len() != 1 {
        return None;
    }
    if !values.iter().all(|leaf| leaf.non_object.is_some()) {
        return None;
    }
    let first = values.first()?;
    Some((first.branch_path(), first.non_object?))
}

/// Processes every field of a struct, returning the regular field defs, the `#[serde(flatten)]`
/// field defs, the per-field serde validation functions and `validate()` body fragments, and the
/// `compile_error!` tokens for any field-level guard violations.
fn collect_struct_fields(
    fields: &mut syn::Fields,
    rename_all: Option<&str>,
    module_name_opt: Option<&str>,
    type_name: &str,
    generics: &syn::Generics,
    container_defaulted: bool,
) -> StructFieldData {
    let type_parameters = type_parameters_in_scope(generics);
    let mut field_defs = Vec::new();
    let mut flattened_fields: Vec<FieldDef> = Vec::new();
    let mut validation_fns: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut validate_bodies: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut guard_errors: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut deferred_attrs: Vec<Vec<syn::Attribute>> = Vec::new();

    for field in fields.iter_mut() {
        #[cfg(feature = "serde")]
        let is_flatten = parse_serde_field_attributes(&field.attrs).flatten;
        #[cfg(not(feature = "serde"))]
        let is_flatten = false;

        // Read before the field is processed, which strips the attributes the declaration carried.
        #[cfg(all(
            feature = "serde",
            any(feature = "typescript", feature = "zod", feature = "jsonschema")
        ))]
        if is_flatten {
            guard_errors.extend(flattened_field_guard_error(field, type_name));
        }

        #[cfg(all(feature = "serde", feature = "zod"))]
        if is_flatten {
            guard_errors.extend(flatten_edge_guard_error(field, type_name));
        }

        let (f_def, validation_fn, validate_body, field_guard_errors) = process_field(
            &FieldContext {
                container_defaulted,
                rename_all,
                schema_module_name: module_name_opt,
                type_name,
                type_parameters: &type_parameters,
                variant_ident: None,
            },
            field,
            &mut deferred_attrs,
        );

        guard_errors.extend(field_guard_errors);

        if is_flatten {
            let _: (&_, &_) = (&validation_fn, &validate_body);
            flattened_fields.push(f_def);
            continue;
        }

        if let Some(vfn) = validation_fn {
            validation_fns.push(vfn);
        }
        if let Some(vb) = validate_body {
            validate_bodies.push(vb);
        }
        push_described_field(&mut field_defs, f_def);
    }

    if guard_errors.is_empty() {
        apply_deferred_field_attrs(fields.iter_mut(), deferred_attrs);
    }

    (
        field_defs,
        flattened_fields,
        validation_fns,
        validate_bodies,
        guard_errors,
    )
}

/// Panics unless the struct has no string constraints — those are only valid on branded newtypes.
fn assert_no_struct_string_constraints(args: &ModelSchemaArgs) {
    assert!(
        !args.has_string_constraints(),
        "model_schema constraints (pattern, minLength, maxLength) are only supported on branded newtype structs (#[serde(transparent)] single-field tuple structs)"
    );
}

/// Returns whether a struct is a branded newtype: `#[serde(transparent)]` plus a single field.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn is_branded_newtype(item_struct: &syn::ItemStruct) -> bool {
    has_serde_transparent(&item_struct.attrs)
        && matches!(&item_struct.fields, syn::Fields::Unnamed(f) if f.unnamed.len() == 1)
}

/// Computes the TypeScript name, schema-module name, and module ident for a struct, and registers
/// it in the alias registry so other types can resolve references to it.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn struct_module_idents(
    name: &syn::Ident,
    name_override: Option<&str>,
    surface: Surface,
) -> (String, String, Ident) {
    let item_name = compute_item_export_name(&name.to_string(), name_override);
    let module_name = ident_schema_module_name(&name.to_string());
    let module_ident = Ident::new(&module_name, name.span());
    register_alias_info(
        &name.to_string(),
        &item_name,
        &module_name,
        AliasKind::NoEnumMembers,
    );
    surface.record(&name.to_string());
    (item_name, module_name, module_ident)
}

/// The enum counterpart of [`struct_module_idents`], down to naming the module from the Rust ident.
/// `kind` differs per enum shape: only a plain unit enum is given an `enum_members()`, so only it
/// can back an enum-keyed map — and only it is written as a `z.enum`, every other shape being a
/// union of the members its variants render as.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn enum_module_idents(
    name: &syn::Ident,
    item_name: &str,
    kind: AliasKind,
    surface: Surface,
) -> (String, Ident) {
    let module_name = ident_schema_module_name(&name.to_string());
    let module_ident = Ident::new(&module_name, name.span());
    register_alias_info(&name.to_string(), item_name, &module_name, kind);
    surface.record(&name.to_string());
    (module_name, module_ident)
}

/// Extracts a struct's doc lines and the first ` ```rust example ` block (if any) from them.
///
/// Read in every build that emits a schema surface rather than only the two that render one of the
/// halves: what the author wrote is a fact about the declaration, and the example half already
/// answers `None` where `zod` — the only surface that reads one — is off.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn struct_docs_and_example(item_struct: &syn::ItemStruct) -> (Option<Vec<String>>, Option<String>) {
    let docs_vec = get_struct_docs(item_struct);
    #[cfg(feature = "zod")]
    let example_code = docs_vec
        .as_ref()
        .and_then(|docs| extract_example_from_docs(docs));
    #[cfg(not(feature = "zod"))]
    let example_code = None;
    (docs_vec, example_code)
}

/// The output a struct carrying a `cfg_attr`-wrapped serde attribute on the type is refused with,
/// or `None` when it carries none.
///
/// A hidden `rename_all` reshapes every field name, so the item is refused before a single field
/// is processed.
#[cfg(feature = "serde")]
fn struct_cfg_attr_guard_output(
    item_struct: &syn::ItemStruct,
    rejection: Option<&syn::Error>,
) -> Option<TokenStream> {
    let ident = &item_struct.ident;
    guard_failure_output(
        item_struct,
        Some(ident),
        &[cfg_attr_guard_error(rejection?, &format!("type `{ident}`"))],
    )
}
/// The struct's own `rename_all`, or the `cfg_attr` refusal that pre-empts reading anything.
#[cfg(feature = "serde")]
fn struct_rename_all(item_struct: &syn::ItemStruct) -> Result<Option<String>, TokenStream> {
    let serde_type_meta = parse_serde_type_attributes(&item_struct.attrs);
    if let Some(output) =
        struct_cfg_attr_guard_output(item_struct, serde_type_meta.cfg_attr_rejection.as_ref())
    {
        return Err(output);
    }
    Ok(serde_type_meta.rename_all)
}

fn process_struct(mut item_struct: syn::ItemStruct, args: &ModelSchemaArgs) -> TokenStream {
    #[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
    if is_branded_newtype(&item_struct) {
        return process_branded_newtype(item_struct, args);
    }

    // String constraints (pattern, minLength, maxLength) are only valid on branded newtypes
    assert_no_struct_string_constraints(args);

    let name = item_struct.ident.clone();
    let rust_ident = name.to_string();

    #[cfg(feature = "serde")]
    let rename_all = match struct_rename_all(&item_struct) {
        Ok(rename_all) => rename_all,
        Err(output) => return output,
    };
    #[cfg(not(feature = "serde"))]
    let rename_all: Option<String> = None;

    // A `default` on the container answers a missing key for every field under it, which is one of
    // the things that makes a dropped key readable.
    let container_defaulted = has_serde_default(&item_struct.attrs);

    // A tuple struct's slots have no keys, so the named-field emitters below would render every
    // one of them under the empty ident it carries.
    #[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
    if is_tuple_struct(&item_struct) {
        return process_tuple_struct(item_struct, rename_all.as_deref(), args);
    }

    // Compute schema-module identifiers and register the struct in the alias registry.
    #[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
    let (item_name, module_name, module_ident) =
        struct_module_idents(&name, args.name_override.as_deref(), Surface::object());

    // Extract docs early for example extraction
    #[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
    let docs_and_example = struct_docs_and_example(&item_struct);

    // `Some(..)` selects schema-module-aware field processing; `None` (no schema output feature)
    // skips it so generated code never references a module that won't be emitted.
    #[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
    let module_name_opt = Some(module_name.as_str());
    #[cfg(not(any(feature = "typescript", feature = "zod", feature = "jsonschema")))]
    let module_name_opt: Option<&str> = None;

    // `collected`: (field_defs, flattened_fields, validation_fns, validate_bodies, guard_errors).
    // Bound as a whole so feature-gated field access (`.0`/`.2`/`.3`) marks it used without
    // per-element unused warnings; `collect_struct_fields` is always called for its `item_struct`
    // mutation.
    let collected = collect_struct_fields(
        &mut item_struct.fields,
        rename_all.as_deref(),
        module_name_opt,
        &rust_ident,
        &item_struct.generics,
        container_defaulted,
    );
    #[cfg(not(any(feature = "typescript", feature = "zod", feature = "jsonschema")))]
    let _: &_ = &&collected;

    // A violated field guard makes the whole contract unsound, so the schema surface is dropped
    // and only the original item plus the errors are emitted.
    if let Some(output) = guard_failure_output(&item_struct, Some(&item_struct.ident), &collected.4)
    {
        return output;
    }

    #[cfg(feature = "typescript")]
    let docs = build_jsdoc_body(docs_and_example.0.as_deref(), &item_name);
    #[cfg(all(
        not(feature = "typescript"),
        any(feature = "zod", feature = "jsonschema")
    ))]
    let docs = String::new();

    #[cfg(any(feature = "zod", feature = "typescript", feature = "jsonschema"))]
    let schema_impl_items = struct_schema_impl_items(
        collected.0,
        &collected.1,
        &item_name,
        &rust_ident,
        &item_struct.generics,
        args,
        &docs,
    );

    // schema_example must be directly on the type (not in the module) because the example code
    // uses type names that may not be accessible from the nested module.
    #[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
    let schema_example_method = item_schema_example_method(
        docs_and_example.1.as_ref(),
        &name,
        &item_struct.generics,
        args,
    );

    #[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
    let validate_method = struct_validate_method(&collected.3, &module_ident);

    // Build delegating impl items (schema_example is added directly, not as a delegate).
    #[cfg(any(feature = "zod", feature = "typescript", feature = "jsonschema"))]
    let delegate_impl_items = build_struct_delegate_items(
        &module_ident,
        &item_name,
        &rust_ident,
        &type_parameters_in_scope(&item_struct.generics),
        schema_example_method.as_ref(),
    );

    #[cfg(any(feature = "zod", feature = "typescript", feature = "jsonschema"))]
    {
        assemble_schema_output(&SchemaOutputParts {
            default_types: &args.default_types,
            delegate_impl_items: &delegate_impl_items,
            generics: &item_struct.generics,
            item: &item_struct,
            module_ident: &module_ident,
            name: &name,
            schema_impl_items: &schema_impl_items,
            validate_method: &validate_method,
            validation_fns: &collected.2,
        })
    }

    #[cfg(not(any(feature = "zod", feature = "typescript", feature = "jsonschema")))]
    {
        let output = quote! {
            #item_struct
        };
        log::trace!("{output}");
        TokenStream::from(output)
    }
}

/// Returns whether a struct's slots are positional. A branded newtype is one too, and is
/// dispatched ahead of this question.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
const fn is_tuple_struct(item_struct: &syn::ItemStruct) -> bool {
    matches!(item_struct.fields, syn::Fields::Unnamed(_))
}

/// Whether the slot at `index` of a tuple struct declaring `declared_slots` slots carries serde
/// attributes the wire answers for.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
const fn slot_attributes_reach_the_wire(declared_slots: usize) -> bool {
    declared_slots > 1
}

/// Rejects a tuple-struct slot whose serde attributes drop it out of one of serde's two directions
/// and not the other.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn check_slot_wire_is_readable(
    field: &Field,
    index: usize,
    declared_slots: usize,
    type_name: &str,
    omission: SerdeKeyOmission,
) -> Result<(), syn::Error> {
    if !slot_attributes_reach_the_wire(declared_slots) || !omission.drops_one_direction_only() {
        return Ok(());
    }
    Err(syn::Error::new_spanned(
        field,
        format!(
            "model_schema: slot {index} of tuple struct `{type_name}` carries a serde attribute \
             that drops it from only one of serde's two directions, so the array serde writes is \
             not an array serde reads — one of them carries the slot and the other does not. A \
             slot is written by its place rather than under a key, so there is no optional \
             spelling to describe both and no schema can be written for the pair. Use \
             #[serde(skip)] to take the slot off the wire in both directions, or drop the \
             attribute so the slot is written and read in its place."
        ),
    ))
}

/// Walks a tuple struct's slots, returning the shape they publish and the `compile_error!` tokens
/// of every guard a slot violates.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn collect_tuple_slots(
    fields: &mut syn::Fields,
    rename_all: Option<&str>,
    module_name: &str,
    type_name: &str,
    generics: &syn::Generics,
    container_defaulted: bool,
) -> (TupleStructShape, Vec<proc_macro2::TokenStream>) {
    let declared_slots = fields.len();
    let type_parameters = type_parameters_in_scope(generics);
    let mut slots: Vec<FieldDef> = Vec::new();
    let mut guard_errors: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut deferred_attrs: Vec<Vec<syn::Attribute>> = Vec::new();
    for (index, field) in fields.iter_mut().enumerate() {
        let omission = parse_serde_key_omission(&field.attrs);
        if let Err(rejection) =
            check_slot_wire_is_readable(field, index, declared_slots, type_name, omission)
        {
            guard_errors.push(rejection.to_compile_error());
        }
        let (slot, _, _, slot_guard_errors) = process_field(
            &FieldContext {
                container_defaulted,
                rename_all,
                schema_module_name: Some(module_name),
                type_name,
                type_parameters: &type_parameters,
                variant_ident: None,
            },
            field,
            &mut deferred_attrs,
        );
        guard_errors.extend(slot_guard_errors);
        if slot_attributes_reach_the_wire(declared_slots) && omission.absent_from_wire() {
            continue;
        }
        slots.push(slot);
    }
    if guard_errors.is_empty() {
        apply_deferred_field_attrs(fields.iter_mut(), deferred_attrs);
    }
    (tuple_struct_shape(declared_slots, slots), guard_errors)
}

/// The shape a tuple struct's declared arity and described slots amount to.
///
/// The bare value is keyed on the *declared* arity, not on how many slots came back described: a
/// struct declaring two slots with one of them off the wire writes a one-element array, and reading
/// the described count would collapse it to the bare value serde does not write there.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn tuple_struct_shape(declared_slots: usize, mut slots: Vec<FieldDef>) -> TupleStructShape {
    if declared_slots == 1 && slots.len() == 1 {
        return TupleStructShape::BareValue(Box::new(slots.remove(0)));
    }
    TupleStructShape::Array(slots)
}

/// The TypeScript type a tuple struct describes as.
///
/// The bare value is the slot's own type: serde writes a newtype struct as that value alone, with
/// nothing around it. Every array — the empty one included, which writes `[]` — is the fixed tuple
/// the slots on the wire describe as when they are written as a tuple field.
#[cfg(feature = "typescript")]
fn tuple_struct_ts_body(shape: &TupleStructShape) -> String {
    match shape {
        TupleStructShape::Array(slots) => format!(
            "[{}]",
            slots
                .iter()
                .map(FieldDef::typescript_slot_typename)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        TupleStructShape::BareValue(slot) => slot.typescript_slot_typename(),
    }
}

/// [`tuple_struct_ts_body`] for the Zod surface.
#[cfg(feature = "zod")]
fn tuple_struct_zod_body(shape: &TupleStructShape) -> String {
    match shape {
        TupleStructShape::Array(slots) => format!(
            "z.tuple([{}])",
            slots
                .iter()
                .map(FieldDef::zod_slot_type)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        TupleStructShape::BareValue(slot) => slot.zod_slot_type(),
    }
}

/// [`tuple_struct_ts_body`] for the JSON-schema surface, as a standalone `serde_json::Value`
/// expression, or the diagnostic naming the type when a slot holds a value the dispatch cannot
/// render.
#[cfg(feature = "jsonschema")]
fn tuple_struct_json_body(item_name: &str, shape: &TupleStructShape) -> proc_macro2::TokenStream {
    let described = match shape {
        TupleStructShape::Array(slots) => tuple_json_schema_value(slots),
        TupleStructShape::BareValue(slot) => build_tuple_element_json_schema(slot),
    };
    match described {
        Ok(value) => value,
        Err(rejection) => {
            let message = map_member_rejection_message(&format!("`{item_name}`"), &rejection);
            quote! { compile_error!(#message) }
        }
    }
}

/// Builds the `ts_definition()` method for a tuple struct's schema module.
#[cfg(feature = "typescript")]
fn build_tuple_struct_ts_definition_method(
    docs: &str,
    item_name: &str,
    rust_ident: &str,
    ts_generics: &str,
    ts_body: &str,
) -> proc_macro2::TokenStream {
    let reexport = ident_reexport_ts(rust_ident, item_name, ts_generics);
    let type_str = format!(
        "{}\nexport type {item_name}{ts_generics} = {ts_body};{reexport}",
        jsdoc_block(docs, "")
    );
    quote! {
        pub fn ts_definition() -> String {
            #type_str.to_owned()
        }
    }
}

/// Builds the `zod_schema()` method for a tuple struct's schema module, in the same framing every
/// unbranded type publishes: the annotated binding, or the factory when the tuple struct declares
/// parameters.
#[cfg(feature = "zod")]
fn build_tuple_struct_zod_schema_method(
    item_name: &str,
    rust_ident: &str,
    parameters: &[String],
    default_types: &[(syn::Ident, syn::Type)],
    zod_body: &str,
) -> proc_macro2::TokenStream {
    let reexport = zod_binding_reexport(rust_ident, item_name, parameters);
    let schema_str = zod_published_binding(
        item_name,
        rust_ident,
        parameters,
        default_types,
        "",
        zod_body,
        &reexport,
    );
    quote! {
        pub fn zod_schema() -> String {
            #schema_str.to_owned()
        }
    }
}

/// What a tuple struct publishes: serde writes one slot as that slot's value alone, so the schema
/// *is* the slot's schema and carries exactly what it carries; every other arity is the fixed array
/// `z.tuple` writes, which takes no string check and which no object can be merged with.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn tuple_struct_surface(fields: &syn::Fields, parameters: &[String]) -> Surface {
    let mut slots = fields.iter();
    match (slots.next(), slots.next()) {
        (Some(slot), None) => Surface::written(&get_field_def("_slot", &slot.ty, ""), parameters),
        _ => Surface::array(),
    }
}

/// Processes a tuple struct that is not a branded newtype.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn process_tuple_struct(
    mut item_struct: syn::ItemStruct,
    rename_all: Option<&str>,
    args: &ModelSchemaArgs,
) -> TokenStream {
    let name = item_struct.ident.clone();
    let (item_name, module_name, module_ident) = struct_module_idents(
        &name,
        args.name_override.as_deref(),
        tuple_struct_surface(
            &item_struct.fields,
            &type_parameters_in_scope(&item_struct.generics),
        ),
    );

    let (shape, guard_errors) = collect_tuple_slots(
        &mut item_struct.fields,
        rename_all,
        &module_name,
        &name.to_string(),
        &item_struct.generics,
        has_serde_default(&item_struct.attrs),
    );

    // A violated slot guard makes the whole contract unsound, so the schema surface is dropped and
    // only the original item plus the errors are emitted.
    if let Some(output) =
        guard_failure_output(&item_struct, Some(&item_struct.ident), &guard_errors)
    {
        return output;
    }

    #[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
    let docs_and_example = struct_docs_and_example(&item_struct);

    let schema_impl_items: Vec<proc_macro2::TokenStream> = vec![
        #[cfg(feature = "jsonschema")]
        json_schema_methods(
            &item_name,
            &tuple_struct_json_body(&item_name, &shape),
            &schema_parameters(&item_struct.generics, args),
        ),
        #[cfg(feature = "typescript")]
        build_tuple_struct_ts_definition_method(
            &build_jsdoc_body(docs_and_example.0.as_deref(), &item_name),
            &item_name,
            &name.to_string(),
            &ts_generic_params(&item_struct.generics),
            &tuple_struct_ts_body(&shape),
        ),
        #[cfg(feature = "zod")]
        build_tuple_struct_zod_schema_method(
            &item_name,
            &name.to_string(),
            &type_parameters_in_scope(&item_struct.generics),
            &args.default_types,
            &tuple_struct_zod_body(&shape),
        ),
    ];

    // schema_example must be directly on the type (not in the module) because the example code
    // uses type names that may not be accessible from the nested module.
    let schema_example_method = item_schema_example_method(
        docs_and_example.1.as_ref(),
        &name,
        &item_struct.generics,
        args,
    );

    let delegate_impl_items = build_struct_delegate_items(
        &module_ident,
        &item_name,
        &name.to_string(),
        &type_parameters_in_scope(&item_struct.generics),
        schema_example_method.as_ref(),
    );

    assemble_schema_output(&SchemaOutputParts {
        default_types: &args.default_types,
        delegate_impl_items: &delegate_impl_items,
        generics: &item_struct.generics,
        item: &item_struct,
        module_ident: &module_ident,
        name: &name,
        schema_impl_items: &schema_impl_items,
        validate_method: &None,
        validation_fns: &[],
    })
}

/// Builds the `validate_value`/`deserialize_value` functions for a constrained branded newtype.
///
/// Returns `None` when the newtype has no string constraints. A path inner is measured by the
/// string serde writes for it, exactly as a path field is; every other inner is reached through
/// `ToString`, so it works for `String`, `ObjectId`, and any generic `ID_TYPE` implementing
/// `Display`.
#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
fn build_branded_validation(
    args: &ModelSchemaArgs,
    is_generic: bool,
    inner_ty: &syn::Type,
) -> Option<BrandedValidation> {
    args.has_string_constraints().then(|| {
        let measures_path = branded_inner_measures_path(inner_ty);
        let (checked_param, rendering) = checked_value_parts(measures_path);
        let checked_v = branded_checked_value(measures_path, &quote! { v });
        let mut checks: Vec<proc_macro2::TokenStream> = Vec::new();

        if let Some(min_len) = args.min_length {
            checks.push(quote! {
                if value.len() < #min_len {
                    return Err(format!(
                        "value is too short: minimum length is {}, got {}",
                        #min_len, value.len()
                    ));
                }
            });
        }
        if let Some(max_len) = args.max_length {
            checks.push(quote! {
                if value.len() > #max_len {
                    return Err(format!(
                        "value is too long: maximum length is {}, got {}",
                        #max_len, value.len()
                    ));
                }
            });
        }
        if let Some(pattern) = &args.pattern {
            checks.push(pattern_check(
                pattern,
                &quote! {
                    return Err(format!(
                        "value does not match pattern '{}'",
                        #pattern
                    ));
                },
            ));
        }

        let validate_fn = quote! {
            pub fn validate_value(#checked_param) -> Result<(), String> {
                #rendering
                #(#checks)*
                Ok(())
            }
        };

        let deserialize_fn = if is_generic {
            quote! {
                pub fn deserialize_value<'de, D, T>(deserializer: D) -> Result<T, D::Error>
                where
                    D: serde::Deserializer<'de>,
                    T: serde::Deserialize<'de> + std::fmt::Display,
                {
                    use serde::Deserialize;
                    let v = T::deserialize(deserializer)?;
                    validate_value(#checked_v).map_err(serde::de::Error::custom)?;
                    Ok(v)
                }
            }
        } else {
            // Deliberately unspanned: a `to_string()` carrying the user's span is judged by the
            // consumer's lints as hand-written, and on a `String` inner it is a redundant clone.
            // The inner field's `Display` requirement is blamed by the static assertion instead,
            // which is inert wherever it lands.
            quote! {
                pub fn deserialize_value<'de, D>(deserializer: D) -> Result<#inner_ty, D::Error>
                where
                    D: serde::Deserializer<'de>,
                {
                    use serde::Deserialize;
                    let v = <#inner_ty>::deserialize(deserializer)?;
                    validate_value(#checked_v).map_err(serde::de::Error::custom)?;
                    Ok(v)
                }
            }
        };

        BrandedValidation {
            checked_inner: branded_checked_value(measures_path, &quote! { self.0 }),
            deserialize_fn,
            validate_fn,
        }
    })
}

/// Whether a brand's constrained checks reach its inner value as a path rather than through
/// `Display`.
#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
fn branded_inner_measures_path(inner_ty: &syn::Type) -> bool {
    constrained_shape(inner_ty).is_some_and(|shape| {
        matches!(shape.leaf, ConstraintLeaf::Path)
            && shape
                .wraps
                .iter()
                .all(|wrap| matches!(wrap, ConstraintWrap::Transparent))
    })
}

/// How a constrained brand hands `receiver` to `validate_value`: a path goes borrowed — deref
/// coercion carries it through whatever transparent wrappers it was written under — since the
/// validator renders it itself; every other inner is rendered through `Display` first.
#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
fn branded_checked_value(
    measures_path: bool,
    receiver: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    if measures_path {
        quote! { &#receiver }
    } else {
        quote! { &#receiver.to_string() }
    }
}

/// Builds the schema a branded newtype's constraints are written into: `base_inserts` first — what
/// the inner type describes as before the brand narrows it — then one insert per constraint the
/// brand declares.
#[cfg(feature = "jsonschema")]
fn branded_schema_obj_over(
    args: &ModelSchemaArgs,
    base_inserts: &proc_macro2::TokenStream,
    base_pattern: BasePattern,
) -> proc_macro2::TokenStream {
    let mut constraint_inserts: Vec<proc_macro2::TokenStream> = Vec::new();

    if let Some(min_len) = args.min_length {
        constraint_inserts.push(quote! {
            schema_obj.insert("minLength".to_string(), serde_json::Value::Number(serde_json::Number::from(#min_len as u64)));
        });
    }
    if let Some(max_len) = args.max_length {
        constraint_inserts.push(quote! {
            schema_obj.insert("maxLength".to_string(), serde_json::Value::Number(serde_json::Number::from(#max_len as u64)));
        });
    }
    if let Some(pattern) = &args.pattern {
        constraint_inserts.push(match base_pattern {
            BasePattern::Absent => quote! {
                schema_obj.insert("pattern".to_string(), serde_json::Value::String(#pattern.to_string()));
            },
            #[cfg(feature = "object_id")]
            BasePattern::Stated => quote! {
                schema_obj.insert("allOf".to_string(), serde_json::json!([{ "pattern": #pattern }]));
            },
        });
    }

    quote! {
        {
            let mut schema_obj = serde_json::Map::new();
            #base_inserts
            #(#constraint_inserts)*
            serde_json::Value::Object(schema_obj)
        }
    }
}

/// Builds the string schema a branded newtype's constraints are written into: `type_name` as the
/// `"type"`, then one insert per constraint the brand declares.
#[cfg(feature = "jsonschema")]
fn branded_constrained_schema_obj(
    args: &ModelSchemaArgs,
    type_name: &str,
) -> proc_macro2::TokenStream {
    branded_schema_obj_over(
        args,
        &quote! {
            schema_obj.insert("type".to_string(), serde_json::Value::String(#type_name.to_string()));
        },
        BasePattern::Absent,
    )
}

/// The string a chrono-typed inner writes, carrying the `"format"` keyword [`chrono_json_schema_format`]
/// gives that type — the one field position carries for it — and narrowed by the brand's own
/// constraints, which sit beside `type` and `format` the way they sit beside `type` alone.
#[cfg(all(feature = "jsonschema", feature = "chrono"))]
fn branded_chrono_schema(args: &ModelSchemaArgs, format: &str) -> proc_macro2::TokenStream {
    branded_schema_obj_over(
        args,
        &quote! {
            schema_obj.insert("type".to_string(), serde_json::Value::String("string".to_string()));
            schema_obj.insert("format".to_string(), serde_json::Value::String(#format.to_string()));
        },
        BasePattern::Absent,
    )
}

/// The `$oid` member an `ObjectId` brand carries: the hex string the type always holds, narrowed by
/// the brand's own constraints, so a brand that declares none still describes the hex the type it
/// wraps describes wherever else it is written.
///
/// The hex is the base's own `pattern`, which is why the brand's is layered beside it rather than
/// written over it — see [`BasePattern`].
#[cfg(all(feature = "jsonschema", feature = "object_id"))]
fn branded_object_id_hex_schema(args: &ModelSchemaArgs) -> proc_macro2::TokenStream {
    branded_schema_obj_over(
        args,
        &quote! {
            schema_obj.insert("type".to_string(), serde_json::Value::String("string".to_string()));
            schema_obj.insert("pattern".to_string(), serde_json::Value::String(#OBJECT_ID_HEX_PATTERN.to_string()));
        },
        BasePattern::Stated,
    )
}

/// The description a brand carries for an inner no `"type"` keyword names: the one the slot
/// dispatch gives that type, so the brand describes as the type it wraps describes wherever else
/// it is written. An inner the dispatch cannot render replaces the body with the single diagnostic
/// naming the brand, as an unrenderable slot does in every other position.
#[cfg(feature = "jsonschema")]
fn branded_slot_json_schema(
    args: &ModelSchemaArgs,
    inner: &FieldDef,
    def_name: &str,
) -> proc_macro2::TokenStream {
    match build_tuple_element_json_schema(inner) {
        Ok(value) => branded_layered_over(args, &value),
        Err(rejection) => {
            let message = map_member_rejection_message(&format!("`{def_name}`"), &rejection);
            quote! { compile_error!(#message) }
        }
    }
}

/// `described` narrowed by the brand's own constraints from inside an `allOf`, or `described` alone
/// when the brand declares none.
#[cfg(feature = "jsonschema")]
fn branded_layered_over(
    args: &ModelSchemaArgs,
    described: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let mut narrowing: Vec<proc_macro2::TokenStream> = Vec::new();
    if let Some(min_len) = args.min_length {
        let len = min_len as u64;
        narrowing.push(quote! { "minLength": #len });
    }
    if let Some(max_len) = args.max_length {
        let len = max_len as u64;
        narrowing.push(quote! { "maxLength": #len });
    }
    if let Some(pattern) = &args.pattern {
        narrowing.push(quote! { "pattern": #pattern });
    }
    if narrowing.is_empty() {
        return described.clone();
    }
    quote! {
        serde_json::json!({ "allOf": [#described, { #(#narrowing),* }] })
    }
}

/// Builds the `json_schema()` method for a branded newtype's schema module.
#[cfg(feature = "jsonschema")]
fn build_branded_json_schema_method(
    args: &ModelSchemaArgs,
    json_inner: &BrandedJsonInner,
    def_name: &str,
    parameters: &[SchemaParameter],
) -> proc_macro2::TokenStream {
    let body = match json_inner {
        #[cfg(feature = "chrono")]
        BrandedJsonInner::Chrono(format) => branded_chrono_schema(args, format),
        BrandedJsonInner::Scalar(type_name) => branded_constrained_schema_obj(args, type_name),
        #[cfg(feature = "object_id")]
        BrandedJsonInner::ObjectId => {
            object_id_json_schema_value(&branded_object_id_hex_schema(args))
        }
        BrandedJsonInner::Slot(inner) => branded_slot_json_schema(args, inner, def_name),
    };
    json_schema_methods(def_name, &body, parameters)
}

/// The `FieldDef` every surface renders from: the written one with each name that is one of the
/// enclosing item's own type parameters classified as the parameter it is.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn surface_field_def(generics: &syn::Generics, written: &FieldDef) -> FieldDef {
    let mut erased = written.clone();
    erased.erase_type_parameters(&type_parameters_in_scope(generics));
    erased
}

/// The one def both validating surfaces read a branded newtype's inner off, so neither can render
/// a type parameter the other has erased — see [`surface_field_def`].
#[cfg(any(feature = "zod", feature = "jsonschema"))]
fn branded_value_inner(generics: &syn::Generics, inner_ty: &syn::Type) -> FieldDef {
    surface_field_def(generics, &get_field_def("_inner", inner_ty, ""))
}

/// Resolves the TypeScript inner type name and generic parameter list for a branded newtype.
#[cfg(feature = "typescript")]
fn branded_ts_type_and_generics(
    generic_params: &[String],
    inner_ty: &syn::Type,
) -> (String, String) {
    let ts_inner_type = get_field_def("_inner", inner_ty, "").typescript_typename();
    let ts_generics = if generic_params.is_empty() {
        String::new()
    } else {
        format!("<{}>", generic_params.join(", "))
    };
    (ts_inner_type, ts_generics)
}

/// Resolves the JSON schema shape for a branded newtype's inner field, read off the def
/// [`surface_field_def`] has already erased the brand's own type parameters out of.
#[cfg(feature = "jsonschema")]
fn branded_json_inner(inner: &FieldDef) -> BrandedJsonInner {
    #[cfg(feature = "object_id")]
    if branded_inner_is_object_id(inner) {
        return BrandedJsonInner::ObjectId;
    }
    if branded_inner_composite(inner).is_some() {
        return BrandedJsonInner::Slot(Box::new(inner.clone()));
    }
    #[cfg(feature = "chrono")]
    if let Some(format) = chrono_json_schema_format(&inner.field_type) {
        return BrandedJsonInner::Chrono(format);
    }
    // A name is not a shape, so it is not described here at all: it is deferred to the type it
    // names, through the reference every other position defers it through — which is what makes a
    // forward declaration and a cycle behave for a brand as they behave for a field.
    if matches!(inner.field_type, FieldDefType::SiblingType(..)) {
        return BrandedJsonInner::Slot(Box::new(inner.clone()));
    }
    // Every shape the shared mapping leaves unanswered has been returned above, so the fallback
    // stands for the one thing left over: a spelling that writes a bare string.
    BrandedJsonInner::Scalar(
        scalar_json_type_keyword(&inner.field_type)
            .unwrap_or("string")
            .to_owned(),
    )
}

/// The Zod string checks a branded newtype's `minLength`, `maxLength`, and `pattern` render to,
/// or the empty string when it carries none.
#[cfg(feature = "zod")]
fn branded_zod_string_checks(args: &ModelSchemaArgs) -> String {
    let mut checks = String::new();
    if let Some(min_len) = args.min_length {
        checks = format!("{checks}.min({min_len})");
    }
    if let Some(max_len) = args.max_length {
        checks = format!("{checks}.max({max_len})");
    }
    if let Some(pattern) = &args.pattern {
        let literal_body = escape_js_regex_literal(pattern);
        checks = format!("{checks}.check(z.regex(/{literal_body}/))");
    }
    checks
}

/// The same three constraints [`branded_zod_string_checks`] renders, spelled instead as one
/// `.check(...)` call over the base check functions (`z.minLength`, `z.maxLength`, `z.regex`) —
/// the surface every Zod schema exposes, unlike `.min`/`.max`, which live on `ZodString` alone and
/// so are not guaranteed to exist on whatever a deferred or folded default argument is annotated
/// as. See [`zod_default_block`] for where the two spellings are chosen between.
#[cfg(feature = "zod")]
fn branded_zod_base_checks(args: &ModelSchemaArgs) -> String {
    let mut checks = Vec::new();
    if let Some(min_len) = args.min_length {
        checks.push(format!("z.minLength({min_len})"));
    }
    if let Some(max_len) = args.max_length {
        checks.push(format!("z.maxLength({max_len})"));
    }
    if let Some(pattern) = &args.pattern {
        let literal_body = escape_js_regex_literal(pattern);
        checks.push(format!("z.regex(/{literal_body}/)"));
    }
    if checks.is_empty() {
        String::new()
    } else {
        format!(".check({})", checks.join(", "))
    }
}

/// Whether a branded newtype's inner is an `ObjectId` written on its own, and so reaches the wire
/// as the `$oid` object rather than as any string.
///
/// An arrayed inner is excluded: what it writes is the array around that object, which is not a
/// shape this dispatch describes.
#[cfg(all(feature = "object_id", any(feature = "zod", feature = "jsonschema")))]
const fn branded_inner_is_object_id(inner: &FieldDef) -> bool {
    matches!(inner.field_type, FieldDefType::ObjectId) && !inner.is_array()
}

/// What a branded newtype's inner writes when it writes a composite, and `None` when it writes a
/// value one `"type"` keyword and one Zod class already name.
#[cfg(any(feature = "jsonschema", all(feature = "typescript", feature = "zod")))]
fn branded_inner_composite(inner: &FieldDef) -> Option<BrandedComposite> {
    if inner.is_array() {
        return Some(BrandedComposite::Array);
    }
    if let FieldDefType::SiblingType(name, args) = &inner.field_type
        && matches!(args.as_slice(), [_])
        && is_sequence_wrapper(name)
    {
        return Some(BrandedComposite::Array);
    }
    match &inner.field_type {
        FieldDefType::Map(..) => Some(BrandedComposite::Map),
        FieldDefType::Tuple(..) => Some(BrandedComposite::Tuple),
        FieldDefType::TypeParam(_) | FieldDefType::Unknown => Some(BrandedComposite::Opaque),
        FieldDefType::Boolean
        | FieldDefType::Char
        | FieldDefType::F32
        | FieldDefType::F64
        | FieldDefType::I8
        | FieldDefType::I16
        | FieldDefType::I32
        | FieldDefType::I64
        | FieldDefType::Isize
        | FieldDefType::SiblingType(..)
        | FieldDefType::String
        | FieldDefType::StringLiteral(_)
        | FieldDefType::U8
        | FieldDefType::U16
        | FieldDefType::U32
        | FieldDefType::U64
        | FieldDefType::Usize => None,
        #[cfg(feature = "object_id")]
        FieldDefType::ObjectId => None,
        #[cfg(feature = "chrono")]
        FieldDefType::DateTime
        | FieldDefType::NaiveDate
        | FieldDefType::NaiveDateTime
        | FieldDefType::NaiveTime => None,
    }
}

/// Resolves the Zod base schema for a branded newtype's inner type, applying string constraints.
#[cfg(feature = "zod")]
fn branded_zod_inner(args: &ModelSchemaArgs, inner: &FieldDef) -> String {
    if !inner.is_array() && matches!(inner.field_type, FieldDefType::TypeParam(_)) {
        return inner.zod_type();
    }
    let checks = branded_zod_string_checks(args);
    #[cfg(feature = "object_id")]
    if branded_inner_is_object_id(inner) {
        return get_object_id_zod_schema_with(&checks);
    }
    format!("{}{checks}", inner.zod_type())
}

/// The Zod class a branded newtype's base schema is an instance of, for the exported binding's
/// type annotation.
#[cfg(all(feature = "zod", feature = "typescript"))]
fn branded_zod_type_name(inner: &FieldDef) -> String {
    #[cfg(feature = "object_id")]
    if branded_inner_is_object_id(inner) {
        return "ZodObject".to_owned();
    }
    if let Some(composite) = branded_inner_composite(inner) {
        return match composite {
            BrandedComposite::Array => "ZodArray".to_owned(),
            BrandedComposite::Map => "ZodRecord".to_owned(),
            BrandedComposite::Opaque => "ZodUnknown".to_owned(),
            BrandedComposite::Tuple => "ZodTuple".to_owned(),
        };
    }
    if let FieldDefType::SiblingType(_, args) = &inner.field_type
        && args.is_empty()
    {
        return format!("typeof {}", inner.zod_type());
    }
    match inner.typescript_typename().as_str() {
        "number" => "ZodNumber".to_owned(),
        "boolean" => "ZodBoolean".to_owned(),
        _ => "ZodString".to_owned(),
    }
}

/// Builds the `ts_definition()` method for a branded newtype's schema module.
#[cfg(feature = "typescript")]
fn build_branded_ts_definition_method(
    item_name: &str,
    rust_ident: &str,
    ts_generics: &str,
    ts_inner_type: &str,
) -> proc_macro2::TokenStream {
    let reexport = ident_reexport_ts(rust_ident, item_name, ts_generics);
    #[cfg(feature = "zod")]
    {
        let type_str = format!(
            "export type {item_name}{ts_generics} = {ts_inner_type} & $brand<\"{item_name}\">;{reexport}"
        );
        quote! {
            pub fn ts_definition() -> String {
                #type_str.to_string()
            }
        }
    }
    #[cfg(not(feature = "zod"))]
    {
        let unique_symbol = format!("declare const __brand_{item_name}: unique symbol;");
        let type_str = format!(
            "export type {item_name}{ts_generics} = {ts_inner_type} & {{ readonly [__brand_{item_name}]: true }};{reexport}"
        );
        quote! {
            pub fn ts_definition() -> String {
                format!("{}\n{}", #unique_symbol, #type_str)
            }
        }
    }
}

/// The brand marker a build's flavour spells, appended to whatever schema the inner rendered.
///
/// The name is a *type* argument, which only TypeScript reads. Zod's runtime brand takes none — it
/// hands the schema back and the marker exists in the type alone — so a build emitting no
/// TypeScript writes the bare call, which is the same value under a spelling JavaScript parses.
#[cfg(feature = "zod")]
fn zod_brand_call(item_name: &str) -> String {
    #[cfg(feature = "typescript")]
    {
        format!(".brand<\"{item_name}\">()")
    }
    #[cfg(not(feature = "typescript"))]
    {
        let _: &str = item_name;
        ".brand()".to_owned()
    }
}

/// The value a branded newtype's binding holds: the inner's own schema, the brand, and the
/// description, in the order the receiver's type admits.
#[cfg(feature = "zod")]
fn branded_zod_expression(
    args: &ModelSchemaArgs,
    item_name: &str,
    parameters: &[String],
    inner: &FieldDef,
    plain_description: &str,
) -> String {
    let value = branded_zod_inner(args, inner);
    let brand = zod_brand_call(item_name);
    let described = format!(".meta({{\n  description: \"{plain_description}\",\n}})");
    if parameters.is_empty() {
        format!("{value}{brand}{described}")
    } else {
        format!("{value}{described}{brand}")
    }
}

/// Builds the `zod_schema()` method for a branded newtype's schema module.
#[cfg(feature = "zod")]
fn build_branded_zod_schema_method(
    args: &ModelSchemaArgs,
    item_name: &str,
    rust_ident: &str,
    parameters: &[String],
    inner: &FieldDef,
    plain_description: &str,
) -> proc_macro2::TokenStream {
    let expression = branded_zod_expression(args, item_name, parameters, inner, plain_description);
    let reexport = zod_binding_reexport(rust_ident, item_name, parameters);
    let body = if parameters.is_empty() {
        branded_zod_const_block(item_name, inner, &expression, &reexport)
    } else {
        let checks = branded_zod_string_checks(args);
        let branded_checks = BrandedDefaultChecks {
            base: branded_zod_base_checks(args),
            chained: checks,
        };
        let constrained_default = match (&inner.field_type, branded_checks.chained.is_empty()) {
            (FieldDefType::TypeParam(parameter), false) if !inner.is_array() => {
                Some((parameter.as_str(), &branded_checks))
            }
            _ => None,
        };
        #[cfg(feature = "typescript")]
        let brand = Some(
            if let FieldDefType::TypeParam(parameter) = &inner.field_type
                && !inner.is_array()
            {
                BrandedAnnotation::BareParameter(parameter.as_str())
            } else {
                BrandedAnnotation::Widened(branded_zod_type_name(inner))
            },
        );
        let defaults = ZodDefaultInputs {
            #[cfg(feature = "typescript")]
            brand,
            constrained: constrained_default,
            default_types: &args.default_types,
        };
        zod_factory_block(
            item_name,
            rust_ident,
            parameters,
            &defaults,
            "",
            &expression,
            &reexport,
        )
    };
    quote! {
        pub fn zod_schema() -> String {
            #body.to_owned()
        }
    }
}

/// The binding a brand that declares no parameter publishes: the raw schema, then the exported
/// `const` annotated with the branded class the inner's own rendering produces.
#[cfg(feature = "zod")]
fn branded_zod_const_block(
    item_name: &str,
    inner: &FieldDef,
    expression: &str,
    reexport: &str,
) -> String {
    #[cfg(feature = "typescript")]
    {
        format!(
            "const {item_name}$RawSchema = {expression};\n\nexport const {item_name}$Schema: \
             $ZodBranded<{}, \"{item_name}\"> = {item_name}$RawSchema;{reexport}",
            branded_zod_type_name(inner)
        )
    }
    #[cfg(not(feature = "typescript"))]
    {
        let _: &FieldDef = inner;
        format!("export const {item_name}$Schema = {expression};{reexport}")
    }
}

/// Builds the delegate methods (on the newtype impl) that forward to its schema module.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn build_branded_delegate_items(
    module_ident: &Ident,
    has_example: bool,
) -> Vec<proc_macro2::TokenStream> {
    #[cfg(not(feature = "zod"))]
    let _: &_ = &has_example;

    #[cfg(feature = "typescript")]
    let delegate_ts = quote! {
        pub fn ts_definition() -> String {
            #module_ident::Schema::ts_definition()
        }
    };

    #[cfg(feature = "zod")]
    let delegate_zod = if has_example {
        quote! {
            pub fn zod_schema() -> String {
                let base_schema = #module_ident::Schema::zod_schema();
                let example_json = serde_json::to_string(&Self::schema_example()).unwrap();
                // The one `.meta({` a brand writes closes on its own line, and it is the only
                // place a newline precedes a `})` in what the module emitted — so the close is
                // the anchor whether the brand or the description was written last.
                if let Some(pos) = base_schema.find("\n})") {
                    let mut result = base_schema[..pos].to_string();
                    result.push_str(&format!("\n  example: {},", example_json));
                    result.push_str(&base_schema[pos..]);
                    result
                } else {
                    base_schema
                }
            }
        }
    } else {
        quote! {
            pub fn zod_schema() -> String {
                #module_ident::Schema::zod_schema()
            }
        }
    };

    #[cfg(feature = "jsonschema")]
    let delegate_json_schema = quote! {
        pub fn json_schema() -> serde_json::Value {
            #module_ident::Schema::json_schema()
        }
    };

    vec![
        #[cfg(feature = "jsonschema")]
        delegate_json_schema,
        #[cfg(feature = "typescript")]
        delegate_ts,
        #[cfg(feature = "zod")]
        delegate_zod,
    ]
}

/// Builds the `Display` impl for a branded newtype, delegating to the inner field's `Display`.
///
/// The delegating call carries the inner field's span, so the method-resolution failure it raises
/// on a non-`Display` inner is reported at the field rather than at `#[model_schema()]`. Only the
/// span changes; the emitted tokens are the same either way.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn build_branded_display_impl(
    generics: &syn::Generics,
    name: &Ident,
    inner_field: &Field,
) -> proc_macro2::TokenStream {
    let (_, type_generics, _) = generics.split_for_impl();
    let mut display_generics = generics.clone();
    for param in &mut display_generics.params {
        if let syn::GenericParam::Type(tp) = param {
            tp.bounds.push(syn::parse_quote!(std::fmt::Display));
        }
    }
    let (display_impl_generics, _, display_where_clause) = display_generics.split_for_impl();
    let delegate = quote_spanned! {inner_field.ty.span()=> self.0.fmt(f) };
    quote! {
        impl #display_impl_generics std::fmt::Display for #name #type_generics #display_where_clause {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                #delegate
            }
        }
    }
}

/// Builds a branded newtype's `Display` impl together with the static assertion guarding it.
/// `no_display` drops the impl; it drops the assertion only when nothing else needs `Display`.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn build_branded_display_tokens(
    generics: &syn::Generics,
    name: &Ident,
    inner_field: &Field,
    args: &ModelSchemaArgs,
) -> proc_macro2::TokenStream {
    // Only the serde build emits the validation functions that call `to_string()`.
    #[cfg(feature = "serde")]
    let validation_needs_display =
        args.has_string_constraints() && !branded_inner_measures_path(&inner_field.ty);
    #[cfg(not(feature = "serde"))]
    let validation_needs_display = false;

    if args.no_display && !validation_needs_display {
        return quote! {};
    }
    let display_assertion = build_branded_display_assertion(inner_field, generics);
    if args.no_display {
        return display_assertion;
    }
    let display_impl = build_branded_display_impl(generics, name, inner_field);
    quote! {
        #display_assertion
        #display_impl
    }
}

/// Builds a static assertion that the branded newtype's inner type implements `Display`, spanned
/// on the inner field so a violation surfaces as an `E0277` naming the trait at the field instead
/// of the `E0599` raised by `self.0.fmt(f)` deep inside the generated impl.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn build_branded_display_assertion(
    inner_field: &Field,
    generics: &syn::Generics,
) -> proc_macro2::TokenStream {
    if type_mentions_generic_param(&inner_field.ty, generics) {
        return quote! {};
    }
    let inner_ty = &inner_field.ty;
    // The bound goes in a `where` clause: these tokens carry the user's span, so a consumer's
    // lints judge them as if they were hand-written there.
    quote_spanned! {inner_field.ty.span()=>
        const _: () = {
            const fn assert_display<T>()
            where
                T: std::fmt::Display,
            {
            }
            assert_display::<#inner_ty>();
        };
    }
}

/// Reports whether `ty` names any of `generics`' parameters (type, lifetime, or const).
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn type_mentions_generic_param(ty: &syn::Type, generics: &syn::Generics) -> bool {
    let param_names: Vec<String> = generics
        .params
        .iter()
        .map(|param| match param {
            syn::GenericParam::Type(type_param) => type_param.ident.to_string(),
            syn::GenericParam::Lifetime(lifetime_param) => {
                lifetime_param.lifetime.ident.to_string()
            }
            syn::GenericParam::Const(const_param) => const_param.ident.to_string(),
        })
        .collect();
    !param_names.is_empty() && tokens_name_any(&quote! { #ty }, &param_names)
}

/// Reports whether `tokens` contains an identifier from `names` at any nesting depth.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn tokens_name_any(tokens: &proc_macro2::TokenStream, names: &[String]) -> bool {
    tokens.clone().into_iter().any(|tree| match tree {
        proc_macro2::TokenTree::Ident(ident) => names.iter().any(|name| ident == name.as_str()),
        proc_macro2::TokenTree::Group(group) => tokens_name_any(&group.stream(), names),
        proc_macro2::TokenTree::Punct(_) | proc_macro2::TokenTree::Literal(_) => false,
    })
}

/// Builds the `schema_example()` method for a branded newtype from extracted example code.
#[cfg(feature = "zod")]
fn build_branded_schema_example(
    example_code: Option<&String>,
    name: &Ident,
    generic_params: &[String],
    args: &ModelSchemaArgs,
) -> proc_macro2::TokenStream {
    let Some(code) = example_code else {
        return quote! {};
    };
    let code_tokens: proc_macro2::TokenStream = code.parse().unwrap();
    let value_ty = schema_example_value_type(name, generic_params, &args.default_types);
    quote! {
        pub fn schema_example() -> serde_json::Value {
            let value: #value_ty = {
                #code_tokens
            };
            serde_json::to_value(&value).unwrap()
        }
    }
}

/// Injects serde `deserialize_with`/`bound` attributes onto a constrained branded newtype and
/// builds its `validation_tokens` and `validate()` method. Returns the (possibly mutated) struct
/// together with empty token streams when the newtype has no constraints.
#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
fn inject_branded_serde_attrs(
    mut owned_struct: syn::ItemStruct,
    branded_validation: Option<&BrandedValidation>,
    is_generic: bool,
    generic_params: &[String],
    module_name: &str,
    module_ident: &Ident,
) -> (
    syn::ItemStruct,
    proc_macro2::TokenStream,
    proc_macro2::TokenStream,
) {
    let Some(validation) = branded_validation else {
        return (owned_struct, quote! {}, quote! {});
    };

    // Add Display bound + serde bound to generic params so serde deserialize_with works.
    if is_generic {
        for param in &mut owned_struct.generics.params {
            if let syn::GenericParam::Type(tp) = param {
                tp.bounds.push(syn::parse_quote!(std::fmt::Display));
            }
        }
        let bounds: Vec<String> = generic_params
            .iter()
            .map(|p| format!("{p}: serde::de::DeserializeOwned + std::fmt::Display"))
            .collect();
        let bound_str = bounds.join(", ");
        let bound_lit = syn::LitStr::new(&bound_str, proc_macro2::Span::call_site());
        let bound_attr: syn::Attribute = syn::parse_quote! {
            #[serde(bound(deserialize = #bound_lit))]
        };
        owned_struct.attrs.push(bound_attr);
    }

    let deserialize_with_path = format!("{module_name}::deserialize_value");
    let path_lit = syn::LitStr::new(&deserialize_with_path, proc_macro2::Span::call_site());
    let serde_attr: syn::Attribute = syn::parse_quote! {
        #[serde(deserialize_with = #path_lit)]
    };
    if let syn::Fields::Unnamed(fields) = &mut owned_struct.fields {
        fields.unnamed.first_mut().unwrap().attrs.push(serde_attr);
    }

    let validate_fn = &validation.validate_fn;
    let deserialize_fn = &validation.deserialize_fn;
    let checked_inner = &validation.checked_inner;
    let validation_tokens = quote! {
        #validate_fn
        #deserialize_fn
    };
    let validate_method = quote! {
        pub fn validate(&self) -> Result<(), Vec<String>> {
            let mut errors = Vec::new();
            if let Err(e) = #module_ident::validate_value(#checked_inner) {
                errors.push(e);
            }
            if errors.is_empty() { Ok(()) } else { Err(errors) }
        }
    };
    (owned_struct, validation_tokens, validate_method)
}

/// Assembles the final macro output for a branded newtype: the (possibly attribute-injected)
/// struct, its `Display` impl, the schema module, and the type's delegate impl.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn assemble_branded_output(parts: &BrandedNewtypeOutput) -> TokenStream {
    let (_, type_generics, _) = parts.generics_for_ty.split_for_impl();
    let (impl_generics, _, where_clause) = parts.generics.split_for_impl();
    let item_struct = parts.item_struct;
    let display_tokens = parts.display_tokens;
    let module_ident = parts.module_ident;
    let schema_impl_items = parts.schema_impl_items;
    let validation_tokens = parts.validation_tokens;
    let name = parts.name;
    let delegate_impl_items = parts.delegate_impl_items;
    let schema_example_tokens = parts.schema_example_tokens;
    // A generic brand's `validate()` moves to its own default-only `impl` — see
    // `branded_validate_split`'s doc comment.
    let (validate_method, default_validate_impl) = branded_validate_split(
        parts.validate_method.clone(),
        name,
        parts.generics_for_ty,
        parts.default_types,
    );

    let output = quote! {
        #item_struct

        #display_tokens

        pub mod #module_ident {
            use super::*;

            #[non_exhaustive]
            pub struct Schema;

            impl Schema {
                #(#schema_impl_items)*
            }

            #validation_tokens
        }

        impl #impl_generics #name #type_generics #where_clause {
            #(#delegate_impl_items)*
            #schema_example_tokens
            #validate_method
        }

        #default_validate_impl
    };

    log::trace!("{output}");

    TokenStream::from(output)
}

/// The output a branded newtype its guards refused is replaced by, or `None` when it earns none.
///
/// Read before the type is registered in the alias registry: a rejected brand emits no schema, so
/// nothing else should be able to resolve a reference to one.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn branded_guard_failure_output(
    item_struct: &syn::ItemStruct,
    args: &ModelSchemaArgs,
) -> Option<TokenStream> {
    guard_failure_output(
        item_struct,
        Some(&item_struct.ident),
        &branded_guard_errors(item_struct, args),
    )
}

/// Processes a branded newtype — a `#[serde(transparent)]` struct holding exactly one unnamed
/// field — into the parts [`assemble_branded_output`] joins: the struct itself, the `Display` impl
/// it earns, its schema module, and the delegate impl forwarding to that module.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn register_branded_newtype(
    item_struct: &syn::ItemStruct,
    rust_ident: &str,
    item_name: &str,
    module_name: &str,
) {
    let inner_field = item_struct.fields.iter().next().unwrap();
    register_alias_info(
        rust_ident,
        item_name,
        module_name,
        branded_alias_kind(inner_field),
    );
    // A brand is written straight onto its inner's schema — `.brand()` hands back that same
    // instance — so it publishes whatever its inner publishes.
    Surface::written(
        &branded_inner_value_surface(&item_struct.generics, inner_field),
        &type_parameters_in_scope(&item_struct.generics),
    )
    .record(rust_ident);
}

/// Registers the brand, then records the consult question its own registration could not answer —
/// in that order, so a brand naming itself cannot answer its own.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn register_brand_with_questions(
    item_struct: &syn::ItemStruct,
    args: &ModelSchemaArgs,
    rust_ident: &str,
    item_name: &str,
    module_name: &str,
) {
    register_branded_newtype(item_struct, rust_ident, item_name, module_name);
    if let Some(question) = deferred_shape_question(item_struct, args) {
        record_shape_question(question);
    }
}

#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn process_branded_newtype(item_struct: syn::ItemStruct, args: &ModelSchemaArgs) -> TokenStream {
    if let Some(output) = branded_guard_failure_output(&item_struct, args) {
        return output;
    }

    let name = item_struct.ident.clone();
    let rust_ident = name.to_string();
    let item_name = compute_item_export_name(&rust_ident, args.name_override.as_deref());
    let module_name = ident_schema_module_name(&rust_ident);
    let module_ident = Ident::new(&module_name, name.span());

    register_brand_with_questions(&item_struct, args, &rust_ident, &item_name, &module_name);

    #[cfg(feature = "zod")]
    let docs_vec = get_struct_docs(&item_struct);

    #[cfg(feature = "zod")]
    let example_code = docs_vec
        .as_ref()
        .and_then(|docs| extract_example_from_docs(docs));

    #[cfg(feature = "zod")]
    let plain_description = item_description(docs_vec.as_deref(), &item_name);

    let generic_params = type_parameters_in_scope(&item_struct.generics);

    let inner_field = item_struct.fields.iter().next().unwrap();
    let inner_ty = &inner_field.ty;

    // `ts_pair`: (ts_inner_type, ts_generics).
    #[cfg(feature = "typescript")]
    let ts_pair = branded_ts_type_and_generics(&generic_params, inner_ty);

    #[cfg(not(any(feature = "typescript", feature = "zod")))]
    let _: &_ = &inner_ty;

    #[cfg(any(feature = "zod", feature = "jsonschema"))]
    let value_inner = branded_value_inner(&item_struct.generics, inner_ty);

    #[cfg(feature = "jsonschema")]
    let json_inner = branded_json_inner(&value_inner);

    // --- Generate ts_definition method ---
    #[cfg(feature = "typescript")]
    let ts_definition_method =
        build_branded_ts_definition_method(&item_name, &rust_ident, &ts_pair.1, &ts_pair.0);

    // --- Generate zod_schema method ---
    #[cfg(feature = "zod")]
    let zod_schema_method = build_branded_zod_schema_method(
        args,
        &item_name,
        &rust_ident,
        &generic_params,
        &value_inner,
        &plain_description,
    );

    // --- Generate validation code for constrained branded newtypes ---
    #[cfg(feature = "serde")]
    let branded_validation = build_branded_validation(args, !generic_params.is_empty(), inner_ty);

    // --- Build schema module impl items ---
    #[cfg(feature = "jsonschema")]
    let json_schema_method = build_branded_json_schema_method(
        args,
        &json_inner,
        &item_name,
        &schema_parameters(&item_struct.generics, args),
    );

    let schema_impl_items: Vec<proc_macro2::TokenStream> = vec![
        #[cfg(feature = "jsonschema")]
        json_schema_method,
        #[cfg(feature = "typescript")]
        ts_definition_method,
        #[cfg(feature = "zod")]
        zod_schema_method,
    ];

    // --- Generate schema_example method (goes on the type impl, not the module) ---
    #[cfg(feature = "zod")]
    let has_example = example_code.is_some();
    #[cfg(not(feature = "zod"))]
    let has_example = false;

    #[cfg(feature = "zod")]
    let schema_example_tokens =
        build_branded_schema_example(example_code.as_ref(), &name, &generic_params, args);
    #[cfg(not(feature = "zod"))]
    let schema_example_tokens = quote! {};

    // --- Generate delegate methods ---
    let delegate_impl_items = build_branded_delegate_items(&module_ident, has_example);

    // `generics` is for the impl block (Display bounds added when constrained); `generics_for_ty`
    // is the unmodified clone used for the type alias.
    let generics_for_ty = item_struct.generics.clone();
    let generics = branded_impl_generics(&item_struct, !generic_params.is_empty(), args);

    // --- Generate Display impl for branded newtypes (unless the brand opted out) ---
    let display_tokens =
        build_branded_display_tokens(&item_struct.generics, &name, inner_field, args);

    // --- Inject serde(deserialize_with) on inner field and generate validate() ---
    #[cfg(feature = "serde")]
    let (output_struct, validation_tokens, validate_method) = inject_branded_serde_attrs(
        item_struct,
        branded_validation.as_ref(),
        !generic_params.is_empty(),
        &generic_params,
        &module_name,
        &module_ident,
    );
    #[cfg(not(feature = "serde"))]
    let output_struct = item_struct;
    #[cfg(not(feature = "serde"))]
    let validation_tokens = quote! {};
    #[cfg(not(feature = "serde"))]
    let validate_method = quote! {};

    assemble_branded_output(&BrandedNewtypeOutput {
        default_types: &args.default_types,
        delegate_impl_items: &delegate_impl_items,
        display_tokens: &display_tokens,
        generics: &generics,
        generics_for_ty: &generics_for_ty,
        item_struct: &output_struct,
        module_ident: &module_ident,
        name: &name,
        schema_example_tokens: &schema_example_tokens,
        schema_impl_items: &schema_impl_items,
        validate_method: &validate_method,
        validation_tokens: &validation_tokens,
    })
}

/// Clones a branded newtype's generics, adding a `Display` bound to each type parameter when the
/// newtype carries string constraints (needed for `.to_string()`-based validation on generic inner
/// types). Without the `serde` feature no bounds are added.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn branded_impl_generics(
    item_struct: &syn::ItemStruct,
    is_generic: bool,
    args: &ModelSchemaArgs,
) -> syn::Generics {
    #[cfg(feature = "serde")]
    let mut generics = item_struct.generics.clone();
    #[cfg(not(feature = "serde"))]
    let generics = item_struct.generics.clone();
    #[cfg(not(feature = "serde"))]
    let _: &_ = &(is_generic, args);
    #[cfg(feature = "serde")]
    if is_generic && args.has_string_constraints() {
        for param in &mut generics.params {
            if let syn::GenericParam::Type(tp) = param {
                tp.bounds.push(syn::parse_quote!(std::fmt::Display));
            }
        }
    }
    generics
}

/// Processes an enum item and generates TypeScript and Zod schema definitions for it.
fn process_enum(item_enum: syn::ItemEnum, args: &ModelSchemaArgs) -> TokenStream {
    let name = item_enum.ident.clone();

    #[cfg(feature = "serde")]
    let serde_type_meta = parse_serde_type_attributes(&item_enum.attrs);

    #[cfg(feature = "serde")]
    if let Some(output) = guard_failure_output(
        &item_enum,
        Some(&item_enum.ident),
        &enum_cfg_attr_guard_errors(&item_enum, &serde_type_meta),
    ) {
        return output;
    }

    // Every enum shape below is written under this one name, and `enum_module_idents` registers it,
    // so the override reaches the shape's own surfaces and every reference to it alike.
    let item_name = compute_item_export_name(&name.to_string(), args.name_override.as_deref());

    if is_plain_enum(&item_enum) {
        #[cfg(feature = "serde")]
        let rename_all = serde_type_meta.rename_all.as_deref();

        #[cfg(not(feature = "serde"))]
        let rename_all = None;

        process_plain_enum(item_enum, &name, rename_all, &item_name, args)
    } else {
        #[cfg(feature = "serde")]
        if serde_type_meta.untagged {
            return process_untagged_enum(item_enum, &name, &item_name, args);
        }

        // Neither tagging key named, so serde writes the externally tagged form and that is what
        // the surfaces describe. Only the attributes the `serde` feature reads tell the two forms
        // apart; without it no declaration can be distinguished and the adjacent form stands.
        #[cfg(feature = "serde")]
        if serde_type_meta.tag.is_none() && serde_type_meta.content.is_none() {
            return process_externally_tagged_enum(
                item_enum,
                &name,
                serde_type_meta.rename_all.as_deref(),
                &item_name,
                args,
            );
        }

        // A tag with no content is serde's internally tagged form: what a variant writes joins the
        // tag in one object rather than sitting under a key of its own.
        #[cfg(feature = "serde")]
        if let (Some(tag), None) = (
            serde_type_meta.tag.as_ref(),
            serde_type_meta.content.as_ref(),
        ) {
            return process_internally_tagged_enum(
                item_enum,
                &name,
                tag,
                serde_type_meta.rename_all.as_deref(),
                &item_name,
                args,
            );
        }

        #[cfg(feature = "serde")]
        let (tag_name, content_name, rename_all) = (
            serde_type_meta
                .tag
                .as_ref()
                .map_or_else(|| "type".to_owned(), Clone::clone),
            serde_type_meta
                .content
                .as_ref()
                .map_or_else(|| "value".to_owned(), Clone::clone),
            serde_type_meta.rename_all,
        );

        #[cfg(not(feature = "serde"))]
        let (tag_name, content_name, rename_all): (String, String, Option<String>) =
            ("type".to_owned(), "value".to_owned(), None);

        process_discriminated_enum(
            item_enum,
            &name,
            &tag_name,
            &content_name,
            rename_all.as_deref(),
            &item_name,
            args,
        )
    }
}

/// Flattens an item's doc comments into the plain lines both its `JSDoc` body and its description
/// are spelled from: ` ```rust example ` blocks stripped, leading `*` and surrounding whitespace
/// trimmed, blank lines dropped.
#[cfg(any(feature = "typescript", feature = "zod"))]
fn item_plain_doc_lines(doc_lines: &[String]) -> Vec<String> {
    strip_examples_from_docs(doc_lines)
        .into_iter()
        .flat_map(|v| {
            v.lines()
                .map(|line| {
                    let trimmed = line.trim();
                    trimmed
                        .strip_prefix('*')
                        .unwrap_or(trimmed)
                        .trim()
                        .to_owned()
                })
                .collect::<Vec<_>>()
        })
        .filter(|s| !s.is_empty())
        .collect()
}

/// An item's doc lines, flattened the way the calling surface reads them, or the one line it falls
/// back to when it says nothing: the name it is exported under, not the one it is declared under,
/// so a `JSDoc` header never contradicts the `export type` one line beneath it and a description
/// never names an item something no surface exports.
///
/// Whether anything was said is read off the flattened lines rather than off the attribute, because
/// the flattening is what decides: a ` ```rust example ` block is Rust source and is dropped, so an
/// item documented with nothing else has an attribute and still says nothing, and is left naming
/// itself the way an undocumented one is.
///
/// Every item shape and member reaches that fallback through here, so no path can drift from the
/// rest by spelling it separately. What a caller brings of its own is `flatten` — how its docs
/// reach the surface, not what it says when it has none. Ungated because the member call sites
/// are: a field's docs are read whatever features are on.
fn item_lines_or_name(
    docs_vec: Option<&[String]>,
    item_name: &str,
    flatten: impl FnOnce(&[String]) -> Vec<String>,
) -> Vec<String> {
    let lines = docs_vec.map(flatten).unwrap_or_default();
    if lines.is_empty() {
        vec![item_name.to_owned()]
    } else {
        lines
    }
}

/// The `JSDoc` body an alias's `export type` is emitted under.
#[cfg(feature = "typescript")]
fn alias_jsdoc_body(docs_vec: Option<&[String]>, export_name: &str) -> String {
    item_jsdoc_body(&item_lines_or_name(
        docs_vec,
        export_name,
        strip_examples_from_docs,
    ))
}

/// The `JSDoc` body a set of lines is written as: each prefixed with ` * `, the block closed by a
/// bare ` * ` so it ends on an empty line. Ungated for the reason [`item_lines_or_name`] is.
fn item_jsdoc_body(lines: &[String]) -> String {
    lines
        .iter()
        .map(|l| format!(" * {l}"))
        .chain([" * ".to_owned()])
        .collect::<Vec<_>>()
        .join("\n")
}

/// The complete `JSDoc` block a body is written as, at the indent of whatever it documents: the
/// opener, the body's own lines carried to that indent, and the `*/` that closes it.
fn jsdoc_block(body: &str, indent: &str) -> String {
    let mut block = format!("{indent}/**\n");
    for line in body.lines() {
        let _ = writeln!(block, "{indent}{line}");
    }
    let _ = write!(block, "{indent} */");
    block
}

/// The `JSDoc` block a member is written under: [`jsdoc_block`] at the one indent every emitted
/// member sits at.
fn member_jsdoc_block(body: &str) -> String {
    jsdoc_block(body, MEMBER_INDENT)
}

/// The spelling a member key is written as on the text surfaces.
fn ts_member_key(key: &str) -> String {
    let mut chars = key.chars();
    let heads_an_identifier = chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_' || first == '$');
    if heads_an_identifier
        && chars.all(|char| char.is_ascii_alphanumeric() || char == '_' || char == '$')
    {
        return key.to_owned();
    }
    format!("\"{}\"", key.replace('\\', "\\\\").replace('"', "\\\""))
}

/// The one-line description a set of lines is published as, escaped for the `description: "…"` it
/// is spliced into.
///
/// The escape reaches the exported name an undocumented item falls back to and does nothing there:
/// a `name = "…"` override is refused unless it spells an identifier, and a Rust ident cannot be
/// written with a quote in it, so no exported name carries one.
#[cfg(any(feature = "typescript", feature = "zod"))]
fn describe_item_lines(lines: &[String]) -> String {
    lines.join("\\n").replace('"', "\\\"")
}

/// The escaped one-line description an item publishes, falling back to the exported name when it
/// carries no docs — spelled from the same two bodies as the description of a shape that publishes
/// a `JSDoc` header alongside it, so the two cannot drift apart.
///
/// Gated on the feature of its one caller: a brand is the only shape that publishes a description
/// and no `JSDoc` header of its own, and a description is a zod surface.
#[cfg(feature = "zod")]
fn item_description(docs_vec: Option<&[String]>, item_name: &str) -> String {
    describe_item_lines(&item_lines_or_name(
        docs_vec,
        item_name,
        item_plain_doc_lines,
    ))
}

/// Flattens an item's doc comments into a `JSDoc` body and an escaped one-line description, both
/// written from one pass over the same lines (with ` ```rust example ` blocks stripped). Falls
/// back to the exported name when there are no docs, matching every other item path.
#[cfg(any(feature = "typescript", feature = "zod"))]
fn build_item_docs_and_description(
    docs_vec: Option<&[String]>,
    item_name: &str,
) -> (String, String) {
    let plain_lines = item_lines_or_name(docs_vec, item_name, item_plain_doc_lines);

    (
        item_jsdoc_body(&plain_lines),
        describe_item_lines(&plain_lines),
    )
}

/// Collects a plain enum's serialized variant names (respecting serde renames) and per-variant
/// doc strings (the latter only populated when the `typescript` feature is enabled).
fn collect_plain_enum_options(
    item_enum: &mut syn::ItemEnum,
    rename_all: Option<&str>,
) -> (Vec<String>, Vec<String>) {
    let mut enum_options = Vec::new();
    #[cfg(feature = "typescript")]
    let mut enum_variant_docs = Vec::new();
    #[cfg(not(feature = "typescript"))]
    let enum_variant_docs: Vec<String> = Vec::new();

    for item in &mut item_enum.variants {
        #[cfg(feature = "serde")]
        let field_rename = parse_serde_field_attributes(&item.attrs).rename;
        #[cfg(not(feature = "serde"))]
        let field_rename: Option<String> = None;

        let final_name =
            get_final_variant_name(&item.ident.to_string(), field_rename.as_deref(), rename_all);
        enum_options.push(final_name);

        #[cfg(feature = "typescript")]
        {
            // The one member body not written as ` * ` lines: a plain enum's variants are commented
            // inside the union rather than over a property. The example is dropped the same way,
            // and a variant documented with nothing else is left as an undocumented one.
            let variant_docs = get_variant_docs(item).map_or_else(String::new, |doc_lines| {
                strip_examples_from_docs(&doc_lines).join("\n")
            });
            enum_variant_docs.push(variant_docs);
        }
    }

    (enum_options, enum_variant_docs)
}

/// Builds the TypeScript union body (`  | "Variant"`, with `JSDoc` per variant) for a plain enum.
#[cfg(feature = "typescript")]
fn build_plain_enum_type_code(enum_options: &[String], enum_variant_docs: &[String]) -> String {
    enum_options
        .iter()
        .enumerate()
        .map(|(idx, v)| {
            let docs = &enum_variant_docs[idx];
            if docs.is_empty() {
                format!("  | \"{v}\"")
            } else {
                let formatted_docs = docs
                    .lines()
                    .map(|line| {
                        let trimmed = line.trim();
                        // Strip leading asterisk if present (from block comments)
                        let content = trimmed.strip_prefix('*').unwrap_or(trimmed).trim();
                        if content.is_empty() {
                            "  *".to_owned()
                        } else {
                            format!("  * {content}")
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                format!("  /*\n{formatted_docs}\n  */\n  | \"{v}\"")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Processes a plain enum (simple string enum in TypeScript) and generates its definitions.
fn process_plain_enum(
    mut item_enum: syn::ItemEnum,
    name: &syn::Ident,
    rename_all: Option<&str>,
    item_name: &str,
    args: &ModelSchemaArgs,
) -> TokenStream {
    // Compute the schema module name and register the enum so other types can find it.
    #[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
    // A plain enum is written as a `z.enum` of its variant names, which takes no string check.
    let (_, module_ident) = enum_module_idents(
        name,
        item_name,
        AliasKind::EnumMembers,
        Surface::enumerated(),
    );
    #[cfg(any(feature = "typescript", feature = "zod"))]
    let rust_ident = name.to_string();

    // Extract docs early for example extraction
    #[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
    let docs_vec = get_enum_docs(&item_enum);

    let (enum_options, enum_variant_docs) = collect_plain_enum_options(&mut item_enum, rename_all);
    #[cfg(not(feature = "typescript"))]
    let _: &_ = &&enum_variant_docs;

    #[cfg(feature = "typescript")]
    let type_code = build_plain_enum_type_code(&enum_options, &enum_variant_docs);

    #[cfg(feature = "zod")]
    let schema_code = enum_options
        .iter()
        .map(|v| format!("\"{v}\""))
        .collect::<Vec<_>>()
        .join(", ");

    let enumerated: Vec<proc_macro2::TokenStream> =
        enum_options.iter().map(|v| quote! { #v }).collect();

    #[cfg(any(feature = "typescript", feature = "zod"))]
    let docs_and_description = build_item_docs_and_description(docs_vec.as_deref(), item_name);

    #[cfg(feature = "jsonschema")]
    // No slot to fill: Rust refuses an all-unit enum that leaves a type parameter unused, so a
    // plain enum reaches here declaring none — whatever consts and lifetimes it also binds.
    let json_schema_method = generate_plain_enum_json_schema_method(&enumerated, item_name, &[]);

    #[cfg(feature = "typescript")]
    let ts_definition_method = generate_plain_enum_ts_definition_method(
        &docs_and_description.0,
        item_name,
        &rust_ident,
        &ts_generic_params(&item_enum.generics),
        &type_code,
    );

    // Schema module emits zod_schema without examples; example injection happens in the delegating
    // method on the type to avoid `super::` resolution issues.
    #[cfg(feature = "zod")]
    let zod_schema_method = generate_plain_enum_zod_schema_method(
        item_name,
        &rust_ident,
        &schema_code,
        &docs_and_description.1,
    );

    #[cfg(not(any(feature = "typescript", feature = "zod")))]
    let _: &_ = &item_name;

    // schema_example must be directly on the type (not in the module) because the example code
    // uses type names that may not be accessible from the nested module.
    #[cfg(not(any(feature = "typescript", feature = "zod", feature = "jsonschema")))]
    let _: &_ = &args;

    #[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
    let schema_example_method =
        enum_schema_example_method(docs_vec.as_deref(), name, &item_enum.generics, args);

    #[cfg(any(feature = "zod", feature = "typescript", feature = "jsonschema"))]
    let schema_impl_items: Vec<proc_macro2::TokenStream> = vec![
        #[cfg(feature = "jsonschema")]
        json_schema_method,
        #[cfg(feature = "typescript")]
        ts_definition_method,
        #[cfg(feature = "zod")]
        zod_schema_method,
    ];

    // Build delegating impl items; the plain-enum delegates match the branded ones, with the
    // `schema_example()` method chained on after them when an example exists.
    #[cfg(any(feature = "zod", feature = "typescript", feature = "jsonschema"))]
    let delegate_impl_items: Vec<proc_macro2::TokenStream> =
        build_branded_delegate_items(&module_ident, schema_example_method.is_some())
            .into_iter()
            .chain(schema_example_method)
            .collect();

    let enum_values = &enumerated;

    // A plain enum publishes `enum_members()` from an `impl` of its own rather than through
    // `assemble_schema_output`, so it repeats the declaration's parameters itself. A type
    // parameter it cannot bind — Rust refuses an all-unit enum that leaves one unused — but a
    // const or a lifetime it can, and either has to be carried here or the block names a type
    // that does not exist.
    let (impl_generics, type_generics, where_clause) = item_enum.generics.split_for_impl();

    #[cfg(any(feature = "zod", feature = "typescript", feature = "jsonschema"))]
    let output = quote! {
        #item_enum

        pub mod #module_ident {
            use super::*;

            #[non_exhaustive]
            pub struct Schema;

            impl Schema {
                #(#schema_impl_items)*
            }
        }

        impl #impl_generics #name #type_generics #where_clause {
            #(#delegate_impl_items)*

            pub fn enum_members() -> Vec<String> {
                [
                    #(#enum_values),*
                ].iter().map(|v| v.to_string()).collect::<Vec<_>>()
            }
        }
    };

    #[cfg(not(any(feature = "zod", feature = "typescript", feature = "jsonschema")))]
    let output = quote! {
        #item_enum

        impl #impl_generics #name #type_generics #where_clause {
            pub fn enum_members() -> Vec<String> {
                [
                    #(#enum_values),*
                ].iter().map(|v| v.to_string()).collect::<Vec<_>>()
            }
        }
    };

    log::trace!("{output}");

    TokenStream::from(output)
}

/// The kind a variant publishes, which is not always the kind it declares.
fn variant_wire_kind(variant: &syn::Variant) -> VariantKind {
    if lone_slot_off_wire(variant) {
        VariantKind::Unit
    } else {
        classify_variant(variant)
    }
}

/// Whether the variant declares exactly one slot and takes that slot off the wire in both
/// directions — the collapse [`variant_wire_kind`] publishes a unit for.
///
/// Held apart from the kind it produces because two seams need the declaration back: what the
/// collapse is written as is a unit, and what it was *declared* as is what the refusals reading this
/// have to name. A declared unit and a collapsed one share every wire and part only here.
fn lone_slot_off_wire(variant: &syn::Variant) -> bool {
    // `classify_variant` names `TupleSingle` for a variant of exactly one unnamed field, so the
    // first field is the lone slot whose omission decides this.
    matches!(classify_variant(variant), VariantKind::TupleSingle)
        && variant
            .fields
            .iter()
            .next()
            .is_some_and(|field| parse_serde_key_omission(&field.attrs).absent_from_wire())
}

/// Rejects a tuple-variant slot whose serde attributes drop it out of one of serde's two directions
/// and not the other.
fn check_variant_slot_wire_is_readable(
    field: &Field,
    index: usize,
    variant_name: &str,
    type_name: &str,
    omission: SerdeKeyOmission,
) -> Result<(), syn::Error> {
    if field.ident.is_some() || !omission.drops_one_direction_only() {
        return Ok(());
    }
    Err(syn::Error::new_spanned(
        field,
        format!(
            "model_schema: slot {index} of variant `{variant_name}` of enum `{type_name}` carries \
             a serde attribute that drops it from only one of serde's two directions, so what \
             serde writes for the variant is not what serde reads for it — one of them carries the \
             slot and the other does not. A slot is written by its place rather than under a key, \
             so there is no optional spelling to describe both and no schema can be written for \
             the pair. Use #[serde(skip)] to take the slot off the wire in both directions, or \
             drop the attribute so the slot is written and read in its place."
        ),
    ))
}

/// Processes each variant of a discriminated enum, returning per-variant field defs, doc strings,
/// and variant kinds in declaration order, plus the collected serde validation functions and the
/// `validate()` arms those functions are run from.
fn collect_discriminated_variants(
    item_enum: &mut syn::ItemEnum,
    rename_all: Option<&str>,
    enum_module_name_opt: Option<&str>,
) -> DiscriminatedVariantData {
    let type_parameters = type_parameters_in_scope(&item_enum.generics);
    let mut variants: Vec<DiscriminatedVariant> = Vec::new();
    let mut enum_validation_fns: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut guard_errors: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut per_variant_checks: Vec<(proc_macro2::TokenStream, Vec<proc_macro2::TokenStream>)> =
        Vec::new();
    let mut deferred_attrs: Vec<Vec<syn::Attribute>> = Vec::new();
    let enum_type_name = item_enum.ident.to_string();

    for item in &mut item_enum.variants {
        #[cfg(feature = "serde")]
        let field_rename = parse_serde_field_attributes(&item.attrs).rename;
        #[cfg(not(feature = "serde"))]
        let field_rename: Option<String> = None;

        let variant_ident = item.ident.to_string();
        let final_name =
            get_final_variant_name(&variant_ident, field_rename.as_deref(), rename_all);
        // The Rust shape the `validate()` arm is matched by, beside the wire shape the surfaces
        // describe: a variant that publishes no slot is still declared holding one.
        let declared_kind = classify_variant(item);
        let wire_kind = variant_wire_kind(item);
        // A struct variant is where serde accepts a container-level `default`, so it is the
        // container its own fields' omissions are read against.
        let variant_defaulted = has_serde_default(&item.attrs);

        let mut field_defs: Vec<FieldDef> = Vec::new();
        let mut bound: Vec<(proc_macro2::Ident, proc_macro2::Ident)> = Vec::new();
        let mut checks: Vec<proc_macro2::TokenStream> = Vec::new();
        let total_fields = item.fields.len();
        for (index, field) in item.fields.iter_mut().enumerate() {
            let omission = parse_serde_key_omission(&field.attrs);
            let positional = field.ident.is_none();
            if let Err(rejection) = check_variant_slot_wire_is_readable(
                field,
                index,
                &variant_ident,
                &enum_type_name,
                omission,
            ) {
                guard_errors.push(rejection.to_compile_error());
            }
            let (f_def, validation_fn, validate_body, field_guard_errors) = process_field(
                &FieldContext {
                    container_defaulted: variant_defaulted,
                    rename_all,
                    schema_module_name: enum_module_name_opt,
                    type_name: &enum_type_name,
                    type_parameters: &type_parameters,
                    variant_ident: Some(&variant_ident),
                },
                field,
                &mut deferred_attrs,
            );
            if let Some(vfn) = validation_fn {
                enum_validation_fns.push(vfn);
            }
            // A constrained positional slot is refused by its own guard, so a member with a body to
            // run is always one the arm can name.
            if let (Some(body), Some(ident)) = (validate_body, field.ident.as_ref()) {
                let binding = member_binding(ident);
                bound.push((ident.clone(), binding));
                checks.push(body);
            }
            guard_errors.extend(field_guard_errors);
            if positional && omission.absent_from_wire() {
                continue;
            }
            push_described_field(&mut field_defs, f_def);
        }
        per_variant_checks.push((
            variant_check_pattern(&item.ident, &declared_kind, total_fields, &bound),
            checks,
        ));

        let discriminator_docs = build_jsdoc_body(get_variant_docs(item).as_deref(), &final_name);
        variants.push(DiscriminatedVariant {
            discriminator_value: final_name,
            docs: discriminator_docs,
            field_defs,
            kind: wire_kind,
        });
    }

    if guard_errors.is_empty() {
        apply_deferred_field_attrs(
            item_enum
                .variants
                .iter_mut()
                .flat_map(|item| item.fields.iter_mut()),
            deferred_attrs,
        );
    }

    (
        variants,
        enum_validation_fns,
        guard_errors,
        build_member_check_arms(per_variant_checks),
    )
}

/// Renders the TypeScript type fragments, Zod schema fragments (with optional-field lists), and
/// JSON-schema fragments for each variant of a discriminated enum.
fn render_discriminated_variants(
    tag_name: &str,
    content_name: &str,
    item_name: &str,
    variants: &[DiscriminatedVariant],
) -> RenderedVariants {
    let mut type_code_items = Vec::new();
    let mut schema_code_items = Vec::new();
    let mut json_schema_variants: Vec<proc_macro2::TokenStream> = Vec::new();

    for variant in variants {
        let (variant_type_code, variant_schema_code, optional_fields, json_schema_variant) =
            generate_variant_code(
                tag_name,
                content_name,
                &variant.discriminator_value,
                &variant.field_defs,
                &variant.kind,
                &variant.docs,
                item_name,
            );
        type_code_items.push(variant_type_code);
        schema_code_items.push((variant_schema_code, optional_fields));
        json_schema_variants.push(json_schema_variant);
    }

    (type_code_items, schema_code_items, json_schema_variants)
}

/// Builds the JSON-schema `oneOf` object expression for a discriminated enum from its per-variant
/// JSON-schema fragments.
#[cfg(feature = "jsonschema")]
fn discriminated_main_schema_code(
    json_schema_variants: &[proc_macro2::TokenStream],
) -> proc_macro2::TokenStream {
    quote! {
        let mut schema_obj = serde_json::Map::new();
        schema_obj.insert("type".to_string(), serde_json::Value::String("object".to_string()));
        schema_obj.insert("oneOf".to_string(), {
            let result: Vec<serde_json::Value> = vec![
                #(#json_schema_variants), *
            ];

            serde_json::Value::Array(result)
        });

        serde_json::Value::Object(schema_obj)
    }
}

/// Refuses every variant of an adjacently tagged enum whose declared lone slot is off the wire in
/// both directions, one error per offender so the expansion names them all.
#[cfg(feature = "serde")]
fn adjacent_collapsed_slot_guard_errors(
    item_enum: &syn::ItemEnum,
    tag_name: &str,
    content_name: &str,
) -> Vec<proc_macro2::TokenStream> {
    let type_name = &item_enum.ident;
    item_enum
        .variants
        .iter()
        .filter(|variant| lone_slot_off_wire(variant))
        .map(|variant| {
            let variant_name = &variant.ident;
            syn::Error::new_spanned(
                variant,
                format!(
                    "model_schema: variant `{variant_name}` of enum `{type_name}` declares one \
                     slot and takes it off the wire in both directions, so serde writes it as \
                     `{{\"{tag_name}\":\"{variant_name}\"}}` — the payload a unit variant writes — \
                     and then refuses to read that same payload back, asking for the \
                     `{content_name}` key it never wrote. Only \
                     `{{\"{tag_name}\":\"{variant_name}\",\"{content_name}\":null}}` reads, so what \
                     serde writes for the variant and what serde reads for it have no payload in \
                     common and no schema can be written for the pair. Declare `{variant_name}` as \
                     a unit variant: it writes the identical \
                     `{{\"{tag_name}\":\"{variant_name}\"}}` and reads it back. Or drop the \
                     attribute so the slot is written and read in its place."
                ),
            )
            .to_compile_error()
        })
        .collect()
}

/// Builds the Zod `z.discriminatedUnion` expression for a discriminated enum from its per-variant
/// member schemas, beside [`discriminated_main_schema_code`] which answers the same for JSON.
#[cfg(feature = "zod")]
fn discriminated_zod_schema_code(tag_name: &str, members: &[(String, Vec<String>)]) -> String {
    format!(
        "z.discriminatedUnion(\"{tag_name}\", [{}])",
        members
            .iter()
            .map(|(member, _opts)| format!("z.strictObject({member})"))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

/// Processes a discriminated enum (tagged union in TypeScript) and generates its definitions.
fn process_discriminated_enum(
    mut item_enum: syn::ItemEnum,
    name: &syn::Ident,
    tag_name: &str,
    content_name: &str,
    rename_all: Option<&str>,
    item_name: &str,
    args: &ModelSchemaArgs,
) -> TokenStream {
    // Compute the schema module name and register the enum so other types can find it.
    #[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
    // Every other enum shape is written as a union of what its variants render as.
    let (module_name, module_ident) =
        enum_module_idents(name, item_name, AliasKind::NoEnumMembers, Surface::union());

    // Extract docs early for example extraction
    #[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
    let docs_vec = get_enum_docs(&item_enum);

    // Process each variant in the enum.
    #[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
    let enum_module_name_opt = Some(module_name.as_str());
    #[cfg(not(any(feature = "typescript", feature = "zod", feature = "jsonschema")))]
    let enum_module_name_opt = None;

    // Read before the walk, which rewrites the very attributes the collapse is read off, and
    // returned before it so no held-back attribute is written onto an item the guard refuses.
    #[cfg(feature = "serde")]
    if let Some(output) = guard_failure_output(
        &item_enum,
        Some(&item_enum.ident),
        &adjacent_collapsed_slot_guard_errors(&item_enum, tag_name, content_name),
    ) {
        return output;
    }

    // Bind both result tuples whole so feature-gated field access marks them used (no per-element
    // guards): `variants` = (variants, validation_fns, guard_errors);
    // `rendered` = (ts, zod, json).
    let variants = collect_discriminated_variants(&mut item_enum, rename_all, enum_module_name_opt);
    if let Some(output) = guard_failure_output(&item_enum, Some(&item_enum.ident), &variants.2) {
        return output;
    }
    let rendered = render_discriminated_variants(tag_name, content_name, item_name, &variants.0);
    #[cfg(not(any(feature = "typescript", feature = "zod", feature = "jsonschema")))]
    let _: &_ = &(name, &rendered, args);

    #[cfg(feature = "jsonschema")]
    let main_schema_code = discriminated_main_schema_code(&rendered.2);

    #[cfg(feature = "typescript")]
    let type_code = rendered.0.join(" | ");

    #[cfg(feature = "zod")]
    let schema_code = discriminated_zod_schema_code(tag_name, &rendered.1);

    #[cfg(feature = "typescript")]
    let docs = build_jsdoc_body(docs_vec.as_deref(), item_name);

    #[cfg(feature = "jsonschema")]
    let json_schema_method =
        enum_json_schema_methods(&main_schema_code, item_name, &item_enum.generics, args);

    #[cfg(feature = "typescript")]
    let ts_definition_method = generate_discriminated_enum_ts_definition_method(
        &docs,
        item_name,
        &name.to_string(),
        &ts_generic_params(&item_enum.generics),
        &type_code,
    );

    // Schema module emits zod_schema without examples; example injection happens in the delegating
    // method on the type to avoid `super::` resolution issues.
    #[cfg(feature = "zod")]
    let zod_schema_method = generate_discriminated_enum_zod_schema_method(
        item_name,
        &name.to_string(),
        &type_parameters_in_scope(&item_enum.generics),
        &args.default_types,
        &schema_code,
    );

    #[cfg(not(any(feature = "typescript", feature = "zod")))]
    let _: &_ = &item_name;

    // schema_example must be directly on the type (not in the module) because the example code
    // uses type names that may not be accessible from the nested module.
    #[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
    let schema_example_method =
        enum_schema_example_method(docs_vec.as_deref(), name, &item_enum.generics, args);

    #[cfg(any(feature = "zod", feature = "typescript", feature = "jsonschema"))]
    let schema_impl_items: Vec<proc_macro2::TokenStream> = vec![
        #[cfg(feature = "jsonschema")]
        json_schema_method,
        #[cfg(feature = "typescript")]
        ts_definition_method,
        #[cfg(feature = "zod")]
        zod_schema_method,
    ];

    // Build delegating impl items; the discriminated-enum delegates match the struct ones (the
    // `zod_schema` example injection uses the same `.meta()`-before-`;` form).
    #[cfg(any(feature = "zod", feature = "typescript", feature = "jsonschema"))]
    let delegate_impl_items = build_struct_delegate_items(
        &module_ident,
        item_name,
        &name.to_string(),
        &type_parameters_in_scope(&item_enum.generics),
        schema_example_method.as_ref(),
    );

    #[cfg(any(feature = "zod", feature = "typescript", feature = "jsonschema"))]
    {
        assemble_schema_output(&SchemaOutputParts {
            default_types: &args.default_types,
            delegate_impl_items: &delegate_impl_items,
            generics: &item_enum.generics,
            item: &item_enum,
            module_ident: &module_ident,
            name,
            schema_impl_items: &schema_impl_items,
            validate_method: &build_enum_validate_method(&variants.3, &module_ident),
            validation_fns: &variants.1,
        })
    }

    #[cfg(not(any(feature = "zod", feature = "typescript", feature = "jsonschema")))]
    {
        let output = quote! {
            #item_enum
        };
        log::trace!("{output}");
        TokenStream::from(output)
    }
}

/// The object schema a struct variant's fields sit in when they are written under a key of their
/// own rather than beside a discriminator.
///
/// The per-field entries are the ones the adjacent form writes, so the two placements describe the
/// same fields identically and only differ in where the object sits.
#[cfg(all(feature = "serde", feature = "jsonschema"))]
fn named_content_json_value(json_fields: &[proc_macro2::TokenStream]) -> proc_macro2::TokenStream {
    quote! {
        {
            let mut properties = serde_json::Map::new();
            let mut required: Vec<serde_json::Value> = Vec::new();
            #(#json_fields)*
            serde_json::json!({
                "type": "object",
                "properties": serde_json::Value::Object(properties),
                "required": serde_json::Value::Array(required),
                "additionalProperties": false
            })
        }
    }
}

/// The diagnostic a variant whose content has no rendering produces, in the value position the
/// content itself would have occupied.
#[cfg(all(feature = "serde", feature = "jsonschema"))]
fn external_content_rejection_value(
    discriminator_value: &str,
    rejection: &MapMemberRejection,
) -> proc_macro2::TokenStream {
    let message =
        map_member_rejection_message(&format!("variant `{discriminator_value}`"), rejection);
    quote! { compile_error!(#message) }
}

/// Renders what one variant of an externally tagged enum writes under its key.
#[cfg(feature = "serde")]
fn render_external_content(
    kind: &VariantKind,
    field_defs: &[FieldDef],
    discriminator_value: &str,
    self_type_name: &str,
) -> (String, String, proc_macro2::TokenStream) {
    #[cfg(not(feature = "jsonschema"))]
    let _: &str = discriminator_value;

    match kind {
        VariantKind::Unit => (String::new(), String::new(), quote! {}),
        VariantKind::TupleSingle => {
            // `classify_variant` names this kind only for a variant of exactly one unnamed field.
            let fld = &field_defs[0];

            #[cfg(feature = "zod")]
            let zod = fld.zod_slot_type();
            #[cfg(not(feature = "zod"))]
            let zod = String::new();

            #[cfg(feature = "jsonschema")]
            let json = build_tuple_element_json_schema(fld).unwrap_or_else(|rejection| {
                external_content_rejection_value(discriminator_value, &rejection)
            });
            #[cfg(not(feature = "jsonschema"))]
            let json = quote! {};

            (fld.typescript_slot_typename(), zod, json)
        }
        VariantKind::TupleMultiple => {
            let ts = format!(
                "[{}]",
                field_defs
                    .iter()
                    .map(super::field_type::FieldDef::typescript_slot_typename)
                    .collect::<Vec<_>>()
                    .join(", ")
            );

            #[cfg(feature = "zod")]
            let zod = format!(
                "z.tuple([{}])",
                field_defs
                    .iter()
                    .map(super::field_type::FieldDef::zod_slot_type)
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            #[cfg(not(feature = "zod"))]
            let zod = String::new();

            #[cfg(feature = "jsonschema")]
            let json = tuple_json_schema_value(field_defs).unwrap_or_else(|rejection| {
                external_content_rejection_value(discriminator_value, &rejection)
            });
            #[cfg(not(feature = "jsonschema"))]
            let json = quote! {};

            (ts, zod, json)
        }
        VariantKind::Named => {
            let mut parts = VariantParts {
                json_fields: Vec::new(),
                optional_fields: Vec::new(),
                schema_code: String::new(),
                type_code: String::new(),
            };
            write_named_variant_fields(field_defs, None, self_type_name, &mut parts);

            #[cfg(feature = "zod")]
            let zod = format!("z.strictObject({{\n{}}})", parts.schema_code);
            #[cfg(not(feature = "zod"))]
            let zod = String::new();

            #[cfg(feature = "jsonschema")]
            let json = named_content_json_value(&parts.json_fields);
            #[cfg(not(feature = "jsonschema"))]
            let json = quote! {};

            (format!("{{\n{}}}", parts.type_code), zod, json)
        }
    }
}

/// Renders one variant of an externally tagged enum as a union member.
#[cfg(feature = "serde")]
fn render_external_variant(
    variant: &DiscriminatedVariant,
    self_type_name: &str,
) -> (String, String, proc_macro2::TokenStream) {
    let key = &variant.discriminator_value;
    let docs = &variant.docs;

    if matches!(variant.kind, VariantKind::Unit) {
        #[cfg(feature = "jsonschema")]
        let json = quote! { serde_json::json!({ "type": "string", "const": #key }) };
        #[cfg(not(feature = "jsonschema"))]
        let json = quote! {};

        return (
            format!("{}\n  \"{key}\"", member_jsdoc_block(docs)),
            format!("z.literal(\"{key}\")"),
            json,
        );
    }

    let (content_ts, content_zod, content_json) =
        render_external_content(&variant.kind, &variant.field_defs, key, self_type_name);

    #[cfg(feature = "zod")]
    let zod = {
        // A `Named` variant defers each recursive field inside the object it renders, so only the
        // kinds with no inner object need the key itself to carry the deferral.
        let defer_key = !matches!(variant.kind, VariantKind::Named)
            && variant
                .field_defs
                .iter()
                .any(|fld| fld.contains_type_reference(self_type_name));
        if defer_key {
            format!("z.strictObject({{\n  get \"{key}\"() {{ return {content_zod}; }},\n}})")
        } else {
            format!("z.strictObject({{\n  \"{key}\": {content_zod},\n}})")
        }
    };
    #[cfg(not(feature = "zod"))]
    let zod = {
        let _: &str = &content_zod;
        String::new()
    };

    // Built key by key rather than through `serde_json::json!`: a struct variant's content is a
    // block of statements, which the macro's value position cannot parse.
    #[cfg(feature = "jsonschema")]
    let json = quote! {
        {
            let mut schema_obj = serde_json::Map::new();
            schema_obj.insert(
                "type".to_string(),
                serde_json::Value::String("object".to_string()),
            );
            let mut properties = serde_json::Map::new();
            properties.insert(#key.to_string(), #content_json);
            schema_obj.insert(
                "properties".to_string(),
                serde_json::Value::Object(properties),
            );
            schema_obj.insert(
                "required".to_string(),
                serde_json::Value::Array(vec![serde_json::Value::String(#key.to_string())]),
            );
            schema_obj.insert(
                "additionalProperties".to_string(),
                serde_json::Value::Bool(false),
            );
            serde_json::Value::Object(schema_obj)
        }
    };
    #[cfg(not(feature = "jsonschema"))]
    let json = {
        let _: &_ = &content_json;
        quote! {}
    };

    (
        format!(
            "{{\n{}\n  \"{key}\": {content_ts};\n}}",
            member_jsdoc_block(docs)
        ),
        zod,
        json,
    )
}

/// Records what an object flattening an externally tagged enum joins for each of its variants, in
/// the order the union writes them.
#[cfg(all(feature = "serde", any(feature = "typescript", feature = "zod")))]
fn record_external_flatten_operands(
    rust_ident: &str,
    variants: &[DiscriminatedVariant],
    members: &[(String, String, proc_macro2::TokenStream)],
) {
    #[cfg(feature = "typescript")]
    let exclusions = sibling_key_exclusions(
        &variants
            .iter()
            .map(|variant| Some(vec![variant.discriminator_value.clone()]))
            .collect::<Vec<_>>(),
    );
    let operands: Vec<FlattenVariant> = variants
        .iter()
        .zip(members)
        .enumerate()
        .map(|(index, (variant, member))| {
            let unit = matches!(variant.kind, VariantKind::Unit);
            let key = &variant.discriminator_value;
            #[cfg(feature = "typescript")]
            let typescript = {
                let tagged = if unit {
                    format!(
                        "{{\n{}\n  \"{key}\": null;\n}}",
                        member_jsdoc_block(&variant.docs)
                    )
                } else {
                    member.0.clone()
                };
                close_tagged_flatten_member(&tagged, &exclusions[index])
            };
            #[cfg(not(feature = "typescript"))]
            let _: usize = index;
            #[cfg(feature = "zod")]
            let zod = if unit {
                format!("z.strictObject({{\n  \"{key}\": z.null(),\n}})")
            } else {
                member.1.clone()
            };
            FlattenVariant {
                #[cfg(feature = "typescript")]
                typescript,
                #[cfg(feature = "zod")]
                zod,
            }
        })
        .collect();
    record_flatten_variants(rust_ident, &operands);
}

/// Which of a flattened choice's keys each member has to say it does not carry, one list per member
/// and in the order the union writes them.
#[cfg(all(feature = "serde", feature = "typescript"))]
fn sibling_key_exclusions(member_keys: &[Option<Vec<String>>]) -> Vec<Vec<String>> {
    member_keys
        .iter()
        .map(|member| {
            let Some(own) = member.as_ref() else {
                return Vec::new();
            };
            let mut excluded: Vec<String> = Vec::new();
            for key in member_keys.iter().flatten().flatten() {
                if !own.contains(key) && !excluded.contains(key) {
                    excluded.push(key.clone());
                }
            }
            excluded
        })
        .collect()
}

/// One externally tagged variant's flatten operand with its siblings' tags marked absent, written in
/// the key-per-line shape [`render_external_variant`] already wrote the variant in.
///
/// A member that is no object literal keeps its spelling: there is no key list to add a line to, and
/// the exclusion is best-effort in exactly the way the excess-property posture around it is.
#[cfg(all(feature = "serde", feature = "typescript"))]
fn close_tagged_flatten_member(member: &str, excluded: &[String]) -> String {
    if excluded.is_empty() {
        return member.to_owned();
    }
    let Some(keys) = member.strip_suffix("\n}") else {
        return member.to_owned();
    };
    let mut closed = keys.to_owned();
    for key in excluded {
        let _ = write!(closed, "\n  \"{key}\"?: never;");
    }
    closed.push_str("\n}");
    closed
}

/// Joins an externally tagged enum's rendered members into its three union surfaces: the
/// JSON-schema body, the TypeScript union, and the Zod union.
///
/// Member order is the enum's declaration order on every surface, as it is for the other two enum
/// forms.
#[cfg(feature = "serde")]
fn join_external_union(
    members: &[(String, String, proc_macro2::TokenStream)],
) -> (proc_macro2::TokenStream, String, String) {
    #[cfg(not(any(feature = "typescript", feature = "zod", feature = "jsonschema")))]
    let _: &_ = &members;

    #[cfg(feature = "jsonschema")]
    let main_schema_code = {
        let json_members = members.iter().map(|(_, _, json)| json);
        quote! {
            let mut schema_obj = serde_json::Map::new();
            schema_obj.insert("oneOf".to_string(), {
                let result: Vec<serde_json::Value> = vec![
                    #(#json_members), *
                ];

                serde_json::Value::Array(result)
            });

            serde_json::Value::Object(schema_obj)
        }
    };
    #[cfg(not(feature = "jsonschema"))]
    let main_schema_code = quote! {};

    #[cfg(feature = "typescript")]
    let type_code = members
        .iter()
        .map(|(ts, _, _)| ts.as_str())
        .collect::<Vec<_>>()
        .join(" | ");
    #[cfg(not(feature = "typescript"))]
    let type_code = String::new();

    #[cfg(feature = "zod")]
    let schema_code = format!(
        "z.union([{}])",
        members
            .iter()
            .map(|(_, zod, _)| zod.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );
    #[cfg(not(feature = "zod"))]
    let schema_code = String::new();

    (main_schema_code, type_code, schema_code)
}

/// Processes an enum that carries no serde tagging attributes and generates its definitions.
#[cfg(feature = "serde")]
fn process_externally_tagged_enum(
    mut item_enum: syn::ItemEnum,
    name: &syn::Ident,
    rename_all: Option<&str>,
    item_name: &str,
    args: &ModelSchemaArgs,
) -> TokenStream {
    // Compute the schema module name and register the enum so other types can find it.
    #[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
    // Every other enum shape is written as a union of what its variants render as.
    let (module_name, module_ident) = enum_module_idents(
        name,
        item_name,
        AliasKind::NoEnumMembers,
        Surface::externally_tagged(&item_enum.variants),
    );

    #[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
    let docs_vec = get_enum_docs(&item_enum);

    #[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
    let enum_module_name_opt = Some(module_name.as_str());
    #[cfg(not(any(feature = "typescript", feature = "zod", feature = "jsonschema")))]
    let enum_module_name_opt = None;

    let self_type_name = item_enum.ident.to_string();
    let variants = collect_discriminated_variants(&mut item_enum, rename_all, enum_module_name_opt);
    if let Some(output) = guard_failure_output(&item_enum, Some(&item_enum.ident), &variants.2) {
        return output;
    }

    let members: Vec<(String, String, proc_macro2::TokenStream)> = variants
        .0
        .iter()
        .map(|variant| render_external_variant(variant, &self_type_name))
        .collect();

    // Recorded once the variants have rendered, because the flatten-edge spelling of a data-carrying
    // one is the member's own and only this expansion holds it.
    #[cfg(all(feature = "serde", any(feature = "typescript", feature = "zod")))]
    record_external_flatten_operands(&name.to_string(), &variants.0, &members);

    let (main_schema_code, type_code, schema_code) = join_external_union(&members);
    #[cfg(not(feature = "jsonschema"))]
    let _: &_ = &main_schema_code;
    #[cfg(not(feature = "typescript"))]
    let _: &_ = &type_code;
    #[cfg(not(feature = "zod"))]
    let _: &_ = &schema_code;
    #[cfg(not(any(feature = "typescript", feature = "zod", feature = "jsonschema")))]
    let _: &_ = &(name, args);

    #[cfg(feature = "typescript")]
    let docs = build_jsdoc_body(docs_vec.as_deref(), item_name);

    #[cfg(feature = "jsonschema")]
    let json_schema_method =
        enum_json_schema_methods(&main_schema_code, item_name, &item_enum.generics, args);

    #[cfg(feature = "typescript")]
    let ts_definition_method = generate_discriminated_enum_ts_definition_method(
        &docs,
        item_name,
        &name.to_string(),
        &ts_generic_params(&item_enum.generics),
        &type_code,
    );

    #[cfg(feature = "zod")]
    let zod_schema_method = generate_discriminated_enum_zod_schema_method(
        item_name,
        &name.to_string(),
        &type_parameters_in_scope(&item_enum.generics),
        &args.default_types,
        &schema_code,
    );

    #[cfg(not(any(feature = "typescript", feature = "zod")))]
    let _: &_ = &item_name;

    #[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
    let schema_example_method =
        enum_schema_example_method(docs_vec.as_deref(), name, &item_enum.generics, args);

    #[cfg(any(feature = "zod", feature = "typescript", feature = "jsonschema"))]
    let schema_impl_items: Vec<proc_macro2::TokenStream> = vec![
        #[cfg(feature = "jsonschema")]
        json_schema_method,
        #[cfg(feature = "typescript")]
        ts_definition_method,
        #[cfg(feature = "zod")]
        zod_schema_method,
    ];

    #[cfg(any(feature = "zod", feature = "typescript", feature = "jsonschema"))]
    let delegate_impl_items = build_struct_delegate_items(
        &module_ident,
        item_name,
        &name.to_string(),
        &type_parameters_in_scope(&item_enum.generics),
        schema_example_method.as_ref(),
    );

    #[cfg(any(feature = "zod", feature = "typescript", feature = "jsonschema"))]
    {
        assemble_schema_output(&SchemaOutputParts {
            default_types: &args.default_types,
            delegate_impl_items: &delegate_impl_items,
            generics: &item_enum.generics,
            item: &item_enum,
            module_ident: &module_ident,
            name,
            schema_impl_items: &schema_impl_items,
            validate_method: &build_enum_validate_method(&variants.3, &module_ident),
            validation_fns: &variants.1,
        })
    }

    #[cfg(not(any(feature = "zod", feature = "typescript", feature = "jsonschema")))]
    {
        let _: &_ = &(&variants.1, &variants.3);
        let output = quote! {
            #item_enum
        };
        log::trace!("{output}");
        TokenStream::from(output)
    }
}

/// How an internally tagged newtype variant's inner value fares beside the tag.
#[cfg(feature = "serde")]
fn tagged_content(inner: &FieldDef) -> TaggedContent {
    if inner.is_optional() {
        return TaggedContent::Refused("an optional");
    }
    if inner.array_depth > 0 {
        return TaggedContent::Refused("a sequence");
    }
    match inner.field_type {
        FieldDefType::SiblingType(..) => TaggedContent::Flattened,
        FieldDefType::Boolean => TaggedContent::Refused("a boolean"),
        FieldDefType::F32 | FieldDefType::F64 => TaggedContent::Refused("a float"),
        FieldDefType::I8
        | FieldDefType::I16
        | FieldDefType::I32
        | FieldDefType::I64
        | FieldDefType::Isize
        | FieldDefType::U8
        | FieldDefType::U16
        | FieldDefType::U32
        | FieldDefType::U64
        | FieldDefType::Usize => TaggedContent::Refused("an integer"),
        FieldDefType::Char | FieldDefType::String | FieldDefType::StringLiteral(_) => {
            TaggedContent::Refused("a string")
        }
        #[cfg(feature = "chrono")]
        FieldDefType::DateTime
        | FieldDefType::NaiveDate
        | FieldDefType::NaiveDateTime
        | FieldDefType::NaiveTime => TaggedContent::Refused("a string"),
        FieldDefType::Tuple(_) => TaggedContent::Refused("a tuple"),
        FieldDefType::Map(..) => TaggedContent::Unnameable("a map"),
        #[cfg(feature = "object_id")]
        FieldDefType::ObjectId => TaggedContent::Unnameable("an ObjectId"),
        FieldDefType::TypeParam(_) => TaggedContent::Unnameable("a type parameter"),
        FieldDefType::Unknown => TaggedContent::Unnameable("a type the expansion cannot resolve"),
    }
}

/// The `compile_error!` tokens for every variant an internally tagged enum cannot carry.
///
/// Collected from the declaration rather than from the collected variants so each diagnostic keeps
/// the span of the thing it rejects: the inner type for a newtype variant, the variant itself for a
/// tuple one.
#[cfg(feature = "serde")]
fn internally_tagged_guard_errors(
    item_enum: &syn::ItemEnum,
    tag_name: &str,
) -> Vec<proc_macro2::TokenStream> {
    let enum_name = &item_enum.ident;
    item_enum
        .variants
        .iter()
        .filter_map(|variant| {
            let syn::Fields::Unnamed(unnamed) = &variant.fields else {
                return None;
            };
            let variant_name = &variant.ident;
            if unnamed.unnamed.len() > 1 {
                return Some(
                    syn::Error::new_spanned(
                        variant,
                        format!(
                            "model_schema: variant `{variant_name}` of internally tagged enum \
                             `{enum_name}` is a tuple variant, which `#[serde(tag = \
                             \"{tag_name}\")]` cannot carry: the elements have no names to be \
                             written under beside the tag, and serde's own derive refuses the \
                             declaration outright — `#[serde(tag = \"...\")] cannot be used with \
                             tuple variants`. Name a `content` key so the elements get an array \
                             of their own, or write them as named fields."
                        ),
                    )
                    .to_compile_error(),
                );
            }
            // An empty tuple variant carries nothing: serde writes it as the tag alone, which is
            // what the unit arm already describes. So does a variant whose lone slot is off the
            // wire — captured, `One(#[serde(skip)] String)` writes and reads `{"type":"One"}`
            // whatever the slot holds, so what the inner type would have written beside the tag is
            // never asked.
            let field = unnamed.unnamed.first()?;
            if parse_serde_key_omission(&field.attrs).absent_from_wire() {
                return None;
            }
            let inner = get_field_def("_inner", &field.ty, "");
            let message = match tagged_content(&inner) {
                #[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
                TaggedContent::Flattened => {
                    let inner_name = flattened_plain_enum(&inner)?;
                    format!(
                        "model_schema: variant `{variant_name}` of internally tagged enum \
                         `{enum_name}` wraps `{inner_name}`, which serde does not write as an \
                         object: a plain enum writes its own variant name, so nothing of it joins \
                         the tag — serde writes that name as a key holding null, which a schema \
                         closed around the tag rejects. Name a `content` key so the value gets an \
                         object of its own, or wrap it in a struct whose fields can sit beside the \
                         tag. {FLATTENED_PLAIN_ENUM_SCOPE}"
                    )
                }
                #[cfg(not(any(feature = "typescript", feature = "zod", feature = "jsonschema")))]
                TaggedContent::Flattened => return None,
                TaggedContent::Refused(shape) => format!(
                    "model_schema: variant `{variant_name}` of internally tagged enum \
                     `{enum_name}` wraps {shape}, which cannot sit beside the tag: `#[serde(tag \
                     = \"{tag_name}\")]` with no `content` writes the variant's data as members \
                     of the object the tag is written in, and only a value serde writes as an \
                     object has members to put there. serde refuses this one at run time — \
                     `cannot serialize tagged newtype variant {enum_name}::{variant_name} \
                     containing {shape}`. Name a `content` key so the value gets an object of its \
                     own, or wrap it in a struct whose fields can sit beside the tag."
                ),
                TaggedContent::Unnameable(shape) => format!(
                    "model_schema: variant `{variant_name}` of internally tagged enum \
                     `{enum_name}` wraps {shape}, whose members the expansion cannot name: \
                     `#[serde(tag = \"{tag_name}\")]` with no `content` writes them beside the \
                     tag, and a schema closed around the tag alone rejects every one of them. \
                     Name a `content` key so the value gets an object of its own, or wrap it in a \
                     struct whose fields can sit beside the tag."
                ),
            };
            Some(syn::Error::new_spanned(field, message).to_compile_error())
        })
        .collect()
}

/// Renders one variant of an internally tagged enum as a union member.
#[cfg(feature = "serde")]
fn render_internal_variant(
    variant: &DiscriminatedVariant,
    tag_name: &str,
    self_type_name: &str,
) -> (String, String, proc_macro2::TokenStream, bool) {
    let mut parts = tagged_variant_parts(tag_name, &variant.discriminator_value, &variant.docs);
    match variant.kind {
        VariantKind::Named => {
            write_named_variant_fields(
                &variant.field_defs,
                Some(tag_name),
                self_type_name,
                &mut parts,
            );
        }
        // A unit variant writes the tag alone; a tuple variant only reaches here as the flattened
        // newtype the guard admits, whose content joins the finished member below.
        VariantKind::Unit | VariantKind::TupleSingle | VariantKind::TupleMultiple => {}
    }
    parts.type_code.push('}');
    parts.schema_code.push('}');

    #[cfg(feature = "jsonschema")]
    let tag_object =
        tagged_variant_json_object(tag_name, &variant.discriminator_value, &parts.json_fields);

    let flattened = matches!(variant.kind, VariantKind::TupleSingle)
        .then(|| variant.field_defs.first())
        .flatten();

    let Some(inner) = flattened else {
        #[cfg(feature = "jsonschema")]
        let json = quote! { serde_json::Value::Object(#tag_object) };
        #[cfg(not(feature = "jsonschema"))]
        let json = quote! {};

        return (
            parts.type_code,
            format!("z.strictObject({})", parts.schema_code),
            json,
            false,
        );
    };

    #[cfg(feature = "jsonschema")]
    let json = {
        let variant_name = &variant.discriminator_value;
        merged_object_value(
            &tag_object,
            // The same reference a `#[serde(flatten)]` base contributes: both name a type whose
            // members join the object being written.
            &[flatten_merged_source(inner)],
            &MergeDiagnostic {
                cycle_remedy: "name a `content` key so the content gets an object of its own",
                edge: &format!("the content of variant `{variant_name}`,"),
                non_object_remedy: "name a `content` key so the content gets an object of its own",
                subject: self_type_name,
            },
        )
    };
    #[cfg(not(feature = "jsonschema"))]
    let json = quote! {};

    // The content joins the member through [`deferred_zod_operand`] for the reason a flattened
    // base does: it names a `const` of its own, and one macro invocation sees one type.
    #[cfg(feature = "zod")]
    let zod = format!(
        "z.strictObject({}).and({})",
        parts.schema_code,
        deferred_zod_operand(&inner.zod_type())
    );
    #[cfg(not(feature = "zod"))]
    let zod = String::new();

    (
        format!("{} & {}", parts.type_code, inner.typescript_typename()),
        zod,
        json,
        true,
    )
}

/// Joins an internally tagged enum's rendered members into its three union surfaces.
///
/// The Zod union is a `discriminatedUnion` only while every member is an object carrying the tag in
/// its own shape. A flattened member is an intersection, which has no shape of its own to read the
/// discriminator out of, so one of those turns the whole union into a plain `z.union`.
#[cfg(feature = "serde")]
fn join_internal_union(
    members: &[(String, String, proc_macro2::TokenStream, bool)],
    tag_name: &str,
) -> (proc_macro2::TokenStream, String, String) {
    // Only the Zod surface names the tag: the other two read the members alone.
    #[cfg(not(feature = "zod"))]
    let _: &str = tag_name;
    #[cfg(not(any(feature = "typescript", feature = "zod", feature = "jsonschema")))]
    let _: &_ = &members;

    #[cfg(feature = "jsonschema")]
    let main_schema_code = discriminated_main_schema_code(
        &members
            .iter()
            .map(|(_, _, json, _)| json.clone())
            .collect::<Vec<_>>(),
    );
    #[cfg(not(feature = "jsonschema"))]
    let main_schema_code = quote! {};

    #[cfg(feature = "typescript")]
    let type_code = members
        .iter()
        .map(|(ts, _, _, _)| ts.as_str())
        .collect::<Vec<_>>()
        .join(" | ");
    #[cfg(not(feature = "typescript"))]
    let type_code = String::new();

    #[cfg(feature = "zod")]
    let schema_code = {
        let joined = members
            .iter()
            .map(|(_, zod, _, _)| zod.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        if members.iter().any(|&(_, _, _, flattened)| flattened) {
            format!("z.union([{joined}])")
        } else {
            format!("z.discriminatedUnion(\"{tag_name}\", [{joined}])")
        }
    };
    #[cfg(not(feature = "zod"))]
    let schema_code = String::new();

    (main_schema_code, type_code, schema_code)
}

/// Processes an internally tagged enum (`#[serde(tag = "...")]` with no `content`) and generates
/// its definitions.
#[cfg(feature = "serde")]
fn process_internally_tagged_enum(
    mut item_enum: syn::ItemEnum,
    name: &syn::Ident,
    tag_name: &str,
    rename_all: Option<&str>,
    item_name: &str,
    args: &ModelSchemaArgs,
) -> TokenStream {
    #[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
    // Every other enum shape is written as a union of what its variants render as.
    let (module_name, module_ident) =
        enum_module_idents(name, item_name, AliasKind::NoEnumMembers, Surface::union());

    #[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
    let docs_vec = get_enum_docs(&item_enum);

    #[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
    let enum_module_name_opt = Some(module_name.as_str());
    #[cfg(not(any(feature = "typescript", feature = "zod", feature = "jsonschema")))]
    let enum_module_name_opt = None;

    if let Some(output) = guard_failure_output(
        &item_enum,
        Some(&item_enum.ident),
        &internally_tagged_guard_errors(&item_enum, tag_name),
    ) {
        return output;
    }

    let variants = collect_discriminated_variants(&mut item_enum, rename_all, enum_module_name_opt);
    if let Some(output) = guard_failure_output(&item_enum, Some(&item_enum.ident), &variants.2) {
        return output;
    }

    let members: Vec<(String, String, proc_macro2::TokenStream, bool)> = variants
        .0
        .iter()
        .map(|variant| render_internal_variant(variant, tag_name, &item_enum.ident.to_string()))
        .collect();

    let (main_schema_code, type_code, schema_code) = join_internal_union(&members, tag_name);
    #[cfg(not(feature = "jsonschema"))]
    let _: &_ = &main_schema_code;
    #[cfg(not(feature = "typescript"))]
    let _: &_ = &type_code;
    #[cfg(not(feature = "zod"))]
    let _: &_ = &schema_code;
    #[cfg(not(any(feature = "typescript", feature = "zod", feature = "jsonschema")))]
    let _: &_ = &(name, args);

    #[cfg(feature = "typescript")]
    let docs = build_jsdoc_body(docs_vec.as_deref(), item_name);

    #[cfg(feature = "jsonschema")]
    let json_schema_method =
        enum_json_schema_methods(&main_schema_code, item_name, &item_enum.generics, args);

    #[cfg(feature = "typescript")]
    let ts_definition_method = generate_discriminated_enum_ts_definition_method(
        &docs,
        item_name,
        &name.to_string(),
        &ts_generic_params(&item_enum.generics),
        &type_code,
    );

    #[cfg(feature = "zod")]
    let zod_schema_method = generate_discriminated_enum_zod_schema_method(
        item_name,
        &name.to_string(),
        &type_parameters_in_scope(&item_enum.generics),
        &args.default_types,
        &schema_code,
    );

    #[cfg(not(any(feature = "typescript", feature = "zod")))]
    let _: &_ = &item_name;

    #[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
    let schema_example_method =
        enum_schema_example_method(docs_vec.as_deref(), name, &item_enum.generics, args);

    #[cfg(any(feature = "zod", feature = "typescript", feature = "jsonschema"))]
    let schema_impl_items: Vec<proc_macro2::TokenStream> = vec![
        #[cfg(feature = "jsonschema")]
        json_schema_method,
        #[cfg(feature = "typescript")]
        ts_definition_method,
        #[cfg(feature = "zod")]
        zod_schema_method,
    ];

    #[cfg(any(feature = "zod", feature = "typescript", feature = "jsonschema"))]
    let delegate_impl_items = build_struct_delegate_items(
        &module_ident,
        item_name,
        &name.to_string(),
        &type_parameters_in_scope(&item_enum.generics),
        schema_example_method.as_ref(),
    );

    #[cfg(any(feature = "zod", feature = "typescript", feature = "jsonschema"))]
    {
        assemble_schema_output(&SchemaOutputParts {
            default_types: &args.default_types,
            delegate_impl_items: &delegate_impl_items,
            generics: &item_enum.generics,
            item: &item_enum,
            module_ident: &module_ident,
            name,
            schema_impl_items: &schema_impl_items,
            validate_method: &build_enum_validate_method(&variants.3, &module_ident),
            validation_fns: &variants.1,
        })
    }

    #[cfg(not(any(feature = "zod", feature = "typescript", feature = "jsonschema")))]
    {
        let _: &_ = &(&variants.1, &variants.3);
        let output = quote! {
            #item_enum
        };
        log::trace!("{output}");
        TokenStream::from(output)
    }
}

/// Renders one variant of an untagged enum as a union member.
#[cfg(feature = "serde")]
fn render_untagged_variant(
    kind: &VariantKind,
    variant: &syn::Variant,
    field_defs: &[FieldDef],
    self_type_name: &str,
) -> Result<(String, String, proc_macro2::TokenStream), syn::Error> {
    match (kind, field_defs) {
        (VariantKind::TupleSingle, [fld]) => Ok(render_untagged_tuple_single(fld, self_type_name)),
        (VariantKind::Named, _) => Ok(render_untagged_named(field_defs, self_type_name)),
        (VariantKind::Unit, _) if lone_slot_off_wire(variant) => {
            Err(collapsed_untagged_variant_error(variant))
        }
        (VariantKind::Unit, _) => Err(unsupported_untagged_variant_error(
            variant,
            "a unit variant",
        )),
        (VariantKind::TupleSingle | VariantKind::TupleMultiple, _) => {
            Err(unsupported_untagged_variant_error(
                variant,
                &format!("a tuple variant with {} fields", variant.fields.len()),
            ))
        }
    }
}

/// Refuses an untagged variant that reached the unit wire by taking its lone slot off it, spanned on
/// the variant.
#[cfg(feature = "serde")]
fn collapsed_untagged_variant_error(variant: &syn::Variant) -> syn::Error {
    let variant_name = &variant.ident;
    syn::Error::new_spanned(
        variant,
        format!(
            "model_schema: variant `{variant_name}`: the lone slot of `{variant_name}` is off the \
             wire in both directions, so serde writes and reads the variant as the bare `null` a \
             unit variant is written as, whatever the slot holds — and an untagged union has no \
             member spelling for `null`: a member is written as its inner type or as an object of \
             named fields, and the slot this one would have been written as never reaches the wire. \
             Keep the slot on the wire so the member has a value to be written as, or remove \
             `{variant_name}` from the union."
        ),
    )
}

/// Refuses a variant shape the untagged union has no member spelling for, spanned on the variant.
#[cfg(feature = "serde")]
fn unsupported_untagged_variant_error(variant: &syn::Variant, shape: &str) -> syn::Error {
    let variant_name = &variant.ident;
    syn::Error::new_spanned(
        variant,
        format!(
            "model_schema: variant `{variant_name}`: `#[serde(untagged)]` supports newtype \
             (`V(T)`) and struct (`V {{ … }}`) variants only — `{variant_name}` is {shape}, which \
             the union has no member spelling for: a member is written as the inner type or as an \
             object of named fields, and a variant that is neither has nothing to be written as. \
             Give it a single inner type or named fields, or drop `#[serde(untagged)]`."
        ),
    )
}

/// Renders a `TupleSingle` (`S(T)`) untagged variant as a union member (`T` / `T$Schema` / value).
///
/// The inner value is a slot: untagged, the variant carries no key of its own, so the content *is*
/// the whole serialized value and a `None` there reaches the wire as a bare `null` rather than
/// going absent. All three surfaces read it through the slot spellings for that reason.
#[cfg(feature = "serde")]
fn render_untagged_tuple_single(
    fld: &FieldDef,
    self_type_name: &str,
) -> (String, String, proc_macro2::TokenStream) {
    let ts = fld.typescript_slot_typename();

    #[cfg(feature = "zod")]
    let zod = fld.zod_slot_type();
    #[cfg(not(feature = "zod"))]
    let zod = String::new();

    #[cfg(feature = "jsonschema")]
    let json_val = nullable_slot_json_schema_value(fld, field_json_schema_value(fld));
    #[cfg(not(feature = "jsonschema"))]
    let json_val = quote! {};

    let _: &str = self_type_name;
    (ts, zod, json_val)
}

/// Renders a `Named` (`{ a: A }`) untagged variant as a union member (object type / strictObject /
/// object schema).
#[cfg(feature = "serde")]
fn render_untagged_named(
    field_defs: &[FieldDef],
    self_type_name: &str,
) -> (String, String, proc_macro2::TokenStream) {
    let ts_fields = field_defs
        .iter()
        .map(|fld| {
            format!(
                "{}{}: {}",
                ts_member_key(&fld.name),
                fld.optional_key_marker(),
                fld.typescript_typename()
            )
        })
        .collect::<Vec<_>>()
        .join("; ");
    let ts = format!("{{ {ts_fields} }}");

    #[cfg(feature = "zod")]
    let zod = {
        let mut body = String::from("z.strictObject({ ");
        for fld in field_defs {
            let zod_field_type = fld.zod_type();
            let key = ts_member_key(&fld.name);
            if fld.contains_type_reference(self_type_name) {
                let _ = write!(body, "get {key}() {{ return {zod_field_type}; }}, ");
            } else {
                let _ = write!(body, "{key}: {zod_field_type}, ");
            }
        }
        body.push_str("})");
        body
    };
    #[cfg(not(feature = "zod"))]
    let zod = {
        let _: &str = self_type_name;
        String::new()
    };

    #[cfg(feature = "jsonschema")]
    let json_val = untagged_named_json_value(field_defs);
    #[cfg(not(feature = "jsonschema"))]
    let json_val = quote! {};

    (ts, zod, json_val)
}

/// The keys one untagged member names, and `None` where nothing here proves them.
///
/// A `Named` variant writes its own fields and is the whole of what it writes, so its keys are the
/// ones the member already spells. Every other member is written as the type it names — the merge
/// reads what that name published for the questions it can ask a registry, and its key list is not
/// one of them.
#[cfg(all(feature = "serde", feature = "typescript"))]
fn untagged_member_keys(kind: &VariantKind, field_defs: &[FieldDef]) -> Option<Vec<String>> {
    matches!(kind, VariantKind::Named)
        .then(|| field_defs.iter().map(|fld| fld.name.clone()).collect())
}

/// One untagged member's flatten operand with its siblings' keys marked absent, written in the
/// one-line shape [`render_untagged_named`] already wrote the member in.
///
/// A member that is no object literal keeps its spelling, which is the same answer
/// [`untagged_member_keys`] gives it: a name is what it is written as, and nothing here proves what
/// it would have to be told to leave out.
#[cfg(all(feature = "serde", feature = "typescript"))]
fn close_untagged_flatten_member(member: &str, excluded: &[String]) -> String {
    if excluded.is_empty() {
        return member.to_owned();
    }
    let Some(keys) = member.strip_suffix(" }") else {
        return member.to_owned();
    };
    let mut closed = keys.trim_end().to_owned();
    for key in excluded {
        if !closed.ends_with('{') {
            closed.push(';');
        }
        let _ = write!(closed, " {}?: never", ts_member_key(key));
    }
    closed.push_str(" }");
    closed
}

/// Builds the `{ type: object, properties, required, additionalProperties: false }` JSON-schema
/// value token for a `Named` untagged variant.
#[cfg(all(feature = "serde", feature = "jsonschema"))]
fn untagged_named_json_value(field_defs: &[FieldDef]) -> proc_macro2::TokenStream {
    let property_inserts = field_defs.iter().map(|fld| {
        let name_str = fld.name.clone();
        let value = field_json_schema_value(fld);
        let required_insert = if fld.key_is_required() {
            quote! {
                required.push(serde_json::Value::String(#name_str.to_string()));
            }
        } else {
            quote! {}
        };
        quote! {
            properties.insert(#name_str.to_string(), #value);
            #required_insert
        }
    });
    quote! {
        {
            let mut object_schema = serde_json::Map::new();
            object_schema.insert(
                "type".to_string(),
                serde_json::Value::String("object".to_string()),
            );
            let mut properties = serde_json::Map::new();
            let mut required: Vec<serde_json::Value> = Vec::new();
            #(#property_inserts)*
            object_schema.insert(
                "properties".to_string(),
                serde_json::Value::Object(properties),
            );
            object_schema.insert(
                "required".to_string(),
                serde_json::Value::Array(required),
            );
            object_schema.insert(
                "additionalProperties".to_string(),
                serde_json::Value::Bool(false),
            );
            serde_json::Value::Object(object_schema)
        }
    }
}

/// Builds the `{ "type": "string", ... }` JSON-schema value token for a `String` field, including
/// any `pattern` / `minLength` / `maxLength` constraints from its `model_schema_prop` metadata.
#[cfg(all(feature = "serde", feature = "jsonschema"))]
fn string_field_json_schema_value(fld: &FieldDef) -> proc_macro2::TokenStream {
    let meta = fld.model_schema_prop_meta.as_ref();
    let pattern_insert = meta.and_then(|m| m.pattern.clone()).map(|p| {
        quote! {
            string_schema.insert(
                "pattern".to_string(),
                serde_json::Value::String(#p.to_string()),
            );
        }
    });
    let min_insert = meta.and_then(|m| m.min_length).map(|n| {
        let len = n as u64;
        quote! {
            string_schema.insert(
                "minLength".to_string(),
                serde_json::Value::Number(serde_json::Number::from(#len)),
            );
        }
    });
    let max_insert = meta.and_then(|m| m.max_length).map(|n| {
        let len = n as u64;
        quote! {
            string_schema.insert(
                "maxLength".to_string(),
                serde_json::Value::Number(serde_json::Number::from(#len)),
            );
        }
    });
    quote! {
        ({
            let mut string_schema = serde_json::Map::new();
            string_schema.insert(
                "type".to_string(),
                serde_json::Value::String("string".to_string()),
            );
            #pattern_insert
            #min_insert
            #max_insert
            serde_json::Value::Object(string_schema)
        })
    }
}

/// Builds a standalone `serde_json::Value` token expression for a single field, with `Vec<T>`
/// array wrapping. Sibling type of [`flatten_field_json_schema_ref`]; used by untagged enum
/// members where the JSON value is consumed directly (not inserted under a property name).
#[cfg(all(feature = "serde", feature = "jsonschema"))]
fn field_json_schema_value(fld: &FieldDef) -> proc_macro2::TokenStream {
    // A covered sequence wrapper writes the JSON array of its element, so the member is dispatched
    // as the arrayed element it stands for — through the seam field position reads it through, and
    // the array levels it carries are the whole field's, which is why it replaces this call rather
    // than being wrapped again.
    if let Some(element_field) = sequence_wrapper_field(fld) {
        return field_json_schema_value(&element_field);
    }

    let inner = match &fld.field_type {
        FieldDefType::SiblingType(name, arguments) => {
            sibling_json_schema_value(name, arguments, fld.type_span)
        }
        FieldDefType::String => string_field_json_schema_value(fld),
        FieldDefType::Char => {
            quote! { serde_json::json!({ "type": "string", "minLength": 1, "maxLength": 1 }) }
        }
        FieldDefType::StringLiteral(literal) => {
            quote! { serde_json::json!({ "type": "string", "const": #literal }) }
        }
        FieldDefType::U8
        | FieldDefType::U16
        | FieldDefType::U32
        | FieldDefType::U64
        | FieldDefType::I8
        | FieldDefType::I16
        | FieldDefType::I32
        | FieldDefType::I64
        | FieldDefType::Usize
        | FieldDefType::Isize
        | FieldDefType::F32
        | FieldDefType::F64
        | FieldDefType::Boolean => {
            // The arm has matched exactly the types the mapping answers a keyword for.
            let keyword = scalar_json_type_keyword(&fld.field_type).unwrap();
            quote! { serde_json::json!({ "type": #keyword }) }
        }
        #[cfg(feature = "object_id")]
        FieldDefType::ObjectId => object_id_json_schema_value(&object_id_hex_json_schema()),
        #[cfg(feature = "chrono")]
        FieldDefType::NaiveDate
        | FieldDefType::NaiveTime
        | FieldDefType::NaiveDateTime
        | FieldDefType::DateTime => {
            // The arm has matched exactly the types the mapping answers for.
            let item = chrono_json_schema_item(&fld.field_type).unwrap();
            quote! { serde_json::json!(#item) }
        }
        FieldDefType::Map(key, value) => match map_json_schema_value(key, value) {
            Ok(map_schema) => map_schema,
            Err(rejection) => return member_rejection_value(&fld.name, &rejection),
        },
        FieldDefType::Tuple(elements) => match tuple_json_schema_value(elements) {
            Ok(tuple_schema) => tuple_schema,
            Err(rejection) => return member_rejection_value(&fld.name, &rejection),
        },
        // A parameter is the filling that reached it, and a value the expansion cannot name at all
        // admits any value — the permissive empty schema, as in field position.
        FieldDefType::TypeParam(_) | FieldDefType::Unknown => opaque_json_schema_value(fld),
    };

    arrayed_json_schema_value(fld, inner)
}

/// [`map_member_rejection_error`] for a position that holds a value rather than writing a
/// statement — an untagged variant's member, whose value is consumed straight into the union's
/// `anyOf`.
#[cfg(all(feature = "serde", feature = "jsonschema"))]
fn member_rejection_value(
    field_name: &str,
    rejection: &MapMemberRejection,
) -> proc_macro2::TokenStream {
    let message = map_member_rejection_message(&field_label(field_name), rejection);
    quote! { compile_error!(#message) }
}

/// One untagged member's field def in the two readings the guards need: as the surfaces will
/// render it, every reference to one of the enclosing item's own parameters already the opaque
/// value, and as the author spelled it. [`process_field`] reads the same pair off the same erase.
#[cfg(feature = "serde")]
fn untagged_member_field_defs(
    field: &Field,
    field_name: &str,
    type_parameters: &[String],
) -> (FieldDef, FieldDef) {
    let written = get_field_def(field_name, &field.ty, "");
    let mut rendered = written.clone();
    rendered.erase_type_parameters(type_parameters);
    (rendered, written)
}
/// The [`FieldContext`] one untagged variant hands each of its members: the variant is the
/// container the omission is read against, and the enum is the type the names resolve in.
#[cfg(feature = "serde")]
const fn untagged_variant_context<'ctx>(
    variant_defaulted: bool,
    schema_module_name: Option<&'ctx str>,
    enum_type_name: &'ctx str,
    type_parameters: &'ctx [String],
    variant_name: &'ctx str,
) -> FieldContext<'ctx> {
    FieldContext {
        container_defaulted: variant_defaulted,
        rename_all: None,
        schema_module_name,
        type_name: enum_type_name,
        type_parameters,
        variant_ident: Some(variant_name),
    }
}

/// One untagged member's finished def, beside the guard verdicts it earned: built next to its
/// written twin, stamped, guarded (serde's checks and this crate's own combined), resolved, and
/// carrying its meta — the whole of what the collector's loop needs back for the field.
///
/// The context's `container_defaulted` is the variant's own `#[serde(default)]`: a struct variant
/// is where serde accepts one, and it is the container this member's omission is read against.
#[cfg(feature = "serde")]
fn untagged_member_field_def(
    ctx: &FieldContext<'_>,
    field: &Field,
    field_name: &str,
    prop_meta: ModelSchemaPropMeta,
    serde_field_meta: &SerdeFieldMeta,
    positional_constraint_error: Option<proc_macro2::TokenStream>,
) -> (FieldDef, Vec<proc_macro2::TokenStream>) {
    let (mut field_def, written_def) =
        untagged_member_field_defs(field, field_name, ctx.type_parameters);
    apply_serde_key_omission(&mut field_def, field);
    let serde_guard_errors = field_guard_errors(
        field,
        field_name,
        field_def.is_optional(),
        serde_field_meta,
        ctx.container_defaulted,
        positional_constraint_error,
    );
    let member_guard_errors = collect_field_guard_errors(
        field,
        &field_def,
        &written_def,
        field_name,
        &prop_meta,
        serde_guard_errors,
    );
    field_def.resolve_self_references(ctx.type_name, ctx.type_parameters);
    apply_model_schema_prop_meta(&mut field_def, prop_meta, field_name);
    (field_def, member_guard_errors)
}

/// Collects each untagged variant's union-member parts: the TypeScript member type, the Zod member
/// schema, and the JSON-schema value token, plus the `compile_error!` tokens for any field-level
/// guard violations. The first three are always returned; the caller selects which to use based on
/// the enabled features.
#[cfg(feature = "serde")]
fn collect_untagged_variant_members(
    variant: &mut syn::Variant,
    enum_type_name: &str,
    schema_module_name: Option<&str>,
    type_parameters: &[String],
) -> UntaggedVariantMembers {
    let variant_name = variant.ident.to_string();
    let variant_defaulted = has_serde_default(&variant.attrs);
    let mut walked = UntaggedVariantMembers {
        bound: Vec::new(),
        checks: Vec::new(),
        deferred_attrs: Vec::new(),
        field_defs: Vec::new(),
        guard_errors: Vec::new(),
        validation_fns: Vec::new(),
    };

    for (index, field) in variant.fields.iter_mut().enumerate() {
        let omission = parse_serde_key_omission(&field.attrs);
        let positional = field.ident.is_none();
        if let Err(rejection) = check_variant_slot_wire_is_readable(
            field,
            index,
            &variant_name,
            enum_type_name,
            omission,
        ) {
            walked.guard_errors.push(rejection.to_compile_error());
        }
        let field_name = field_ident_string(field);
        let prop_meta = parse_model_schema_prop_attributes(&field.attrs);
        let serde_field_meta = parse_serde_field_attributes(&field.attrs);

        let new_attrs = declaration_attrs(field);
        let mut injected_attrs: Vec<syn::Attribute> = Vec::new();
        let (validation_fn, validate_body, positional_constraint_error) = generate_field_validation(
            field,
            schema_module_name,
            &field_name,
            Some(&variant_name),
            &prop_meta,
            &mut injected_attrs,
        );
        field.attrs = new_attrs;
        walked.deferred_attrs.push(injected_attrs);
        walked.validation_fns.extend(validation_fn);
        if let (Some(body), Some(ident)) = (validate_body, field.ident.as_ref()) {
            walked.bound.push((ident.clone(), member_binding(ident)));
            walked.checks.push(body);
        }

        let member_ctx = untagged_variant_context(
            variant_defaulted,
            schema_module_name,
            enum_type_name,
            type_parameters,
            &variant_name,
        );
        let (member_def, member_guard_errors) = untagged_member_field_def(
            &member_ctx,
            field,
            &field_name,
            prop_meta,
            &serde_field_meta,
            positional_constraint_error,
        );
        walked.guard_errors.extend(member_guard_errors);
        if positional && omission.absent_from_wire() {
            continue;
        }
        push_described_field(&mut walked.field_defs, member_def);
    }
    walked
}

#[cfg(feature = "serde")]
fn collect_untagged_members(
    item_enum: &mut syn::ItemEnum,
    schema_module_name: Option<&str>,
) -> UntaggedMemberData {
    let enum_type_name = item_enum.ident.to_string();
    let type_parameters = type_parameters_in_scope(&item_enum.generics);
    let mut ts_parts: Vec<String> = Vec::new();
    #[cfg(feature = "typescript")]
    let mut ts_member_keys: Vec<Option<Vec<String>>> = Vec::new();
    let mut zod_parts: Vec<String> = Vec::new();
    #[cfg(feature = "zod")]
    let mut zod_merge_parts: ZodMergeParts = Vec::new();
    #[cfg(not(feature = "zod"))]
    let zod_merge_parts: ZodMergeParts = Vec::new();
    let mut json_parts: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut guard_errors: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut validation_fns: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut per_variant_checks: Vec<(proc_macro2::TokenStream, Vec<proc_macro2::TokenStream>)> =
        Vec::new();
    let mut deferred_attrs: Vec<Vec<syn::Attribute>> = Vec::new();

    for variant in &mut item_enum.variants {
        let declared_kind = classify_variant(variant);
        let kind = variant_wire_kind(variant);
        let total_fields = variant.fields.len();
        let mut walked = collect_untagged_variant_members(
            variant,
            &enum_type_name,
            schema_module_name,
            &type_parameters,
        );
        guard_errors.append(&mut walked.guard_errors);
        validation_fns.append(&mut walked.validation_fns);
        deferred_attrs.append(&mut walked.deferred_attrs);
        let field_defs = walked.field_defs;

        per_variant_checks.push((
            variant_check_pattern(&variant.ident, &declared_kind, total_fields, &walked.bound),
            walked.checks,
        ));

        match render_untagged_variant(&kind, variant, &field_defs, &enum_type_name) {
            Ok((ts, zod, json_val)) => {
                #[cfg(feature = "zod")]
                zod_merge_parts.extend(zod_merge_branches(
                    &kind,
                    &field_defs,
                    &zod,
                    ts_parts.len() + 1,
                ));
                #[cfg(feature = "typescript")]
                ts_member_keys.push(untagged_member_keys(&kind, &field_defs));
                ts_parts.push(ts);
                zod_parts.push(zod);
                json_parts.push(json_val);
            }
            Err(err) => guard_errors.push(err.to_compile_error()),
        }
    }

    if guard_errors.is_empty() {
        apply_deferred_field_attrs(
            item_enum
                .variants
                .iter_mut()
                .flat_map(|variant| variant.fields.iter_mut()),
            deferred_attrs,
        );
    }

    #[cfg(feature = "typescript")]
    let ts_merge_parts = ts_flatten_operands(&ts_parts, &ts_member_keys);
    #[cfg(not(feature = "typescript"))]
    let ts_merge_parts: Vec<String> = Vec::new();

    (
        ts_parts,
        ts_merge_parts,
        zod_parts,
        zod_merge_parts,
        json_parts,
        guard_errors,
        validation_fns,
        build_member_check_arms(per_variant_checks),
    )
}

/// What an object flattening an untagged enum spells in the enum's name's place, one operand per
/// member — and nothing where the name already spells the same payload set.
#[cfg(all(feature = "serde", feature = "typescript"))]
fn ts_flatten_operands(members: &[String], member_keys: &[Option<Vec<String>>]) -> Vec<String> {
    let operands: Vec<String> = sibling_key_exclusions(member_keys)
        .iter()
        .zip(members)
        .map(|(excluded, member)| close_untagged_flatten_member(member, excluded))
        .collect();
    if operands == members {
        Vec::new()
    } else {
        operands
    }
}

/// What one rendered member contributes to an object that merges the enum: the members of the union
/// it names, when it names one the registry has recorded, and the member as rendered otherwise.
#[cfg(all(feature = "serde", feature = "zod"))]
fn zod_merge_branches(
    kind: &VariantKind,
    field_defs: &[FieldDef],
    rendered: &str,
    branch: usize,
) -> Vec<ZodUnionMember> {
    let leaves = match (kind, field_defs) {
        (VariantKind::TupleSingle, [fld]) => zod_member_leaves(fld, rendered),
        _ => vec![ZodUnionMember {
            branch: Vec::new(),
            non_object: None,
            spelling: rendered.to_owned(),
        }],
    };
    leaves
        .into_iter()
        .map(|leaf| {
            let mut trail = vec![branch];
            trail.extend(leaf.branch);
            ZodUnionMember {
                branch: trail,
                non_object: leaf.non_object,
                spelling: leaf.spelling,
            }
        })
        .collect()
}

/// What one rendered member contributes below its own position: one entry for the member itself,
/// the leaves of the union it names when it names one the registry has recorded, and — where it was
/// written under an `Option` — those under the choice the value is, beside the choice the absence
/// is.
#[cfg(all(feature = "serde", feature = "zod"))]
fn zod_member_leaves(fld: &FieldDef, rendered: &str) -> Vec<ZodUnionMember> {
    let nested = fld.zod_union_members();
    let mut leaves = if !nested.is_empty() {
        nested
    } else if let Some(published) = named_wire_leaves(fld) {
        // The name stands for a choice of its own. Its leaves carry the trail, and the spelling
        // stays the member's: the operand a merge would join is still the one binding the name
        // publishes, whatever the document behind it branches into.
        published
            .into_iter()
            .map(|leaf| ZodUnionMember {
                branch: leaf.branch,
                non_object: leaf.non_object,
                spelling: rendered.to_owned(),
            })
            .collect()
    } else {
        vec![ZodUnionMember {
            branch: Vec::new(),
            non_object: written_non_object_wire(fld),
            spelling: rendered.to_owned(),
        }]
    };
    if fld.is_optional() {
        for leaf in &mut leaves {
            leaf.branch.insert(0, 1);
        }
        leaves.push(ZodUnionMember {
            branch: vec![2],
            non_object: Some("null"),
            spelling: rendered.to_owned(),
        });
    }
    leaves
}

/// What serde writes a written type as, when that type proves it is not an object, and `None` when
/// nothing here proves it. Asked of an untagged union's member and of the whole surface an item
/// publishes alike, which is what keeps a member and the name standing for it on one answer.
#[cfg(all(feature = "serde", any(feature = "zod", feature = "typescript")))]
fn written_non_object_wire(fld: &FieldDef) -> Option<&'static str> {
    if fld.is_array() {
        return Some("array");
    }
    if matches!(fld.field_type, FieldDefType::Tuple(_)) {
        return Some("array");
    }
    if let FieldDefType::SiblingType(name, _) = &fld.field_type {
        return is_sequence_wrapper(name)
            .then_some("array")
            .or_else(|| registered_non_object_wire(name));
    }
    // Everything the two structural questions above leave is a value one keyword describes or
    // nothing here can name, which is exactly what the shared mapping answers.
    scalar_json_type_keyword(&fld.field_type)
}

/// What a `#[model_schema()]` item publishes, as leaves in the merge's vocabulary — the answer
/// [`crate::utils::record_wire_leaves`] stores for every name a flattened member can then reach
/// through.
#[cfg(all(feature = "serde", any(feature = "zod", feature = "typescript")))]
fn published_wire_leaves(written: &FieldDef) -> Vec<WireLeaf> {
    if written.is_optional() {
        let mut bare = written.clone();
        bare.nullable_levels
            .retain(|&level| level != bare.array_depth);
        let mut leaves = published_wire_leaves(&bare);
        for leaf in &mut leaves {
            leaf.branch.insert(0, 1);
        }
        leaves.push(WireLeaf {
            branch: vec![2],
            non_object: Some("null"),
        });
        return leaves;
    }
    named_wire_leaves(written).unwrap_or_else(|| {
        vec![WireLeaf {
            branch: Vec::new(),
            non_object: written_non_object_wire(written),
        }]
    })
}

/// The leaves an externally tagged enum's variants write, one per variant and in the order the
/// `oneOf` it publishes writes them.
#[cfg(all(feature = "serde", any(feature = "zod", feature = "typescript")))]
fn external_variant_wire_leaves(variants: &Punctuated<syn::Variant, Token![,]>) -> Vec<WireLeaf> {
    variants
        .iter()
        .enumerate()
        .map(|(index, variant)| WireLeaf {
            branch: vec![index + 1],
            non_object: matches!(variant_wire_kind(variant), VariantKind::Unit).then_some("string"),
        })
        .collect()
}

/// The leaves the item a field names published, where the field's whole wire *is* that name's and
/// one of those leaves proves serde writes no object at its position — and `None` everywhere the
/// one-leaf dispatch already answers.
#[cfg(all(feature = "serde", any(feature = "zod", feature = "typescript")))]
fn named_wire_leaves(written: &FieldDef) -> Option<Vec<WireLeaf>> {
    if written.is_array() {
        return None;
    }
    let FieldDefType::SiblingType(name, _) = &written.field_type else {
        return None;
    };
    let leaves = lookup_alias_info(name)?.wire;
    leaves
        .iter()
        .any(|leaf| leaf.non_object.is_some())
        .then_some(leaves)
}

/// Builds the schema module's impl items for an untagged enum: its JSON schema, its `TypeScript`
/// definition, and its Zod schema, in the order the module publishes them.
#[cfg(all(
    feature = "serde",
    any(feature = "zod", feature = "typescript", feature = "jsonschema")
))]
fn build_untagged_schema_impl_items(
    // The declaration itself rather than the pieces read off it: the name and the parameters are
    // read by different surfaces, and the JSON one reads the parameters beside the attribute.
    item_enum: &syn::ItemEnum,
    item_name: &str,
    docs_vec: Option<&[String]>,
    args: &ModelSchemaArgs,
    ts_parts: &[String],
    zod_parts: &[String],
    json_parts: &[proc_macro2::TokenStream],
) -> Vec<proc_macro2::TokenStream> {
    #[cfg(not(feature = "jsonschema"))]
    let _: &_ = &args;
    #[cfg(not(feature = "typescript"))]
    let _: &_ = &(ts_parts, docs_vec);
    #[cfg(not(feature = "zod"))]
    let _: &_ = &zod_parts;
    #[cfg(not(feature = "jsonschema"))]
    let _: &_ = &json_parts;

    #[cfg(feature = "jsonschema")]
    let main_schema_code = quote! {
        let mut schema_obj = serde_json::Map::new();
        schema_obj.insert("anyOf".to_string(), {
            let result: Vec<serde_json::Value> = vec![
                #(#json_parts), *
            ];

            serde_json::Value::Array(result)
        });

        serde_json::Value::Object(schema_obj)
    };

    #[cfg(feature = "typescript")]
    let type_code = ts_parts.join(" | ");

    #[cfg(feature = "zod")]
    let schema_code = format!("z.union([{}])", zod_parts.join(", "));

    #[cfg(feature = "typescript")]
    let docs = build_jsdoc_body(docs_vec, item_name);

    #[cfg(feature = "jsonschema")]
    let json_schema_method =
        enum_json_schema_methods(&main_schema_code, item_name, &item_enum.generics, args);

    #[cfg(feature = "typescript")]
    let ts_definition_method = generate_discriminated_enum_ts_definition_method(
        &docs,
        item_name,
        &item_enum.ident.to_string(),
        &ts_generic_params(&item_enum.generics),
        &type_code,
    );

    #[cfg(feature = "zod")]
    let zod_schema_method = generate_discriminated_enum_zod_schema_method(
        item_name,
        &item_enum.ident.to_string(),
        &type_parameters_in_scope(&item_enum.generics),
        &args.default_types,
        &schema_code,
    );

    vec![
        #[cfg(feature = "jsonschema")]
        json_schema_method,
        #[cfg(feature = "typescript")]
        ts_definition_method,
        #[cfg(feature = "zod")]
        zod_schema_method,
    ]
}

/// Processes an untagged enum (`#[serde(untagged)]`) and generates its definitions.
///
/// Emits a TypeScript union (`A | B`), a Zod `z.union([...])`, and a JSON-schema `anyOf`.
/// Mirrors [`process_discriminated_enum`]'s setup/assembly so all feature combinations compile.
#[cfg(feature = "serde")]
fn process_untagged_enum(
    mut item_enum: syn::ItemEnum,
    name: &syn::Ident,
    item_name: &str,
    args: &ModelSchemaArgs,
) -> TokenStream {
    // Compute the schema module name and register the enum so other types can find it.
    #[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
    // Every other enum shape is written as a union of what its variants render as.
    let (module_name, module_ident) =
        enum_module_idents(name, item_name, AliasKind::NoEnumMembers, Surface::union());

    // Extract docs early for example extraction
    #[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
    let docs_vec = get_enum_docs(&item_enum);

    #[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
    let enum_module_name_opt = Some(module_name.as_str());
    #[cfg(not(any(feature = "typescript", feature = "zod", feature = "jsonschema")))]
    let enum_module_name_opt = None;

    // Render each variant into its union member (TS / Zod / JSON parts).
    let (
        ts_parts,
        ts_merge_parts,
        zod_parts,
        zod_merge_parts,
        json_parts,
        guard_errors,
        enum_validation_fns,
        validate_arms,
    ) = collect_untagged_members(&mut item_enum, enum_module_name_opt);

    // A violated field guard makes the whole contract unsound, so the schema surface is dropped
    // and only the original item plus the errors are emitted.
    if let Some(output) = guard_failure_output(&item_enum, Some(&item_enum.ident), &guard_errors) {
        return output;
    }

    // Recorded only past the guards, so nothing merges an enum whose own schema was dropped.
    #[cfg(feature = "zod")]
    record_zod_union_members(&name.to_string(), &zod_merge_parts);
    #[cfg(not(feature = "zod"))]
    let _: &_ = &zod_merge_parts;
    #[cfg(feature = "typescript")]
    record_ts_union_members(&name.to_string(), &ts_merge_parts);
    #[cfg(not(feature = "typescript"))]
    let _: &_ = &ts_merge_parts;

    #[cfg(not(any(feature = "typescript", feature = "zod", feature = "jsonschema")))]
    let _: &_ = &(name, item_name, &ts_parts, &zod_parts, &json_parts, args);

    #[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
    let schema_example_method =
        enum_schema_example_method(docs_vec.as_deref(), name, &item_enum.generics, args);

    #[cfg(any(feature = "zod", feature = "typescript", feature = "jsonschema"))]
    let schema_impl_items = build_untagged_schema_impl_items(
        &item_enum,
        item_name,
        docs_vec.as_deref(),
        args,
        &ts_parts,
        &zod_parts,
        &json_parts,
    );

    // The union's own accessor is where a violated member bound is named in Rust at all: the read
    // path hands the bound to serde, which drops the sentence with the candidate it removes (see
    // `collect_untagged_members`), so this is the one surface that answers with the constraint
    // rather than with `data did not match any variant`.
    #[cfg(any(feature = "zod", feature = "typescript", feature = "jsonschema"))]
    let validate_method = build_enum_validate_method(&validate_arms, &module_ident);
    #[cfg(any(feature = "zod", feature = "typescript", feature = "jsonschema"))]
    let delegate_impl_items = build_struct_delegate_items(
        &module_ident,
        item_name,
        &name.to_string(),
        &type_parameters_in_scope(&item_enum.generics),
        schema_example_method.as_ref(),
    );

    #[cfg(any(feature = "zod", feature = "typescript", feature = "jsonschema"))]
    {
        assemble_schema_output(&SchemaOutputParts {
            default_types: &args.default_types,
            delegate_impl_items: &delegate_impl_items,
            generics: &item_enum.generics,
            item: &item_enum,
            module_ident: &module_ident,
            name,
            schema_impl_items: &schema_impl_items,
            validate_method: &validate_method,
            validation_fns: &enum_validation_fns,
        })
    }

    #[cfg(not(any(feature = "zod", feature = "typescript", feature = "jsonschema")))]
    {
        let _: &_ = &(&enum_validation_fns, &validate_arms);
        let output = quote! {
            #item_enum
        };
        log::trace!("{output}");
        TokenStream::from(output)
    }
}

#[cfg(feature = "jsonschema")]
fn generate_type_schema(
    fld: &FieldDef,
    field_name_str: &str,
    type_json_schema: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let schema = arrayed_json_schema_value(fld, type_json_schema.clone());
    quote! {
        properties.insert(#field_name_str.to_string(), #schema);
    }
}

/// The open TypeScript and Zod member every variant of a tagged enum starts from: the tag holding
/// the variant's discriminator, with room for whatever the variant writes beside it.
///
/// Both tagged forms open the same way — the tag is a key of the variant's own object — so the
/// spelling lives here once rather than in each form's renderer.
fn tagged_variant_parts(
    tag_name: &str,
    discriminator_value: &str,
    discriminator_docs: &str,
) -> VariantParts {
    let tag_key = ts_member_key(tag_name);
    VariantParts {
        json_fields: Vec::new(),
        optional_fields: Vec::new(),
        schema_code: format!("{{\n  {tag_key}: z.literal(\"{discriminator_value}\"),\n"),
        type_code: format!(
            "{{\n{}\n  {tag_key}: \"{discriminator_value}\";\n",
            member_jsdoc_block(discriminator_docs)
        ),
    }
}

/// The closed object a tagged variant describes as, as a `serde_json::Map` expression: the tag
/// pinned to the variant's discriminator, plus whatever the variant's own fields contributed.
///
/// A `Map` rather than a `Value` because the internally tagged form merges further members into it
/// before it becomes one.
#[cfg(feature = "jsonschema")]
fn tagged_variant_json_object(
    tag_name: &str,
    discriminator_value: &str,
    json_fields: &[proc_macro2::TokenStream],
) -> proc_macro2::TokenStream {
    let discriminator_value_str = discriminator_value.to_owned();
    let tag_name_str = tag_name.to_owned();
    quote! {
        {
            let mut schema_obj = serde_json::Map::new();
            schema_obj.insert(
                "additionalProperties".to_string(),
                serde_json::Value::Bool(false),
            );
            let mut properties = serde_json::Map::new();
            let mut required = Vec::new();

            properties.insert(
                #tag_name_str.to_string(),
                serde_json::json!({
                    "type": "string",
                    "const": #discriminator_value_str,
                }),
            );
            required.push(serde_json::Value::String(#tag_name_str.to_string()));

            #(#json_fields)*

            schema_obj.insert(
                "properties".to_string(),
                serde_json::Value::Object(properties),
            );

            schema_obj.insert("required".to_string(), serde_json::Value::Array(required));

            schema_obj
        }
    }
}

/// Generates TypeScript and Zod schema code for a discriminated enum variant.
fn generate_variant_code(
    tag_name: &str,
    content_name: &str,
    discriminator_value: &str,
    field_defs: &[FieldDef],
    variant_kind: &VariantKind,
    discriminator_docs: &str,
    self_type_name: &str,
) -> (String, String, Vec<String>, proc_macro2::TokenStream) {
    let mut parts = tagged_variant_parts(tag_name, discriminator_value, discriminator_docs);

    match variant_kind {
        VariantKind::Unit => {
            // Unit variant: no additional fields beyond the discriminator
            // TypeScript: { type: "Variant" }
            // Zod: { type: z.literal("Variant") }
        }
        VariantKind::Named => {
            write_named_variant_fields(field_defs, Some(tag_name), self_type_name, &mut parts);
        }
        VariantKind::TupleSingle => {
            write_tuple_single_variant_fields(field_defs, content_name, self_type_name, &mut parts);
        }
        VariantKind::TupleMultiple => {
            write_tuple_multiple_variant_fields(
                field_defs,
                content_name,
                self_type_name,
                &mut parts,
            );
        }
    }

    // Complete the type and schema code
    parts.type_code.push('}');
    parts.schema_code.push('}');

    #[cfg(feature = "jsonschema")]
    let json_schema_variant = {
        let object = tagged_variant_json_object(tag_name, discriminator_value, &parts.json_fields);
        quote! { serde_json::Value::Object(#object) }
    };

    #[cfg(not(feature = "jsonschema"))]
    let json_schema_variant = quote! {};

    (
        parts.type_code,
        parts.schema_code,
        parts.optional_fields,
        json_schema_variant,
    )
}

/// Writes the named-field portion of an enum variant (TypeScript, Zod, JSON Schema).
///
/// `tag_name` is the key the discriminator occupies beside these fields, whose JSON-schema entry
/// the caller has already written; `None` where the variant's fields sit in an object of their own
/// and no key is taken.
fn write_named_variant_fields(
    field_defs: &[FieldDef],
    tag_name: Option<&str>,
    self_type_name: &str,
    parts: &mut VariantParts,
) {
    let variant_type_code = &mut parts.type_code;
    let variant_schema_code = &mut parts.schema_code;
    let optional_fields = &mut parts.optional_fields;
    let json_schema_variant_fields = &mut parts.json_fields;
    for fld in field_defs {
        let key = ts_member_key(&fld.name);

        let _ = writeln!(
            variant_type_code,
            "{}\n  {}{}: {};",
            member_jsdoc_block(&fld.docs),
            key,
            fld.optional_key_marker(),
            fld.typescript_typename()
        );

        #[cfg(feature = "zod")]
        {
            let zod_field_type = fld.zod_type();
            let is_recursive = fld.contains_type_reference(self_type_name);

            if is_recursive {
                let _ = writeln!(
                    variant_schema_code,
                    "  get {key}() {{ return {zod_field_type}; }},"
                );
            } else {
                let _ = writeln!(variant_schema_code, "  {key}: {zod_field_type},");
            }
        }

        #[cfg(not(feature = "zod"))]
        {
            let _: &_ = &(&variant_schema_code, self_type_name);
        }

        #[cfg(feature = "jsonschema")]
        if tag_name != Some(fld.name.as_str()) {
            json_schema_variant_fields.push(build_field_schema(fld));
        }
        #[cfg(not(feature = "jsonschema"))]
        let _: &_ = &(tag_name, &json_schema_variant_fields);

        if fld.is_optional() {
            optional_fields.push(fld.name.clone());
        }
    }
}

/// Pushes the JSON-schema property/required entries for a single-element tuple variant value.
#[cfg(feature = "jsonschema")]
fn push_single_tuple_json_field(
    json_schema_variant_fields: &mut Vec<proc_macro2::TokenStream>,
    content_name: &str,
    fld: &FieldDef,
) {
    let content_name_str = content_name.to_owned();
    let field_schema = match build_tuple_element_json_schema(fld) {
        Ok(field_schema) => field_schema,
        Err(rejection) => {
            json_schema_variant_fields.push(map_member_rejection_error(content_name, &rejection));
            return;
        }
    };
    json_schema_variant_fields.push(quote! {
        properties.insert(#content_name_str.to_string(), #field_schema);
        required.push(serde_json::Value::String(#content_name_str.to_string()));
    });
}

/// Writes the single-element tuple portion of a discriminated enum variant.
///
/// The content key is a slot: serde writes it for every variant that has one, so a `None` there
/// reaches the wire as a `null` under the key rather than dropping it. All three surfaces read the
/// element through the slot spellings for that reason.
fn write_tuple_single_variant_fields(
    field_defs: &[FieldDef],
    content_name: &str,
    self_type_name: &str,
    parts: &mut VariantParts,
) {
    let variant_type_code = &mut parts.type_code;
    let variant_schema_code = &mut parts.schema_code;
    let json_schema_variant_fields = &mut parts.json_fields;
    let Some(fld) = field_defs.first() else {
        let _: (&_, &_, &_) = (
            self_type_name,
            &variant_schema_code,
            &json_schema_variant_fields,
        );
        return;
    };
    let content_key = ts_member_key(content_name);

    let _ = writeln!(
        variant_type_code,
        "  /** Tuple value */\n  {}: {};",
        content_key,
        fld.typescript_slot_typename()
    );

    #[cfg(feature = "zod")]
    {
        let zod_field_type = fld.zod_slot_type();
        let is_recursive = fld.contains_type_reference(self_type_name);

        if is_recursive {
            let _ = writeln!(
                variant_schema_code,
                "  get {content_key}() {{ return {zod_field_type}; }},"
            );
        } else {
            let _ = writeln!(variant_schema_code, "  {content_key}: {zod_field_type},");
        }
    }

    #[cfg(not(feature = "zod"))]
    {
        let _: &_ = &(&variant_schema_code, self_type_name);
    }

    // JSON Schema for single tuple value
    #[cfg(feature = "jsonschema")]
    push_single_tuple_json_field(json_schema_variant_fields, content_name, fld);
    #[cfg(not(feature = "jsonschema"))]
    let _: &_ = &json_schema_variant_fields;
}

/// Writes the multi-element tuple portion of a discriminated enum variant.
fn write_tuple_multiple_variant_fields(
    field_defs: &[FieldDef],
    content_name: &str,
    self_type_name: &str,
    parts: &mut VariantParts,
) {
    let variant_type_code = &mut parts.type_code;
    let variant_schema_code = &mut parts.schema_code;
    let json_schema_variant_fields = &mut parts.json_fields;
    // Multi-element tuple: use TypeScript tuple type `value: [T1, T2, ...]`
    let ts_tuple_types: Vec<String> = field_defs
        .iter()
        .map(super::field_type::FieldDef::typescript_slot_typename)
        .collect();
    let ts_tuple = format!("[{}]", ts_tuple_types.join(", "));

    let tuple_desc: Vec<String> = field_defs
        .iter()
        .enumerate()
        .map(|(i, _)| format!("element {i}"))
        .collect();
    let content_key = ts_member_key(content_name);
    let _ = writeln!(
        variant_type_code,
        "  /** Tuple: [{}] */\n  {}: {};",
        tuple_desc.join(", "),
        content_key,
        ts_tuple
    );

    #[cfg(feature = "zod")]
    {
        let zod_tuple_types: Vec<String> = field_defs
            .iter()
            .map(super::field_type::FieldDef::zod_slot_type)
            .collect();
        let zod_tuple = format!("z.tuple([{}])", zod_tuple_types.join(", "));

        let is_recursive = field_defs
            .iter()
            .any(|fld| fld.contains_type_reference(self_type_name));

        if is_recursive {
            let _ = writeln!(
                variant_schema_code,
                "  get {content_key}() {{ return {zod_tuple}; }},"
            );
        } else {
            let _ = writeln!(variant_schema_code, "  {content_key}: {zod_tuple},");
        }
    }

    #[cfg(not(feature = "zod"))]
    {
        let _: &_ = &(&variant_schema_code, self_type_name);
    }

    // JSON Schema for tuple (using prefixItems)
    #[cfg(feature = "jsonschema")]
    {
        let content_name_str = content_name.to_owned();
        match tuple_json_schema_value(field_defs) {
            Ok(tuple_schema) => json_schema_variant_fields.push(quote! {
                properties.insert(#content_name_str.to_string(), #tuple_schema);
                required.push(serde_json::Value::String(#content_name_str.to_string()));
            }),
            Err(rejection) => {
                json_schema_variant_fields
                    .push(map_member_rejection_error(content_name, &rejection));
            }
        }
    }
    #[cfg(not(feature = "jsonschema"))]
    let _: &_ = &&json_schema_variant_fields;
}

/// Arrays `item_schema` once per array level the field carries, and hands it back untouched when
/// the field carries none.
#[cfg(feature = "jsonschema")]
fn arrayed_json_schema_value(
    fld: &FieldDef,
    item_schema: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    (0..fld.array_depth).fold(item_schema, |level_schema, level| {
        let items = if fld.is_nullable_at(level) {
            quote! { serde_json::json!({ "anyOf": [#level_schema, { "type": "null" }] }) }
        } else {
            level_schema
        };
        let bounds = fixed_length_json_schema_bounds(fld, level);
        quote! { serde_json::json!({ "type": "array", "items": #items #bounds }) }
    })
}

/// [`arrayed_json_schema_value`] for a caller holding a literal fragment: each wrap nests inside
/// the one `serde_json::json!` the fragment is written into rather than materializing a value.
#[cfg(feature = "jsonschema")]
fn arrayed_json_schema_fragment(
    fld: &FieldDef,
    item_schema: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    (0..fld.array_depth).fold(item_schema.clone(), |level_schema, level| {
        let items = if fld.is_nullable_at(level) {
            quote! { { "anyOf": [#level_schema, { "type": "null" }] } }
        } else {
            level_schema
        };
        let bounds = fixed_length_json_schema_bounds(fld, level);
        quote! { { "type": "array", "items": #items #bounds } }
    })
}

/// The arity a fixed-size `[T; N]` level pins, as the `minItems`/`maxItems` pair a tuple pins its
/// own arity with — the same constraint, the two spellings serde writes a fixed-length array from.
///
/// Empty for every other level, which is what leaves an unbounded array unbounded. The pair carries
/// its own leading comma so that emptiness costs the caller nothing.
#[cfg(feature = "jsonschema")]
fn fixed_length_json_schema_bounds(fld: &FieldDef, level: u8) -> proc_macro2::TokenStream {
    fld.fixed_length_at(level).map_or_else(
        proc_macro2::TokenStream::new,
        |length| quote! { , "minItems": #length, "maxItems": #length },
    )
}

/// Which instant a chrono type's string spells, as the JSON-schema `"format"` keyword that names
/// it, and `None` for every type that writes no such string.
#[cfg(all(feature = "jsonschema", feature = "chrono"))]
const fn chrono_json_schema_format(field_type: &FieldDefType) -> Option<&'static str> {
    match *field_type {
        FieldDefType::NaiveDate => Some("date"),
        FieldDefType::NaiveTime => Some("time"),
        FieldDefType::NaiveDateTime | FieldDefType::DateTime => Some("date-time"),
        FieldDefType::Boolean
        | FieldDefType::Char
        | FieldDefType::F32
        | FieldDefType::F64
        | FieldDefType::I8
        | FieldDefType::I16
        | FieldDefType::I32
        | FieldDefType::I64
        | FieldDefType::Isize
        | FieldDefType::Map(..)
        | FieldDefType::SiblingType(..)
        | FieldDefType::String
        | FieldDefType::StringLiteral(_)
        | FieldDefType::Tuple(..)
        | FieldDefType::TypeParam(_)
        | FieldDefType::U8
        | FieldDefType::U16
        | FieldDefType::U32
        | FieldDefType::U64
        | FieldDefType::Unknown
        | FieldDefType::Usize => None,
        #[cfg(feature = "object_id")]
        FieldDefType::ObjectId => None,
    }
}

/// The JSON schema literal a chrono type describes as — the string it writes, carrying the
/// `"format"` keyword that says which instant it spells — and `None` for every type that writes no
/// such string.
#[cfg(all(feature = "jsonschema", feature = "chrono"))]
fn chrono_json_schema_item(field_type: &FieldDefType) -> Option<proc_macro2::TokenStream> {
    let format = chrono_json_schema_format(field_type)?;
    Some(quote! { { "type": "string", "format": #format } })
}

/// The JSON schema literal for a type that renders inline as a scalar — the object body itself,
/// which a caller writing inside a `serde_json::json!` inlines and one needing a standalone
/// `serde_json::Value` wraps — or `None` for the composite types (sibling references, maps,
/// tuples, unknowns) that have no inline rendering.
#[cfg(feature = "jsonschema")]
fn scalar_field_json_schema_item(fld: &FieldDef) -> Option<proc_macro2::TokenStream> {
    let item_schema = match &fld.field_type {
        FieldDefType::String => quote! { { "type": "string" } },
        // Fixed at 1 rather than read from `model_schema_prop`: a `char` carries none of those
        // constraints, and this is what serde writes for it wherever it stands.
        FieldDefType::Char => quote! { { "type": "string", "minLength": 1, "maxLength": 1 } },
        FieldDefType::StringLiteral(literal) => {
            quote! { { "type": "string", "const": #literal } }
        }
        FieldDefType::U8
        | FieldDefType::U16
        | FieldDefType::U32
        | FieldDefType::U64
        | FieldDefType::I8
        | FieldDefType::I16
        | FieldDefType::I32
        | FieldDefType::I64
        | FieldDefType::Usize
        | FieldDefType::Isize
        | FieldDefType::F32
        | FieldDefType::F64
        | FieldDefType::Boolean => {
            // The arm has matched exactly the types the mapping answers a keyword for, so the `?`
            // never fires — it is how the chrono arm beside it reads its own mapping too.
            let keyword = scalar_json_type_keyword(&fld.field_type)?;
            quote! { { "type": #keyword } }
        }
        #[cfg(feature = "chrono")]
        FieldDefType::NaiveDate
        | FieldDefType::NaiveTime
        | FieldDefType::NaiveDateTime
        | FieldDefType::DateTime => chrono_json_schema_item(&fld.field_type)?,
        #[cfg(feature = "object_id")]
        FieldDefType::ObjectId => object_id_json_schema_item(&object_id_hex_json_schema()),
        FieldDefType::TypeParam(_)
        | FieldDefType::Unknown
        | FieldDefType::SiblingType(..)
        | FieldDefType::Map(..)
        | FieldDefType::Tuple(..) => return None,
    };
    Some(item_schema)
}

/// [`build_map_member_item`] for a tuple element, which differs for exactly one value type: a tuple
/// renders as the fixed-arity array its own field position renders, which is the one value the map
/// path has no renderer for. An element is dispatched by the tuple builder itself, so the nesting
/// costs nothing but the recursion.
#[cfg(feature = "jsonschema")]
fn build_tuple_element_item(value: &FieldDef) -> Result<MapMemberItem, MapMemberRejection> {
    if let FieldDefType::Tuple(elements) = &value.field_type {
        return Ok(MapMemberItem::Value(tuple_json_schema_value(elements)?));
    }
    build_map_member_item(value)
}

/// Builds the base JSON schema for a tuple element, ignoring `is_optional`, or `None` when the
/// element holds a value the dispatch cannot render.
#[cfg(feature = "jsonschema")]
fn build_tuple_element_base_json_schema(
    fld: &FieldDef,
) -> Result<proc_macro2::TokenStream, MapMemberRejection> {
    let value = normalized_slot_value(fld);
    let item = build_tuple_element_item(&value)?;
    Ok(arrayed_json_schema_value(&value, item.into_value()))
}

/// The `anyOf [<base>, null]` form for a value in a slot that cannot be dropped — a tuple element
/// or a map entry — or `None` when the value is not an `Option`. Only an object key can be
/// omitted; in either of these positions serde writes a `None` as JSON `null`, so the schema has
/// to admit it.
#[cfg(feature = "jsonschema")]
fn nullable_slot_json_schema(
    fld: &FieldDef,
    base: &proc_macro2::TokenStream,
) -> Option<proc_macro2::TokenStream> {
    fld.is_optional()
        .then(|| quote! { { "anyOf": [#base, { "type": "null" }] } })
}

/// [`nullable_slot_json_schema`] for a caller that needs a standalone `serde_json::Value`: the
/// nullable form is a literal fragment, so it is wrapped, while `base` already is such a value.
#[cfg(feature = "jsonschema")]
fn nullable_slot_json_schema_value(
    fld: &FieldDef,
    base: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    nullable_slot_json_schema(fld, &base)
        .map_or(base, |nullable| quote! { serde_json::json!(#nullable) })
}

/// The schema module a sibling type's `Schema::json_schema()` lives in.
#[cfg(feature = "jsonschema")]
fn sibling_schema_module_ident(name: &str, span: proc_macro2::Span) -> Ident {
    let module_name = match lookup_alias_info(name) {
        Some(alias) => alias.module_name,
        None => ident_schema_module_name(name),
    };
    Ident::new(module_name.as_str(), span)
}

/// A sibling's own schema as a standalone `serde_json::Value` expression: the module the reference
/// resolves to, asked for the schema it publishes.
#[cfg(feature = "jsonschema")]
fn sibling_json_schema_value(
    name: &str,
    arguments: &[FieldDef],
    span: proc_macro2::Span,
) -> proc_macro2::TokenStream {
    let module_ident = sibling_schema_module_ident(name, span);
    if arguments.is_empty() {
        return quote_spanned! {span=> #module_ident::Schema::json_schema_within(in_flight, hoisted_defs) };
    }
    let documents = arguments
        .iter()
        .map(|argument| argument_json_schema_value(name, argument));
    // Held in a local rather than written into the call: an argument that is itself a reference
    // describes through this same call and borrows the run's own two values to do it, and a
    // borrow taken for the outer call would still be live while the inner one asked for its own.
    //
    // Parenthesised because the block is written wherever the argumentless call was, and some of
    // those positions are inside a `serde_json::json!` literal — where a bare block opens an
    // object rather than an expression.
    quote_spanned! {span=>
        ({
            let arguments = [#(#documents),*];
            #module_ident::Schema::json_schema_within_with(in_flight, hoisted_defs, &arguments)
        })
    }
}

/// One reference-site argument as the document that fills the parameter it stands at.
#[cfg(feature = "jsonschema")]
fn argument_json_schema_value(name: &str, argument: &FieldDef) -> proc_macro2::TokenStream {
    build_tuple_element_json_schema(argument).unwrap_or_else(|rejection| {
        let message = map_member_rejection_message(&format!("`{name}`"), &rejection);
        quote! { compile_error!(#message) }
    })
}

/// The local the enclosing item's document reaches one of its own type parameters through, as the
/// `serde_json::Value` expression a position holding that parameter is described by.
#[cfg(feature = "jsonschema")]
fn json_argument_value(parameter: &str) -> proc_macro2::TokenStream {
    let binding = json_argument_ident(parameter);
    quote! { #binding.clone() }
}

/// The local one type parameter's argument document is bound to, as the ident both ends spell it
/// with — the binding the module writes and the reads the body makes of it.
#[cfg(feature = "jsonschema")]
fn json_argument_ident(parameter: &str) -> Ident {
    Ident::new(
        json_argument_binding(parameter).as_str(),
        proc_macro2::Span::call_site(),
    )
}

/// The document a value with no type of its own is described by: the filling of a type parameter,
/// and the permissive empty schema for a value the expansion cannot name at all.
#[cfg(feature = "jsonschema")]
fn opaque_json_schema_value(fld: &FieldDef) -> proc_macro2::TokenStream {
    if let FieldDefType::TypeParam(parameter) = &fld.field_type {
        return json_argument_value(parameter);
    }
    quote! { serde_json::json!({}) }
}

/// The argument slots an item's schema module publishes: one per type parameter it declares, in
/// declaration order, each carrying the document the standalone form fills it with.
#[cfg(feature = "jsonschema")]
fn schema_parameters(generics: &syn::Generics, args: &ModelSchemaArgs) -> Vec<SchemaParameter> {
    type_parameters_in_scope(generics)
        .iter()
        .map(|parameter| SchemaParameter {
            binding: json_argument_ident(parameter),
            default: declared_filling_json_schema_value(parameter, &args.default_types),
        })
        .collect()
}

/// The document a parameter's declared filling describes as, rendered through the dispatch every
/// other type position is rendered through — so the filling describes exactly as a field written
/// with that type describes.
#[cfg(feature = "jsonschema")]
fn declared_filling_json_schema_value(
    parameter: &str,
    default_types: &[(syn::Ident, syn::Type)],
) -> proc_macro2::TokenStream {
    default_types
        .iter()
        .find(|(declared, _)| declared == parameter)
        .map_or_else(
            || quote! { serde_json::json!({}) },
            |(_, filling)| {
                argument_json_schema_value(parameter, &get_field_def(parameter, filling, ""))
            },
        )
}

/// Builds JSON schema for a tuple element (used for tuple fields and for tuple variants), or the
/// rejection when the element holds a value the dispatch cannot render — which the callers turn
/// into the single diagnostic naming the field.
#[cfg(feature = "jsonschema")]
fn build_tuple_element_json_schema(
    fld: &FieldDef,
) -> Result<proc_macro2::TokenStream, MapMemberRejection> {
    Ok(nullable_slot_json_schema_value(
        fld,
        build_tuple_element_base_json_schema(fld)?,
    ))
}

/// The fixed-arity array a tuple describes as — the form serde writes it in — or the rejection when
/// any element holds a value the dispatch cannot render.
#[cfg(feature = "jsonschema")]
fn tuple_json_schema_value(
    elements: &[FieldDef],
) -> Result<proc_macro2::TokenStream, MapMemberRejection> {
    let arity = elements.len();
    let element_schemas = elements
        .iter()
        .map(build_tuple_element_json_schema)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(quote! {
        serde_json::json!({
            "type": "array",
            "prefixItems": [#(#element_schemas),*],
            "items": false,
            "minItems": #arity,
            "maxItems": #arity
        })
    })
}

/// What a sequence-spelled type holds, and `None` for everything else.
///
/// A sequence reaches a key in either of the two forms the parser leaves: the array levels it
/// collapses a `Vec` or a `[T; N]` onto, and the wrapper name it keeps for every other spelling,
/// which each surface normalizes onto its element at render time. Both write the same JSON array,
/// so the classification below reads them the same way.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn sequence_wrapper_element(fld: &FieldDef) -> Option<&FieldDef> {
    let FieldDefType::SiblingType(wrapper_name, wrapper_args) = &fld.field_type else {
        return None;
    };
    let [element] = wrapper_args.as_slice() else {
        return None;
    };
    is_sequence_wrapper(wrapper_name).then_some(element)
}

/// The field a sequence-spelled type stands for: its element, carrying the array level the wrapper
/// writes — and `None` for everything else.
#[cfg(feature = "jsonschema")]
fn sequence_wrapper_field(fld: &FieldDef) -> Option<FieldDef> {
    sequence_wrapper_element(fld).map(|element| fld.collection_element_field(element))
}

/// The classification every position reads a map key through.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn map_key_path(key: &FieldDef) -> MapKeyPath<'_> {
    if key.array_depth > 0 || sequence_wrapper_element(key).is_some() {
        return MapKeyPath::Refused(MapKeyRejection::Sequenced(map_key_element_name(key)));
    }
    if key.is_optional() {
        return MapKeyPath::Refused(MapKeyRejection::Optional(map_key_element_name(key)));
    }
    match &key.field_type {
        // A parameter names no type here, so nothing about the key can be enumerated or narrowed —
        // but serde has already said the one thing an object key needs: every instantiation this
        // map has either writes its keys as strings or refuses the whole map at serialization, so
        // the open member set is true of every instantiation that serializes at all.
        FieldDefType::String | FieldDefType::TypeParam(_) => MapKeyPath::Open,
        FieldDefType::SiblingType(key_type_name, args) if args.is_empty() => {
            match registered_key_kind(key_type_name) {
                Some(AliasKind::StringWire) => MapKeyPath::Open,
                Some(AliasKind::Stringified) => MapKeyPath::Unnarrowed,
                _ => MapKeyPath::Enumerated(key_type_name.as_str()),
            }
        }
        FieldDefType::Tuple(..) => MapKeyPath::Refused(unwritable_key(key, WRITTEN_AS_ARRAY)),
        FieldDefType::Map(..) => MapKeyPath::Refused(unwritable_key(key, WRITTEN_AS_OBJECT)),
        #[cfg(feature = "object_id")]
        FieldDefType::ObjectId => MapKeyPath::Refused(unwritable_key(key, WRITTEN_AS_OBJECT)),
        FieldDefType::SiblingType(..)
        | FieldDefType::Unknown
        | FieldDefType::Boolean
        | FieldDefType::Char
        | FieldDefType::StringLiteral(_)
        | FieldDefType::U8
        | FieldDefType::U16
        | FieldDefType::U32
        | FieldDefType::U64
        | FieldDefType::I8
        | FieldDefType::I16
        | FieldDefType::I32
        | FieldDefType::I64
        | FieldDefType::Usize
        | FieldDefType::Isize
        | FieldDefType::F32
        | FieldDefType::F64 => MapKeyPath::Unnarrowed,
        #[cfg(feature = "chrono")]
        FieldDefType::DateTime
        | FieldDefType::NaiveDate
        | FieldDefType::NaiveDateTime
        | FieldDefType::NaiveTime => MapKeyPath::Unnarrowed,
    }
}

/// The rejection a key earns for its own wire form, naming the key as written and what serde writes
/// for it instead.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn unwritable_key(key: &FieldDef, written_as: &'static str) -> MapKeyRejection {
    MapKeyRejection::Unwritable {
        key_name: map_key_element_name(key),
        written_as,
    }
}

/// Why this key has no rendering, and `None` for every key that may still have one.
///
/// A key the dispatch already refuses is ruled out by its own spelling. Below that, only a target
/// the registry positively rules out is named — an unregistered name, a foreign type or one
/// expanded after the type that writes the map, cannot be ruled out at this expansion and is left
/// alone.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn key_rejection(key: &FieldDef) -> Option<MapKeyRejection> {
    match map_key_path(key) {
        MapKeyPath::Refused(rejection) => Some(rejection),
        MapKeyPath::Enumerated(key_type_name) => proves_no_enum_members(key_type_name)
            .then(|| MapKeyRejection::NoEnumMembers(key_type_name.to_owned())),
        MapKeyPath::Open | MapKeyPath::Unnarrowed => None,
    }
}

/// The Rust spelling of what a map key holds, for the diagnostic a key with no rendering earns.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn map_key_element_name(key: &FieldDef) -> String {
    if let Some(element) = sequence_wrapper_element(key) {
        return map_key_element_name(element);
    }
    match &key.field_type {
        FieldDefType::SiblingType(key_type_name, _) | FieldDefType::TypeParam(key_type_name) => {
            key_type_name.clone()
        }
        FieldDefType::String | FieldDefType::StringLiteral(_) => "String".to_owned(),
        FieldDefType::Boolean => "bool".to_owned(),
        FieldDefType::Char => "char".to_owned(),
        FieldDefType::U8 => "u8".to_owned(),
        FieldDefType::U16 => "u16".to_owned(),
        FieldDefType::U32 => "u32".to_owned(),
        FieldDefType::U64 => "u64".to_owned(),
        FieldDefType::I8 => "i8".to_owned(),
        FieldDefType::I16 => "i16".to_owned(),
        FieldDefType::I32 => "i32".to_owned(),
        FieldDefType::I64 => "i64".to_owned(),
        FieldDefType::Usize => "usize".to_owned(),
        FieldDefType::Isize => "isize".to_owned(),
        FieldDefType::F32 => "f32".to_owned(),
        FieldDefType::F64 => "f64".to_owned(),
        FieldDefType::Map(..) => "HashMap<_, _>".to_owned(),
        FieldDefType::Tuple(..) => "(_, _)".to_owned(),
        FieldDefType::Unknown => "_".to_owned(),
        #[cfg(feature = "object_id")]
        FieldDefType::ObjectId => "ObjectId".to_owned(),
        #[cfg(feature = "chrono")]
        FieldDefType::DateTime => "DateTime".to_owned(),
        #[cfg(feature = "chrono")]
        FieldDefType::NaiveDate => "NaiveDate".to_owned(),
        #[cfg(feature = "chrono")]
        FieldDefType::NaiveDateTime => "NaiveDateTime".to_owned(),
        #[cfg(feature = "chrono")]
        FieldDefType::NaiveTime => "NaiveTime".to_owned(),
    }
}

/// Whether the registry rules the named type out as a source of `enum_members()`. `false` covers
/// both the plain enums that have them and the names this expansion never saw registered.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn proves_no_enum_members(key_type_name: &str) -> bool {
    lookup_alias_info(key_type_name).is_some_and(|key_alias| {
        matches!(
            key_alias.kind,
            AliasKind::NoEnumMembers | AliasKind::StringWire | AliasKind::Stringified
        )
    })
}

/// What the registry says serde writes for a key spelled by this name, and `None` for a name it
/// never saw registered.
///
/// An unregistered name earns nothing: the expansion cannot tell it from a plain enum declared
/// later, which is why the dispatch above leaves such a key on the enumerating path.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn registered_key_kind(key_type_name: &str) -> Option<AliasKind> {
    lookup_alias_info(key_type_name).map(|key_alias| key_alias.kind)
}

/// Why the first map key this field reaches, at any depth, has no rendering.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn map_key_rejection(fld: &FieldDef) -> Option<MapKeyRejection> {
    match &fld.field_type {
        FieldDefType::Map(key, value) => key_rejection(key)
            .or_else(|| map_key_rejection(key))
            .or_else(|| map_key_rejection(value)),
        FieldDefType::SiblingType(_, generics) => generics.iter().find_map(map_key_rejection),
        FieldDefType::Tuple(elements) => elements.iter().find_map(map_key_rejection),
        FieldDefType::TypeParam(_)
        | FieldDefType::Unknown
        | FieldDefType::StringLiteral(_)
        | FieldDefType::Boolean
        | FieldDefType::Char
        | FieldDefType::String
        | FieldDefType::U8
        | FieldDefType::U16
        | FieldDefType::U32
        | FieldDefType::U64
        | FieldDefType::I8
        | FieldDefType::I16
        | FieldDefType::I32
        | FieldDefType::I64
        | FieldDefType::Usize
        | FieldDefType::Isize
        | FieldDefType::F32
        | FieldDefType::F64 => None,
        #[cfg(feature = "object_id")]
        FieldDefType::ObjectId => None,
        #[cfg(feature = "chrono")]
        FieldDefType::NaiveDate
        | FieldDefType::NaiveTime
        | FieldDefType::NaiveDateTime
        | FieldDefType::DateTime => None,
    }
}

/// What a key with no members is reported as. The `subject` names where the map was written — a
/// field, an alias — which is what the author can act on and all that differs between them.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn non_enum_map_key_message(subject: &str, key_type_name: &str) -> String {
    format!(
        "{subject}: a map key must be a plain `#[model_schema()]` enum, whose members become the object's keys — `{key_type_name}` resolves to a type with no `enum_members()`"
    )
}

/// What a sequence-wrapped key is reported as, worded like its sibling above and carrying the same
/// `subject`.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn sequenced_map_key_message(subject: &str, element_name: &str) -> String {
    format!(
        "{subject}: a map key must be a value serde writes as a string, which is what a JSON object key is — this key is a sequence of `{element_name}`, and serde refuses to serialize a map keyed by one at all"
    )
}

/// What an `Option`-wrapped key is reported as, worded like its siblings above and carrying the same
/// `subject`.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn optional_map_key_message(subject: &str, inner_name: &str) -> String {
    format!(
        "{subject}: a map key must be a value serde writes as a string, which is what a JSON object key is — this key is an `Option<{inner_name}>`, whose `Some` serde writes as the bare `{inner_name}` while a `None` has no string form at all and makes serde refuse the whole map; key it by `{inner_name}`"
    )
}

/// What a key serde writes as neither a string nor anything it stringifies for the author is
/// reported as, worded like its siblings above and carrying the same `subject`.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn unwritable_map_key_message(subject: &str, key_name: &str, written_as: &str) -> String {
    format!(
        "{subject}: a map key must be a value serde writes as a string, which is what a JSON object key is — serde writes `{key_name}` as {written_as}, and refuses to serialize a map keyed by one at all"
    )
}

/// What a map key with no rendering is reported as, whichever reason it has.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn map_key_rejection_message(subject: &str, rejection: &MapKeyRejection) -> String {
    match rejection {
        MapKeyRejection::NoEnumMembers(key_type_name) => {
            non_enum_map_key_message(subject, key_type_name)
        }
        MapKeyRejection::Sequenced(element_name) => {
            sequenced_map_key_message(subject, element_name)
        }
        MapKeyRejection::Optional(inner_name) => optional_map_key_message(subject, inner_name),
        MapKeyRejection::Unwritable {
            key_name,
            written_as,
        } => unwritable_map_key_message(subject, key_name, written_as),
    }
}

/// Rejects a field reaching a map key no surface can write.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn check_map_key(field: &Field, field_def: &FieldDef, label: &str) -> Result<(), syn::Error> {
    let Some(rejection) = map_key_rejection(field_def) else {
        return Ok(());
    };
    Err(syn::Error::new_spanned(
        field,
        map_key_rejection_message(label, &rejection),
    ))
}

/// The `json!` literal a map whose keys cannot be narrowed describes as: an object, with nothing
/// said about its members. Written once so field and slot positions state the same thing.
#[cfg(feature = "jsonschema")]
fn unnarrowed_key_map_json_schema_item() -> proc_macro2::TokenStream {
    quote! { { "type": "object", "additionalProperties": true } }
}

/// [`unnarrowed_key_map_json_schema_item`] as a standalone `serde_json::Value` expression, for the
/// positions that hold a map as a value rather than writing it into a literal.
#[cfg(feature = "jsonschema")]
fn unnarrowed_key_map_json_schema_value(key: &FieldDef) -> proc_macro2::TokenStream {
    log::trace!("Map Key Type {:?}", key.field_type);
    let item = unnarrowed_key_map_json_schema_item();
    quote! { serde_json::json!(#item) }
}

/// The rendering a map whose value is itself a map carries, dispatched on the inner key exactly as
/// the field position dispatches on the outer one.
#[cfg(feature = "jsonschema")]
fn build_nested_map_member_item(
    inner_key: &FieldDef,
    inner_value: &FieldDef,
) -> Result<MapMemberItem, MapMemberRejection> {
    log::trace!(
        "Map Value is another Map => inner_key: {inner_key:?}, inner_value: {inner_value:?}"
    );

    Ok(match map_key_path(inner_key) {
        MapKeyPath::Enumerated(key_type_name) => MapMemberItem::Value(
            enum_key_map_json_schema_value(key_type_name, inner_key.type_span, inner_value)?,
        ),
        MapKeyPath::Open => {
            let inner_member = build_map_member_schema(inner_value)?;
            MapMemberItem::Fragment(
                quote! { { "type": "object", "additionalProperties": #inner_member } },
            )
        }
        MapKeyPath::Unnarrowed => MapMemberItem::Fragment(unnarrowed_key_map_json_schema_item()),
        MapKeyPath::Refused(rejection) => return Err(MapMemberRejection::Key(rejection)),
    })
}

/// Wraps a member's base schema for the slot it sits in — arrayed once per array level the value
/// carries, nullable when it is an `Option`.
#[cfg(feature = "jsonschema")]
fn map_member_slot_schema(
    value: &FieldDef,
    item_schema: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let arrayed = arrayed_json_schema_fragment(value, item_schema);
    nullable_slot_json_schema(value, &arrayed).unwrap_or(arrayed)
}

/// [`map_member_slot_schema`] for a member already materialized as a `serde_json::Value`: each wrap
/// materializes a value of its own instead of nesting inside the one `serde_json::json!` a literal
/// fragment sits in.
#[cfg(feature = "jsonschema")]
fn map_member_slot_value(
    value: &FieldDef,
    item_value: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    nullable_slot_json_schema_value(value, arrayed_json_schema_value(value, item_value))
}

/// The `FieldDef` a value in a slot — a map member, a tuple element — dispatches as.
#[cfg(feature = "jsonschema")]
fn normalized_slot_value(value: &FieldDef) -> FieldDef {
    let mut normalized = value.clone();
    while let Some(element_field) = sequence_wrapper_field(&normalized) {
        normalized = element_field;
    }
    normalized
}

/// The rendering a value in a slot carries, with the slot wraps left to the caller, or the
/// rejection when the value type has no rendering here.
#[cfg(feature = "jsonschema")]
fn build_map_member_item(value: &FieldDef) -> Result<MapMemberItem, MapMemberRejection> {
    Ok(match &value.field_type {
        FieldDefType::Map(inner_key, inner_value) => {
            build_nested_map_member_item(inner_key, inner_value)?
        }
        // The member is the sibling's own schema, as it is in field position — an expression, not a
        // literal, so it is already the value form.
        FieldDefType::SiblingType(value_type_name, value_args) => {
            log::trace!(
                "Slot SiblingType => value_type_name: {value_type_name}, value_args: {value_args:?}"
            );
            MapMemberItem::Value(sibling_json_schema_value(
                value_type_name,
                value_args,
                value.type_span,
            ))
        }
        // The one `$oid` object every position spells, which a member carries as written.
        #[cfg(feature = "object_id")]
        FieldDefType::ObjectId => {
            MapMemberItem::Fragment(object_id_json_schema_item(&object_id_hex_json_schema()))
        }
        // A parameter is the filling that reached it, and a value the expansion cannot name at all
        // admits any value — the permissive empty schema, as in field position. The filling is an
        // expression rather than a literal, so it is already the value form.
        FieldDefType::TypeParam(parameter) => MapMemberItem::Value(json_argument_value(parameter)),
        FieldDefType::Unknown => MapMemberItem::Fragment(quote! { {} }),
        // The shared mapping renders every type named here except a tuple, which is the lone
        // `None`. Named exhaustively rather than caught by a wildcard: a new variant must be given
        // a member schema, not silently widened into an open object.
        FieldDefType::Boolean
        | FieldDefType::Char
        | FieldDefType::F32
        | FieldDefType::F64
        | FieldDefType::I8
        | FieldDefType::I16
        | FieldDefType::I32
        | FieldDefType::I64
        | FieldDefType::Isize
        | FieldDefType::String
        | FieldDefType::StringLiteral(_)
        | FieldDefType::Tuple(..)
        | FieldDefType::U8
        | FieldDefType::U16
        | FieldDefType::U32
        | FieldDefType::U64
        | FieldDefType::Usize => MapMemberItem::Fragment(scalar_slot_item(value)?),
        #[cfg(feature = "chrono")]
        FieldDefType::DateTime
        | FieldDefType::NaiveDate
        | FieldDefType::NaiveDateTime
        | FieldDefType::NaiveTime => MapMemberItem::Fragment(scalar_slot_item(value)?),
    })
}

/// [`scalar_field_json_schema_item`] for a slot, where a tuple is the one type reaching it with no
/// inline rendering — every other type the scalar mapping names renders there.
#[cfg(feature = "jsonschema")]
fn scalar_slot_item(fld: &FieldDef) -> Result<proc_macro2::TokenStream, MapMemberRejection> {
    scalar_field_json_schema_item(fld).ok_or(MapMemberRejection::Tuple)
}

/// The `additionalProperties` schema every member of a `String`-keyed map carries, or the rejection
/// when the value type has no rendering here.
#[cfg(feature = "jsonschema")]
fn build_map_member_schema(
    value: &FieldDef,
) -> Result<proc_macro2::TokenStream, MapMemberRejection> {
    let normalized = normalized_slot_value(value);
    Ok(build_map_member_item(&normalized)?.into_member_schema(&normalized))
}

/// What a value the mapping cannot render is reported as. The `subject` names where the value was
/// written — a field, an alias — which is what the author can act on and all that differs between
/// those positions, so the reasons are worded once.
#[cfg(feature = "jsonschema")]
fn map_member_rejection_message(subject: &str, rejection: &MapMemberRejection) -> String {
    match rejection {
        MapMemberRejection::Key(key_rejection) => map_key_rejection_message(subject, key_rejection),
        MapMemberRejection::Tuple => format!(
            "{subject}: a tuple is not supported as a map value — give the value a `#[model_schema()]` struct instead"
        ),
    }
}

/// The one diagnostic a slot the mapping cannot render produces — on either key path, at any depth,
/// and in a tuple slot the value is reached through.
#[cfg(feature = "jsonschema")]
fn map_member_rejection_error(
    field_name_str: &str,
    rejection: &MapMemberRejection,
) -> proc_macro2::TokenStream {
    let message = map_member_rejection_message(&format!("field `{field_name_str}`"), rejection);
    quote! { compile_error!(#message); }
}

/// The object a `String`-keyed map describes as, as a standalone `serde_json::Value` expression, or
/// the rejection when the value type has no rendering here.
///
/// A `String` key enumerates nothing, so one `additionalProperties` schema stands for every
/// member — and it is the value type's own rendering, which the key never widens.
#[cfg(feature = "jsonschema")]
fn string_key_map_json_schema_value(
    value: &FieldDef,
) -> Result<proc_macro2::TokenStream, MapMemberRejection> {
    let value_schema = build_map_member_schema(value)?;
    Ok(quote! {
        serde_json::json!({
            "type": "object",
            "additionalProperties": #value_schema
        })
    })
}

/// The object an enum-keyed map describes as, as a standalone `serde_json::Value` expression: one
/// property per member the key enumerates, each carrying the value type's own rendering, and closed
/// to every other key. `Err` when the value has no rendering here, or when the registry proves the
/// key carries no members.
#[cfg(feature = "jsonschema")]
fn enum_key_map_json_schema_value(
    key_type_name: &str,
    key_span: proc_macro2::Span,
    value: &FieldDef,
) -> Result<proc_macro2::TokenStream, MapMemberRejection> {
    if proves_no_enum_members(key_type_name) {
        return Err(MapMemberRejection::Key(MapKeyRejection::NoEnumMembers(
            key_type_name.to_owned(),
        )));
    }

    let normalized = normalized_slot_value(value);
    let member_value = build_map_member_item(&normalized)?.into_member_value(&normalized);
    let key_type_name_ident = Ident::new(key_type_name, key_span);
    let key_members = quote_spanned! {key_span=> #key_type_name_ident::enum_members() };
    Ok(quote! {
        serde_json::json!({
            "type": "object",
            "properties": ({
                let value_schema = #member_value;
                let mut map_properties = serde_json::Map::new();
                for enum_key in #key_members {
                    map_properties.insert(enum_key.to_string(), value_schema.clone());
                }
                map_properties
            }),
            "additionalProperties": false
        })
    })
}

/// The object a map describes as, as a standalone `serde_json::Value` expression, dispatched on the
/// classification its key earns — or the rejection when the key has no rendering here, or the value
/// none the member dispatch can write.
#[cfg(feature = "jsonschema")]
fn map_json_schema_value(
    key: &FieldDef,
    value: &FieldDef,
) -> Result<proc_macro2::TokenStream, MapMemberRejection> {
    match map_key_path(key) {
        MapKeyPath::Enumerated(key_type_name) => {
            enum_key_map_json_schema_value(key_type_name, key.type_span, value)
        }
        MapKeyPath::Open => string_key_map_json_schema_value(value),
        MapKeyPath::Unnarrowed => Ok(unnarrowed_key_map_json_schema_value(key)),
        MapKeyPath::Refused(rejection) => Err(MapMemberRejection::Key(rejection)),
    }
}

/// The `properties` insertion a map-typed field produces.
#[cfg(feature = "jsonschema")]
fn build_map_field_schema(
    fld: &FieldDef,
    key: &FieldDef,
    value: &FieldDef,
    field_name_str: &str,
) -> proc_macro2::TokenStream {
    log::trace!("Map => field_name: {field_name_str}, key: {key:?}, value: {value:?}");

    match map_json_schema_value(key, value) {
        Ok(map_schema) => {
            let field_schema = arrayed_json_schema_value(fld, map_schema);
            quote! {
                properties.insert(#field_name_str.to_string(), #field_schema);
            }
        }
        Err(rejection) => map_member_rejection_error(field_name_str, &rejection),
    }
}

/// Builds the JSON schema for a `String` field, applying any length/pattern constraints.
#[cfg(feature = "jsonschema")]
fn build_string_field_schema(fld: &FieldDef, field_name_str: &str) -> proc_macro2::TokenStream {
    // Extract string constraints from model_schema_prop_meta
    let min_len_opt = fld
        .model_schema_prop_meta
        .as_ref()
        .and_then(|m| m.min_length);
    let max_len_opt = fld
        .model_schema_prop_meta
        .as_ref()
        .and_then(|m| m.max_length);
    let pattern_opt = fld
        .model_schema_prop_meta
        .as_ref()
        .and_then(|m| m.pattern.as_deref().map(str::to_owned));

    let min_len_insert = min_len_opt.map(|min_len| {
        quote! { schema_obj.insert("minLength".to_string(), serde_json::json!(#min_len)); }
    });
    let max_len_insert = max_len_opt.map(|max_len| {
        quote! { schema_obj.insert("maxLength".to_string(), serde_json::json!(#max_len)); }
    });
    let pattern_insert = pattern_opt.as_ref().map(|pattern| {
        quote! { schema_obj.insert("pattern".to_string(), serde_json::json!(#pattern)); }
    });

    let schema = arrayed_json_schema_value(fld, quote! { serde_json::Value::Object(schema_obj) });
    quote! {
        properties.insert(#field_name_str.to_string(), {
            let mut schema_obj = serde_json::Map::new();
            schema_obj.insert("type".to_string(), serde_json::json!("string"));
            #min_len_insert
            #max_len_insert
            #pattern_insert
            #schema
        });
    }
}

/// Builds the JSON schema for a string literal field (`const` value).
#[cfg(feature = "jsonschema")]
fn build_string_literal_field_schema(
    fld: &FieldDef,
    field_name_str: &str,
    literal: &str,
) -> proc_macro2::TokenStream {
    let schema = arrayed_json_schema_value(
        fld,
        quote! { serde_json::json!({ "type": "string", "const": #literal }) },
    );
    quote! {
        properties.insert(#field_name_str.to_string(), { #schema });
    }
}

/// Builds the JSON schema for a numeric field (`integer` or `number`), applying min/max constraints.
#[cfg(feature = "jsonschema")]
fn build_numeric_field_schema(
    fld: &FieldDef,
    field_name_str: &str,
    json_type: &str,
) -> proc_macro2::TokenStream {
    let minimum_opt = fld.model_schema_prop_meta.as_ref().and_then(|m| m.minimum);
    let maximum_opt = fld.model_schema_prop_meta.as_ref().and_then(|m| m.maximum);
    let minimum_insert = minimum_opt.map(|min| {
        quote! { schema_obj.insert("minimum".to_string(), serde_json::json!(#min)); }
    });
    let maximum_insert = maximum_opt.map(|max| {
        quote! { schema_obj.insert("maximum".to_string(), serde_json::json!(#max)); }
    });

    let schema = arrayed_json_schema_value(fld, quote! { serde_json::Value::Object(schema_obj) });
    quote! {
        properties.insert(#field_name_str.to_string(), {
            let mut schema_obj = serde_json::Map::new();
            schema_obj.insert("type".to_string(), serde_json::json!(#json_type));
            #minimum_insert
            #maximum_insert
            #schema
        });
    }
}

/// Builds the JSON schema for a `bool` field.
#[cfg(feature = "jsonschema")]
fn build_boolean_field_schema(fld: &FieldDef, field_name_str: &str) -> proc_macro2::TokenStream {
    let schema =
        arrayed_json_schema_value(fld, quote! { serde_json::json!({ "type": "boolean" }) });
    quote! {
        properties.insert(#field_name_str.to_string(), { #schema });
    }
}

/// Builds the JSON schema for a `char` field: the one-character string serde writes for it, with
/// `minLength`/`maxLength` fixed at 1 rather than read from `model_schema_prop` — a `char` field
/// carries none of those constraints.
#[cfg(feature = "jsonschema")]
fn build_char_field_schema(fld: &FieldDef, field_name_str: &str) -> proc_macro2::TokenStream {
    let schema = arrayed_json_schema_value(
        fld,
        quote! { serde_json::json!({ "type": "string", "minLength": 1, "maxLength": 1 }) },
    );
    quote! {
        properties.insert(#field_name_str.to_string(), { #schema });
    }
}

/// Builds the JSON schema for a `SiblingType` field (references to other generated types).
#[cfg(feature = "jsonschema")]
fn build_sibling_type_field_schema(
    fld: &FieldDef,
    field_name_str: &str,
    name: &str,
    lst: &[FieldDef],
) -> proc_macro2::TokenStream {
    log::trace!("SiblingType => name: {name}, lst: {lst:?}");
    // The element is dispatched as the arrayed field it stands for, so a sequence wrapper renders
    // exactly as the `Vec` of the same element does — element by element, at every type. Which
    // wrappers those are is the surfaces' one shared answer, so no name reaches one surface as an
    // array and another as a schema module of its own.
    if let Some(element_field) = sequence_wrapper_field(fld) {
        return build_field_type_schema(&element_field, field_name_str);
    }

    // Every remaining shape is carried by the named type's own schema module: the non-generic
    // sibling (lst.is_empty()), and the generic branded wrapper like DocumentTypeId<String>, whose
    // schema is defined on the wrapper and whose type params do not affect it. A map is not among
    // them — the parser claims both 2-argument map idents before the sibling fallback is reached,
    // so a map arrives as a `Map` and is rendered once, there. A name this arm cannot resolve is
    // the compile error the reference raises at the type, which is what keeps a second rendering —
    // free to widen or to drop the array the one rendering carries — from growing back here.
    generate_type_schema(
        fld,
        field_name_str,
        &sibling_json_schema_value(name, lst, fld.type_span),
    )
}

/// The `json!` literal an `ObjectId` describes as — the closed `$oid` object serde writes — with
/// `hex_schema` as the schema of the hex string it holds.
#[cfg(all(feature = "jsonschema", feature = "object_id"))]
fn object_id_json_schema_item(hex_schema: &proc_macro2::TokenStream) -> proc_macro2::TokenStream {
    quote! { {
        "type": "object",
        "properties": {
            "$oid": (#hex_schema)
        },
        "required": ["$oid"],
        "additionalProperties": false
    } }
}

/// [`object_id_json_schema_item`] as a standalone `serde_json::Value` expression, for the positions
/// that hold the `$oid` object as a value rather than writing it into a literal.
#[cfg(all(feature = "jsonschema", feature = "object_id"))]
fn object_id_json_schema_value(hex_schema: &proc_macro2::TokenStream) -> proc_macro2::TokenStream {
    let item = object_id_json_schema_item(hex_schema);
    quote! { serde_json::json!(#item) }
}

/// The hex string an `ObjectId`'s `$oid` member holds, where no brand narrows it further.
#[cfg(all(feature = "jsonschema", feature = "object_id"))]
fn object_id_hex_json_schema() -> proc_macro2::TokenStream {
    quote! { serde_json::json!({ "type": "string", "pattern": #OBJECT_ID_HEX_PATTERN }) }
}

/// Builds the JSON schema for a `MongoDB` `ObjectId` field (`{ "$oid": string }`).
#[cfg(all(feature = "jsonschema", feature = "object_id"))]
fn build_object_id_field_schema(fld: &FieldDef, field_name_str: &str) -> proc_macro2::TokenStream {
    let schema = arrayed_json_schema_value(
        fld,
        object_id_json_schema_value(&object_id_hex_json_schema()),
    );
    quote! {
        properties.insert(#field_name_str.to_string(), { #schema });
    }
}

/// Builds the JSON schema for a string field with a specific `format` (e.g. date/time/date-time).
#[cfg(all(feature = "jsonschema", feature = "chrono"))]
fn build_string_format_field_schema(
    fld: &FieldDef,
    field_name_str: &str,
    format: &str,
) -> proc_macro2::TokenStream {
    let schema = arrayed_json_schema_value(
        fld,
        quote! { serde_json::json!({ "type": "string", "format": #format }) },
    );
    quote! {
        properties.insert(#field_name_str.to_string(), { #schema });
    }
}

/// Builds JSON schema for a tuple struct field.
#[cfg(feature = "jsonschema")]
fn build_tuple_field_schema(
    fld: &FieldDef,
    field_name_str: &str,
    lst: &[FieldDef],
) -> proc_macro2::TokenStream {
    let tuple_schema = match tuple_json_schema_value(lst) {
        Ok(tuple_schema) => tuple_schema,
        Err(rejection) => return map_member_rejection_error(field_name_str, &rejection),
    };

    let schema = arrayed_json_schema_value(fld, tuple_schema);
    quote! {
        properties.insert(#field_name_str.to_string(), #schema);
    }
}

/// Builds the JSON schema for a field with no type of its own — one of the enclosing item's type
/// parameters, or an opaque value: a `serde_json::Value`, a function pointer, or any other type the
/// parser could not classify.
#[cfg(feature = "jsonschema")]
fn build_unknown_field_schema(fld: &FieldDef, field_name_str: &str) -> proc_macro2::TokenStream {
    log::trace!("Unknown => field_name: {field_name_str}, fld: {fld:?}");

    let schema = arrayed_json_schema_value(fld, opaque_json_schema_value(fld));
    quote! {
        properties.insert(#field_name_str.to_string(), #schema);
    }
}

/// The `properties` insertion a field's type produces, without the `required` push.
///
/// The name is passed rather than read off `fld`: a collection element is dispatched through here
/// standing in for the field it is the element of, and inserts under that field's name.
#[cfg(feature = "jsonschema")]
fn build_field_type_schema(fld: &FieldDef, field_name_str: &str) -> proc_macro2::TokenStream {
    let field_type = &fld.field_type;

    match field_type {
        FieldDefType::String => build_string_field_schema(fld, field_name_str),
        FieldDefType::StringLiteral(literal) => {
            build_string_literal_field_schema(fld, field_name_str, literal)
        }
        FieldDefType::U32
        | FieldDefType::U16
        | FieldDefType::U8
        | FieldDefType::U64
        | FieldDefType::I8
        | FieldDefType::I16
        | FieldDefType::I32
        | FieldDefType::I64
        | FieldDefType::Usize
        | FieldDefType::Isize
        | FieldDefType::F32
        | FieldDefType::F64 => {
            // The arm has matched exactly the types the mapping answers a keyword for.
            let keyword = scalar_json_type_keyword(field_type).unwrap();
            build_numeric_field_schema(fld, field_name_str, keyword)
        }
        FieldDefType::Boolean => build_boolean_field_schema(fld, field_name_str),
        FieldDefType::Char => build_char_field_schema(fld, field_name_str),
        #[cfg(feature = "object_id")]
        FieldDefType::ObjectId => build_object_id_field_schema(fld, field_name_str),
        #[cfg(feature = "chrono")]
        FieldDefType::NaiveDate
        | FieldDefType::NaiveTime
        | FieldDefType::NaiveDateTime
        | FieldDefType::DateTime => {
            // The arm has matched exactly the types the mapping answers for.
            let format = chrono_json_schema_format(&fld.field_type).unwrap();
            build_string_format_field_schema(fld, field_name_str, format)
        }
        FieldDefType::SiblingType(name, lst) => {
            build_sibling_type_field_schema(fld, field_name_str, name, lst)
        }
        FieldDefType::Map(key, value) => build_map_field_schema(fld, key, value, field_name_str),
        FieldDefType::Tuple(lst) => build_tuple_field_schema(fld, field_name_str, lst),
        // Named exhaustively rather than caught by a wildcard: a new variant must be given a
        // schema here, not silently routed to whatever the last arm happens to emit.
        FieldDefType::TypeParam(_) | FieldDefType::Unknown => {
            build_unknown_field_schema(fld, field_name_str)
        }
    }
}

/// Builds JSON schema for a field.
#[cfg(feature = "jsonschema")]
fn build_field_schema(fld: &FieldDef) -> proc_macro2::TokenStream {
    let field_name_str = fld.name.clone();
    let schema_code = build_field_type_schema(fld, &field_name_str);

    let required_code = if fld.key_is_required() {
        quote! {
            required.push(serde_json::Value::String(#field_name_str.to_string()));
        }
    } else {
        quote! {}
    };

    quote! {
        #schema_code
        #required_code
    }
}

/// Writes the TypeScript type and conditionally Zod schema for a field to the provided buffers.
///
/// The `self_type_name` parameter is used to detect recursive type references.
/// When a field references the type being defined, we use JavaScript getter syntax
/// to defer the reference and avoid "use before declaration" errors.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn write_field_type_and_schema(
    type_code: &mut String,
    fld: &FieldDef,
    self_type_name: Option<&str>,
) -> String {
    let key = ts_member_key(&fld.name);

    // Always write TypeScript type
    let _ = writeln!(
        type_code,
        "{}\n  {}{}: {};",
        member_jsdoc_block(&fld.docs),
        key,
        fld.optional_key_marker(),
        fld.typescript_typename()
    );

    // Conditionally return the Zod schema fragment
    #[cfg(feature = "zod")]
    {
        let zod_type = fld.zod_type();

        // A reference back to the item being defined, and a reference forward to one declared
        // below it, are the two a value cannot be read for while this object literal is being
        // built — see `reaches_a_type_declared_later` for why deferring the second ends every
        // cycle a set of generic types can form.
        let defer = self_type_name.is_some_and(|name| fld.contains_type_reference(name))
            || fld.reaches_a_type_declared_later();

        if defer {
            format!("  get {key}() {{ return {zod_type}; }},\n")
        } else {
            // Normal property syntax
            format!("  {key}: {zod_type},\n")
        }
    }

    #[cfg(not(feature = "zod"))]
    {
        // When zod feature is disabled, there is no schema fragment to emit
        let _: &_ = &self_type_name; // Suppress unused variable warning
        let _: &str = &key;
        String::new()
    }
}

/// The binding a walk step introduces, numbered by depth so no two steps of one chain collide.
#[cfg(feature = "serde")]
fn wrap_binding(depth: usize) -> proc_macro2::Ident {
    proc_macro2::Ident::new(&format!("value_{depth}"), proc_macro2::Span::call_site())
}

/// The name a variant's constrained member is bound under in the arm that matched it.
fn member_binding(field_ident: &proc_macro2::Ident) -> proc_macro2::Ident {
    proc_macro2::Ident::new(
        &format!("member_{field_ident}"),
        proc_macro2::Span::call_site(),
    )
}

/// The expression a check reads its value from, in whichever position the member was written.
#[cfg(feature = "serde")]
fn member_access_expr(
    access: MemberAccess,
    field_ident_tok: &proc_macro2::Ident,
) -> proc_macro2::TokenStream {
    match access {
        MemberAccess::SelfField => quote! { &self.#field_ident_tok },
        MemberAccess::VariantBinding => {
            let binding = member_binding(field_ident_tok);
            quote! { #binding }
        }
    }
}

/// Builds the `validate()` contribution for a field, reaching through its wrappers to run the
/// check on the value the constraint actually describes.
#[cfg(feature = "serde")]
fn build_field_validation(
    wraps: &[ConstraintWrap],
    access: MemberAccess,
    field_ident_tok: &proc_macro2::Ident,
    validate_value_fn_ident: &proc_macro2::Ident,
) -> proc_macro2::TokenStream {
    let checked = member_access_expr(access, field_ident_tok);
    if wraps.is_empty() {
        return quote! {
            if let Err(e) = #validate_value_fn_ident(#checked) {
                errors.push(e);
            }
        };
    }
    let head = wrap_binding(0);
    let walk = walk_wraps(wraps, &head, 1, validate_value_fn_ident, CheckSink::Collect);
    quote! {
        {
            let #head = #checked;
            #walk
        }
    }
}

/// Emits the reach-through for one wrapper and, at the end of the chain, the check itself.
#[cfg(feature = "serde")]
fn walk_wraps(
    wraps: &[ConstraintWrap],
    value: &proc_macro2::Ident,
    depth: usize,
    validate_value_fn_ident: &proc_macro2::Ident,
    sink: CheckSink,
) -> proc_macro2::TokenStream {
    let Some((wrap, rest)) = wraps.split_first() else {
        return match sink {
            CheckSink::Collect => quote! {
                if let Err(e) = #validate_value_fn_ident(#value) {
                    errors.push(e);
                }
            },
            CheckSink::Fail => quote! {
                #validate_value_fn_ident(#value)?;
            },
        };
    };
    let next = wrap_binding(depth);
    let inner = walk_wraps(
        rest,
        &next,
        depth.saturating_add(1),
        validate_value_fn_ident,
        sink,
    );
    match *wrap {
        // A `None` writes nothing, so there is nothing for the constraint to describe.
        ConstraintWrap::Optional => quote! {
            if let Some(#next) = #value {
                #inner
            }
        },
        ConstraintWrap::Sequence => quote! {
            for #next in #value {
                #inner
            }
        },
        ConstraintWrap::Transparent => quote! {
            let #next = &**#value;
            #inner
        },
    }
}

/// Builds the serde hook for a field written under wrappers: it deserializes the field's own
/// declared type and then runs the same walk `validate()` runs, so the wire is gated where the
/// constraint lands rather than where the field happens to be spelled.
#[cfg(feature = "serde")]
fn build_wrapped_deserializer(
    deserialize_fn_ident: &proc_macro2::Ident,
    validate_value_fn_ident: &proc_macro2::Ident,
    field_ty: &syn::Type,
    lifetimes: &[syn::Lifetime],
    wraps: &[ConstraintWrap],
) -> proc_macro2::TokenStream {
    let head = wrap_binding(0);
    let walk = walk_wraps(wraps, &head, 1, validate_value_fn_ident, CheckSink::Fail);
    quote! {
        pub fn #deserialize_fn_ident<'de, #(#lifetimes,)* D>(deserializer: D) -> Result<#field_ty, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            // Nested so that each hook carries its own: a schema module holds one hook per
            // constrained field and a shared name would have to be emitted exactly once.
            fn deserialize_validated<'de, D, T, F>(deserializer: D, check: F) -> Result<T, D::Error>
            where
                D: serde::Deserializer<'de>,
                T: serde::Deserialize<'de>,
                F: FnOnce(&T) -> Result<(), String>,
            {
                use serde::Deserialize;
                let value = T::deserialize(deserializer)?;
                check(&value).map_err(serde::de::Error::custom)?;
                Ok(value)
            }

            deserialize_validated(deserializer, |#head: &#field_ty| {
                #walk
                Ok(())
            })
        }
    }
}

/// The stem the per-field helpers are named from: `validate_{stem}_value` and `deserialize_{stem}`.
///
/// A field name is unique only within the variant that declares it, while one schema module holds
/// every variant's helpers — so a variant's field carries its variant into the stem, and two
/// variants naming one field name two constraints instead of colliding. A struct field has no
/// variant and keeps the bare field name its helpers have always been spelled with.
#[cfg(feature = "serde")]
fn helper_name_stem(field_ident: &str, variant_ident: Option<&str>) -> String {
    variant_ident.map_or_else(
        || field_ident.to_owned(),
        |variant| format!("{}_{field_ident}", to_snake_case(variant)),
    )
}

/// The check a `pattern` constraint holds `value` to, taking `failure` where the value is turned
/// away.
#[cfg(feature = "serde")]
fn pattern_check(pattern: &str, failure: &proc_macro2::TokenStream) -> proc_macro2::TokenStream {
    trivial_pattern(pattern).map_or_else(
        || {
            quote! {
                {
                    use std::sync::LazyLock;
                    static RE: LazyLock<regex::Regex> = LazyLock::new(|| {
                        regex::Regex::new(#pattern).unwrap()
                    });
                    if !RE.is_match(value) {
                        #failure
                    }
                }
            }
        },
        |trivial| {
            let turned_away = pattern_rejects(&trivial);
            quote! {
                if #turned_away {
                    #failure
                }
            }
        },
    )
}

/// The condition under which a trivial pattern turns `value` away — the negation of what it
/// accepts, which is the form the emitted check reads it in.
#[cfg(feature = "serde")]
fn pattern_rejects(trivial: &TrivialPattern) -> proc_macro2::TokenStream {
    match trivial {
        TrivialPattern::IsEmpty => quote! { !value.is_empty() },
        TrivialPattern::Equals(needle) => quote! { value != #needle },
        TrivialPattern::StartsWith(needle) => {
            let sought = needle_pattern(needle);
            quote! { !value.starts_with(#sought) }
        }
        TrivialPattern::EndsWith(needle) => {
            let sought = needle_pattern(needle);
            quote! { !value.ends_with(#sought) }
        }
        TrivialPattern::Contains(needle) => {
            let sought = needle_pattern(needle);
            quote! { !value.contains(#sought) }
        }
    }
}

/// A needle in the spelling the `str` pattern methods want it where the call is written into a
/// crate that denies `clippy::single_char_pattern`: one character as a `char`, anything else as
/// the string it is. Both name the same pattern to the same method.
#[cfg(feature = "serde")]
fn needle_pattern(needle: &str) -> proc_macro2::TokenStream {
    let mut chars = needle.chars();
    if let (Some(only), None) = (chars.next(), chars.next()) {
        return quote! { #only };
    }
    quote! { #needle }
}

/// The parameter a string validator takes and the rendering its checks read `value` from.
///
/// A path is the one leaf the checks cannot be handed as-is: it arrives borrowed — the form every
/// reach into one already ends at — and is rendered once through `to_string_lossy`, the string
/// serde writes for it. Every other leaf already is that string, and renders nothing.
#[cfg(feature = "serde")]
fn checked_value_parts(
    measures_path: bool,
) -> (proc_macro2::TokenStream, proc_macro2::TokenStream) {
    if measures_path {
        (
            quote! { path: &std::path::Path },
            quote! {
                let rendered = path.to_string_lossy();
                let value: &str = &rendered;
            },
        )
    } else {
        (quote! { value: &str }, quote! {})
    }
}

/// Generates the static validator for a string-shaped field with constraints, plus the serde
/// deserializer — written against the constrained value itself when the field is bare, and against
/// the field's declared type when it is wrapped.
#[cfg(feature = "serde")]
fn generate_string_validation_code(
    field_ident: &str,
    helper_stem: &str,
    meta: &ModelSchemaPropMeta,
    shape: &ConstrainedShape,
    field_ty: &syn::Type,
    access: MemberAccess,
) -> FieldValidationCode {
    let wraps: &[ConstraintWrap] = &shape.wraps;
    let validate_value_fn_name = format!("validate_{helper_stem}_value");
    let validate_value_fn_ident =
        proc_macro2::Ident::new(&validate_value_fn_name, proc_macro2::Span::call_site());
    let deserialize_fn_name = format!("deserialize_{helper_stem}");
    let deserialize_fn_ident =
        proc_macro2::Ident::new(&deserialize_fn_name, proc_macro2::Span::call_site());

    let measures_path = matches!(shape.leaf, ConstraintLeaf::Path);
    let (checked_param, rendering) = checked_value_parts(measures_path);

    let field_name_lit = field_ident.to_owned();

    let mut checks: Vec<proc_macro2::TokenStream> = Vec::new();

    if let Some(min_len) = meta.min_length {
        checks.push(quote! {
            if value.len() < #min_len {
                return Err(format!(
                    "'{}' is too short: minimum length is {}, got {}",
                    #field_name_lit, #min_len, value.len()
                ));
            }
        });
    }

    if let Some(max_len) = meta.max_length {
        checks.push(quote! {
            if value.len() > #max_len {
                return Err(format!(
                    "'{}' is too long: maximum length is {}, got {}",
                    #field_name_lit, #max_len, value.len()
                ));
            }
        });
    }

    if let Some(pattern) = &meta.pattern {
        checks.push(pattern_check(
            pattern,
            &quote! {
                return Err(format!(
                    "'{}' does not match pattern '{}'",
                    #field_name_lit, #pattern
                ));
            },
        ));
    }

    let deserializer = if wraps.is_empty() {
        // The owned form of the leaf, which is what a bare field of it is declared as: the
        // borrowed form is unsized and cannot be a field by value.
        let owned = if measures_path {
            quote! { std::path::PathBuf }
        } else {
            quote! { String }
        };
        quote! {
            pub fn #deserialize_fn_ident<'de, D>(deserializer: D) -> Result<#owned, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                use serde::Deserialize;
                let s = #owned::deserialize(deserializer)?;
                #validate_value_fn_ident(&s).map_err(serde::de::Error::custom)?;
                Ok(s)
            }
        }
    } else {
        build_wrapped_deserializer(
            &deserialize_fn_ident,
            &validate_value_fn_ident,
            field_ty,
            &shape.lifetimes,
            wraps,
        )
    };

    let module_items = quote! {
        pub fn #validate_value_fn_ident(#checked_param) -> Result<(), String> {
            #rendering
            #(#checks)*
            Ok(())
        }

        #deserializer
    };

    let field_ident_tok = proc_macro2::Ident::new(field_ident, proc_macro2::Span::call_site());

    let validate_body =
        build_field_validation(wraps, access, &field_ident_tok, &validate_value_fn_ident);

    FieldValidationCode {
        module_items,
        validate_body,
    }
}

/// Generates the static validator for a numeric field with constraints, plus the serde deserializer
/// — see `generate_string_validation_code` for how the two spellings differ.
#[cfg(feature = "serde")]
fn generate_numeric_validation_code(
    field_ident: &str,
    helper_stem: &str,
    rust_type_str: &str,
    meta: &ModelSchemaPropMeta,
    shape: &ConstrainedShape,
    field_ty: &syn::Type,
    access: MemberAccess,
) -> FieldValidationCode {
    let wraps: &[ConstraintWrap] = &shape.wraps;
    let validate_value_fn_name = format!("validate_{helper_stem}_value");
    let validate_value_fn_ident =
        proc_macro2::Ident::new(&validate_value_fn_name, proc_macro2::Span::call_site());
    let deserialize_fn_name = format!("deserialize_{helper_stem}");
    let deserialize_fn_ident =
        proc_macro2::Ident::new(&deserialize_fn_name, proc_macro2::Span::call_site());

    let rust_type_ident: proc_macro2::TokenStream = rust_type_str.parse().unwrap();
    let field_name_lit = field_ident.to_owned();

    let mut checks: Vec<proc_macro2::TokenStream> = Vec::new();

    if let Some(minimum) = meta.minimum {
        // Cast to the correct type for comparison
        let min_cast: proc_macro2::TokenStream =
            format!("{minimum} as {rust_type_str}").parse().unwrap();
        checks.push(quote! {
            if *value < #min_cast {
                return Err(format!(
                    "'{}' is too small: minimum is {}, got {}",
                    #field_name_lit, #minimum, value
                ));
            }
        });
    }

    if let Some(maximum) = meta.maximum {
        let max_cast: proc_macro2::TokenStream =
            format!("{maximum} as {rust_type_str}").parse().unwrap();
        checks.push(quote! {
            if *value > #max_cast {
                return Err(format!(
                    "'{}' is too large: maximum is {}, got {}",
                    #field_name_lit, #maximum, value
                ));
            }
        });
    }

    let deserializer = if wraps.is_empty() {
        quote! {
            pub fn #deserialize_fn_ident<'de, D>(deserializer: D) -> Result<#rust_type_ident, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                use serde::Deserialize;
                let v = #rust_type_ident::deserialize(deserializer)?;
                #validate_value_fn_ident(&v).map_err(serde::de::Error::custom)?;
                Ok(v)
            }
        }
    } else {
        build_wrapped_deserializer(
            &deserialize_fn_ident,
            &validate_value_fn_ident,
            field_ty,
            &shape.lifetimes,
            wraps,
        )
    };

    let module_items = quote! {
        pub fn #validate_value_fn_ident(value: &#rust_type_ident) -> Result<(), String> {
            #(#checks)*
            Ok(())
        }

        #deserializer
    };

    let field_ident_tok = proc_macro2::Ident::new(field_ident, proc_macro2::Span::call_site());

    let validate_body =
        build_field_validation(wraps, access, &field_ident_tok, &validate_value_fn_ident);

    FieldValidationCode {
        module_items,
        validate_body,
    }
}

/// Reads a field's type down to the value a constraint can land on, collecting the wrappers on the
/// way.
#[cfg(feature = "serde")]
fn constrained_shape(ty: &syn::Type) -> Option<ConstrainedShape> {
    let mut wraps = Vec::new();
    let mut lifetimes: Vec<syn::Lifetime> = Vec::new();
    let mut current = ty;
    loop {
        current = written_type(current);
        if let syn::Type::Array(array) = current {
            wraps.push(ConstraintWrap::Sequence);
            current = &array.elem;
        } else if let syn::Type::Slice(slice) = current {
            wraps.push(ConstraintWrap::Sequence);
            current = &slice.elem;
        } else if let syn::Type::Path(path) = current {
            let segment = path.path.segments.last()?;
            if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                wraps.push(generic_wrap(&segment.ident.to_string())?);
                collect_lifetimes(args, &mut lifetimes);
                current = sole_type_argument(args)?;
            } else if matches!(segment.arguments, syn::PathArguments::None) {
                let leaf = leaf_for_ident(&segment.ident.to_string())?;
                return Some(ConstrainedShape {
                    leaf,
                    lifetimes,
                    wraps,
                });
            } else {
                return None;
            }
        } else {
            return None;
        }
    }
}

/// Adds the lifetimes a wrapper spells to the ones already collected, skipping `'static` — which
/// needs no declaration — and any already there, since a lifetime can only be declared once.
#[cfg(feature = "serde")]
fn collect_lifetimes(
    args: &syn::AngleBracketedGenericArguments,
    lifetimes: &mut Vec<syn::Lifetime>,
) {
    for arg in &args.args {
        if let syn::GenericArgument::Lifetime(lifetime) = arg
            && lifetime.ident != "static"
            && !lifetimes.iter().any(|seen| seen.ident == lifetime.ident)
        {
            lifetimes.push(lifetime.clone());
        }
    }
}

/// The wrapper a generic type stands for, or `None` if it is not one the constraint reads through.
#[cfg(feature = "serde")]
fn generic_wrap(ident: &str) -> Option<ConstraintWrap> {
    if ident == "Option" {
        Some(ConstraintWrap::Optional)
    } else if is_sequence_wrapper(ident) {
        Some(ConstraintWrap::Sequence)
    } else if is_transparent_wrapper(ident) {
        Some(ConstraintWrap::Transparent)
    } else {
        None
    }
}

/// The one type argument a wrapper holds. A lifetime writes nothing and is not one, which is what
/// lets `Cow<'a, str>` answer here exactly as `Box<str>` does.
#[cfg(feature = "serde")]
fn sole_type_argument(args: &syn::AngleBracketedGenericArguments) -> Option<&syn::Type> {
    let mut types = args.args.iter().filter_map(|arg| {
        if let syn::GenericArgument::Type(ty) = arg {
            Some(ty)
        } else {
            None
        }
    });
    let only = types.next()?;
    types.next().is_none().then_some(only)
}

/// The leaf a bare type name stands for. `str` is `String`'s borrowed form and answers as one, as
/// `Path` does for `PathBuf`; the numerics name themselves, since the validator's parameter is
/// written from the name.
#[cfg(feature = "serde")]
fn leaf_for_ident(ident: &str) -> Option<ConstraintLeaf> {
    match ident {
        "String" | "str" => Some(ConstraintLeaf::Str),
        "PathBuf" | "Path" => Some(ConstraintLeaf::Path),
        "u8" => Some(ConstraintLeaf::Number("u8")),
        "u16" => Some(ConstraintLeaf::Number("u16")),
        "u32" => Some(ConstraintLeaf::Number("u32")),
        "u64" => Some(ConstraintLeaf::Number("u64")),
        "i8" => Some(ConstraintLeaf::Number("i8")),
        "i16" => Some(ConstraintLeaf::Number("i16")),
        "i32" => Some(ConstraintLeaf::Number("i32")),
        "i64" => Some(ConstraintLeaf::Number("i64")),
        "usize" => Some(ConstraintLeaf::Number("usize")),
        "isize" => Some(ConstraintLeaf::Number("isize")),
        "f32" => Some(ConstraintLeaf::Number("f32")),
        "f64" => Some(ConstraintLeaf::Number("f64")),
        _ => None,
    }
}

fn validate_as_number_flag(field_type: &FieldDefType, flag_set: bool) -> Result<(), String> {
    #[cfg(not(feature = "chrono"))]
    let _: &FieldDefType = field_type;

    #[cfg(feature = "chrono")]
    let is_datetime = matches!(field_type, FieldDefType::DateTime);
    #[cfg(not(feature = "chrono"))]
    let is_datetime = false;

    if flag_set && !is_datetime {
        return Err("#[model_schema_prop(as_number)] requires a chrono DateTime<Tz> field".into());
    }
    Ok(())
}

fn validate_ts_optional_flag(field_optional: bool, flag_set: bool) -> Result<(), String> {
    if flag_set && !field_optional {
        return Err("#[model_schema_prop(ts_optional)] requires an Option<T> field".into());
    }
    Ok(())
}

/// The field's ident as a string, empty for a positional slot that has none.
fn field_ident_string(field: &Field) -> String {
    field
        .ident
        .as_ref()
        .map(ToString::to_string)
        .unwrap_or_default()
}

/// Hands the field def the two serde facts the object surfaces spend: whether the field's value
/// leaves the key out of the serialized object rather than writing it, and whether the key is out
/// of both directions at once.
fn apply_serde_key_omission(field_def: &mut FieldDef, field: &Field) {
    let omission = parse_serde_key_omission(&field.attrs);
    field_def.omits_value = field.ident.is_some() && omission.omits_key;
    field_def.absent_from_wire = field.ident.is_some() && omission.absent_from_wire();
}

/// Adds the field to the list every surface is built from, unless the wire carries its key in
/// neither direction.
fn push_described_field(field_defs: &mut Vec<FieldDef>, field_def: FieldDef) {
    if !field_def.absent_from_wire {
        field_defs.push(field_def);
    }
}

/// Rejects a named `Option` field whose serde attributes let a `None` reach the wire as `null`.
#[cfg(feature = "serde")]
fn check_optional_field_serialization(field: &Field, is_optional: bool) -> Result<(), syn::Error> {
    let Some(ident) = field.ident.as_ref() else {
        return Ok(());
    };
    if !is_optional || parse_serde_key_omission(&field.attrs).omits_key {
        return Ok(());
    }
    Err(syn::Error::new_spanned(
        field,
        format!(
            "model_schema: field `{ident}` is `Option` but is serialized as `null` when `None`, \
             while the generated schema only accepts the key being absent. Add \
             #[serde(skip_serializing_if = \"Option::is_none\")] (plus `default` if the type \
             derives Deserialize), or `skip` / `skip_serializing`."
        ),
    ))
}

/// Rejects a named field whose serde attributes drop its key on the way out while serde still
/// insists on finding that key on the way in.
#[cfg(feature = "serde")]
fn check_omitted_key_is_readable(
    field: &Field,
    is_optional: bool,
    container_defaulted: bool,
) -> Result<(), syn::Error> {
    let Some(ident) = field.ident.as_ref() else {
        return Ok(());
    };
    let omission = parse_serde_key_omission(&field.attrs);
    if is_optional
        || container_defaulted
        || omission.defaulted
        || omission.skips_deserializing
        || !omission.omits_key
    {
        return Ok(());
    }
    Err(syn::Error::new_spanned(
        field,
        format!(
            "model_schema: field `{ident}` is left out of the serialized object by its serde \
             attribute, but serde still requires the key when deserializing, so the payload it \
             writes cannot be read back. Add #[serde(default)] to the field (or to the type), or \
             use `skip` if the field should never be read from the payload."
        ),
    ))
}

/// The serde-read guard errors the field violates.
///
/// A hidden serde attribute leaves every serde-read diagnostic unreliable — the `Option`-null guard
/// included, since the wrapper is exactly what kept its evidence out of the meta. The
/// positional-constraint guard reads no serde attribute, so it stands whatever the wrapper hid.
#[cfg(feature = "serde")]
fn field_guard_errors(
    field: &Field,
    raw_field_ident: &str,
    is_optional: bool,
    serde_field_meta: &SerdeFieldMeta,
    container_defaulted: bool,
    positional_constraint_error: Option<proc_macro2::TokenStream>,
) -> Vec<proc_macro2::TokenStream> {
    positional_constraint_error
        .into_iter()
        .chain(serde_field_meta.cfg_attr_rejection.as_ref().map_or_else(
            || {
                check_optional_field_serialization(field, is_optional)
                    .and_then(|()| {
                        check_omitted_key_is_readable(field, is_optional, container_defaulted)
                    })
                    .err()
                    .map(|err| err.to_compile_error())
            },
            |rejection| {
                Some(cfg_attr_guard_error(
                    rejection,
                    &field_label(raw_field_ident),
                ))
            },
        ))
        .collect()
}

/// Every guard error the field violates: the two the type earns — the `OsString` guard and the
/// map-key guard, neither of which any attribute can hide — then everything the
/// `model_schema_prop` attribute earned, then the serde-side guards when any fired.
fn collect_field_guard_errors(
    field: &Field,
    field_def: &FieldDef,
    written_def: &FieldDef,
    raw_field_ident: &str,
    prop_meta: &ModelSchemaPropMeta,
    serde_guard_errors: Vec<proc_macro2::TokenStream>,
) -> Vec<proc_macro2::TokenStream> {
    let label = field_label(raw_field_ident);

    #[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
    let map_key_error = check_map_key(field, field_def, &label)
        .err()
        .map(|err| err.to_compile_error());
    #[cfg(not(any(feature = "typescript", feature = "zod", feature = "jsonschema")))]
    let map_key_error: Option<proc_macro2::TokenStream> = None;

    check_os_string_field(field, field_def, &label)
        .err()
        .map(|err| err.to_compile_error())
        .into_iter()
        .chain(map_key_error)
        .chain(model_schema_prop_guard_errors(
            field,
            field_def,
            written_def,
            &label,
            prop_meta,
        ))
        .chain(serde_guard_errors)
        .collect()
}

/// Every guard the field's `model_schema_prop` attribute earns: what the parser refused, then an
/// unparseable `pattern`, then a bound written where the type renders none, an `as` naming a type
/// the field is not written as, and the three misuses the flag validators answer for.
fn model_schema_prop_guard_errors(
    field: &Field,
    field_def: &FieldDef,
    written_def: &FieldDef,
    label: &str,
    prop_meta: &ModelSchemaPropMeta,
) -> Vec<proc_macro2::TokenStream> {
    let refusals = [
        check_fixed_shape_constraints(field, field_def, prop_meta, label).err(),
        check_as_type_override(field, written_def, prop_meta, label).err(),
        check_as_preprocess_conflict(field, prop_meta, label).err(),
        flag_guard_error(
            field,
            label,
            validate_ts_optional_flag(field_def.is_optional(), prop_meta.ts_optional),
        ),
        flag_guard_error(
            field,
            label,
            validate_as_number_flag(&field_def.field_type, prop_meta.as_number),
        ),
    ];

    prop_meta
        .attr_rejection
        .as_ref()
        .map(|rejection| attr_guard_error(rejection, label))
        .into_iter()
        .chain(
            prop_meta
                .pattern_rejection
                .as_ref()
                .map(|rejection| pattern_guard_error(rejection, label)),
        )
        .chain(
            refusals
                .into_iter()
                .flatten()
                .map(|err| err.to_compile_error()),
        )
        .collect()
}

/// Turns a flag validator's refusal into a spanned error at the field that carries the flag.
///
/// The validator keeps its message: it is the one place the misuse is spelled, and a caller that
/// re-spells it maintains the same sentence twice.
fn flag_guard_error(field: &Field, label: &str, result: Result<(), String>) -> Option<syn::Error> {
    result
        .err()
        .map(|message| syn::Error::new_spanned(field, format!("model_schema: {label}: {message}")))
}

/// The bound keys a `model_schema_prop` meta carries, named as they were written.
///
/// A guard that answers for where a bound may sit names the ones it found, so the author is pointed
/// at the keys to remove rather than at the attribute as a whole.
fn written_constraint_keys(prop_meta: &ModelSchemaPropMeta) -> Vec<&'static str> {
    [
        ("minLength", prop_meta.min_length.is_some()),
        ("maxLength", prop_meta.max_length.is_some()),
        ("pattern", prop_meta.pattern.is_some()),
        ("minimum", prop_meta.minimum.is_some()),
        ("maximum", prop_meta.maximum.is_some()),
    ]
    .into_iter()
    .filter_map(|(key, written)| written.then_some(key))
    .collect()
}

/// Rejects a length, pattern or range written on a field no surface reads one beside.
fn check_fixed_shape_constraints(
    field: &Field,
    field_def: &FieldDef,
    prop_meta: &ModelSchemaPropMeta,
    label: &str,
) -> Result<(), syn::Error> {
    let written = written_constraint_keys(prop_meta);
    if written.is_empty() {
        return Ok(());
    }
    let keys = written.join("`, `");
    if let Some(name) = field_def.fixed_shape_name() {
        return Err(syn::Error::new_spanned(
            field,
            format!(
                "model_schema: {label}: `{keys}` cannot apply to `{name}` — this crate writes that \
                 type's schema whole, not as the plain string or number a bound is spelled \
                 against, and no surface reads a bound beside it: the constraint would reach \
                 neither Zod, nor the JSON schema, nor the generated validator. Drop it, or carry \
                 the value in a `String` field the bound can measure."
            ),
        ));
    }
    if let Some(shape) = field_def.composite_shape_name() {
        return Err(syn::Error::new_spanned(
            field,
            format!(
                "model_schema: {label}: `{keys}` cannot apply to {shape} — a map renders its keys \
                 and its values, a tuple renders each of its elements, and every surface builds \
                 those from the members: the bound names no value here, and `model_schema_prop` \
                 has no way to say which member it meant, so the constraint would reach neither \
                 Zod, nor the JSON schema, nor the generated validator. Constrain the member \
                 instead — declare its type as a branded newtype carrying the bound — or drop it."
            ),
        ));
    }
    let Some(parameter) = field_def.parameter_shape_name() else {
        return Ok(());
    };
    Err(syn::Error::new_spanned(
        field,
        format!(
            "model_schema: {label}: `{keys}` cannot apply to the type parameter `{parameter}` — \
             the value's type is whatever the instantiation supplies, and one schema is written \
             for every instantiation: Zod and the JSON schema describe the value as the opaque one \
             a bound cannot be spelled against, and neither the generated validator nor serde \
             holds it to anything written here, so the constraint would reach none of them. \
             Constrain the argument instead — declare the type the instantiation supplies as a \
             branded newtype carrying the bound — or drop it."
        ),
    ))
}

/// Rejects an `as = Type` naming anything but the type the field already renders.
fn check_as_type_override(
    field: &Field,
    field_def: &FieldDef,
    prop_meta: &ModelSchemaPropMeta,
    label: &str,
) -> Result<(), syn::Error> {
    let Some(as_type) = prop_meta.as_type.as_ref() else {
        return Ok(());
    };
    let target = get_field_def(&field_def.name, as_type, "");
    if *field_def == target || field_def.value_under_wrappers() == target {
        return Ok(());
    }
    let written = quote!(#as_type).to_string();
    let field_type = &field.ty;
    let declared = quote!(#field_type).to_string();
    Err(syn::Error::new_spanned(
        field,
        format!(
            "model_schema: {label}: `as = {written}` names a type the field does not render — it is \
             written as `{declared}`. The difference cannot be honored: every surface is written \
             from the declared type, which is the one serde reads and writes, and a \
             `serialize_with` names a function whose output the expansion cannot see, so emitting \
             `{written}` here would describe a payload serde never writes. Declare the field as the \
             type the wire carries, or drop the `as`."
        ),
    ))
}

/// Rejects `as` and `preprocess` written on the same field.
///
/// `as` names the type the surfaces render and `preprocess` wraps the schema that rendering
/// produced; this crate defines no order between them, so a field carrying both says nothing
/// either surface can act on.
fn check_as_preprocess_conflict(
    field: &Field,
    prop_meta: &ModelSchemaPropMeta,
    label: &str,
) -> Result<(), syn::Error> {
    if prop_meta.as_type.is_none() || prop_meta.preprocess.is_empty() {
        return Ok(());
    }
    Err(syn::Error::new_spanned(
        field,
        format!(
            "model_schema: {label}: `as` and `preprocess` cannot be written on the same field — \
             `as` names the type the surfaces render and `preprocess` wraps the schema that \
             rendering produced, and this crate defines no order between the two. Write one or the \
             other."
        ),
    ))
}

/// Rejects a field that reaches an `OsString`/`OsStr`, at any depth.
///
/// serde writes both as an externally tagged enum naming the target platform — `{"Unix":[u8, …]}`
/// or `{"Windows":[u16, …]}` — so one Rust field has two wire forms and no schema describes both.
/// Their owned string counterparts are what a portable field is written as instead.
fn check_os_string_field(
    field: &Field,
    field_def: &FieldDef,
    label: &str,
) -> Result<(), syn::Error> {
    let Some(name) = field_def.os_string_name() else {
        return Ok(());
    };
    Err(syn::Error::new_spanned(
        field,
        format!(
            "model_schema: {label} reaches `{name}`, which serde writes as an externally tagged \
             enum naming the target platform (`{{\"Unix\":[u8, ...]}}` or \
             `{{\"Windows\":[u16, ...]}}`), not a string, so no schema can describe it portably. \
             Use `String`, or `PathBuf` for a filesystem path."
        ),
    ))
}

/// Processes a field and returns its definition, optional module items (validators/deserializers),
/// optional `validate_body` (contribution to the type-level `validate()` method), and the
/// `compile_error!` tokens for every guard the field violates.
fn process_field(
    ctx: &FieldContext<'_>,
    field: &mut Field,
    deferred_attrs: &mut Vec<Vec<syn::Attribute>>,
) -> (
    FieldDef,
    Option<proc_macro2::TokenStream>,
    Option<proc_macro2::TokenStream>,
    Vec<proc_macro2::TokenStream>,
) {
    // Only the serde side has anything to hang on a field; every other build holds back nothing.
    #[cfg(feature = "serde")]
    let mut injected_attrs: Vec<syn::Attribute> = Vec::new();
    #[cfg(not(feature = "serde"))]
    let injected_attrs: Vec<syn::Attribute> = Vec::new();

    #[cfg(feature = "serde")]
    let serde_field_meta = parse_serde_field_attributes(&field.attrs);
    #[cfg(feature = "serde")]
    let field_rename = serde_field_meta.rename.clone();
    #[cfg(not(feature = "serde"))]
    let field_rename: Option<String> = None;

    let raw_field_ident = field_ident_string(field);

    // Parse model_schema_prop attributes before filtering them out
    let model_schema_prop_meta = parse_model_schema_prop_attributes(&field.attrs);

    let new_attrs = declaration_attrs(field);

    // Generate validation code and hold back the serde attribute it hangs on the field
    #[cfg(feature = "serde")]
    let (validation_fn, validate_body, positional_constraint_error) = generate_field_validation(
        field,
        ctx.schema_module_name,
        &raw_field_ident,
        ctx.variant_ident,
        &model_schema_prop_meta,
        &mut injected_attrs,
    );

    #[cfg(not(feature = "serde"))]
    let (validation_fn, validate_body): (
        Option<proc_macro2::TokenStream>,
        Option<proc_macro2::TokenStream>,
    ) = (None, None);
    #[cfg(not(feature = "serde"))]
    let _: &_ = &(ctx.schema_module_name, ctx.variant_ident);

    field.attrs = new_attrs;
    deferred_attrs.push(injected_attrs);

    let field_type: &syn::Type = &field.ty;

    let final_name =
        get_final_field_name(&raw_field_ident, field_rename.as_deref(), ctx.rename_all);
    let field_docs = build_jsdoc_body(get_field_docs(field).as_deref(), &final_name);

    let mut field_def = get_field_def(&final_name, field_type, &field_docs);

    // Resolve `Self` references to the concrete type name so recursive fields
    // (e.g. `Vec<Self>`) are treated exactly like `Vec<EnclosingType>`. Resolved before the guards
    // read the field so each one asks its question of the type the surfaces will render.
    field_def.resolve_self_references(ctx.type_name, ctx.type_parameters);

    // The field as the author spelled it, kept for the one guard that reads a written *name*: an
    // `as = Type` may name one of the item's own parameters, and the target it is compared against
    // is built from the written type too.
    let written_def = field_def.clone();

    // Every other guard, and the constraint docs below, ask what the surfaces will render — where
    // one of the item's own parameters is the opaque value rather than a reference to a type of
    // that name. Erased here so a guard and the renderer standing behind it read one def.
    field_def.erase_type_parameters(ctx.type_parameters);

    apply_serde_key_omission(&mut field_def, field);

    #[cfg(feature = "serde")]
    let serde_guard_errors = field_guard_errors(
        field,
        &raw_field_ident,
        field_def.is_optional(),
        &serde_field_meta,
        ctx.container_defaulted,
        positional_constraint_error,
    );
    #[cfg(not(feature = "serde"))]
    let serde_guard_errors: Vec<proc_macro2::TokenStream> = {
        let _: bool = ctx.container_defaulted;
        Vec::new()
    };

    let guard_errors = collect_field_guard_errors(
        field,
        &field_def,
        &written_def,
        &raw_field_ident,
        &model_schema_prop_meta,
        serde_guard_errors,
    );

    apply_model_schema_prop_meta(&mut field_def, model_schema_prop_meta, &final_name);

    (field_def, validation_fn, validate_body, guard_errors)
}

/// Whether the field needs a `#[serde(default)]` written for it alongside the `deserialize_with`.
#[cfg(feature = "serde")]
fn needs_injected_default(wraps: &[ConstraintWrap], has_default: bool) -> bool {
    let first_opaque = wraps
        .iter()
        .find(|wrap| !matches!(**wrap, ConstraintWrap::Transparent));
    matches!(first_opaque, Some(ConstraintWrap::Optional)) && !has_default
}

/// The `compile_error!` tokens for a length or range constraint written on a positional field.
///
/// Both helpers such a constraint generates are named from the field ident, and `validate()` reaches
/// the value through that same ident — a spelling a tuple slot has none of.
#[cfg(feature = "serde")]
fn positional_constraint_guard_error(
    field: &Field,
    raw_field_ident: &str,
) -> proc_macro2::TokenStream {
    syn::Error::new_spanned(
        field,
        format!(
            "model_schema: {}: pattern, minLength, maxLength, minimum and maximum are unsupported \
             on a positional field — the generated validator, the generated deserializer and the \
             `validate()` accessor are all named from the field ident, which a tuple slot has \
             none of. Move the element into a struct variant with a named field, or drop the \
             constraint.",
            field_label(raw_field_ident)
        ),
    )
    .to_compile_error()
}

/// Generates per-field serde validation code (static validator + `deserialize_with`) and, when
/// constraints apply, collects the corresponding `#[serde(deserialize_with = ...)]` attribute into
/// `injected_attrs` — plus the `#[serde(default)]` that keeps an optional key optional under one.
#[cfg(feature = "serde")]
fn generate_field_validation(
    field: &Field,
    schema_module_name: Option<&str>,
    raw_field_ident: &str,
    variant_ident: Option<&str>,
    model_schema_prop_meta: &ModelSchemaPropMeta,
    injected_attrs: &mut Vec<syn::Attribute>,
) -> (
    Option<proc_macro2::TokenStream>,
    Option<proc_macro2::TokenStream>,
    Option<proc_macro2::TokenStream>,
) {
    let has_string_constraints = model_schema_prop_meta.min_length.is_some()
        || model_schema_prop_meta.max_length.is_some()
        || model_schema_prop_meta.pattern.is_some();
    let has_numeric_constraints =
        model_schema_prop_meta.minimum.is_some() || model_schema_prop_meta.maximum.is_some();

    if raw_field_ident.is_empty() && (has_string_constraints || has_numeric_constraints) {
        return (
            None,
            None,
            Some(positional_constraint_guard_error(field, raw_field_ident)),
        );
    }

    let (Some(module_name), Some(shape)) = (schema_module_name, constrained_shape(&field.ty))
    else {
        return (None, None, None);
    };

    let helper_stem = helper_name_stem(raw_field_ident, variant_ident);
    // The variant that scopes the helper names is the same thing that says where the value is
    // reached from: a member of one is bound by the arm that matched it, and a struct's field is
    // read off `self`.
    let access = if variant_ident.is_some() {
        MemberAccess::VariantBinding
    } else {
        MemberAccess::SelfField
    };
    let generated = match shape.leaf {
        ConstraintLeaf::Path | ConstraintLeaf::Str => has_string_constraints.then(|| {
            generate_string_validation_code(
                raw_field_ident,
                &helper_stem,
                model_schema_prop_meta,
                &shape,
                &field.ty,
                access,
            )
        }),
        ConstraintLeaf::Number(rust_type) => has_numeric_constraints.then(|| {
            generate_numeric_validation_code(
                raw_field_ident,
                &helper_stem,
                rust_type,
                model_schema_prop_meta,
                &shape,
                &field.ty,
                access,
            )
        }),
    };
    let Some(validation_code) = generated else {
        return (None, None, None);
    };

    let deserialize_with_path = format!("{module_name}::deserialize_{helper_stem}");
    let path_lit = syn::LitStr::new(&deserialize_with_path, proc_macro2::Span::call_site());
    injected_attrs.push(syn::parse_quote! {
        #[serde(deserialize_with = #path_lit)]
    });
    if needs_injected_default(&shape.wraps, has_serde_default(&field.attrs)) {
        injected_attrs.push(syn::parse_quote! {
            #[serde(default)]
        });
    }

    (
        Some(validation_code.module_items),
        Some(validation_code.validate_body),
        None,
    )
}

/// Hands the field the `model_schema_prop` metadata the surfaces read it back off, and applies the
/// one key that names the type rather than constrains it.
fn apply_model_schema_prop_meta(
    field_def: &mut FieldDef,
    prop_meta: ModelSchemaPropMeta,
    final_name: &str,
) {
    field_def.model_schema_prop_meta = (prop_meta.as_type.is_some()
        || prop_meta.literal.is_some()
        || prop_meta.min_length.is_some()
        || prop_meta.max_length.is_some()
        || prop_meta.pattern.is_some()
        || prop_meta.minimum.is_some()
        || prop_meta.maximum.is_some()
        || prop_meta.ts_optional
        || prop_meta.as_number
        || !prop_meta.preprocess.is_empty())
    .then_some(prop_meta);

    if let Some(meta) = &field_def.model_schema_prop_meta
        && let Some(literal) = &meta.literal
    {
        field_def.field_type = FieldDefType::StringLiteral(literal.clone());
    }

    apply_constraint_docs(field_def, final_name);
}

/// Appends length/range constraint information to a field's generated docs.
///
/// The sentence states the bound as a rule the value is held to, so it is written only where
/// something holds the value to it. A placement [`check_fixed_shape_constraints`] refuses gets
/// none: there the bound reaches no surface, and the doc would be the only place it appeared at all
/// — the claim standing exactly where nothing backs it.
fn apply_constraint_docs(field_def: &mut FieldDef, final_name: &str) {
    if field_def.constraints_reach_nothing() {
        return;
    }
    let Some(meta) = &field_def.model_schema_prop_meta else {
        return;
    };
    let mut constraint_docs: Vec<String> = Vec::new();
    if let Some(min_len) = meta.min_length {
        constraint_docs.push(format!(" * Minimum length: {min_len}"));
    }
    if let Some(max_len) = meta.max_length {
        constraint_docs.push(format!(" * Maximum length: {max_len}"));
    }
    if let Some(minimum) = meta.minimum {
        constraint_docs.push(format!(" * Minimum value: {minimum}"));
    }
    if let Some(maximum) = meta.maximum {
        constraint_docs.push(format!(" * Maximum value: {maximum}"));
    }
    if !constraint_docs.is_empty() {
        let extra_docs = constraint_docs.join("\n");
        field_def.docs = if field_def.docs.is_empty() {
            format!(" * {final_name}\n * \n{extra_docs}")
        } else {
            format!("{}\n{}", field_def.docs, extra_docs)
        };
    }
}

/// Gets the serialized name of a struct field. serde cases fields by different rules than enum
/// variants, so the two must not share one entry point.
fn get_final_field_name(
    name: &str,
    field_rename: Option<&str>,
    rename_all: Option<&str>,
) -> String {
    field_rename.map_or_else(
        || resolve_rename_rule(rename_all).apply_to_field(name),
        str::to_owned,
    )
}

fn get_final_variant_name(
    name: &str,
    variant_rename: Option<&str>,
    rename_all: Option<&str>,
) -> String {
    variant_rename.map_or_else(
        || resolve_rename_rule(rename_all).apply_to_variant(name),
        str::to_owned,
    )
}

#[cfg(feature = "jsonschema")]
/// Generates the JSON schema method conditionally based on the jsonschema feature.
fn generate_json_schema_method(
    json_schema_fields: &[proc_macro2::TokenStream],
    flatten_json_schemas: &[MergedSource],
    def_name: &str,
    parameters: &[SchemaParameter],
) -> proc_macro2::TokenStream {
    generate_struct_json_schema_method_impl(
        json_schema_fields,
        flatten_json_schemas,
        def_name,
        parameters,
    )
}

/// What a struct's `#[serde(flatten)]` fields contribute to the object it writes.
#[cfg(feature = "jsonschema")]
fn flatten_merged_sources(flattened_fields: &[FieldDef]) -> Vec<MergedSource> {
    flattened_fields.iter().map(flatten_merged_source).collect()
}

/// What a value whose members join the object being written contributes to it, labelled with the
/// name the author gave it: the type for a named one, the field otherwise — a shape with no name of
/// its own is only ever pointed at through where it was written.
#[cfg(feature = "jsonschema")]
fn flatten_merged_source(fld: &FieldDef) -> MergedSource {
    if let FieldDefType::SiblingType(name, arguments) = &fld.field_type {
        return MergedSource {
            label: name.clone(),
            optional: fld.is_optional(),
            value: sibling_json_schema_value(name, arguments, fld.type_span),
        };
    }
    let value = if let FieldDefType::TypeParam(parameter) = &fld.field_type {
        json_argument_value(parameter)
    } else {
        quote! { serde_json::json!({ "type": "object" }) }
    };
    MergedSource {
        label: fld.name.clone(),
        optional: fld.is_optional(),
        value,
    }
}

/// What one merged source is written as inside the intersection.
#[cfg(feature = "typescript")]
fn ts_merged_operand(operand: &MergedOperand) -> String {
    let spelling = &operand.spelling;
    match operand.absence {
        SourceAbsence::Never => spelling.clone(),
        SourceAbsence::Field => format!("({spelling} | {{ [K in keyof {spelling}]?: never }})"),
        SourceAbsence::Published => {
            format!("({spelling} | {{ [K in keyof NonNullable<{spelling}>]?: never }})")
        }
    }
}

#[cfg(feature = "typescript")]
/// Generates the TypeScript definition method (TypeScript types only, no Zod schema).
fn generate_ts_definition_method(
    docs: &str,
    item_name: &str,
    rust_ident: &str,
    ts_generics: &str,
    type_code: &str,
    fields_empty: bool,
    flatten_types: &[MergedOperand],
) -> proc_macro2::TokenStream {
    let reexport = ident_reexport_ts(rust_ident, item_name, ts_generics);
    let has_flatten = !flatten_types.is_empty();
    let operands: Vec<String> = flatten_types.iter().map(ts_merged_operand).collect();
    let intersection_only = operands.join(" & ");
    let intersection_suffix: String = operands.iter().fold(String::new(), |mut acc, t| {
        let _ = write!(acc, " & {t}");
        acc
    });

    let typescript_type_gen = if fields_empty {
        if has_flatten {
            quote::quote! {
                format!("{}export type {}{} = {};{}", docs, #item_name, #ts_generics, #intersection_only, #reexport)
            }
        } else {
            quote::quote! {
                format!("{}export type {}{} = Record<string, never>;{}", docs, #item_name, #ts_generics, #reexport)
            }
        }
    } else if has_flatten {
        quote::quote! {
            format!("{}export type {}{} = {{\n{}}}{};{}", docs, #item_name, #ts_generics, #type_code, #intersection_suffix, #reexport)
        }
    } else {
        quote::quote! {
            format!("{}export type {}{} = {{\n{}}};{}", docs, #item_name, #ts_generics, #type_code, #reexport)
        }
    };

    #[cfg(feature = "jsonschema")]
    let json_docs_gen = bind_item_jsdoc_local(docs, true);

    #[cfg(not(feature = "jsonschema"))]
    let json_docs_gen = bind_item_jsdoc_local(docs, false);

    quote::quote! {
        pub fn ts_definition() -> String {
            #json_docs_gen
            #typescript_type_gen
        }
    }
}

/// A schema an intersection is built from, written so the name it carries is read when the
/// intersection is used rather than while the `const` holding it is being initialized.
#[cfg(feature = "zod")]
fn deferred_zod_operand(schema: &str) -> String {
    format!("z.lazy(() => {schema})")
}

/// The tokens `typescript_preamble!()` expands to — the preamble as a string literal, or the
/// `compile_error!` for a call that was handed arguments it has none of.
#[cfg(feature = "zod")]
pub fn typescript_preamble_tokens(input: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
    if !input.is_empty() {
        return syn::Error::new_spanned(
            input,
            "tixschema: `typescript_preamble!()` takes no arguments",
        )
        .to_compile_error();
    }
    quote! { #TYPESCRIPT_PREAMBLE }
}

/// The suffix the binding an item publishes is named with. A generic type publishes a factory —
/// one schema per filling, built on demand — where a type that declares no parameter publishes the
/// one schema it has.
#[cfg(feature = "zod")]
fn zod_binding_suffix(rust_ident: &str, parameters: &[String]) -> &'static str {
    record_zod_factory(rust_ident, !parameters.is_empty());
    if parameters.is_empty() {
        "$Schema"
    } else {
        "$SchemaFactory"
    }
}

/// The zod re-export text an item's module ends with — the factory's own alongside the default's,
/// so a renamed generic item's alias answers to both names exactly as its own declaration does.
///
/// [`ident_reexport_zod`] writes one binding at a time; a type with no parameter has only one to
/// write, and a generic type has two — its factory and the default this task adds beside it.
#[cfg(feature = "zod")]
fn zod_binding_reexport(rust_ident: &str, export_name: &str, parameters: &[String]) -> String {
    let mut reexport = ident_reexport_zod(
        rust_ident,
        export_name,
        zod_binding_suffix(rust_ident, parameters),
    );
    if !parameters.is_empty() {
        reexport.push_str(&ident_reexport_zod(
            rust_ident,
            export_name,
            "$SchemaDefault",
        ));
    }
    reexport
}

/// The identifier holding the expression a factory's arguments compose into. Bound beside the
/// factory rather than inlined in it, so the factory's return type can be read back off it and
/// neither can drift from the other.
#[cfg(feature = "zod")]
fn zod_factory_builder_name(item_name: &str) -> String {
    format!("build{item_name}$Schema")
}

/// The `TypeScript` parameter list a factory and everything written around it are declared under —
/// `<IdType extends ZodType, DateType extends ZodType>`.
///
/// Every parameter is a parameter of the function for real. A bare `ZodType` annotation compiles
/// and infers nothing: `ZodType` defaults its own parameters, so a field validated by such an
/// argument comes back as `unknown` and the caller learns nothing from the schema it was handed.
#[cfg(all(feature = "zod", feature = "typescript"))]
fn zod_factory_bounds(parameters: &[String]) -> String {
    format!(
        "<{}>",
        parameters
            .iter()
            .map(|parameter| format!("{parameter} extends ZodType"))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

/// The argument list a factory and its builder are written with, one line per parameter. Required,
/// every one of them: a default would let a call site say nothing about a filling and still get a
/// schema back, which is the silent mis-validation the factory exists to prevent.
#[cfg(feature = "zod")]
fn zod_factory_arguments(parameters: &[String]) -> String {
    parameters.iter().fold(String::new(), |mut acc, parameter| {
        let argument = zod_factory_argument(parameter);
        #[cfg(feature = "typescript")]
        let _ = write!(acc, "\n  {argument}: {parameter},");
        #[cfg(not(feature = "typescript"))]
        let _ = write!(acc, "\n  {argument},");
        acc
    })
}

/// The cache interface keying on the parameter at `depth`, named for how many parameters it has
/// already resolved: the root resolves none, and each level below carries the ones above it.
#[cfg(all(feature = "zod", feature = "typescript"))]
fn zod_cache_level_name(item_name: &str, depth: usize) -> String {
    if depth == 0 {
        format!("{item_name}$SchemaFactoryCache")
    } else {
        format!("{item_name}$SchemaFactoryCacheL{depth}")
    }
}

/// What a lookup at `depth` hands back: the level below it while parameters remain, and the schema
/// itself once the last one is resolved.
#[cfg(all(feature = "zod", feature = "typescript"))]
fn zod_cache_level_value(item_name: &str, parameters: &[String], depth: usize) -> String {
    if depth + 1 == parameters.len() {
        format!("{item_name}$SchemaOf<{}>", parameters.join(", "))
    } else {
        format!(
            "{}<{}>",
            zod_cache_level_name(item_name, depth + 1),
            parameters[..=depth].join(", ")
        )
    }
}

/// One cache interface per parameter, each written generic over the key it is about to resolve and
/// carrying the parameters resolved above it.
///
/// Nesting spelled to the exact depth the type declares rather than looped over: a loop needs a
/// key type it cannot name and a value it cannot type, which is `unknown` and a cast at every
/// level. Written out, a lookup comes back already typed and the factory body asserts nothing.
#[cfg(all(feature = "zod", feature = "typescript"))]
fn zod_cache_interfaces(item_name: &str, parameters: &[String]) -> String {
    let mut written = String::new();
    for depth in (1..parameters.len()).chain(once(0)) {
        let key = &parameters[depth];
        let value = zod_cache_level_value(item_name, parameters, depth);
        let held = if depth == 0 {
            String::new()
        } else {
            zod_factory_bounds(&parameters[..depth])
        };
        let _ = write!(
            written,
            "interface {}{held} {{\n  get<{key} extends ZodType>(key: {key}): {value} | \
             undefined;\n  set<{key} extends ZodType>(key: {key}, value: {value}): this;\n}}\n\n",
            zod_cache_level_name(item_name, depth)
        );
    }
    written
}

/// The factory's body: walk down to the map the last argument keys, hand back what is already
/// there, and otherwise build once, store, and return the very schema that was stored — which is
/// what makes two calls with the same arguments the same schema rather than two that agree.
///
/// Every argument is read on the way down, so no two argument lists share a slot: a change in the
/// first re-keys the outermost map and a change in the last re-keys the one the schema sits in.
#[cfg(feature = "zod")]
fn zod_factory_body(item_name: &str, parameters: &[String]) -> String {
    let arguments: Vec<String> = parameters.iter().map(|p| zod_factory_argument(p)).collect();
    let mut body = String::new();
    let mut holder = format!("{item_name}$SchemaFactoryCache");
    for depth in 1..parameters.len() {
        let below = format!("by{}", parameters[depth]);
        let key = &arguments[depth - 1];
        #[cfg(feature = "typescript")]
        let fresh = format!(
            "createSchemaCache<{}<{}>>()",
            zod_cache_level_name(item_name, depth),
            parameters[..depth].join(", ")
        );
        #[cfg(not(feature = "typescript"))]
        let fresh = "createSchemaCache()".to_owned();
        let _ = write!(
            body,
            "  let {below} = {holder}.get({key});\n  if (!{below}) {{\n    {below} = \
             {fresh};\n    {holder}.set({key}, {below});\n  }}\n\n"
        );
        holder = below;
    }
    let last = arguments.last().map_or_else(String::new, Clone::clone);
    let _ = write!(
        body,
        "  const hit = {holder}.get({last});\n  if (hit) return hit;\n\n  {};\n  {holder}.set({last}, \
         schema);\n  return schema;",
        zod_factory_memoized_binding(item_name, parameters)
    );
    body
}

/// The statement a factory binds the schema it is about to cache to, without its terminator.
///
/// Written once and read twice: [`zod_factory_body`] emits it, and the delegate injecting the
/// item's example anchors on it — so the anchor cannot drift from the line it names.
#[cfg(feature = "zod")]
fn zod_factory_memoized_binding(item_name: &str, parameters: &[String]) -> String {
    format!(
        "const schema = {}({})",
        zod_factory_builder_name(item_name),
        parameters
            .iter()
            .map(|parameter| zod_factory_argument(parameter))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

/// The `FieldDef` one `default_types` entry parses as: the declared filling for `parameter`, or —
/// absent one — `String`, [`schema_example_value_type`]'s own fallback for the identical gap. The
/// fallback is only ever the *answer* in a build without `jsonschema`, the one feature that
/// requires every parameter to declare one — but the constrained-brand guard,
/// [`branded_constraint_inner_error`], reads this in every build, string constraints being a
/// question the Zod and Rust surfaces answer regardless of `jsonschema`.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn declared_default_field(parameter: &str, default_types: &[(syn::Ident, syn::Type)]) -> FieldDef {
    default_types
        .iter()
        .find(|(declared, _)| declared == parameter)
        .map_or_else(
            || get_field_def(parameter, &syn::parse_quote!(String), ""),
            |(_, ty)| get_field_def(parameter, ty, ""),
        )
}

/// The rendering one declared default composes: the ordinary rendering [`FieldDef::zod_type`]
/// gives every field, deferred wherever that rendering names another item's own module-scope
/// binding — its factory, called with fresh arguments exactly as an ordinary field naming it does,
/// or, where the default names that item at exactly the arguments it calls its own, that item's
/// `$SchemaDefault` directly (the fold).
#[cfg(feature = "zod")]
fn default_zod_rendering(field: &FieldDef) -> DefaultZodRendering {
    if field.array_depth == 0
        && !field.is_optional()
        && let FieldDefType::SiblingType(name, args) = &field.field_type
    {
        if publishes_zod_factory(name)
            && let Some(info) = lookup_alias_info(name)
        {
            let rendered: Vec<String> = args.iter().map(FieldDef::zod_type).collect();
            if zod_default_arguments(name).as_deref() == Some(rendered.as_slice()) {
                return DefaultZodRendering::Deferred(format!(
                    "{}$SchemaDefault",
                    info.export_name
                ));
            }
        }
        return DefaultZodRendering::Deferred(field.zod_type());
    }
    if field.names_a_sibling_binding() {
        return DefaultZodRendering::Deferred(field.zod_type());
    }
    DefaultZodRendering::Eager(field.zod_type())
}

/// What a generic item appends after its factory: the factory called with each parameter's
/// declared default, so `X$SchemaDefault === X$SchemaFactory(<the same arguments>)` by
/// construction — through the very factory a hand-written call would go through, never the
/// builder, which skips the memo and would build a second schema for the same filling.
#[cfg(feature = "zod")]
fn zod_default_block(
    item_name: &str,
    rust_ident: &str,
    parameters: &[String],
    defaults: &ZodDefaultInputs<'_>,
) -> String {
    let fields: Vec<FieldDef> = parameters
        .iter()
        .map(|parameter| declared_default_field(parameter, defaults.default_types))
        .collect();
    let fold_keys: Vec<String> = fields.iter().map(FieldDef::zod_type).collect();
    record_zod_default_arguments(rust_ident, fold_keys);
    let renderings: Vec<DefaultZodRendering> = fields.iter().map(default_zod_rendering).collect();

    #[cfg(feature = "typescript")]
    let annotation =
        branded_default_annotation(item_name, defaults, parameters, &fields, &renderings);
    #[cfg(not(feature = "typescript"))]
    let annotation = String::new();

    let arguments: Vec<String> = parameters
        .iter()
        .zip(renderings)
        .map(|(parameter, rendering)| match defaults.constrained {
            Some((target, checks)) if target == parameter.as_str() => match rendering {
                DefaultZodRendering::Eager(schema) => {
                    format!("{schema}{}", checks.chained)
                }
                DefaultZodRendering::Deferred(schema) => {
                    deferred_zod_operand(&format!("{schema}{}", checks.base))
                }
            },
            _ => rendering.into_argument(),
        })
        .collect();

    format!(
        "\n\nexport const {item_name}$SchemaDefault{annotation} = {item_name}$SchemaFactory({});",
        arguments.join(", ")
    )
}

/// The `$SchemaDefault` annotation [`zod_default_block`] writes: the plain `ZodType<Name<...>>`
/// spelling for an ordinary generic item — tsc already accepts a factory's return type there, since
/// nothing in the chain calls `.brand()`, so nothing narrows the classic `ZodType` interface's own
/// deprecated `_output` field out from under it. A branded newtype's factory always ends its chain
/// in `.brand()` (see [`branded_zod_expression`]), so its return type is always some
/// `$ZodBranded<Argument, Name>` — never the plain `Name<...>` instantiation `ZodType`
/// names — and the annotation has to read the same way, exactly as [`branded_zod_const_block`]
/// already annotates the non-generic case. The class is written bare throughout, per
/// [`branded_zod_type_name`].
#[cfg(all(feature = "zod", feature = "typescript"))]
fn branded_default_annotation(
    item_name: &str,
    defaults: &ZodDefaultInputs<'_>,
    parameters: &[String],
    fields: &[FieldDef],
    renderings: &[DefaultZodRendering],
) -> String {
    let plain_annotation = || {
        format!(
            ": ZodType<{item_name}<{}>>",
            fields
                .iter()
                .map(FieldDef::typescript_typename)
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    let Some(brand) = &defaults.brand else {
        return plain_annotation();
    };
    let argument_type = match brand {
        BrandedAnnotation::Widened(class) => class.clone(),
        BrandedAnnotation::BareParameter(target) => {
            // Structurally always found: `target` is read off this same brand's own inner, which
            // names one of its own declared parameters. Falling back to the plain annotation
            // rather than panicking if that ever stopped holding costs nothing real — a branded
            // `$SchemaDefault` is never reached without a parameter list to search.
            let Some(index) = parameters.iter().position(|parameter| parameter == target) else {
                return plain_annotation();
            };
            branded_default_argument_class(&fields[index], &renderings[index])
        }
    };
    format!(": $ZodBranded<{argument_type}, \"{item_name}\">")
}

/// The class a bare-parameter brand's own declared-default argument is annotated with, for
/// [`branded_default_annotation`]: the eager expression's own class from [`branded_zod_type_name`]
/// bare, or that same class wrapped in `ZodLazy<...>` for a deferred one — `typeof {schema}` when
/// the deferred target is a plain binding name, which is every deferred shape
/// [`default_zod_rendering`] folds or defers to a sibling's own `$Schema`/`$SchemaDefault`; the
/// widened class otherwise, for the one shape it is not — a declared default naming a generic
/// sibling at some filling other than that sibling's own, whose un-folded reference is a factory
/// call rather than a name `typeof` can read.
#[cfg(all(feature = "zod", feature = "typescript"))]
fn branded_default_argument_class(field: &FieldDef, rendering: &DefaultZodRendering) -> String {
    match rendering {
        DefaultZodRendering::Eager(_) => branded_zod_type_name(field),
        DefaultZodRendering::Deferred(schema) if !schema.contains('(') => {
            format!("ZodLazy<typeof {schema}>")
        }
        DefaultZodRendering::Deferred(_) => format!("ZodLazy<{}>", branded_zod_type_name(field)),
    }
}

/// What a generic type's Zod surface is written as: the builder holding the schema its arguments
/// compose into, the return type read back off it, the cache interfaces, and the exported factory.
#[cfg(feature = "zod")]
fn zod_factory_block(
    item_name: &str,
    rust_ident: &str,
    parameters: &[String],
    defaults: &ZodDefaultInputs<'_>,
    preamble: &str,
    expression: &str,
    reexport: &str,
) -> String {
    let builder = zod_factory_builder_name(item_name);
    let arguments = zod_factory_arguments(parameters);
    let body = zod_factory_body(item_name, parameters);

    #[cfg(feature = "typescript")]
    let bounds = zod_factory_bounds(parameters);
    #[cfg(not(feature = "typescript"))]
    let bounds = String::new();

    // A merged object binds its own keys ahead of the branches that read them, and those keys are
    // composed from the arguments — so the binding belongs inside the builder, where the arguments
    // are in scope, rather than beside it at module level.
    let built = if preamble.is_empty() {
        format!("const {builder} = {bounds}({arguments}\n) =>\n  {expression};")
    } else {
        format!(
            "const {builder} = {bounds}({arguments}\n) => {{\n  {preamble}  return {expression};\n}};"
        )
    };

    #[cfg(feature = "typescript")]
    let declarations = format!(
        "type {item_name}$SchemaOf{bounds} = ReturnType<\n  typeof {builder}<{}>\n>;\n\n{}const \
         {item_name}$SchemaFactoryCache = \
         createSchemaCache<{item_name}$SchemaFactoryCache>();\n\n",
        parameters.join(", "),
        zod_cache_interfaces(item_name, parameters)
    );
    #[cfg(not(feature = "typescript"))]
    let declarations = format!("const {item_name}$SchemaFactoryCache = createSchemaCache();\n\n");

    #[cfg(feature = "typescript")]
    let returns = format!(": {item_name}$SchemaOf<{}>", parameters.join(", "));
    #[cfg(not(feature = "typescript"))]
    let returns = String::new();

    let default_block = zod_default_block(item_name, rust_ident, parameters, defaults);

    format!(
        "{built}\n\n{declarations}export const {item_name}$SchemaFactory = \
         {bounds}({arguments}\n){returns} => {{\n{body}\n}};{default_block}{reexport}"
    )
}

/// The binding a type that declares no parameter publishes: the raw schema, then the exported
/// `const` annotated with the type it validates.
///
/// The annotation is the only place a TypeScript type is named, so a build without `typescript`
/// writes the same value under a bare `const`.
#[cfg(feature = "zod")]
fn zod_const_block(item_name: &str, preamble: &str, expression: &str, reexport: &str) -> String {
    #[cfg(feature = "typescript")]
    {
        format!(
            "{preamble}const {item_name}$RawSchema = {expression};\n\nexport const \
             {item_name}$Schema: ZodType<{item_name}> = {item_name}$RawSchema;{reexport}"
        )
    }
    #[cfg(not(feature = "typescript"))]
    {
        format!("{preamble}export const {item_name}$Schema = {expression};{reexport}")
    }
}

/// The whole of what a type publishes on the Zod surface: a factory when it declares parameters,
/// and the annotated `const` when it declares none.
#[cfg(feature = "zod")]
fn zod_published_binding(
    item_name: &str,
    rust_ident: &str,
    parameters: &[String],
    default_types: &[(syn::Ident, syn::Type)],
    preamble: &str,
    expression: &str,
    reexport: &str,
) -> String {
    if parameters.is_empty() {
        zod_const_block(item_name, preamble, expression, reexport)
    } else {
        let defaults = ZodDefaultInputs {
            #[cfg(feature = "typescript")]
            brand: None,
            constrained: None,
            default_types,
        };
        zod_factory_block(
            item_name, rust_ident, parameters, &defaults, preamble, expression, reexport,
        )
    }
}

/// The expression the delegate builds its schema-with-example from, read off the same decision
/// [`zod_published_binding`] takes: where the example goes depends on what the item published.
#[cfg(feature = "zod")]
fn zod_example_injection(item_name: &str, parameters: &[String]) -> proc_macro2::TokenStream {
    if parameters.is_empty() {
        return quote! {{
            let example_part = format!(".meta({{\n  example: {}\n}})", example_json);
            if let Some(pos) = defined.rfind(';') {
                let mut injected = defined[..pos].to_string();
                injected.push_str(&example_part);
                injected.push(';');
                injected
            } else {
                format!("{}{}", defined, example_part)
            }
        }};
    }
    let binding = zod_factory_memoized_binding(item_name, parameters);
    let anchor = format!("{binding};");
    quote! {
        defined.replace(
            #anchor,
            &format!("{}.meta({{\n    example: {}\n  }});", #binding, example_json),
        )
    }
}

/// What one merged source contributes to each branch: one schema per branch of the choice it names,
/// and the source itself when it names none.
#[cfg(feature = "zod")]
fn zod_operand_contributions(operand: &MergedOperand) -> Vec<&str> {
    if operand.branches.is_empty() {
        vec![operand.spelling.as_str()]
    } else {
        operand.branches.iter().map(String::as_str).collect()
    }
}

/// What the object's schema is written as: the statements bound ahead of it, and the expression
/// itself.
#[cfg(feature = "zod")]
fn zod_merged_statements(
    item_name: &str,
    own: &str,
    operands: &[MergedOperand],
) -> (String, String) {
    // Built as the joins alone so the root can be decided once every combination is counted: one
    // combination writes the object's own keys where they stood, and more bind them to a name.
    let mut joins = vec![String::new()];
    for operand in operands {
        joins = joins
            .iter()
            .flat_map(|join| {
                let mut grown: Vec<String> = zod_operand_contributions(operand)
                    .into_iter()
                    .map(|schema| format!("{join}.and({})", deferred_zod_operand(schema)))
                    .collect();
                if operand.absence.offered() {
                    grown.push(join.clone());
                }
                grown
            })
            .collect();
    }

    if let [only] = joins.as_slice() {
        return (String::new(), format!("{own}{only}"));
    }

    let own_name = format!("{item_name}$OwnSchema");
    let written = joins.iter().fold(String::new(), |mut acc, join| {
        let _ = writeln!(acc, "  {own_name}{join},");
        acc
    });
    (
        format!("const {own_name} = {own};\n\n"),
        format!("z.union([\n{written}])"),
    )
}

#[cfg(feature = "zod")]
/// Generates the Zod schema method (Zod schemas only, no TypeScript types).
///
/// Each flattened base joins an intersection through [`deferred_zod_operand`]: a base names a
/// `const` of its own, and one macro invocation sees one type, so nothing here can know whether
/// that `const` is declared above the module this writes or below it. How many intersections there
/// are, and what is bound ahead of them, is [`zod_merged_statements`]'.
fn generate_zod_schema_method(
    item_name: &str,
    rust_ident: &str,
    parameters: &[String],
    default_types: &[(syn::Ident, syn::Type)],
    schema_code: &str,
    show_opts: &str,
    flatten_schemas: &[MergedOperand],
) -> proc_macro2::TokenStream {
    #[cfg(feature = "zod")]
    {
        let reexport = zod_binding_reexport(rust_ident, item_name, parameters);
        let own = format!("z.strictObject({{\n{schema_code}}}){show_opts}");
        let (preamble, expression) = zod_merged_statements(item_name, &own, flatten_schemas);
        // Note: Example injection is handled by the delegating method on the type itself.
        let body = zod_published_binding(
            item_name,
            rust_ident,
            parameters,
            default_types,
            &preamble,
            &expression,
            &reexport,
        );

        quote::quote! {
            pub fn zod_schema() -> String {
                #body.to_owned()
            }
        }
    }

    #[cfg(not(feature = "zod"))]
    {
        let _: &_ = &(
            item_name,
            rust_ident,
            parameters,
            default_types,
            schema_code,
            show_opts,
            flatten_schemas,
        );
        quote::quote! {
            // No method: the `zod` feature is off.
        }
    }
}

/// Binds the `docs` local an item's `ts_definition()` opens with: the item's `JSDoc` block, its
/// body enriched with the item's JSON schema where the caller says this build can produce one, and
/// a trailing newline, so what the block documents is the line straight beneath it.
#[cfg(feature = "typescript")]
fn bind_item_jsdoc_local(docs: &str, with_json_schema: bool) -> proc_macro2::TokenStream {
    if with_json_schema {
        quote::quote! {
            let prettified = serde_json::to_string_pretty(&Self::json_schema()).unwrap().lines().map(|l| format!(" * {l}")).collect::<Vec<_>>().join("\n");
            let docs = format!("/**\n{}\n * JSON Schema:\n{}\n */\n", #docs, prettified);
        }
    } else {
        quote::quote! {
            let docs = format!("/**\n{}\n */\n", #docs);
        }
    }
}

/// Binds the `docs` local an enum's `ts_definition()` renders, enriched with the JSON schema when
/// this build of tixschema can produce one.
///
/// The enrichment reads `Self::json_schema()`, so it may only be emitted when tixschema's own
/// features put that method in the schema module — deciding here rather than emitting a `cfg`
/// keeps the reader and the method in agreement regardless of the consumer's feature table.
#[cfg(feature = "typescript")]
fn generate_enum_json_docs_part(docs: &str) -> proc_macro2::TokenStream {
    bind_item_jsdoc_local(docs, cfg!(all(feature = "jsonschema", feature = "zod")))
}

#[cfg(feature = "jsonschema")]
/// Generates the JSON schema method for plain enums conditionally.
fn generate_plain_enum_json_schema_method(
    enumerated: &[proc_macro2::TokenStream],
    def_name: &str,
    parameters: &[SchemaParameter],
) -> proc_macro2::TokenStream {
    #[cfg(feature = "jsonschema")]
    {
        generate_plain_enum_json_schema_method_impl(enumerated, def_name, parameters)
    }

    #[cfg(not(feature = "jsonschema"))]
    {
        let _: &_ = &(enumerated, def_name); // Suppress unused variable warning
        quote::quote! {
            // No method: the `jsonschema` feature is off.
        }
    }
}

#[cfg(feature = "typescript")]
/// Generates the TypeScript definition method for plain enums (TypeScript types only).
fn generate_plain_enum_ts_definition_method(
    docs: &str,
    item_name: &str,
    rust_ident: &str,
    ts_generics: &str,
    type_code: &str,
) -> proc_macro2::TokenStream {
    #[cfg(feature = "typescript")]
    {
        let json_docs_gen = generate_enum_json_docs_part(docs);
        let reexport = ident_reexport_ts(rust_ident, item_name, ts_generics);

        let typescript_type_gen = quote::quote! {
            format!("{}export type {}{} =\n{};{}", docs, #item_name, #ts_generics, #type_code, #reexport)
        };

        quote::quote! {
            pub fn ts_definition() -> String {
                #json_docs_gen
                #typescript_type_gen
            }
        }
    }

    #[cfg(not(feature = "typescript"))]
    {
        quote::quote! {
            // No method: the `typescript` feature is off.
        }
    }
}

#[cfg(feature = "zod")]
/// Generates the Zod schema method for plain enums (Zod schemas only)
/// Note: Example injection is handled by the delegating method on the type itself.
///
/// A plain enum publishes the one schema it has whatever it declares: Rust refuses a *type*
/// parameter no variant uses, and every variant of a plain enum is a unit, so there is never a
/// parameter here for a factory to bind.
fn generate_plain_enum_zod_schema_method(
    item_name: &str,
    rust_ident: &str,
    schema_code: &str,
    description: &str,
) -> proc_macro2::TokenStream {
    #[cfg(feature = "zod")]
    {
        let reexport = ident_reexport_zod(rust_ident, item_name, "$Schema");
        // When typescript feature is enabled, generate TypeScript-style Zod schema
        #[cfg(feature = "typescript")]
        {
            quote::quote! {
                pub fn zod_schema() -> String {
                    format!("const {}$RawSchema = z.enum([{}]).meta({{\n  description: \"{}\",\n}});\n\nexport const {}$Schema: ZodType<{}> = {}$RawSchema;{}", #item_name, #schema_code, #description, #item_name, #item_name, #item_name, #reexport)
                }
            }
        }

        // When typescript feature is disabled, generate JavaScript-style Zod schema
        #[cfg(not(feature = "typescript"))]
        {
            quote::quote! {
                pub fn zod_schema() -> String {
                    format!("export const {}$Schema = z.enum([{}]).meta({{\n  description: \"{}\",\n}});{}", #item_name, #schema_code, #description, #reexport)
                }
            }
        }
    }

    #[cfg(not(feature = "zod"))]
    {
        let _: &_ = &(item_name, rust_ident, schema_code, description);
        quote::quote! {
            // No method: the `zod` feature is off.
        }
    }
}

#[cfg(feature = "jsonschema")]
/// Generates the JSON schema method for discriminated enums conditionally.
fn enum_json_schema_methods(
    main_schema_code: &proc_macro2::TokenStream,
    def_name: &str,
    generics: &syn::Generics,
    args: &ModelSchemaArgs,
) -> proc_macro2::TokenStream {
    json_schema_methods(
        def_name,
        &quote::quote! { { #main_schema_code } },
        &schema_parameters(generics, args),
    )
}

#[cfg(feature = "typescript")]
/// Generates the TypeScript definition method for discriminated enums (TypeScript types only).
fn generate_discriminated_enum_ts_definition_method(
    docs: &str,
    item_name: &str,
    rust_ident: &str,
    ts_generics: &str,
    type_code: &str,
) -> proc_macro2::TokenStream {
    #[cfg(feature = "typescript")]
    {
        let json_docs_gen = generate_enum_json_docs_part(docs);
        let reexport = ident_reexport_ts(rust_ident, item_name, ts_generics);

        quote::quote! {
            pub fn ts_definition() -> String {
                #json_docs_gen
                let bundled_docs = docs;
                format!(r#"{bundled_docs}export type {}{} = {};{}"#, #item_name, #ts_generics, #type_code, #reexport)
            }
        }
    }

    #[cfg(not(feature = "typescript"))]
    {
        quote::quote! {
            // No method: the `typescript` feature is off.
        }
    }
}

#[cfg(feature = "zod")]
/// Generates the Zod schema method for discriminated enums (Zod schemas only)
/// Note: Example injection is handled by the delegating method on the type itself.
fn generate_discriminated_enum_zod_schema_method(
    item_name: &str,
    rust_ident: &str,
    parameters: &[String],
    default_types: &[(syn::Ident, syn::Type)],
    schema_code: &str,
) -> proc_macro2::TokenStream {
    #[cfg(feature = "zod")]
    {
        let reexport = zod_binding_reexport(rust_ident, item_name, parameters);
        let schema_str = zod_published_binding(
            item_name,
            rust_ident,
            parameters,
            default_types,
            "",
            schema_code,
            &reexport,
        );
        quote::quote! {
            pub fn zod_schema() -> String {
                #schema_str.to_owned()
            }
        }
    }

    #[cfg(not(feature = "zod"))]
    {
        let _: &_ = &(
            item_name,
            rust_ident,
            parameters,
            default_types,
            schema_code,
        );
        quote::quote! {
            // No method: the `zod` feature is off.
        }
    }
}

/// Builds the alias module's `ts_definition()`, or nothing when `typescript` is off. The doc
/// block and the generic parameter list are only meaningful to TypeScript, so they are gathered
/// inside the gate rather than by the caller.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn generate_alias_ts_definition_method(
    alias: &ItemType,
    export_name: &str,
    field_def: &FieldDef,
) -> proc_macro2::TokenStream {
    #[cfg(feature = "typescript")]
    {
        let docs_formatted = alias_jsdoc_body(get_item_docs(&alias.attrs).as_deref(), export_name);

        generate_ts_alias_method(
            &docs_formatted,
            export_name,
            &alias.ident.to_string(),
            &ts_generic_params(&alias.generics),
            &surface_field_def(&alias.generics, field_def),
        )
    }
    #[cfg(not(feature = "typescript"))]
    {
        let _: &_ = &(alias, export_name, field_def);
        quote! {}
    }
}

#[cfg(feature = "typescript")]
fn generate_ts_alias_method(
    docs: &str,
    export_name: &str,
    rust_ident: &str,
    ts_generics: &str,
    field_def: &FieldDef,
) -> proc_macro2::TokenStream {
    let alias_name_ts = format!("{export_name}{ts_generics}");
    let target_ts = field_def.typescript_typename();
    let reexport = ident_reexport_ts(rust_ident, export_name, ts_generics);

    let docs_block = jsdoc_block(docs, "");

    quote! {
        pub fn ts_definition() -> String {
            format!(
                "{}\nexport type {} = {};{}",
                #docs_block,
                #alias_name_ts,
                #target_ts,
                #reexport
            )
        }
    }
}

/// The diagnostic an alias whose target the dispatch cannot render emits, in place of the whole
/// `json_schema()` body.
#[cfg(feature = "jsonschema")]
fn alias_json_schema_rejection(
    export_name: &str,
    rejection: &MapMemberRejection,
) -> proc_macro2::TokenStream {
    let message = map_member_rejection_message(&format!("type alias `{export_name}`"), rejection);
    quote! { compile_error!(#message) }
}

/// Builds the alias module's `json_schema()`, or nothing when `jsonschema` is off.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn generate_alias_json_schema_method(
    alias: &ItemType,
    export_name: &str,
    field_def: &FieldDef,
    args: &ModelSchemaArgs,
) -> proc_macro2::TokenStream {
    #[cfg(feature = "jsonschema")]
    {
        let body = build_tuple_element_json_schema(&surface_field_def(&alias.generics, field_def))
            .unwrap_or_else(|rejection| alias_json_schema_rejection(export_name, &rejection));
        json_schema_methods(
            export_name,
            &body,
            &schema_parameters(&alias.generics, args),
        )
    }
    #[cfg(not(feature = "jsonschema"))]
    {
        // Nothing in this build references an alias module's `json_schema()`; the sibling
        // reference that would (`flatten_field_json_schema_ref`) is itself jsonschema-gated.
        let _: &_ = &(alias, export_name, field_def, args);
        quote! {}
    }
}

/// Builds the alias module's `zod_schema()`, or nothing when `zod` is off.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn generate_alias_zod_method(
    alias: &ItemType,
    export_name: &str,
    rust_ident: &str,
    field_def: &FieldDef,
    default_types: &[(syn::Ident, syn::Type)],
) -> proc_macro2::TokenStream {
    #[cfg(feature = "zod")]
    {
        // The alias's rendered Zod is its FieldDef expression (a tuple alias yields
        // the null-flavored `z.tuple([...])`, a scalar yields `z.string()`, a sibling
        // yields `Name$Schema`).
        let schema_code = surface_field_def(&alias.generics, field_def).zod_type();
        let parameters = type_parameters_in_scope(&alias.generics);
        let reexport = zod_binding_reexport(rust_ident, export_name, &parameters);
        let body = zod_published_binding(
            export_name,
            rust_ident,
            &parameters,
            default_types,
            "",
            &schema_code,
            &reexport,
        );
        quote! {
            pub fn zod_schema() -> String {
                #body.to_owned()
            }
        }
    }
    #[cfg(not(feature = "zod"))]
    {
        // Without the `zod` feature, `FieldDef::zod_type` does not exist; nothing in
        // this build has zod enabled, so the schema method would be cfg'd out anyway.
        let _: &_ = &(alias, export_name, rust_ident, field_def, default_types);
        quote! {}
    }
}

#[cfg(test)]
mod tests;

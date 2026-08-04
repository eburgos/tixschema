extern crate alloc;

use alloc::borrow::ToOwned;
use core::fmt::Write as _;

use proc_macro::TokenStream;
use quote::quote;
use syn::parse::Parser as _;
use syn::punctuated::Punctuated;
use syn::{Field, Item, ItemType, Meta, Token, parse_macro_input};

#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
use quote::quote_spanned;

#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
use syn::spanned::Spanned as _;

#[cfg(feature = "typescript")]
use syn::GenericParam;

#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
use syn::Ident;

use crate::{
    field_type::{
        FieldDef, FieldDefType, VariantKind, classify_variant, get_field_def, is_plain_enum,
    },
    utils::{get_field_docs, get_variant_docs},
};

// The item paths take their exported name from `compute_item_export_name` and their module name
// from `ident_schema_module_name`, both of which apply this themselves; what is left is the
// parameter list a generic alias writes.
#[cfg(feature = "typescript")]
use crate::utils::safe_type_name;

#[cfg(feature = "zod")]
use crate::utils::{escape_js_regex_literal, extract_example_from_docs};

#[cfg(all(feature = "zod", feature = "object_id"))]
use crate::features::object_id::get_object_id_zod_schema_with;

#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
use crate::utils::{AliasKind, lookup_alias_info, portable_pattern};

#[cfg(feature = "serde")]
use crate::features::serde::{SerdeFieldMeta, SerdeTypeMeta, has_serde_default};

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
    MergedSource,
    generate_plain_enum_json_schema_method as generate_plain_enum_json_schema_method_impl,
    generate_struct_json_schema_method as generate_struct_json_schema_method_impl,
    json_schema_methods,
};

#[cfg(all(feature = "serde", feature = "jsonschema"))]
use crate::features::jsonschema::{MergeDiagnostic, merged_object_value};

#[cfg(any(feature = "typescript", feature = "zod"))]
use crate::utils::{get_enum_docs, get_struct_docs, strip_examples_from_docs};

#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
use crate::utils::{compute_alias_export_name, ident_schema_module_name};

use crate::utils::compute_item_export_name;

#[cfg(feature = "typescript")]
use crate::utils::{format_docs_for_ts, get_item_docs};

#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
use crate::utils::register_alias_info;

#[cfg(any(
    feature = "typescript",
    feature = "zod",
    feature = "jsonschema",
    feature = "serde"
))]
use crate::utils::to_snake_case;

use crate::rename_rule::resolve_rename_rule;

/// Every argument the `model_schema` parser reads, in the order it tries them, as the
/// unknown-argument rejection names them.
///
/// An argument added to [`apply_arg`] belongs here too: `no_argument_the_parser_reads_is_rejected`
/// walks this list back through the parser and fails on any name here it does not read.
const KNOWN_ARGS: &[&str] = &["name", "pattern", "minLength", "maxLength", "no_display"];

/// What every plain-enum flatten diagnostic says about its own reach, so an author who fixes the
/// one declaration named there knows what was and was not checked around it.
#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
const FLATTENED_PLAIN_ENUM_SCOPE: &str = "Only a type the registry has already classified reaches this guard; one it has not keeps its \
     TypeScript and Zod intersection, and is refused by the JSON surface when the merge runs.";

/// One variant of a discriminated enum, carrying everything its union member is rendered from.
struct DiscriminatedVariant {
    discriminator_value: String,
    docs: String,
    field_defs: Vec<FieldDef>,
    kind: VariantKind,
}

/// Per-variant data collected from a discriminated enum, plus the collected serde validators and
/// the `compile_error!` tokens for any field-level guard violations.
///
/// The variants are a sequence, not a map: their order is the enum's declaration order and it
/// reaches the emitted output verbatim (JSON-schema `oneOf`, the TypeScript union, the Zod
/// `discriminatedUnion`). Any hash-ordered container here would re-randomize that output per
/// build.
type DiscriminatedVariantData = (
    Vec<DiscriminatedVariant>,
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

/// Per-member data collected from an untagged enum: the TypeScript member types, the Zod member
/// schemas, the JSON-schema value tokens, and the `compile_error!` tokens for any field-level
/// guard violations.
#[cfg(feature = "serde")]
type UntaggedMemberData = (
    Vec<String>,
    Vec<String>,
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
    delegate_impl_items: &'parts [proc_macro2::TokenStream],
    display_tokens: &'parts proc_macro2::TokenStream,
    generics: &'parts syn::Generics,
    generics_for_ty: &'parts syn::Generics,
    item_struct: &'parts syn::ItemStruct,
    module_ident: &'parts Ident,
    name: &'parts Ident,
    schema_example_tokens: &'parts proc_macro2::TokenStream,
    schema_impl_items: &'parts [proc_macro2::TokenStream],
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
///
/// These are the shapes no one `"type"` keyword and no one Zod class names, so a brand over one of
/// them is described by nothing but the type it holds — the array, the object, or the fixed-arity
/// array `#[serde(transparent)]` puts on the wire.
///
/// A `SiblingType` is absent: it is a name, not a shape. This expansion cannot resolve it, and the
/// type it names may itself be a brand over one string — which is a shape the brand's own
/// constraints *are* permitted over — so it keeps the description it has.
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
    ///
    /// serde writes a path as the string its `to_str` yields and refuses to write one that has
    /// none, so that rendering is the exact wire value for every path a payload can carry, and the
    /// paths where it substitutes replacement characters are the ones no payload holds. That is
    /// what lets a length or a pattern land on a path at all: it measures what the three surfaces
    /// render a constrained string for.
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
///
/// The form rides on the item because the two wrap differently, and a caller that needs a value can
/// always lift a fragment into one — so one dispatcher serves both the `String`-key path, which
/// inlines its member as `additionalProperties`, and the enum-key path, which binds it.
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
    /// A map key the registry proves carries no `enum_members()`, named so the author can act on it.
    NonEnumKey(String),
    /// A map key written under a sequence wrapper, named by the element it holds.
    SequencedKey(String),
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
    /// A `String` key, which enumerates nothing — one schema stands for every member.
    Open,
    /// A key written under a sequence wrapper, which serde writes as a JSON array. A JSON object
    /// key is a string, so serde refuses such a map outright rather than writing an object — there
    /// is no wire form here for any surface to describe.
    Sequenced,
    /// A key this expansion cannot narrow, leaving the members unconstrained.
    Unnarrowed,
}

/// Why a map key a field reaches has no rendering. Both reasons carry the name the author can act
/// on, and both stand on every surface, the key being read off the field all three render from.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
enum MapKeyRejection {
    /// The registry proves the key type carries no `enum_members()`.
    NoEnumMembers(String),
    /// The key is written under a sequence wrapper, named by the element it holds.
    Sequenced(String),
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

#[derive(Default, Clone)]
struct ModelSchemaArgs {
    /// The parser's refusal of the attribute's arguments — one it does not read, or a value it
    /// cannot read — spanned on the tokens that earned it.
    arg_rejection: Option<syn::Error>,
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
    /// cannot parse, or a construct a JavaScript regex literal cannot carry -- spanned on the
    /// literal it was written as. Only the brand path splices the argument, and only a schema
    /// feature builds that path; without one, a type-level constraint never reaches a surface at
    /// all.
    #[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
    pattern_rejection: Option<syn::Error>,
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
    match portable_pattern(lit_str) {
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
        && let Some(output) =
            guard_failure_output(&item, &[attr_guard_error(rejection, &item_label(&item))])
    {
        return output;
    }
    if let Item::Struct(item_struct) = item {
        process_struct(item_struct, &parsed_args)
    } else if let Item::Enum(item_enum) = item {
        process_enum(item_enum, &parsed_args)
    } else if let Item::Type(item_type) = item {
        process_type_alias(item_type, &parsed_args)
    } else {
        syn::Error::new_spanned(item, "Unsupported target for model_schema")
            .to_compile_error()
            .into()
    }
}

/// Classifies what an alias resolves to, for the registry.
///
/// A `SiblingType` is the only shape that can reach a plain enum, and it answers with whatever the
/// named type registered: an alias of an alias of an enum inherits `EnumMembers` down the chain.
/// A target registered after its alias reads as `Unknown`, which callers must treat as "cannot
/// rule it out" rather than as a negative. An array (`Vec<Slot>`, `[Slot; 4]`) is a collection, not
/// the enum it holds.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn alias_target_kind(alias_field_def: &FieldDef) -> AliasKind {
    let FieldDefType::SiblingType(target_name, generic_args) = &alias_field_def.field_type else {
        return AliasKind::NoEnumMembers;
    };
    if !generic_args.is_empty() || alias_field_def.is_array() {
        return AliasKind::NoEnumMembers;
    }
    lookup_alias_info(target_name).map_or(AliasKind::Unknown, |target| target.kind)
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
///
/// Both flattened positions — an internally tagged newtype variant's content and a
/// `#[serde(flatten)]` field — put what the value writes into the object being written, so only a
/// value serde writes as an object belongs there. A plain enum writes its own variant name instead,
/// which joins nothing and which every closed schema built here rejects.
///
/// The registry is the only thing the expansion holds that can rule this out, and it answers for a
/// name it has already seen: `Unknown` is not a negative, and a struct and a branded newtype
/// register alike. So this is a partial answer by construction, and the merge is where the rest is
/// caught.
#[cfg(all(
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
fn flattened_plain_enum(inner: &FieldDef) -> Option<&str> {
    let FieldDefType::SiblingType(name, _) = &inner.field_type else {
        return None;
    };
    (alias_target_kind(inner) == AliasKind::EnumMembers).then_some(name.as_str())
}

/// The alias schema module is referenced by all three schema features — `typescript` and `zod`
/// through the alias's registered export name, `jsonschema` through a Rust path to
/// `#module_ident::Schema::json_schema()`. So the module and its `register_alias_info` call are
/// driven by the union: gating them on `typescript` alone left the jsonschema references
/// dangling. The module's *contents* are chosen per feature while building the tokens.
///
/// The module is named from the Rust ident, not from the export name, so that a type naming the
/// alias before it has expanded assumes the module the alias goes on to publish.
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

    // Registered above whatever the outcome, so a type naming a refused alias still resolves to the
    // export name the author wrote and the alias's own diagnostic stays the one they act on.
    if let Some(error) = alias_map_key_guard_error(&alias, &export_name, &alias_field_def)
        && let Some(output) = guard_failure_output(&alias, &[error])
    {
        return output;
    }

    let ts_method = generate_alias_ts_definition_method(&alias, &export_name, &alias_field_def);
    let json_schema_method =
        generate_alias_json_schema_method(&alias, &export_name, &alias_field_def);
    let zod_method = generate_alias_zod_method(&export_name, &alias_field_def);

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
/// `export type`. Serves the shapes that publish no zod `description` of their own — structs,
/// tuple structs, and the tagged and untagged enums; the shapes that publish both surfaces spell
/// them from the same lines in `build_item_docs_and_description`.
///
/// The no-docs fallback names the item as it is exported, not as it is declared in Rust, so a
/// `JSDoc` header never contradicts the `export type` one line under it.
///
/// Doc lines reach the body as written: unlike the `item_plain_doc_lines` path, this one strips no
/// ` ```rust example ` block.
#[cfg(feature = "typescript")]
fn build_item_jsdoc(docs_vec: Option<&[String]>, item_name: &str) -> String {
    docs_vec.map_or_else(
        || {
            [item_name.to_owned(), String::new()]
                .into_iter()
                .map(|l| format!(" * {l}"))
                .collect::<Vec<_>>()
                .join("\n")
        },
        |doc_lines| {
            doc_lines
                .iter()
                .flat_map(|v| v.lines().map(ToOwned::to_owned).collect::<Vec<_>>())
                .chain(vec![String::new()])
                .map(|l| format!(" * {l}"))
                .collect::<Vec<_>>()
                .join("\n")
        },
    )
}

/// Builds the type-level `schema_example()` method from extracted example code, if present.
#[cfg(feature = "zod")]
fn build_struct_schema_example(
    example_code: Option<&String>,
    name: &syn::Ident,
) -> Option<proc_macro2::TokenStream> {
    let code = example_code?;
    let code_tokens: proc_macro2::TokenStream = code.parse().unwrap();
    Some(quote! {
        pub fn schema_example() -> serde_json::Value {
            let value: #name = {
                #code_tokens
            };
            serde_json::to_value(&value).unwrap()
        }
    })
}

/// Builds the delegating impl methods (on the type itself) that forward to its schema module.
///
/// The `zod_schema` delegate injects the example into `.meta()` here rather than in the module,
/// because `Self::schema_example()` is reachable here but not from the nested module for
/// function-local types.
#[cfg(any(feature = "zod", feature = "typescript", feature = "jsonschema"))]
fn build_struct_delegate_items(
    module_ident: &Ident,
    schema_example_method: Option<&proc_macro2::TokenStream>,
    validate_method: Option<proc_macro2::TokenStream>,
) -> Vec<proc_macro2::TokenStream> {
    // A `schema_example()` method is emitted iff an example was extracted.
    #[cfg(feature = "zod")]
    let has_example = schema_example_method.is_some();
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
        quote! {
            pub fn zod_schema() -> String {
                let base_schema = #module_ident::Schema::zod_schema();
                let example_json = serde_json::to_string(&Self::schema_example()).unwrap();
                let example_part = format!(".meta({{\n  example: {}\n}})", example_json);
                // Insert .meta() before the final semicolon
                if let Some(pos) = base_schema.rfind(';') {
                    let mut result = base_schema[..pos].to_string();
                    result.push_str(&example_part);
                    result.push(';');
                    result
                } else {
                    format!("{}{}", base_schema, example_part)
                }
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
    items.extend(validate_method);
    items
}

/// Assembles the final macro output for a struct or enum: the item itself, its schema module
/// (with the per-field validation functions), and the type's delegate impl.
#[cfg(any(feature = "zod", feature = "typescript", feature = "jsonschema"))]
fn assemble_schema_output<T>(
    item: &T,
    module_ident: &Ident,
    name: &syn::Ident,
    schema_impl_items: &[proc_macro2::TokenStream],
    validation_fns: &[proc_macro2::TokenStream],
    delegate_impl_items: &[proc_macro2::TokenStream],
) -> TokenStream
where
    T: quote::ToTokens,
{
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

        impl #name {
            #(#delegate_impl_items)*
        }
    };

    log::trace!("{output}");

    TokenStream::from(output)
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

/// Computes the TypeScript types and Zod schemas contributed by a struct's `#[serde(flatten)]`
/// fields (an empty vector for either disabled output feature).
#[cfg(any(feature = "typescript", feature = "zod"))]
fn compute_flatten_outputs(flattened_fields: &[FieldDef]) -> (Vec<String>, Vec<String>) {
    #[cfg(feature = "typescript")]
    let ts_types = flattened_fields
        .iter()
        .map(FieldDef::typescript_typename)
        .collect();
    #[cfg(not(feature = "typescript"))]
    let ts_types = Vec::new();

    #[cfg(feature = "zod")]
    let zod_schemas = flattened_fields.iter().map(FieldDef::zod_type).collect();
    #[cfg(not(feature = "zod"))]
    let zod_schemas = Vec::new();

    (ts_types, zod_schemas)
}

/// The schema methods a struct's module publishes — the JSON-schema document, the TypeScript
/// definition, the Zod schema — each built only where its feature is on.
///
/// The module emits `zod_schema` without examples; example injection happens in the delegating
/// method on the type, to avoid `super::` issues.
///
/// `flattened_fields` are the ones whose own members join the object the struct writes; every
/// other field is written under a key of its own.
#[cfg(any(feature = "zod", feature = "typescript", feature = "jsonschema"))]
fn struct_schema_impl_items(
    field_defs: Vec<FieldDef>,
    flattened_fields: &[FieldDef],
    item_name: &str,
    docs: &str,
) -> Vec<proc_macro2::TokenStream> {
    #[cfg(not(feature = "typescript"))]
    let _: &str = docs;
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
        ),
        #[cfg(feature = "typescript")]
        generate_ts_definition_method(docs, item_name, &bodies.0, bodies.3, &flatten.0),
        #[cfg(feature = "zod")]
        generate_zod_schema_method(item_name, &bodies.1, "", &flatten.1),
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

/// Emits the original item followed by the `compile_error!` tokens of every violated field guard,
/// or `None` when there are none.
///
/// The item itself is kept so downstream references still resolve and the guard message stays the
/// primary error; the schema surface is deliberately dropped, since it would encode a contract the
/// field has already been shown to break.
fn guard_failure_output<ItemT>(
    item: &ItemT,
    guard_errors: &[proc_macro2::TokenStream],
) -> Option<TokenStream>
where
    ItemT: quote::ToTokens,
{
    if guard_errors.is_empty() {
        return None;
    }
    let output = quote! {
        #item
        #(#guard_errors)*
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
///
/// Why a pattern is refused is [`crate::utils::portable_pattern`]'s to say — the crate cannot parse
/// it, or JavaScript reads it as something else — so the refusal is quoted rather than restated.
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
///
/// Reading the attribute is the only thing that acts on it, so a name or value the parser stops at
/// is one no emitter ever sees: without this the surface is emitted as though the attribute had
/// been left off, and what it was written to say — a field's constraint, a type's name — goes
/// unsaid.
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
///
/// The shape is refused outright instead of being guarded, because no attribute can repair it.
/// `#[serde(transparent)]` puts the inner value on the wire by itself, so a `None` arrives as
/// `null`, and nothing suppresses that: `skip_serializing_if` needs a key to omit and a
/// transparent newtype has none. Meanwhile every generated surface contradicts that wire — the
/// TypeScript brand renders `T | undefined & $brand<"Name">`, which parses as
/// `T | (undefined & $brand<"Name">)` and so admits an unbranded `T`; the Zod schema brands a
/// `z.union([T, z.undefined()])` while its own annotation still claims the un-unioned inner; and
/// the JSON schema keeps the inner's `type`, which rejects `null`. The generic arm is no better:
/// it renders the type parameter and drops the `Option` entirely.
///
/// `is_optional` off the same `get_field_def` call the renderers make keeps the guard and the
/// contract from ever disagreeing about what counts as an `Option`.
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
///
/// The three constraints are string checks on every surface the brand renders, and each surface
/// reads them differently the moment the inner stops being a string. A `u64` inner with
/// `minLength = 3` renders `z.number().int().min(3)`, where `.min` is a numeric bound that admits
/// `42`; renders `{"type": "number", "minLength": 3}`, where `minLength` is a string-only keyword
/// that goes inert and enforces nothing; and validates in Rust against `to_string()`, which
/// demands three decimal digits and so rejects `42`. Zod also has no `z.regex` check to apply to a
/// number schema at all. A container inner never reaches that disagreement — it has no `Display`
/// to render.
///
/// A `SiblingType` inner — another brand, an unresolved user type, or a bare generic parameter —
/// is admitted, because expansion cannot know its shape. That is why the constrained path asserts
/// `Display` separately: the guard bounds the schema surfaces, the assertion bounds the Rust one.
///
/// Resolved through the same `get_field_def` call the renderers make, so the guard and the
/// contract cannot disagree about what a shape is.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn branded_constraint_inner_error(
    name: &Ident,
    inner_field: &Field,
    args: &ModelSchemaArgs,
) -> Option<proc_macro2::TokenStream> {
    if !args.has_string_constraints() {
        return None;
    }
    let shape = non_string_inner_shape(&get_field_def("_inner", &inner_field.ty, ""))?;
    Some(
        syn::Error::new_spanned(
            inner_field,
            format!(
                "model_schema: branded newtype `{name}` applies string constraints (pattern, \
                 minLength, maxLength) to a {shape} inner type, which cannot carry them: Zod \
                 reads `.min`/`.max` as bounds on the value itself and has no regex check for a \
                 non-string schema, JSON Schema ignores `minLength`/`maxLength`/`pattern` outside \
                 `\"type\": \"string\"`, and `validate()` measures the inner's `Display` \
                 rendering — three surfaces, three answers. Brand a string-typed inner, or drop \
                 the constraints."
            ),
        )
        .to_compile_error(),
    )
}

/// Names the shape an inner type resolves to that has no string for the constraints to measure, or
/// `None` when it writes exactly one and so can carry them.
///
/// An `ObjectId` answers `None` on that reading rather than on its schema's shape: it writes the
/// `$oid` object, whose single hex member is both what `Display` renders and where every surface
/// puts the checks.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
const fn non_string_inner_shape(inner: &FieldDef) -> Option<&'static str> {
    if inner.is_array() {
        return Some("container");
    }
    match &inner.field_type {
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
        FieldDefType::Unknown => Some("opaque"),
        FieldDefType::String | FieldDefType::StringLiteral(_) | FieldDefType::SiblingType(..) => {
            None
        }
        #[cfg(feature = "object_id")]
        FieldDefType::ObjectId => None,
        #[cfg(feature = "chrono")]
        FieldDefType::NaiveDate
        | FieldDefType::NaiveTime
        | FieldDefType::NaiveDateTime
        | FieldDefType::DateTime => None,
    }
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

/// The `compile_error!` tokens for a brand whose `pattern` argument the `regex` crate rejects, or
/// `None` when it carries none or it parses.
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

/// The `compile_error!` tokens for every guard a branded newtype violates: a `cfg_attr`-wrapped
/// serde attribute on the type or on its inner slot, an `Option` inner type, string constraints
/// over an inner type that cannot carry them, and a `pattern` that is not a regex.
///
/// The branded path renders the inner type straight into the brand instead of walking fields, so
/// it has to collect these itself; the ordinary struct walk never sees a transparent newtype.
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
            &item_struct.ident,
            inner_field,
            args,
        ))
        .chain(branded_pattern_error(&item_struct.ident, args))
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

/// Processes every field of a struct, returning the regular field defs, the `#[serde(flatten)]`
/// field defs, the per-field serde validation functions and `validate()` body fragments, and the
/// `compile_error!` tokens for any field-level guard violations.
fn collect_struct_fields(
    fields: &mut syn::Fields,
    rename_all: Option<&str>,
    module_name_opt: Option<&str>,
    type_name: &str,
) -> StructFieldData {
    let mut field_defs = Vec::new();
    let mut flattened_fields: Vec<FieldDef> = Vec::new();
    let mut validation_fns: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut validate_bodies: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut guard_errors: Vec<proc_macro2::TokenStream> = Vec::new();

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

        let (f_def, validation_fn, validate_body, field_guard_errors) =
            process_field(rename_all, field, module_name_opt, None, type_name);

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
        field_defs.push(f_def);
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
///
/// The registration carries the exported name, so a `name = "…"` override reaches every reference
/// to the struct as well as its own surfaces — a sibling names it in Rust and resolves to the name
/// it is exported under.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn struct_module_idents(name: &syn::Ident, name_override: Option<&str>) -> (String, String, Ident) {
    let item_name = compute_item_export_name(&name.to_string(), name_override);
    let module_name = format!("{}_schema", to_snake_case(&item_name));
    let module_ident = Ident::new(&module_name, name.span());
    register_alias_info(
        &name.to_string(),
        &item_name,
        &module_name,
        AliasKind::NoEnumMembers,
    );
    (item_name, module_name, module_ident)
}

/// The enum counterpart of [`struct_module_idents`]. `kind` differs per enum shape: only a plain
/// unit enum is given an `enum_members()`, so only it can back an enum-keyed map.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn enum_module_idents(name: &syn::Ident, item_name: &str, kind: AliasKind) -> (String, Ident) {
    let module_name = format!("{}_schema", to_snake_case(item_name));
    let module_ident = Ident::new(&module_name, name.span());
    register_alias_info(&name.to_string(), item_name, &module_name, kind);
    (module_name, module_ident)
}

/// Extracts a struct's doc lines and the first ` ```rust example ` block (if any) from them.
#[cfg(any(feature = "typescript", feature = "zod"))]
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
        &[cfg_attr_guard_error(rejection?, &format!("type `{ident}`"))],
    )
}

fn process_struct(mut item_struct: syn::ItemStruct, args: &ModelSchemaArgs) -> TokenStream {
    // Check if this is a branded newtype (transparent single-field tuple struct)
    #[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
    if is_branded_newtype(&item_struct) {
        return process_branded_newtype(item_struct, args);
    }

    // String constraints (pattern, minLength, maxLength) are only valid on branded newtypes
    assert_no_struct_string_constraints(args);

    let name = item_struct.ident.clone();

    #[cfg(feature = "serde")]
    let serde_type_meta = parse_serde_type_attributes(&item_struct.attrs);

    #[cfg(feature = "serde")]
    if let Some(output) =
        struct_cfg_attr_guard_output(&item_struct, serde_type_meta.cfg_attr_rejection.as_ref())
    {
        return output;
    }

    #[cfg(feature = "serde")]
    let rename_all = serde_type_meta.rename_all;
    #[cfg(not(feature = "serde"))]
    let rename_all: Option<String> = None;

    // A tuple struct's slots have no keys, so the named-field emitters below would render every
    // one of them under the empty ident it carries.
    #[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
    if is_tuple_struct(&item_struct) {
        return process_tuple_struct(item_struct, rename_all.as_deref(), args);
    }

    // Compute schema-module identifiers and register the struct in the alias registry.
    #[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
    let (item_name, module_name, module_ident) =
        struct_module_idents(&name, args.name_override.as_deref());

    // Extract docs early for example extraction
    #[cfg(any(feature = "typescript", feature = "zod"))]
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
        &name.to_string(),
    );
    #[cfg(not(any(feature = "typescript", feature = "zod", feature = "jsonschema")))]
    let _: &_ = &&collected;

    // A violated field guard makes the whole contract unsound, so the schema surface is dropped
    // and only the original item plus the errors are emitted.
    if let Some(output) = guard_failure_output(&item_struct, &collected.4) {
        return output;
    }

    #[cfg(feature = "typescript")]
    let docs = build_item_jsdoc(docs_and_example.0.as_deref(), &item_name);
    #[cfg(all(
        not(feature = "typescript"),
        any(feature = "zod", feature = "jsonschema")
    ))]
    let docs = String::new();

    #[cfg(any(feature = "zod", feature = "typescript", feature = "jsonschema"))]
    let schema_impl_items = struct_schema_impl_items(collected.0, &collected.1, &item_name, &docs);

    // schema_example must be directly on the type (not in the module) because the example code
    // uses type names that may not be accessible from the nested module.
    #[cfg(feature = "zod")]
    let schema_example_method = build_struct_schema_example(docs_and_example.1.as_ref(), &name);
    #[cfg(all(
        not(feature = "zod"),
        any(feature = "typescript", feature = "jsonschema")
    ))]
    let schema_example_method: Option<proc_macro2::TokenStream> = None;

    // Generate the type-level validate() method if there are constrained fields.
    #[cfg(all(
        feature = "serde",
        any(feature = "typescript", feature = "zod", feature = "jsonschema")
    ))]
    let validate_method = build_struct_validate_method(&collected.3, &module_ident);
    #[cfg(all(
        not(feature = "serde"),
        any(feature = "typescript", feature = "zod", feature = "jsonschema")
    ))]
    let validate_method: Option<proc_macro2::TokenStream> = None;

    // Build delegating impl items (schema_example is added directly, not as a delegate).
    #[cfg(any(feature = "zod", feature = "typescript", feature = "jsonschema"))]
    let delegate_impl_items = build_struct_delegate_items(
        &module_ident,
        schema_example_method.as_ref(),
        validate_method,
    );

    #[cfg(any(feature = "zod", feature = "typescript", feature = "jsonschema"))]
    {
        assemble_schema_output(
            &item_struct,
            &module_ident,
            &name,
            &schema_impl_items,
            &collected.2,
            &delegate_impl_items,
        )
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

/// Walks a tuple struct's slots, returning their field defs in declaration order and the
/// `compile_error!` tokens of every guard a slot violates.
///
/// Unlike [`collect_struct_fields`] the walk does not split on `#[serde(flatten)]`: a positional
/// slot has no key for a flattened member to merge into, and dropping one here would silently
/// change the arity the surfaces describe. The per-slot validators are dropped because a
/// positional slot cannot carry a constraint — the guard that says so is what comes back instead.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn collect_tuple_slots(
    fields: &mut syn::Fields,
    rename_all: Option<&str>,
    module_name: &str,
    type_name: &str,
) -> (Vec<FieldDef>, Vec<proc_macro2::TokenStream>) {
    let mut slots: Vec<FieldDef> = Vec::new();
    let mut guard_errors: Vec<proc_macro2::TokenStream> = Vec::new();
    for field in fields.iter_mut() {
        let (slot, _, _, slot_guard_errors) =
            process_field(rename_all, field, Some(module_name), None, type_name);
        guard_errors.extend(slot_guard_errors);
        slots.push(slot);
    }
    (slots, guard_errors)
}

/// The TypeScript type a tuple struct describes as.
///
/// A single slot is the slot's own type: serde writes a newtype struct as that value alone, with
/// nothing around it. Every other arity — the empty one included, which writes `[]` — is the fixed
/// tuple the same slots describe as when they are written as a tuple field.
#[cfg(feature = "typescript")]
fn tuple_struct_ts_body(slots: &[FieldDef]) -> String {
    if let [slot] = slots {
        return slot.typescript_slot_typename();
    }
    format!(
        "[{}]",
        slots
            .iter()
            .map(FieldDef::typescript_slot_typename)
            .collect::<Vec<_>>()
            .join(", ")
    )
}

/// [`tuple_struct_ts_body`] for the Zod surface.
#[cfg(feature = "zod")]
fn tuple_struct_zod_body(slots: &[FieldDef]) -> String {
    if let [slot] = slots {
        return slot.zod_slot_type();
    }
    format!(
        "z.tuple([{}])",
        slots
            .iter()
            .map(FieldDef::zod_slot_type)
            .collect::<Vec<_>>()
            .join(", ")
    )
}

/// [`tuple_struct_ts_body`] for the JSON-schema surface, as a standalone `serde_json::Value`
/// expression, or the diagnostic naming the type when a slot holds a value the dispatch cannot
/// render.
#[cfg(feature = "jsonschema")]
fn tuple_struct_json_body(item_name: &str, slots: &[FieldDef]) -> proc_macro2::TokenStream {
    let described = if let [slot] = slots {
        build_tuple_element_json_schema(slot)
    } else {
        tuple_json_schema_value(slots)
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
    ts_body: &str,
) -> proc_macro2::TokenStream {
    let type_str = format!("/**\n{docs}\n **/\nexport type {item_name} = {ts_body};");
    quote! {
        pub fn ts_definition() -> String {
            #type_str.to_owned()
        }
    }
}

/// Builds the `zod_schema()` method for a tuple struct's schema module, in the same framing every
/// unbranded type publishes: the raw schema, then the exported binding annotated with the
/// TypeScript type the module's own `ts_definition()` writes.
#[cfg(feature = "zod")]
fn build_tuple_struct_zod_schema_method(
    item_name: &str,
    zod_body: &str,
) -> proc_macro2::TokenStream {
    #[cfg(feature = "typescript")]
    let schema_str = format!(
        "const {item_name}$RawSchema = {zod_body};\n\nexport const {item_name}$Schema: ZodType<{item_name}> = {item_name}$RawSchema;"
    );
    #[cfg(not(feature = "typescript"))]
    let schema_str = format!("export const {item_name}$Schema = {zod_body};");
    quote! {
        pub fn zod_schema() -> String {
            #schema_str.to_owned()
        }
    }
}

/// Processes a tuple struct that is not a branded newtype.
///
/// serde writes a one-slot tuple struct as the slot's value alone and every other arity as a
/// fixed-arity array, so neither shape is the object the named-field emitters describe. Both are
/// rendered from the slots read in slot position — the position a tuple field's elements are
/// already read in, where a `None` reaches the wire as `null` rather than as an omitted key — so
/// a tuple struct describes as the tuple of its slot types does wherever that tuple is written.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn process_tuple_struct(
    mut item_struct: syn::ItemStruct,
    rename_all: Option<&str>,
    args: &ModelSchemaArgs,
) -> TokenStream {
    let name = item_struct.ident.clone();
    let (item_name, module_name, module_ident) =
        struct_module_idents(&name, args.name_override.as_deref());

    let (slots, guard_errors) = collect_tuple_slots(
        &mut item_struct.fields,
        rename_all,
        &module_name,
        &name.to_string(),
    );

    // A violated slot guard makes the whole contract unsound, so the schema surface is dropped and
    // only the original item plus the errors are emitted.
    if let Some(output) = guard_failure_output(&item_struct, &guard_errors) {
        return output;
    }

    #[cfg(any(feature = "typescript", feature = "zod"))]
    let docs_and_example = struct_docs_and_example(&item_struct);

    let schema_impl_items: Vec<proc_macro2::TokenStream> = vec![
        #[cfg(feature = "jsonschema")]
        json_schema_methods(&item_name, &tuple_struct_json_body(&item_name, &slots)),
        #[cfg(feature = "typescript")]
        build_tuple_struct_ts_definition_method(
            &build_item_jsdoc(docs_and_example.0.as_deref(), &item_name),
            &item_name,
            &tuple_struct_ts_body(&slots),
        ),
        #[cfg(feature = "zod")]
        build_tuple_struct_zod_schema_method(&item_name, &tuple_struct_zod_body(&slots)),
    ];

    // schema_example must be directly on the type (not in the module) because the example code
    // uses type names that may not be accessible from the nested module.
    #[cfg(feature = "zod")]
    let schema_example_method = build_struct_schema_example(docs_and_example.1.as_ref(), &name);
    #[cfg(not(feature = "zod"))]
    let schema_example_method: Option<proc_macro2::TokenStream> = None;

    let delegate_impl_items =
        build_struct_delegate_items(&module_ident, schema_example_method.as_ref(), None);

    assemble_schema_output(
        &item_struct,
        &module_ident,
        &name,
        &schema_impl_items,
        &[],
        &delegate_impl_items,
    )
}

/// Processes a branded newtype (transparent single-field tuple struct) and generates
/// TypeScript branded type definitions and Zod brand schemas.
///
/// A branded newtype is detected when a struct has **both** `#[serde(transparent)]` and exactly
/// one unnamed field. The generated output depends on the active features:
///
/// - With `zod` + `typescript`: emits a Zod `.brand<"Name">()` schema and a
///   `type Name<T> = T & z.$brand<"Name">` alias.
/// - With `typescript` only (no `zod`): emits a `unique symbol` brand pattern and an
///   `assertName()` type-assertion helper function.
///
/// Generic parameters on the struct are preserved in the TypeScript output. For non-generic
/// newtypes, the inner field's Rust type is resolved to its TypeScript equivalent. For generic
/// newtypes, the Zod schema always uses `z.string()` as the base because the generic parameter
/// cannot be resolved at macro-expansion time.
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
            let pattern_lit = pattern.clone();
            checks.push(quote! {
                {
                    use std::sync::LazyLock;
                    static RE: LazyLock<regex::Regex> = LazyLock::new(|| {
                        regex::Regex::new(#pattern_lit).unwrap()
                    });
                    if !RE.is_match(value) {
                        return Err(format!(
                            "value does not match pattern '{}'",
                            #pattern_lit
                        ));
                    }
                }
            });
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
///
/// A brand's constrained value is the inner field itself, with no walk to reach it, so only an
/// inner that *is* a path answers here — one merely holding paths writes no string of its own for
/// the checks to measure, and is refused by [`branded_constraint_inner_error`] before this. A
/// transparent wrapper is nothing on the wire and derefs to what it holds, so a path written under
/// any stack of them is still that same path, borrowed one coercion further out; an `Option` or a
/// sequence is not, which is why only the transparent wraps pass.
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

/// Builds the string schema a branded newtype's constraints are written into: `type_name` as the
/// `"type"`, then one insert per constraint the brand declares.
#[cfg(feature = "jsonschema")]
fn branded_constrained_schema_obj(
    args: &ModelSchemaArgs,
    type_name: &str,
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
        constraint_inserts.push(quote! {
            schema_obj.insert("pattern".to_string(), serde_json::Value::String(#pattern.to_string()));
        });
    }

    quote! {
        {
            let mut schema_obj = serde_json::Map::new();
            schema_obj.insert("type".to_string(), serde_json::Value::String(#type_name.to_string()));
            #(#constraint_inserts)*
            serde_json::Value::Object(schema_obj)
        }
    }
}

/// The description a brand carries for an inner no `"type"` keyword names: the one the slot
/// dispatch gives that type, so the brand describes as the type it wraps describes wherever else
/// it is written. An inner the dispatch cannot render replaces the body with the single diagnostic
/// naming the brand, as an unrenderable slot does in every other position.
///
/// The brand's own constraints are not written beside it. [`branded_constraint_inner_error`]
/// refuses them over an array, a map, a tuple, and an opaque inner, so there is nothing to write —
/// except through the one spelling that guard reads as a name rather than as the array it writes,
/// a sequence wrapper, where they were being written onto a `"type": "string"` describing no value
/// the type can hold.
#[cfg(feature = "jsonschema")]
fn branded_slot_json_schema(inner: &FieldDef, def_name: &str) -> proc_macro2::TokenStream {
    match build_tuple_element_json_schema(inner) {
        Ok(value) => value,
        Err(rejection) => {
            let message = map_member_rejection_message(&format!("`{def_name}`"), &rejection);
            quote! { compile_error!(#message) }
        }
    }
}

/// Builds the `json_schema()` method for a branded newtype's schema module.
#[cfg(feature = "jsonschema")]
fn build_branded_json_schema_method(
    args: &ModelSchemaArgs,
    json_inner: &BrandedJsonInner,
    def_name: &str,
) -> proc_macro2::TokenStream {
    let body = match json_inner {
        BrandedJsonInner::Scalar(type_name) => branded_constrained_schema_obj(args, type_name),
        #[cfg(feature = "object_id")]
        BrandedJsonInner::ObjectId => {
            object_id_json_schema_value(&branded_constrained_schema_obj(args, "string"))
        }
        BrandedJsonInner::Slot(inner) => branded_slot_json_schema(inner, def_name),
    };
    json_schema_methods(def_name, &body)
}

/// Extracts the generic type parameter names from a branded newtype's generics.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn branded_generic_params(generics: &syn::Generics) -> Vec<String> {
    generics
        .params
        .iter()
        .filter_map(|p| {
            if let syn::GenericParam::Type(tp) = p {
                Some(tp.ident.to_string())
            } else {
                None
            }
        })
        .collect()
}

/// Resolves the TypeScript inner type name and generic parameter list for a branded newtype.
#[cfg(feature = "typescript")]
fn branded_ts_type_and_generics(
    is_generic: bool,
    generic_params: &[String],
    inner_ty: &syn::Type,
) -> (String, String) {
    let ts_inner_type = if is_generic {
        generic_params[0].clone()
    } else {
        get_field_def("_inner", inner_ty, "").typescript_typename()
    };
    let ts_generics = if is_generic {
        format!("<{}>", generic_params.join(", "))
    } else {
        String::new()
    };
    (ts_inner_type, ts_generics)
}

/// Resolves the JSON schema shape for a branded newtype's inner field.
///
/// For generic newtypes this is always `"string"` (mirrors the Zod logic). An `ObjectId` and a
/// composite are read off their own `FieldDef`, because neither writes a value one `"type"`
/// keyword describes; every remaining non-generic inner maps from the resolved TypeScript type
/// name.
#[cfg(feature = "jsonschema")]
fn branded_json_inner(is_generic: bool, inner_ty: &syn::Type) -> BrandedJsonInner {
    if is_generic {
        return BrandedJsonInner::Scalar("string".to_owned());
    }
    let inner = get_field_def("_inner", inner_ty, "");
    #[cfg(feature = "object_id")]
    if branded_inner_is_object_id(&inner) {
        return BrandedJsonInner::ObjectId;
    }
    if branded_inner_composite(&inner).is_some() {
        return BrandedJsonInner::Slot(Box::new(inner));
    }
    BrandedJsonInner::Scalar(match inner.typescript_typename().as_str() {
        "number" => "number".to_owned(),
        "boolean" => "boolean".to_owned(),
        _ => "string".to_owned(),
    })
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
///
/// The array level is asked first, so an arrayed inner answers for the array around whatever it
/// holds — which is what `#[serde(transparent)]` puts on the wire — rather than for the item. That
/// is the same question [`branded_inner_is_object_id`] asks to exclude an arrayed `ObjectId`, whose
/// array reaches here instead. A sequence wrapper is that same array under another name, and is
/// read through the one list every surface reads it through.
///
/// Named exhaustively rather than caught by a wildcard: a new variant must be classified, not
/// silently collapsed into the scalar mapping.
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
        FieldDefType::Unknown => Some(BrandedComposite::Opaque),
        FieldDefType::Boolean
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
///
/// The constraints measure the inner's `Display` rendering, which for every inner but an
/// `ObjectId` is the value the schema itself describes. An `ObjectId` writes `{"$oid": hex}`, so
/// its `Display` is the `$oid` member and the checks land there.
#[cfg(feature = "zod")]
fn branded_zod_inner(args: &ModelSchemaArgs, is_generic: bool, inner_ty: &syn::Type) -> String {
    let checks = branded_zod_string_checks(args);
    if is_generic {
        return format!("z.string(){checks}");
    }
    let inner = get_field_def("_inner", inner_ty, "");
    #[cfg(feature = "object_id")]
    if branded_inner_is_object_id(&inner) {
        return get_object_id_zod_schema_with(&checks);
    }
    format!("{}{checks}", inner.zod_type())
}

/// The Zod class a branded newtype's base schema is an instance of, for the exported binding's
/// type annotation.
///
/// Read off the inner's own `FieldDef` rather than off its TypeScript name, so a user type merely
/// spelled `ObjectId` is not mistaken for the `$oid` object, and so a composite is annotated with
/// the class its own value rendering produces instead of falling through to `ZodString`.
///
/// Each name is the class bare, as `ZodObject` is: every one of them defaults its type parameters,
/// so the annotation widens the raw schema rather than restating its element types.
#[cfg(all(feature = "zod", feature = "typescript"))]
fn branded_zod_type_name(is_generic: bool, inner_ty: &syn::Type) -> String {
    if is_generic {
        return "ZodString".to_owned();
    }
    let inner = get_field_def("_inner", inner_ty, "");
    #[cfg(feature = "object_id")]
    if branded_inner_is_object_id(&inner) {
        return "ZodObject".to_owned();
    }
    if let Some(composite) = branded_inner_composite(&inner) {
        return match composite {
            BrandedComposite::Array => "ZodArray".to_owned(),
            BrandedComposite::Map => "ZodRecord".to_owned(),
            BrandedComposite::Opaque => "ZodUnknown".to_owned(),
            BrandedComposite::Tuple => "ZodTuple".to_owned(),
        };
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
    ts_generics: &str,
    ts_inner_type: &str,
) -> proc_macro2::TokenStream {
    #[cfg(feature = "zod")]
    {
        let type_str = format!(
            "export type {item_name}{ts_generics} = {ts_inner_type} & $brand<\"{item_name}\">;"
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
            "export type {item_name}{ts_generics} = {ts_inner_type} & {{ readonly [__brand_{item_name}]: true }};"
        );
        quote! {
            pub fn ts_definition() -> String {
                format!("{}\n{}", #unique_symbol, #type_str)
            }
        }
    }
}

/// Builds the `zod_schema()` method for a branded newtype's schema module.
#[cfg(feature = "zod")]
fn build_branded_zod_schema_method(
    item_name: &str,
    is_generic: bool,
    inner_ty: &syn::Type,
    zod_inner: &str,
    plain_description: &str,
) -> proc_macro2::TokenStream {
    #[cfg(feature = "typescript")]
    {
        let zod_type_name = branded_zod_type_name(is_generic, inner_ty);
        let zod_type_annotation = format!("$ZodBranded<{zod_type_name}, \"{item_name}\">");
        quote! {
            pub fn zod_schema() -> String {
                format!(
                    "const {0}$RawSchema = {1}.brand<\"{0}\">().meta({{\n  description: \"{3}\",\n}});\n\nexport const {0}$Schema: {2} = {0}$RawSchema;",
                    #item_name, #zod_inner, #zod_type_annotation, #plain_description
                )
            }
        }
    }
    #[cfg(not(feature = "typescript"))]
    {
        let _: &_ = &(is_generic, inner_ty);
        quote! {
            pub fn zod_schema() -> String {
                format!(
                    "export const {0}$Schema = {1}.brand<\"{0}\">().meta({{\n  description: \"{2}\",\n}});",
                    #item_name, #zod_inner, #plain_description
                )
            }
        }
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
                // Insert example into .meta() before the first closing \n});
                if let Some(pos) = base_schema.find("\n});") {
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
///
/// A constrained brand validates through `value.to_string()`, so the requirement outlives the impl
/// the brand opted out of. Keeping the assertion is what turns that into an `E0277` at the inner
/// field instead of the `E0599` the `to_string()` call raises against the attribute. A path inner
/// is the exception: its checks read the path's own rendering and call no `to_string()`, so there
/// is nothing left for the assertion to blame.
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
///
/// Emits nothing when the inner type mentions one of the struct's generic parameters: a `const`
/// item cannot name them, and the `Display` bound the impl adds to every type parameter already
/// carries the requirement to the instantiation site.
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
    is_generic: bool,
) -> proc_macro2::TokenStream {
    let Some(code) = example_code else {
        return quote! {};
    };
    let code_tokens: proc_macro2::TokenStream = code.parse().unwrap();
    if is_generic {
        // For generic newtypes, the example constructs a concrete type (e.g., DocumentId<String>).
        // We use String as the concrete type since the Zod schema always uses z.string().
        quote! {
            pub fn schema_example() -> serde_json::Value {
                let value: #name<String> = {
                    #code_tokens
                };
                serde_json::to_value(&value).unwrap()
            }
        }
    } else {
        quote! {
            pub fn schema_example() -> serde_json::Value {
                let value: #name = {
                    #code_tokens
                };
                serde_json::to_value(&value).unwrap()
            }
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
    let validate_method = parts.validate_method;

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
    };

    log::trace!("{output}");

    TokenStream::from(output)
}

#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn process_branded_newtype(item_struct: syn::ItemStruct, args: &ModelSchemaArgs) -> TokenStream {
    // Checked before the type is registered in the alias registry: a rejected brand emits no
    // schema, so nothing else should be able to resolve a reference to one.
    if let Some(output) =
        guard_failure_output(&item_struct, &branded_guard_errors(&item_struct, args))
    {
        return output;
    }

    let name = item_struct.ident.clone();
    let item_name = compute_item_export_name(&name.to_string(), args.name_override.as_deref());
    let module_name = format!("{}_schema", to_snake_case(&item_name));
    let module_ident = Ident::new(&module_name, name.span());

    register_alias_info(
        &name.to_string(),
        &item_name,
        &module_name,
        AliasKind::NoEnumMembers,
    );

    // Extract docs and example
    #[cfg(feature = "zod")]
    let docs_vec = get_struct_docs(&item_struct);

    #[cfg(feature = "zod")]
    let example_code = docs_vec
        .as_ref()
        .and_then(|docs| extract_example_from_docs(docs));

    #[cfg(feature = "zod")]
    let plain_description = item_description(docs_vec.as_deref(), &item_name);

    // Get generic type parameters from the struct
    let generic_params = branded_generic_params(&item_struct.generics);
    let is_generic = !generic_params.is_empty();

    // Get inner field type info
    let inner_field = item_struct.fields.iter().next().unwrap();
    let inner_ty = &inner_field.ty;

    // `ts_pair`: (ts_inner_type, ts_generics).
    #[cfg(feature = "typescript")]
    let ts_pair = branded_ts_type_and_generics(is_generic, &generic_params, inner_ty);

    #[cfg(not(any(feature = "typescript", feature = "zod")))]
    let _: &_ = &inner_ty;

    #[cfg(feature = "jsonschema")]
    let json_inner = branded_json_inner(is_generic, inner_ty);

    #[cfg(feature = "zod")]
    let zod_inner = branded_zod_inner(args, is_generic, inner_ty);

    // --- Generate ts_definition method ---
    #[cfg(feature = "typescript")]
    let ts_definition_method =
        build_branded_ts_definition_method(&item_name, &ts_pair.1, &ts_pair.0);

    // --- Generate zod_schema method ---
    #[cfg(feature = "zod")]
    let zod_schema_method = build_branded_zod_schema_method(
        &item_name,
        is_generic,
        inner_ty,
        &zod_inner,
        &plain_description,
    );

    // --- Generate validation code for constrained branded newtypes ---
    #[cfg(feature = "serde")]
    let branded_validation = build_branded_validation(args, is_generic, inner_ty);

    // --- Build schema module impl items ---
    #[cfg(feature = "jsonschema")]
    let json_schema_method = build_branded_json_schema_method(args, &json_inner, &item_name);

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
        build_branded_schema_example(example_code.as_ref(), &name, is_generic);
    #[cfg(not(feature = "zod"))]
    let schema_example_tokens = quote! {};

    // --- Generate delegate methods ---
    let delegate_impl_items = build_branded_delegate_items(&module_ident, has_example);

    // `generics` is for the impl block (Display bounds added when constrained); `generics_for_ty`
    // is the unmodified clone used for the type alias.
    let generics_for_ty = item_struct.generics.clone();
    let generics = branded_impl_generics(&item_struct, is_generic, args);

    // --- Generate Display impl for branded newtypes (unless the brand opted out) ---
    let display_tokens =
        build_branded_display_tokens(&item_struct.generics, &name, inner_field, args);

    // --- Inject serde(deserialize_with) on inner field and generate validate() ---
    #[cfg(feature = "serde")]
    let (output_struct, validation_tokens, validate_method) = inject_branded_serde_attrs(
        item_struct,
        branded_validation.as_ref(),
        is_generic,
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

        process_plain_enum(item_enum, &name, rename_all, &item_name)
    } else {
        #[cfg(feature = "serde")]
        if serde_type_meta.untagged {
            return process_untagged_enum(item_enum, &name, &item_name);
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

/// The escaped one-line description an item publishes, falling back to the exported name when it
/// carries no docs. Every item shape reaches its description through this one body, so the
/// fallback cannot drift between shapes.
#[cfg(any(feature = "typescript", feature = "zod"))]
fn item_description(docs_vec: Option<&[String]>, item_name: &str) -> String {
    docs_vec.map_or_else(
        || item_name.to_owned(),
        |doc_lines| {
            item_plain_doc_lines(doc_lines)
                .join("\\n")
                .replace('"', "\\\"")
        },
    )
}

/// Flattens an item's doc comments into a `JSDoc` body and an escaped one-line description, both
/// derived from the same lines (with ` ```rust example ` blocks stripped). Falls back to the
/// exported name when there are no docs, matching every other item path.
#[cfg(any(feature = "typescript", feature = "zod"))]
fn build_item_docs_and_description(
    docs_vec: Option<&[String]>,
    item_name: &str,
) -> (String, String) {
    let docs_formatted = docs_vec.map_or_else(
        || {
            [item_name.to_owned(), String::new()]
                .into_iter()
                .map(|l| format!(" * {l}"))
                .collect::<Vec<_>>()
                .join("\n")
        },
        |doc_lines| {
            item_plain_doc_lines(doc_lines)
                .iter()
                .map(|l| format!(" * {l}"))
                .chain(vec![" * ".to_owned()])
                .collect::<Vec<_>>()
                .join("\n")
        },
    );

    (docs_formatted, item_description(docs_vec, item_name))
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
            let variant_docs =
                get_variant_docs(item).map_or_else(String::new, |doc_lines| doc_lines.join("\n"));
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
) -> TokenStream {
    // Compute the schema module name and register the enum so other types can find it.
    #[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
    let (_, module_ident) = enum_module_idents(name, item_name, AliasKind::EnumMembers);

    // Extract docs early for example extraction
    #[cfg(any(feature = "typescript", feature = "zod"))]
    let docs_vec = get_enum_docs(&item_enum);

    #[cfg(feature = "zod")]
    let example_code = docs_vec
        .as_ref()
        .and_then(|docs| extract_example_from_docs(docs));

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

    // Enumerate the strings with indices
    let enumerated: Vec<proc_macro2::TokenStream> = enum_options
        .iter()
        .map(|v| {
            quote! { #v }
        })
        .collect();

    #[cfg(any(feature = "typescript", feature = "zod"))]
    let docs_and_description = build_item_docs_and_description(docs_vec.as_deref(), item_name);

    // Generate schema module methods
    #[cfg(feature = "jsonschema")]
    let json_schema_method = generate_plain_enum_json_schema_method(&enumerated, item_name);

    #[cfg(feature = "typescript")]
    let ts_definition_method =
        generate_plain_enum_ts_definition_method(&docs_and_description.0, item_name, &type_code);

    // Schema module emits zod_schema without examples; example injection happens in the delegating
    // method on the type to avoid `super::` resolution issues.
    #[cfg(feature = "zod")]
    let zod_schema_method =
        generate_plain_enum_zod_schema_method(item_name, &schema_code, &docs_and_description.1);

    #[cfg(not(any(feature = "typescript", feature = "zod")))]
    let _: &_ = &item_name;

    // schema_example must be directly on the type (not in the module) because the example code
    // uses type names that may not be accessible from the nested module.
    #[cfg(feature = "zod")]
    let schema_example_method = build_struct_schema_example(example_code.as_ref(), name);
    #[cfg(all(
        not(feature = "zod"),
        any(feature = "typescript", feature = "jsonschema")
    ))]
    let schema_example_method: Option<proc_macro2::TokenStream> = None;

    // Build schema module impl items (without schema_example)
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

    // Use the enumerated values in the quote! macro
    let enum_values = &enumerated;

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

        impl #name {
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

        impl #name {
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

/// Processes each variant of a discriminated enum, returning per-variant field defs, doc strings,
/// and variant kinds in declaration order, plus the collected serde validation functions.
fn collect_discriminated_variants(
    item_enum: &mut syn::ItemEnum,
    rename_all: Option<&str>,
    enum_module_name_opt: Option<&str>,
) -> DiscriminatedVariantData {
    let mut variants: Vec<DiscriminatedVariant> = Vec::new();
    let mut enum_validation_fns: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut guard_errors: Vec<proc_macro2::TokenStream> = Vec::new();
    let enum_type_name = item_enum.ident.to_string();

    for item in &mut item_enum.variants {
        #[cfg(feature = "serde")]
        let field_rename = parse_serde_field_attributes(&item.attrs).rename;
        #[cfg(not(feature = "serde"))]
        let field_rename: Option<String> = None;

        let variant_ident = item.ident.to_string();
        let final_name =
            get_final_variant_name(&variant_ident, field_rename.as_deref(), rename_all);
        let variant_kind = classify_variant(item);

        let mut field_defs: Vec<FieldDef> = Vec::new();
        for field in &mut item.fields {
            let (f_def, validation_fn, _validate_body, field_guard_errors) = process_field(
                rename_all,
                field,
                enum_module_name_opt,
                Some(&variant_ident),
                &enum_type_name,
            );
            if let Some(vfn) = validation_fn {
                enum_validation_fns.push(vfn);
            }
            guard_errors.extend(field_guard_errors);
            field_defs.push(f_def);
        }

        let discriminator_docs = get_variant_docs(item).map_or_else(
            || {
                [final_name.clone(), String::new()]
                    .into_iter()
                    .map(|l| format!(" * {l}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            },
            |doc_lines| {
                doc_lines
                    .into_iter()
                    .flat_map(|v| v.lines().map(ToOwned::to_owned).collect::<Vec<_>>())
                    .chain(vec![String::new()])
                    .map(|l| format!(" * {l}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            },
        );
        variants.push(DiscriminatedVariant {
            discriminator_value: final_name,
            docs: discriminator_docs,
            field_defs,
            kind: variant_kind,
        });
    }

    (variants, enum_validation_fns, guard_errors)
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

/// Processes a discriminated enum (tagged union in TypeScript) and generates its definitions.
fn process_discriminated_enum(
    mut item_enum: syn::ItemEnum,
    name: &syn::Ident,
    tag_name: &str,
    content_name: &str,
    rename_all: Option<&str>,
    item_name: &str,
) -> TokenStream {
    // Compute the schema module name and register the enum so other types can find it.
    #[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
    let (module_name, module_ident) = enum_module_idents(name, item_name, AliasKind::NoEnumMembers);

    // Extract docs early for example extraction
    #[cfg(any(feature = "typescript", feature = "zod"))]
    let docs_vec = get_enum_docs(&item_enum);

    #[cfg(feature = "zod")]
    let example_code = docs_vec
        .as_ref()
        .and_then(|docs| extract_example_from_docs(docs));

    // Process each variant in the enum.
    #[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
    let enum_module_name_opt = Some(module_name.as_str());
    #[cfg(not(any(feature = "typescript", feature = "zod", feature = "jsonschema")))]
    let enum_module_name_opt = None;

    // Bind both result tuples whole so feature-gated field access marks them used (no per-element
    // guards): `variants` = (variants, validation_fns, guard_errors);
    // `rendered` = (ts, zod, json).
    let variants = collect_discriminated_variants(&mut item_enum, rename_all, enum_module_name_opt);
    if let Some(output) = guard_failure_output(&item_enum, &variants.2) {
        return output;
    }
    let rendered = render_discriminated_variants(tag_name, content_name, item_name, &variants.0);
    #[cfg(not(any(feature = "typescript", feature = "zod", feature = "jsonschema")))]
    let _: &_ = &(name, &rendered);

    #[cfg(feature = "jsonschema")]
    let main_schema_code = discriminated_main_schema_code(&rendered.2);

    #[cfg(feature = "typescript")]
    let type_code = rendered.0.join(" | ");

    // Generate Zod schema conditionally
    #[cfg(feature = "zod")]
    let schema_code = format!(
        "z.discriminatedUnion(\"{tag_name}\", [{}])",
        rendered
            .1
            .iter()
            .map(|(v, _opts)| format!("z.strictObject({}){}", v, ""))
            .collect::<Vec<_>>()
            .join(", ")
    );

    #[cfg(feature = "typescript")]
    let docs = build_item_jsdoc(docs_vec.as_deref(), item_name);

    // Generate schema module methods
    #[cfg(feature = "jsonschema")]
    let json_schema_method =
        generate_discriminated_enum_json_schema_method(&main_schema_code, item_name);

    #[cfg(feature = "typescript")]
    let ts_definition_method =
        generate_discriminated_enum_ts_definition_method(&docs, item_name, &type_code);

    // Schema module emits zod_schema without examples; example injection happens in the delegating
    // method on the type to avoid `super::` resolution issues.
    #[cfg(feature = "zod")]
    let zod_schema_method = generate_discriminated_enum_zod_schema_method(item_name, &schema_code);

    #[cfg(not(any(feature = "typescript", feature = "zod")))]
    let _: &_ = &item_name;

    // schema_example must be directly on the type (not in the module) because the example code
    // uses type names that may not be accessible from the nested module.
    #[cfg(feature = "zod")]
    let schema_example_method = build_struct_schema_example(example_code.as_ref(), name);
    #[cfg(all(
        not(feature = "zod"),
        any(feature = "typescript", feature = "jsonschema")
    ))]
    let schema_example_method: Option<proc_macro2::TokenStream> = None;

    // Build schema module impl items (without schema_example)
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
    // `zod_schema` example injection uses the same `.meta()`-before-`;` form), with no `validate()`.
    #[cfg(any(feature = "zod", feature = "typescript", feature = "jsonschema"))]
    let delegate_impl_items =
        build_struct_delegate_items(&module_ident, schema_example_method.as_ref(), None);

    #[cfg(any(feature = "zod", feature = "typescript", feature = "jsonschema"))]
    {
        assemble_schema_output(
            &item_enum,
            &module_ident,
            name,
            &schema_impl_items,
            &variants.1,
            &delegate_impl_items,
        )
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
///
/// Returns `(typescript, zod, json_value)` for the content alone — the key is the caller's. A
/// `Unit` variant has no content and never reaches here: serde writes it as the bare name.
///
/// Every content is the value the adjacent form puts under its content key, read through the same
/// builders: a newtype variant's single slot, a tuple variant's fixed-arity array, and a struct
/// variant's fields — which the adjacent form spreads beside the tag and this one gathers into an
/// object.
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
///
/// Returns `(typescript, zod, json_value)`. A data-carrying variant is a closed object whose sole
/// key is the variant name; a unit variant is that name alone, which is the whole value serde
/// writes for it.
///
/// The key is quoted on both text surfaces because it is a wire name rather than an identifier: a
/// `#[serde(rename)]` can spell it as something no JavaScript identifier can hold.
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
            format!("/**\n{docs}\n**/\n  \"{key}\""),
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
        format!("{{  /**\n{docs}\n**/\n  \"{key}\": {content_ts};\n}}"),
        zod,
        json,
    )
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
///
/// Absent `tag` / `content` / `untagged`, serde writes the externally tagged form: a data-carrying
/// variant becomes a single-key object under the variant's name, and a unit variant becomes that
/// name as a bare string. The surfaces describe that union — a JSON-schema `oneOf`, a TypeScript
/// union, and a Zod `z.union`. The key carries the discriminator, so there is no one field every
/// member shares and `z.discriminatedUnion` has nothing to switch on.
#[cfg(feature = "serde")]
fn process_externally_tagged_enum(
    mut item_enum: syn::ItemEnum,
    name: &syn::Ident,
    rename_all: Option<&str>,
    item_name: &str,
) -> TokenStream {
    // Compute the schema module name and register the enum so other types can find it.
    #[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
    let (module_name, module_ident) = enum_module_idents(name, item_name, AliasKind::NoEnumMembers);

    #[cfg(any(feature = "typescript", feature = "zod"))]
    let docs_vec = get_enum_docs(&item_enum);

    #[cfg(feature = "zod")]
    let example_code = docs_vec
        .as_ref()
        .and_then(|docs| extract_example_from_docs(docs));

    #[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
    let enum_module_name_opt = Some(module_name.as_str());
    #[cfg(not(any(feature = "typescript", feature = "zod", feature = "jsonschema")))]
    let enum_module_name_opt = None;

    let self_type_name = item_enum.ident.to_string();
    let variants = collect_discriminated_variants(&mut item_enum, rename_all, enum_module_name_opt);
    if let Some(output) = guard_failure_output(&item_enum, &variants.2) {
        return output;
    }

    let members: Vec<(String, String, proc_macro2::TokenStream)> = variants
        .0
        .iter()
        .map(|variant| render_external_variant(variant, &self_type_name))
        .collect();

    let (main_schema_code, type_code, schema_code) = join_external_union(&members);
    #[cfg(not(feature = "jsonschema"))]
    let _: &_ = &main_schema_code;
    #[cfg(not(feature = "typescript"))]
    let _: &_ = &type_code;
    #[cfg(not(feature = "zod"))]
    let _: &_ = &schema_code;
    #[cfg(not(any(feature = "typescript", feature = "zod", feature = "jsonschema")))]
    let _: &_ = &name;

    #[cfg(feature = "typescript")]
    let docs = build_item_jsdoc(docs_vec.as_deref(), item_name);

    #[cfg(feature = "jsonschema")]
    let json_schema_method =
        generate_discriminated_enum_json_schema_method(&main_schema_code, item_name);

    #[cfg(feature = "typescript")]
    let ts_definition_method =
        generate_discriminated_enum_ts_definition_method(&docs, item_name, &type_code);

    #[cfg(feature = "zod")]
    let zod_schema_method = generate_discriminated_enum_zod_schema_method(item_name, &schema_code);

    #[cfg(not(any(feature = "typescript", feature = "zod")))]
    let _: &_ = &item_name;

    #[cfg(feature = "zod")]
    let schema_example_method = build_struct_schema_example(example_code.as_ref(), name);
    #[cfg(all(
        not(feature = "zod"),
        any(feature = "typescript", feature = "jsonschema")
    ))]
    let schema_example_method: Option<proc_macro2::TokenStream> = None;

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
    let delegate_impl_items =
        build_struct_delegate_items(&module_ident, schema_example_method.as_ref(), None);

    #[cfg(any(feature = "zod", feature = "typescript", feature = "jsonschema"))]
    {
        assemble_schema_output(
            &item_enum,
            &module_ident,
            name,
            &schema_impl_items,
            &variants.1,
            &delegate_impl_items,
        )
    }

    #[cfg(not(any(feature = "zod", feature = "typescript", feature = "jsonschema")))]
    {
        let _: &_ = &variants.1;
        let output = quote! {
            #item_enum
        };
        log::trace!("{output}");
        TokenStream::from(output)
    }
}

/// How an internally tagged newtype variant's inner value fares beside the tag.
///
/// Internal tagging has no key for a content: the variant's data is written as members of the
/// object the tag is written in, so only a value serde writes as an object has anything to put
/// there. That is the whole of the split — a named type is one, and every shape that reaches the
/// wire as a scalar, a sequence or an absence is not.
///
/// The wrappers are read before the type they wrap, in the order serde's serializer meets them: an
/// `Option` is refused as an optional even around a sequence, and a sequence is refused as a
/// sequence even around a named type.
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
        FieldDefType::String | FieldDefType::StringLiteral(_) => TaggedContent::Refused("a string"),
        #[cfg(feature = "chrono")]
        FieldDefType::DateTime
        | FieldDefType::NaiveDate
        | FieldDefType::NaiveDateTime
        | FieldDefType::NaiveTime => TaggedContent::Refused("a string"),
        FieldDefType::Tuple(_) => TaggedContent::Refused("a tuple"),
        FieldDefType::Map(..) => TaggedContent::Unnameable("a map"),
        #[cfg(feature = "object_id")]
        FieldDefType::ObjectId => TaggedContent::Unnameable("an ObjectId"),
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
            // what the unit arm already describes.
            let field = unnamed.unnamed.first()?;
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
///
/// Returns `(typescript, zod, json_value, flattened)`, where the first three are the member on each
/// surface and the last says whether it is an intersection rather than a plain object — which is
/// what a `z.discriminatedUnion` option cannot be.
///
/// A unit variant is the tag alone and a struct variant is the tag beside its own fields, both of
/// which the tagged member already spells. A newtype variant is the one the form differs on: what
/// its inner type writes are members of the same object, so the inner's own description joins the
/// tag's rather than sitting under a key.
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

    #[cfg(feature = "zod")]
    let zod = format!(
        "z.strictObject({}).and({})",
        parts.schema_code,
        inner.zod_type()
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
///
/// With no content key there is nowhere for a variant's data to sit but the object the tag is
/// written in: a struct variant's fields are written beside the tag, and so are the members of a
/// newtype variant's inner type. Every other content has nothing to contribute there, and the
/// guards refuse those declarations rather than describing a value serde cannot write.
#[cfg(feature = "serde")]
fn process_internally_tagged_enum(
    mut item_enum: syn::ItemEnum,
    name: &syn::Ident,
    tag_name: &str,
    rename_all: Option<&str>,
    item_name: &str,
) -> TokenStream {
    #[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
    let (module_name, module_ident) = enum_module_idents(name, item_name, AliasKind::NoEnumMembers);

    #[cfg(any(feature = "typescript", feature = "zod"))]
    let docs_vec = get_enum_docs(&item_enum);

    #[cfg(feature = "zod")]
    let example_code = docs_vec
        .as_ref()
        .and_then(|docs| extract_example_from_docs(docs));

    #[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
    let enum_module_name_opt = Some(module_name.as_str());
    #[cfg(not(any(feature = "typescript", feature = "zod", feature = "jsonschema")))]
    let enum_module_name_opt = None;

    if let Some(output) = guard_failure_output(
        &item_enum,
        &internally_tagged_guard_errors(&item_enum, tag_name),
    ) {
        return output;
    }

    let self_type_name = item_enum.ident.to_string();
    let variants = collect_discriminated_variants(&mut item_enum, rename_all, enum_module_name_opt);
    if let Some(output) = guard_failure_output(&item_enum, &variants.2) {
        return output;
    }

    let members: Vec<(String, String, proc_macro2::TokenStream, bool)> = variants
        .0
        .iter()
        .map(|variant| render_internal_variant(variant, tag_name, &self_type_name))
        .collect();

    let (main_schema_code, type_code, schema_code) = join_internal_union(&members, tag_name);
    #[cfg(not(feature = "jsonschema"))]
    let _: &_ = &main_schema_code;
    #[cfg(not(feature = "typescript"))]
    let _: &_ = &type_code;
    #[cfg(not(feature = "zod"))]
    let _: &_ = &schema_code;
    #[cfg(not(any(feature = "typescript", feature = "zod", feature = "jsonschema")))]
    let _: &_ = &name;

    #[cfg(feature = "typescript")]
    let docs = build_item_jsdoc(docs_vec.as_deref(), item_name);

    #[cfg(feature = "jsonschema")]
    let json_schema_method =
        generate_discriminated_enum_json_schema_method(&main_schema_code, item_name);

    #[cfg(feature = "typescript")]
    let ts_definition_method =
        generate_discriminated_enum_ts_definition_method(&docs, item_name, &type_code);

    #[cfg(feature = "zod")]
    let zod_schema_method = generate_discriminated_enum_zod_schema_method(item_name, &schema_code);

    #[cfg(not(any(feature = "typescript", feature = "zod")))]
    let _: &_ = &item_name;

    #[cfg(feature = "zod")]
    let schema_example_method = build_struct_schema_example(example_code.as_ref(), name);
    #[cfg(all(
        not(feature = "zod"),
        any(feature = "typescript", feature = "jsonschema")
    ))]
    let schema_example_method: Option<proc_macro2::TokenStream> = None;

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
    let delegate_impl_items =
        build_struct_delegate_items(&module_ident, schema_example_method.as_ref(), None);

    #[cfg(any(feature = "zod", feature = "typescript", feature = "jsonschema"))]
    {
        assemble_schema_output(
            &item_enum,
            &module_ident,
            name,
            &schema_impl_items,
            &variants.1,
            &delegate_impl_items,
        )
    }

    #[cfg(not(any(feature = "zod", feature = "typescript", feature = "jsonschema")))]
    {
        let _: &_ = &variants.1;
        let output = quote! {
            #item_enum
        };
        log::trace!("{output}");
        TokenStream::from(output)
    }
}

/// Renders one variant of an untagged enum as a union member.
///
/// Returns `(typescript, zod, json_value)` where:
/// - `typescript` is the member type (e.g. `DateString`, `number`, `{ a: A }`)
/// - `zod` is the member schema (e.g. `DateString$Schema`, `z.number().int()`)
/// - `json_value` is a standalone `serde_json::Value` token expression (cfg jsonschema)
///
/// Only `TupleSingle` (`S(T)`) and `Named` (`{ a: A }`) variant kinds are supported; `Unit` and
/// `TupleMultiple` produce a clear compile-time `panic!` because an untagged choice has no
/// discriminator to carry them.
#[cfg(feature = "serde")]
fn render_untagged_variant(
    kind: &VariantKind,
    variant_name: &str,
    field_defs: &[FieldDef],
    self_type_name: &str,
) -> (String, String, proc_macro2::TokenStream) {
    match kind {
        VariantKind::TupleSingle => {
            render_untagged_tuple_single(variant_name, field_defs, self_type_name)
        }
        VariantKind::Named => render_untagged_named(field_defs, self_type_name),
        VariantKind::Unit | VariantKind::TupleMultiple => {
            // An untagged choice has no discriminator to carry these shapes. The assert fails
            // unconditionally here (the match already proved `kind` is Unit/TupleMultiple),
            // surfacing a clear compile-time error during macro expansion.
            assert!(
                matches!(kind, VariantKind::TupleSingle | VariantKind::Named),
                "#[serde(untagged)] supports newtype and struct variants only; `{variant_name}` is unsupported"
            );
            (String::new(), String::new(), quote! {})
        }
    }
}

/// Renders a `TupleSingle` (`S(T)`) untagged variant as a union member (`T` / `T$Schema` / value).
///
/// The inner value is a slot: untagged, the variant carries no key of its own, so the content *is*
/// the whole serialized value and a `None` there reaches the wire as a bare `null` rather than
/// going absent. All three surfaces read it through the slot spellings for that reason.
#[cfg(feature = "serde")]
fn render_untagged_tuple_single(
    variant_name: &str,
    field_defs: &[FieldDef],
    self_type_name: &str,
) -> (String, String, proc_macro2::TokenStream) {
    assert!(
        !field_defs.is_empty(),
        "#[serde(untagged)] newtype variant `{variant_name}` has no inner field"
    );
    let fld = &field_defs[0];

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
    // TypeScript: `{ a: A; b: B }`
    let ts_fields = field_defs
        .iter()
        .map(|fld| {
            format!(
                "{}{}: {}",
                fld.name,
                fld.ts_optional_key_marker(),
                fld.typescript_typename()
            )
        })
        .collect::<Vec<_>>()
        .join("; ");
    let ts = format!("{{ {ts_fields} }}");

    // Zod: `z.strictObject({ a: ..., })`
    #[cfg(feature = "zod")]
    let zod = {
        let mut body = String::from("z.strictObject({ ");
        for fld in field_defs {
            let zod_field_type = fld.zod_type();
            if fld.contains_type_reference(self_type_name) {
                let _ = write!(
                    body,
                    "get {}() {{ return {}; }}, ",
                    fld.name, zod_field_type
                );
            } else {
                let _ = write!(body, "{}: {}, ", fld.name, zod_field_type);
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

/// Builds the `{ type: object, properties, required, additionalProperties: false }` JSON-schema
/// value token for a `Named` untagged variant.
#[cfg(all(feature = "serde", feature = "jsonschema"))]
fn untagged_named_json_value(field_defs: &[FieldDef]) -> proc_macro2::TokenStream {
    let property_inserts = field_defs.iter().map(|fld| {
        let name_str = fld.name.clone();
        let value = field_json_schema_value(fld);
        let required_insert = if fld.is_optional() {
            quote! {}
        } else {
            quote! {
                required.push(serde_json::Value::String(#name_str.to_string()));
            }
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
        {
            let mut string_schema = serde_json::Map::new();
            string_schema.insert(
                "type".to_string(),
                serde_json::Value::String("string".to_string()),
            );
            #pattern_insert
            #min_insert
            #max_insert
            serde_json::Value::Object(string_schema)
        }
    }
}

/// Builds a standalone `serde_json::Value` token expression for a single field, with `Vec<T>`
/// array wrapping. Sibling type of [`flatten_field_json_schema_ref`]; used by untagged enum
/// members where the JSON value is consumed directly (not inserted under a property name).
#[cfg(all(feature = "serde", feature = "jsonschema"))]
fn field_json_schema_value(fld: &FieldDef) -> proc_macro2::TokenStream {
    let inner = match &fld.field_type {
        FieldDefType::SiblingType(name, _) => sibling_json_schema_value(name, fld.type_span),
        FieldDefType::String => string_field_json_schema_value(fld),
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
        | FieldDefType::Isize => quote! { serde_json::json!({ "type": "integer" }) },
        FieldDefType::F32 | FieldDefType::F64 => {
            quote! { serde_json::json!({ "type": "number" }) }
        }
        FieldDefType::Boolean => quote! { serde_json::json!({ "type": "boolean" }) },
        #[cfg(feature = "object_id")]
        FieldDefType::ObjectId => quote! {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "$oid": { "type": "string", "pattern": "^[a-f\\d]{24}$" }
                },
                "required": ["$oid"]
            })
        },
        #[cfg(feature = "chrono")]
        FieldDefType::NaiveDate => {
            quote! { serde_json::json!({ "type": "string", "format": "date" }) }
        }
        #[cfg(feature = "chrono")]
        FieldDefType::NaiveTime => {
            quote! { serde_json::json!({ "type": "string", "format": "time" }) }
        }
        #[cfg(feature = "chrono")]
        FieldDefType::NaiveDateTime | FieldDefType::DateTime => {
            quote! { serde_json::json!({ "type": "string", "format": "date-time" }) }
        }
        // Map / Tuple / Unknown inner shapes are out of scope for v1 untagged members;
        // emit a permissive empty schema rather than silently mis-typing.
        FieldDefType::Map(_, _) | FieldDefType::Tuple(_) | FieldDefType::Unknown => {
            quote! { serde_json::json!({}) }
        }
    };

    arrayed_json_schema_value(fld, inner)
}

/// Collects each untagged variant's union-member parts: the TypeScript member type, the Zod member
/// schema, and the JSON-schema value token, plus the `compile_error!` tokens for any field-level
/// guard violations. The first three are always returned; the caller selects which to use based on
/// the enabled features.
///
/// This path builds its field defs directly rather than through [`process_field`], so it runs the
/// field guards itself. A member renders exactly as a struct field of the same written type does —
/// a named `Option` in the absent form, a map from its key's members — so each guard it escapes
/// would be a rendering this position alone is allowed to write.
#[cfg(feature = "serde")]
fn collect_untagged_members(item_enum: &mut syn::ItemEnum) -> UntaggedMemberData {
    let enum_type_name = item_enum.ident.to_string();
    let mut ts_parts: Vec<String> = Vec::new();
    let mut zod_parts: Vec<String> = Vec::new();
    let mut json_parts: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut guard_errors: Vec<proc_macro2::TokenStream> = Vec::new();

    for variant in &mut item_enum.variants {
        let kind = classify_variant(variant);
        let variant_name = variant.ident.to_string();

        let mut field_defs: Vec<FieldDef> = Vec::new();
        for field in &mut variant.fields {
            let field_name = field
                .ident
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_default();
            let mut field_def = get_field_def(&field_name, &field.ty, "");
            if let Err(err) = check_os_string_field(field, &field_def, &field_label(&field_name)) {
                guard_errors.push(err.to_compile_error());
            }
            #[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
            if let Err(err) = check_map_key(field, &field_def, &field_label(&field_name)) {
                guard_errors.push(err.to_compile_error());
            }
            let serde_field_meta = parse_serde_field_attributes(&field.attrs);
            if let Some(rejection) = serde_field_meta.cfg_attr_rejection.as_ref() {
                guard_errors.push(cfg_attr_guard_error(rejection, &field_label(&field_name)));
            } else if let Err(err) = check_optional_field_serialization(
                field,
                field_def.is_optional(),
                &serde_field_meta,
            ) {
                guard_errors.push(err.to_compile_error());
            } else {
                // No guard violated by this field.
            }
            field_def.resolve_self_references(&enum_type_name);
            field_defs.push(field_def);
        }

        let (ts, zod, json_val) =
            render_untagged_variant(&kind, &variant_name, &field_defs, &enum_type_name);
        ts_parts.push(ts);
        zod_parts.push(zod);
        json_parts.push(json_val);
    }

    (ts_parts, zod_parts, json_parts, guard_errors)
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
) -> TokenStream {
    // Compute the schema module name and register the enum so other types can find it.
    #[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
    let (_, module_ident) = enum_module_idents(name, item_name, AliasKind::NoEnumMembers);

    // Extract docs early for example extraction
    #[cfg(any(feature = "typescript", feature = "zod"))]
    let docs_vec = get_enum_docs(&item_enum);

    #[cfg(feature = "zod")]
    let example_code = docs_vec
        .as_ref()
        .and_then(|docs| extract_example_from_docs(docs));

    // Render each variant into its union member (TS / Zod / JSON parts).
    let (ts_parts, zod_parts, json_parts, guard_errors) = collect_untagged_members(&mut item_enum);

    // A violated field guard makes the whole contract unsound, so the schema surface is dropped
    // and only the original item plus the errors are emitted.
    if let Some(output) = guard_failure_output(&item_enum, &guard_errors) {
        return output;
    }

    #[cfg(not(any(feature = "typescript", feature = "zod", feature = "jsonschema")))]
    let _: &_ = &name;
    #[cfg(not(feature = "zod"))]
    let _: &_ = &&zod_parts;
    #[cfg(not(feature = "jsonschema"))]
    let _: &_ = &&json_parts;

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
    #[cfg(not(feature = "typescript"))]
    let _: &_ = &&ts_parts;

    #[cfg(feature = "zod")]
    let schema_code = format!("z.union([{}])", zod_parts.join(", "));

    #[cfg(feature = "typescript")]
    let docs = build_item_jsdoc(docs_vec.as_deref(), item_name);

    // Generate schema module methods
    #[cfg(feature = "jsonschema")]
    let json_schema_method =
        generate_discriminated_enum_json_schema_method(&main_schema_code, item_name);

    #[cfg(feature = "typescript")]
    let ts_definition_method =
        generate_discriminated_enum_ts_definition_method(&docs, item_name, &type_code);

    #[cfg(feature = "zod")]
    let zod_schema_method = generate_discriminated_enum_zod_schema_method(item_name, &schema_code);

    #[cfg(not(any(feature = "typescript", feature = "zod")))]
    let _: &_ = &item_name;

    #[cfg(feature = "zod")]
    let schema_example_method = build_struct_schema_example(example_code.as_ref(), name);
    #[cfg(all(
        not(feature = "zod"),
        any(feature = "typescript", feature = "jsonschema")
    ))]
    let schema_example_method: Option<proc_macro2::TokenStream> = None;

    // Build schema module impl items (without schema_example)
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
    let delegate_impl_items =
        build_struct_delegate_items(&module_ident, schema_example_method.as_ref(), None);

    // Untagged enums have no per-field serde validation functions.
    let enum_validation_fns: Vec<proc_macro2::TokenStream> = Vec::new();

    #[cfg(any(feature = "zod", feature = "typescript", feature = "jsonschema"))]
    {
        assemble_schema_output(
            &item_enum,
            &module_ident,
            name,
            &schema_impl_items,
            &enum_validation_fns,
            &delegate_impl_items,
        )
    }

    #[cfg(not(any(feature = "zod", feature = "typescript", feature = "jsonschema")))]
    {
        let _: &_ = &enum_validation_fns;
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
    VariantParts {
        json_fields: Vec::new(),
        optional_fields: Vec::new(),
        schema_code: format!("{{\n  {tag_name}: z.literal(\"{discriminator_value}\"),\n"),
        type_code: format!(
            "{{  /**\n{discriminator_docs}\n**/\n  {tag_name}: \"{discriminator_value}\";\n"
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
///
/// Handles different variant kinds:
/// - Unit: `{ type: "Variant" }` (no content field)
/// - Named: `{ type: "Variant", field1: T1, field2: T2 }` (individual named fields)
/// - `TupleSingle`: `{ type: "Variant", value: T }` (single value flattened)
/// - `TupleMultiple`: `{ type: "Variant", value: [T1, T2, ...] }` (tuple array)
///
/// The `self_type_name` is used to detect recursive type references and use getter syntax.
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
        // Add TypeScript type definition
        let _ = writeln!(
            variant_type_code,
            "  /**\n{}\n**/\n  {}{}: {};",
            fld.docs,
            fld.name,
            fld.ts_optional_key_marker(),
            fld.typescript_typename()
        );

        // Add Zod schema definition
        #[cfg(feature = "zod")]
        {
            let zod_field_type = fld.zod_type();
            let is_recursive = fld.contains_type_reference(self_type_name);

            if is_recursive {
                // Use getter syntax to defer the reference
                let _ = writeln!(
                    variant_schema_code,
                    "  get {}() {{ return {}; }},",
                    fld.name, zod_field_type
                );
            } else {
                let _ = writeln!(variant_schema_code, "  {}: {},", fld.name, zod_field_type);
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
    // Add TypeScript type definition with JSDoc comment
    let _ = writeln!(
        variant_type_code,
        "  /** Tuple value */\n  {}: {};",
        content_name,
        fld.typescript_slot_typename()
    );

    // Add Zod schema definition
    #[cfg(feature = "zod")]
    {
        let zod_field_type = fld.zod_slot_type();
        let is_recursive = fld.contains_type_reference(self_type_name);

        if is_recursive {
            // Use getter syntax to defer the reference
            let _ = writeln!(
                variant_schema_code,
                "  get {content_name}() {{ return {zod_field_type}; }},"
            );
        } else {
            let _ = writeln!(variant_schema_code, "  {content_name}: {zod_field_type},");
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
///
/// Every element is a slot: serde writes each position of the tuple, so a `None` there reaches the
/// wire as a `null` in place rather than shortening the tuple. All three surfaces read the elements
/// through the slot spellings for that reason, and the content array is the one
/// [`tuple_json_schema_value`] renders for a tuple field — serde writes the same array in both
/// positions.
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

    // Add TypeScript type definition with JSDoc comment explaining tuple structure
    let tuple_desc: Vec<String> = field_defs
        .iter()
        .enumerate()
        .map(|(i, _)| format!("element {i}"))
        .collect();
    let _ = writeln!(
        variant_type_code,
        "  /** Tuple: [{}] */\n  {}: {};",
        tuple_desc.join(", "),
        content_name,
        ts_tuple
    );

    // Add Zod schema definition using z.tuple()
    #[cfg(feature = "zod")]
    {
        let zod_tuple_types: Vec<String> = field_defs
            .iter()
            .map(super::field_type::FieldDef::zod_slot_type)
            .collect();
        let zod_tuple = format!("z.tuple([{}])", zod_tuple_types.join(", "));

        // Check if any field in the tuple contains a recursive reference
        let is_recursive = field_defs
            .iter()
            .any(|fld| fld.contains_type_reference(self_type_name));

        if is_recursive {
            // Use getter syntax to defer the reference
            let _ = writeln!(
                variant_schema_code,
                "  get {content_name}() {{ return {zod_tuple}; }},"
            );
        } else {
            let _ = writeln!(variant_schema_code, "  {content_name}: {zod_tuple},");
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
///
/// Every value position that can hold a `Vec` — a field, a tuple element, an enum-keyed map member
/// — carries the array-ness on the field itself rather than in its type, so the wrap belongs here
/// once instead of in each renderer. The levels are the ones the field was written at, so a
/// `Vec<Vec<T>>` describes as the array of arrays serde writes for it.
///
/// A level written as an `Option` admits `null` where it sits — inside the array that holds it,
/// which is always written — rather than around the array, which is what the outermost level does
/// and is the caller's to apply.
///
/// A level written as a fixed-size `[T; N]` carries its bounds, so the wrong-length payload serde
/// refuses to deserialize is refused here too.
///
/// `item_schema` is a `serde_json::Value` expression, as is the result. Callers holding a literal
/// fragment want [`arrayed_json_schema_fragment`].
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

/// The JSON schema literal for a type that renders inline as a scalar — the object body itself,
/// which a caller writing inside a `serde_json::json!` inlines and one needing a standalone
/// `serde_json::Value` wraps — or `None` for the composite types (sibling references, maps,
/// tuples, unknowns) that have no inline rendering.
///
/// Neither the array levels nor `is_optional` are consulted: both describe the slot the value sits
/// in, not its type, so each caller wraps this item itself.
#[cfg(feature = "jsonschema")]
fn scalar_field_json_schema_item(fld: &FieldDef) -> Option<proc_macro2::TokenStream> {
    let item_schema = match &fld.field_type {
        FieldDefType::String => quote! { { "type": "string" } },
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
        | FieldDefType::Isize => quote! { { "type": "integer" } },
        FieldDefType::F32 | FieldDefType::F64 => quote! { { "type": "number" } },
        FieldDefType::Boolean => quote! { { "type": "boolean" } },
        #[cfg(feature = "chrono")]
        FieldDefType::NaiveDate => {
            quote! { { "type": "string", "format": "date" } }
        }
        #[cfg(feature = "chrono")]
        FieldDefType::NaiveTime => {
            quote! { { "type": "string", "format": "time" } }
        }
        #[cfg(feature = "chrono")]
        FieldDefType::NaiveDateTime | FieldDefType::DateTime => {
            quote! { { "type": "string", "format": "date-time" } }
        }
        #[cfg(feature = "object_id")]
        FieldDefType::ObjectId => quote! { {
            "type": "object",
            "properties": {
                "$oid": { "type": "string", "pattern": "^[a-f\\d]{24}$" }
            },
            "required": ["$oid"]
        } },
        FieldDefType::Unknown
        | FieldDefType::SiblingType(..)
        | FieldDefType::Map(..)
        | FieldDefType::Tuple(..) => return None,
    };
    Some(item_schema)
}

/// [`build_map_member_item`] for a tuple element, which differs for exactly two value types.
///
/// An `ObjectId` here carries the field-position `$oid` object — patterned, and open — where a
/// `String`-keyed map member carries the closed, unpatterned one; which of the two a slot should
/// carry is unsettled, so this position keeps the rendering it has. A tuple renders as the
/// fixed-arity array its own field position renders, which is the one value the map path has no
/// renderer for: an element is dispatched by the tuple builder itself, so the nesting costs
/// nothing but the recursion.
///
/// Every other value is the member the map path renders, at every depth, so a type describes the
/// same in either slot.
#[cfg(feature = "jsonschema")]
fn build_tuple_element_item(value: &FieldDef) -> Result<MapMemberItem, MapMemberRejection> {
    #[cfg(feature = "object_id")]
    if matches!(value.field_type, FieldDefType::ObjectId) {
        return Ok(MapMemberItem::Fragment(scalar_slot_item(value)?));
    }
    if let FieldDefType::Tuple(elements) = &value.field_type {
        return Ok(MapMemberItem::Value(tuple_json_schema_value(elements)?));
    }
    build_map_member_item(value)
}

/// Builds the base JSON schema for a tuple element, ignoring `is_optional`, or `None` when the
/// element holds a value the dispatch cannot render.
///
/// The element is dispatched in the form its slot normalizes it to, so a sequence wrapper here
/// describes as the `Vec` of the same element does. Everything else is the map path's member
/// dispatch: a sibling is carried by the same reference the field and map-member positions carry,
/// a map carries its own members' renderings, and an opaque value carries the permissive empty
/// schema field position carries — so the type an element holds is described by that type wherever
/// it is named. The nullable wrap is applied by `build_tuple_element_json_schema`, off the element
/// as it was written.
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
///
/// The tokens are a JSON value: a caller writing inside a `serde_json::json!` literal inlines
/// them, one needing a standalone `serde_json::Value` wraps them in `serde_json::json!`.
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
///
/// A declared item's module is named after its exported name, which a `name = "…"` override moves
/// away from the raw ident, so the registry answers first. A name it does not hold is one this
/// expansion has not seen — a type expanded later, or one from another crate — and takes the
/// ident-derived naming, which is all a reference standing before the type has to go on. That
/// naming is what a type alias publishes under, so an alias resolves in either declaration order.
///
/// The ident is spanned on where the sibling was named rather than on the attribute. Nothing the
/// macro can read tells it whether the module will be in scope — a generated module reaches its
/// siblings through `use super::*`, which a type declared inside a function body never joins — so
/// the `E0433` naming a module the author never wrote is the only report there is, and the span is
/// what lands it on the type they did write.
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
///
/// Every position that carries a sibling by reference — a field, a map member, a tuple element, a
/// flattened base — emits this one call, so a type cannot describe as itself in one position and as
/// an open object in another.
///
/// It is the guarded form that is asked for, not the standalone document: the sibling is being
/// written into a document already in progress, and it is the run that knows whether asking has
/// come back around to a name still being written. So the names in flight and the definitions the
/// root will carry travel into the call.
///
/// The whole call is spanned on where the sibling was named, so every failure it can raise — the
/// unresolved module a function-local sibling produces, the missing method a type expanded without
/// `#[model_schema()]` produces — is reported at the type rather than at the attribute. Only the
/// spans change; the tokens are the ones a reference has always carried.
#[cfg(feature = "jsonschema")]
fn sibling_json_schema_value(name: &str, span: proc_macro2::Span) -> proc_macro2::TokenStream {
    let module_ident = sibling_schema_module_ident(name, span);
    quote_spanned! {span=> #module_ident::Schema::json_schema_within(in_flight, hoisted_defs) }
}

/// Builds JSON schema for a tuple element (used for tuple fields and for tuple variants), or the
/// rejection when the element holds a value the dispatch cannot render — which the callers turn
/// into the single diagnostic naming the field.
///
/// An alias's target is rendered through here too: an alias names a type, and this is the dispatch
/// total over the types the crate renders, so what the alias publishes is what a field written as
/// the target would carry.
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
///
/// A tuple field, a tuple nested in a slot, and a multi-element tuple variant's content are the
/// same array, so all three are built here rather than each spelling the bounds itself. The bounds
/// are what pins the minimum an array of `prefixItems` leaves open: draft 2020-12's `"items": false`
/// closes the tail, so without `minItems` a shorter array — one serde can neither write nor read
/// back — still validates.
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

/// The classification every position reads a map key through.
///
/// The sequence spellings come first: they are the key as written, and what they write is a JSON
/// array, which serde never puts in an object key. Reading past one would answer for the element
/// instead — enumerating an enum's members as keys the map cannot hold.
///
/// Below them, only a bare type path can name an enum: a generic spelling is a type this expansion
/// has no `enum_members()` for, and every other type names keys the schema cannot enumerate.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn map_key_path(key: &FieldDef) -> MapKeyPath<'_> {
    if key.array_depth > 0 || sequence_wrapper_element(key).is_some() {
        return MapKeyPath::Sequenced;
    }
    match &key.field_type {
        FieldDefType::String => MapKeyPath::Open,
        FieldDefType::SiblingType(key_type_name, args) if args.is_empty() => {
            MapKeyPath::Enumerated(key_type_name.as_str())
        }
        FieldDefType::SiblingType(..)
        | FieldDefType::Unknown
        | FieldDefType::Map(..)
        | FieldDefType::Tuple(..)
        | FieldDefType::Boolean
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
        #[cfg(feature = "object_id")]
        FieldDefType::ObjectId => MapKeyPath::Unnarrowed,
        #[cfg(feature = "chrono")]
        FieldDefType::DateTime
        | FieldDefType::NaiveDate
        | FieldDefType::NaiveDateTime
        | FieldDefType::NaiveTime => MapKeyPath::Unnarrowed,
    }
}

/// Why this key has no rendering, and `None` for every key that may still have one.
///
/// A sequence-wrapped key is ruled out by its own spelling: serde writes it as a JSON array, which
/// is no object key at all. Below that, only a target the registry positively rules out is named —
/// an unregistered name, a foreign type or one expanded after the type that writes the map, cannot
/// be ruled out at this expansion and is left alone.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn key_rejection(key: &FieldDef) -> Option<MapKeyRejection> {
    match map_key_path(key) {
        MapKeyPath::Sequenced => Some(MapKeyRejection::Sequenced(map_key_element_name(key))),
        MapKeyPath::Enumerated(key_type_name) => proves_no_enum_members(key_type_name)
            .then(|| MapKeyRejection::NoEnumMembers(key_type_name.to_owned())),
        MapKeyPath::Open | MapKeyPath::Unnarrowed => None,
    }
}

/// The Rust spelling of what a map key holds, for the diagnostic a key with no rendering earns.
///
/// The sequence wrapper itself is not named: a `Vec` or a `[T; N]` reaches here as the array levels
/// the parser collapsed it onto, so naming one spelling would name a spelling the author may not
/// have written. The element survives that, so it is what the message says — by its own name where
/// it has one, and by the shape it is written as where it has none, the arity of a tuple key not
/// being what the refusal turns on.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn map_key_element_name(key: &FieldDef) -> String {
    if let Some(element) = sequence_wrapper_element(key) {
        return map_key_element_name(element);
    }
    match &key.field_type {
        FieldDefType::SiblingType(key_type_name, _) => key_type_name.clone(),
        FieldDefType::String | FieldDefType::StringLiteral(_) => "String".to_owned(),
        FieldDefType::Boolean => "bool".to_owned(),
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
    lookup_alias_info(key_type_name)
        .is_some_and(|key_alias| key_alias.kind == AliasKind::NoEnumMembers)
}

/// Why the first map key this field reaches, at any depth, has no rendering.
///
/// Every surface renders such a key: TypeScript writes it as a `Record`'s key type, Zod as a
/// `z.record` key schema, and the JSON schema calls `enum_members()` on it to spell the object's
/// properties. So the key is answered for once, off the field all three render from, rather than
/// by whichever of them a build happens to enable.
///
/// Recurses through `SiblingType` generics, `Map` keys/values, and `Tuple` elements — the walk
/// [`FieldDef::os_string_name`] makes. What position the map sits in is not what decides whether
/// its key can be written.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn map_key_rejection(fld: &FieldDef) -> Option<MapKeyRejection> {
    match &fld.field_type {
        FieldDefType::Map(key, value) => key_rejection(key)
            .or_else(|| map_key_rejection(key))
            .or_else(|| map_key_rejection(value)),
        FieldDefType::SiblingType(_, generics) => generics.iter().find_map(map_key_rejection),
        FieldDefType::Tuple(elements) => elements.iter().find_map(map_key_rejection),
        FieldDefType::Unknown
        | FieldDefType::StringLiteral(_)
        | FieldDefType::Boolean
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
///
/// serde is the whole reason: it writes a JSON object key as a string, a sequence writes an array,
/// and rather than falling back to any other form it refuses the map outright — so there is no wire
/// here to describe, not a wire described badly. The element is named because the wrapper cannot
/// be: the parser has already collapsed every sequence spelling onto array levels.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn sequenced_map_key_message(subject: &str, element_name: &str) -> String {
    format!(
        "{subject}: a map key must be a value serde writes as a string, which is what a JSON object key is — this key is a sequence of `{element_name}`, and serde refuses to serialize a map keyed by one at all"
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
    }
}

/// Rejects a field reaching a map key no surface can write.
///
/// A key the registry proves carries no `enum_members()` is one such: those members are what every
/// surface writes the map's keys from, so a key that has none leaves each of them describing an
/// object whose keys nothing can supply — and leaves the JSON schema calling a method the author
/// never wrote, which rustc blames `#[model_schema()]` for. The key is written as a *type path*,
/// which resolves through any alias, so an alias of a non-enum is named here as the alias the
/// author wrote.
///
/// A sequence-wrapped key is the other: serde writes no object for it at all, so every surface
/// would be describing a shape the map can never take.
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
        MapKeyPath::Sequenced => {
            return Err(MapMemberRejection::SequencedKey(map_key_element_name(
                inner_key,
            )));
        }
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
///
/// Every sequence wrapper writes a JSON array of its element, so the value describes as the `Vec`
/// of that element does: the array-ness moves onto the element, which is the one form a parsed
/// `Vec` value already arrives in, at whatever nesting it was written. Which wrappers those are is
/// the surfaces' one shared answer, so no name reaches a slot as an array on one surface and a
/// schema module of its own on another.
///
/// An `Option` on either side of the wrapper survives the normalization at the level it was
/// written at — inside, it is a `null` among the array's items; outside, it stands in place of the
/// whole array, which a slot cannot drop the way an object key can. The two are different values on
/// the wire, so the member schema keeps them apart.
#[cfg(feature = "jsonschema")]
fn normalized_slot_value(value: &FieldDef) -> FieldDef {
    let mut normalized = value.clone();
    while let FieldDefType::SiblingType(wrapper_name, wrapper_args) = &normalized.field_type
        && is_sequence_wrapper(wrapper_name)
        && let [element] = wrapper_args.as_slice()
    {
        normalized = normalized.collection_element_field(element);
    }
    normalized
}

/// The rendering a value in a slot carries, with the slot wraps left to the caller, or the
/// rejection when the value type has no rendering here.
///
/// A map value is dispatched through this same function, so a nested map carries the rendering its
/// own key and value types give it at every depth rather than an open object. A tuple element is
/// dispatched through it too (via [`build_tuple_element_item`]), so the two slot positions cannot
/// disagree about what a type describes as.
///
/// A tuple is the one value type the mapping cannot render — a tuple element overrides that arm
/// with the array its own field position renders, a map member has no such renderer — and a nested
/// map can be turned away for its key. Either way the callers turn the rejection into the single
/// diagnostic naming the field.
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
            MapMemberItem::Value(sibling_json_schema_value(value_type_name, value.type_span))
        }
        // A map member's `$oid` object is closed and unpatterned, where the shared mapping's
        // field-position rendering carries the hex pattern and leaves the object open.
        #[cfg(feature = "object_id")]
        FieldDefType::ObjectId => MapMemberItem::Fragment(quote! { {
            "type": "object",
            "properties": { "$oid": { "type": "string" } },
            "required": ["$oid"],
            "additionalProperties": false
        } }),
        // An opaque value carries no type name to narrow with, so a member admits any value: the
        // permissive empty schema, as in field position.
        FieldDefType::Unknown => MapMemberItem::Fragment(quote! { {} }),
        // The shared mapping renders every type named here except a tuple, which is the lone
        // `None`. Named exhaustively rather than caught by a wildcard: a new variant must be given
        // a member schema, not silently widened into an open object.
        FieldDefType::Boolean
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
        MapMemberRejection::NonEnumKey(key_type_name) => {
            non_enum_map_key_message(subject, key_type_name)
        }
        MapMemberRejection::SequencedKey(element_name) => {
            sequenced_map_key_message(subject, element_name)
        }
        MapMemberRejection::Tuple => format!(
            "{subject}: a tuple is not supported as a map value — give the value a `#[model_schema()]` struct instead"
        ),
    }
}

/// The one diagnostic a slot the mapping cannot render produces — on either key path, at any depth,
/// and in a tuple slot the value is reached through.
///
/// It replaces the branch's whole output rather than joining it: an emission left in place hands
/// the author a schema the expansion has already rejected, and on the enum-key path a second error
/// on the `value_schema` the failed binding never bound — macro-internal state the author cannot
/// act on. The field and the type are what the author can act on, so each message names both.
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

/// [`build_map_member_item`] for a member of an enum-keyed map, which differs for exactly one value
/// type: an `ObjectId` member here carries the field-position `$oid` object — patterned, and open —
/// where a `String`-keyed member carries the closed, unpatterned one. Which of the two a map member
/// should carry is unsettled, so each key path keeps the rendering it has.
#[cfg(feature = "jsonschema")]
fn build_enum_key_map_member_item(value: &FieldDef) -> Result<MapMemberItem, MapMemberRejection> {
    #[cfg(feature = "object_id")]
    if matches!(value.field_type, FieldDefType::ObjectId) {
        return Ok(MapMemberItem::Fragment(scalar_slot_item(value)?));
    }
    build_map_member_item(value)
}

/// The object an enum-keyed map describes as, as a standalone `serde_json::Value` expression: one
/// property per member the key enumerates, each carrying the value type's own rendering, and closed
/// to every other key. `Err` when the value has no rendering here, or when the registry proves the
/// key carries no members.
///
/// The members are built by a block rather than spelled into a literal — only the expansion's own
/// runtime knows them — and the block is bound inside a `serde_json::json!`, which makes the whole
/// emission an expression. That is what lets one rendering serve every position a map is written
/// in: a field's insertion, a member of an enclosing map, a tuple's `prefixItems` slot. A key that
/// enumerates its members therefore enumerates them at any depth.
///
/// [`check_non_enum_map_key`] walks every field an item renders from and drops the whole schema
/// surface when one reaches a key with no members, so no such key should arrive here. The question
/// is asked again because the `enum_members()` call is *this* function's to emit, and it is only
/// sound for a key that has them: whatever reaches this dispatch, a key the registry rules out
/// leaves with a diagnostic naming it rather than with rustc blaming `#[model_schema()]` for a
/// method the author never wrote.
///
/// The registry rules out only what it has already seen, and it is filled as items expand: a key
/// declared after the type that writes the map, like one foreign to this crate, is unclassified
/// here and keeps the emitting path — it must, since nothing distinguishes it from a type that
/// does carry `enum_members()`. So the call is spanned on the key the field names, and the `E0599`
/// a key without the method raises is reported at the user's type instead of at `#[model_schema()]`.
#[cfg(feature = "jsonschema")]
fn enum_key_map_json_schema_value(
    key_type_name: &str,
    key_span: proc_macro2::Span,
    value: &FieldDef,
) -> Result<proc_macro2::TokenStream, MapMemberRejection> {
    if proves_no_enum_members(key_type_name) {
        return Err(MapMemberRejection::NonEnumKey(key_type_name.to_owned()));
    }

    let normalized = normalized_slot_value(value);
    let member_value = build_enum_key_map_member_item(&normalized)?.into_member_value(&normalized);
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

/// The `properties` insertion a map-typed field produces.
///
/// Every key path builds the map as a standalone value, which is then wrapped for the slot the
/// field is: a sequence wrapper around a map is the *field's* array, not the map's, so the wrap
/// every other field type applies is applied here too and a `Vec<HashMap<..>>` field describes as
/// the array the map-member and tuple-element positions already describe it as. Nullability stays
/// the `required` list's business, as it is for every other field type — only a slot that cannot be
/// dropped spells a `None` as `null`.
#[cfg(feature = "jsonschema")]
fn build_map_field_schema(
    fld: &FieldDef,
    key: &FieldDef,
    value: &FieldDef,
    field_name_str: &str,
) -> proc_macro2::TokenStream {
    log::trace!("Map => field_name: {field_name_str}, key: {key:?}, value: {value:?}");

    let rendered = match map_key_path(key) {
        MapKeyPath::Enumerated(key_type_name) => {
            enum_key_map_json_schema_value(key_type_name, key.type_span, value)
        }
        MapKeyPath::Open => string_key_map_json_schema_value(value),
        MapKeyPath::Unnarrowed => Ok(unnarrowed_key_map_json_schema_value(key)),
        MapKeyPath::Sequenced => Err(MapMemberRejection::SequencedKey(map_key_element_name(key))),
    };
    match rendered {
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

    // Generate constraint insertion statements
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
    if let [element] = lst
        && is_sequence_wrapper(name)
    {
        build_field_type_schema(&fld.collection_element_field(element), field_name_str)
    } else {
        // Every remaining shape is carried by the named type's own schema module: the non-generic
        // sibling (lst.is_empty()), and the generic branded wrapper like DocumentTypeId<String>,
        // whose schema is defined on the wrapper and whose type params do not affect it. A map is
        // not among them — the parser claims both 2-argument map idents before the sibling fallback
        // is reached, so a map arrives as a `Map` and is rendered once, there. A name this arm
        // cannot resolve is the compile error the reference raises at the type, which is what keeps
        // a second rendering — free to widen or to drop the array the one rendering carries — from
        // growing back here.
        generate_type_schema(
            fld,
            field_name_str,
            &sibling_json_schema_value(name, fld.type_span),
        )
    }
}

/// The JSON schema an `ObjectId` describes — the closed `$oid` object serde writes — with
/// `hex_schema` as the schema of the hex string it holds.
///
/// Field position and the branded path read this one spelling, so the transparent brand and the
/// inner it wraps cannot drift apart. The map-member positions still carry their own renderings.
#[cfg(all(feature = "jsonschema", feature = "object_id"))]
fn object_id_json_schema_value(hex_schema: &proc_macro2::TokenStream) -> proc_macro2::TokenStream {
    quote! {
        serde_json::json!({
            "type": "object",
            "properties": {
                "$oid": (#hex_schema)
            },
            "required": ["$oid"],
            "additionalProperties": false
        })
    }
}

/// Builds the JSON schema for a `MongoDB` `ObjectId` field (`{ "$oid": string }`).
#[cfg(all(feature = "jsonschema", feature = "object_id"))]
fn build_object_id_field_schema(fld: &FieldDef, field_name_str: &str) -> proc_macro2::TokenStream {
    let schema = arrayed_json_schema_value(
        fld,
        object_id_json_schema_value(&quote! { serde_json::json!({ "type": "string" }) }),
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
///
/// A Rust tuple field `(A, B, ...)` serializes (via serde) as a fixed-length JSON array, which is
/// what [`tuple_json_schema_value`] renders — the same array a tuple nested in a slot and a
/// multi-element tuple variant's content render, so the positions cannot drift apart.
///
/// An element the dispatch cannot render replaces the whole insertion with the lone diagnostic,
/// as an unrenderable map value does: a schema left in place would describe a field the expansion
/// has already rejected.
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

/// Builds the JSON schema for an opaque field — a `serde_json::Value`, a function pointer, or any
/// other type the parser could not classify. No type name is carried, so there is no schema module
/// to reference: the field admits any value, which is the permissive empty schema. Matches the
/// `unknown` TypeScript type and the `z.unknown()` Zod schema emitted for the same field.
#[cfg(feature = "jsonschema")]
fn build_unknown_field_schema(fld: &FieldDef, field_name_str: &str) -> proc_macro2::TokenStream {
    log::trace!("Unknown => field_name: {field_name_str}, fld: {fld:?}");

    let schema = arrayed_json_schema_value(fld, quote! { serde_json::json!({}) });
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
        | FieldDefType::Isize => build_numeric_field_schema(fld, field_name_str, "integer"),
        FieldDefType::F32 | FieldDefType::F64 => {
            build_numeric_field_schema(fld, field_name_str, "number")
        }
        FieldDefType::Boolean => build_boolean_field_schema(fld, field_name_str),
        #[cfg(feature = "object_id")]
        FieldDefType::ObjectId => build_object_id_field_schema(fld, field_name_str),
        #[cfg(feature = "chrono")]
        FieldDefType::NaiveDate => build_string_format_field_schema(fld, field_name_str, "date"),
        #[cfg(feature = "chrono")]
        FieldDefType::NaiveTime => build_string_format_field_schema(fld, field_name_str, "time"),
        #[cfg(feature = "chrono")]
        FieldDefType::NaiveDateTime => {
            build_string_format_field_schema(fld, field_name_str, "date-time")
        }
        #[cfg(feature = "chrono")]
        FieldDefType::DateTime => {
            build_string_format_field_schema(fld, field_name_str, "date-time")
        }
        FieldDefType::SiblingType(name, lst) => {
            build_sibling_type_field_schema(fld, field_name_str, name, lst)
        }
        FieldDefType::Map(key, value) => build_map_field_schema(fld, key, value, field_name_str),
        FieldDefType::Tuple(lst) => build_tuple_field_schema(fld, field_name_str, lst),
        // Named exhaustively rather than caught by a wildcard: a new variant must be given a
        // schema here, not silently routed to whatever the last arm happens to emit.
        FieldDefType::Unknown => build_unknown_field_schema(fld, field_name_str),
    }
}

/// Builds JSON schema for a field.
#[cfg(feature = "jsonschema")]
fn build_field_schema(fld: &FieldDef) -> proc_macro2::TokenStream {
    let field_name_str = fld.name.clone();
    let schema_code = build_field_type_schema(fld, &field_name_str);

    let required_code = if fld.is_optional() {
        quote! {}
    } else {
        quote! {
            required.push(serde_json::Value::String(#field_name_str.to_string()));
        }
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
    // Always write TypeScript type
    let _ = writeln!(
        type_code,
        "  /**\n{}\n**/\n  {}{}: {};",
        fld.docs,
        fld.name,
        fld.ts_optional_key_marker(),
        fld.typescript_typename()
    );

    // Conditionally return the Zod schema fragment
    #[cfg(feature = "zod")]
    {
        let zod_type = fld.zod_type();

        // Check if this field contains a recursive reference to self
        let is_recursive = self_type_name.is_some_and(|name| fld.contains_type_reference(name));

        if is_recursive {
            // Use getter syntax to defer the reference
            format!("  get {}() {{ return {}; }},\n", fld.name, zod_type)
        } else {
            // Normal property syntax
            format!("  {}: {},\n", fld.name, zod_type)
        }
    }

    #[cfg(not(feature = "zod"))]
    {
        // When zod feature is disabled, there is no schema fragment to emit
        let _: &_ = &self_type_name; // Suppress unused variable warning
        String::new()
    }
}

/// The binding a walk step introduces, numbered by depth so no two steps of one chain collide.
#[cfg(feature = "serde")]
fn wrap_binding(depth: usize) -> proc_macro2::Ident {
    proc_macro2::Ident::new(&format!("value_{depth}"), proc_macro2::Span::call_site())
}

/// Builds the `validate()` contribution for a field, reaching through its wrappers to run the
/// check on the value the constraint actually describes.
///
/// A bare field is checked in place, which is the whole of the body when there is nothing to reach
/// through. Otherwise the chain is bound once and walked: the head binding keeps every later step
/// dereferencing a binding rather than a fresh borrow, and the block keeps its names off the
/// enclosing body, where the next field's chain reuses them.
#[cfg(feature = "serde")]
fn build_field_validation(
    wraps: &[ConstraintWrap],
    field_ident_tok: &proc_macro2::Ident,
    validate_value_fn_ident: &proc_macro2::Ident,
) -> proc_macro2::TokenStream {
    if wraps.is_empty() {
        return quote! {
            if let Err(e) = #validate_value_fn_ident(&self.#field_ident_tok) {
                errors.push(e);
            }
        };
    }
    let head = wrap_binding(0);
    let walk = walk_wraps(wraps, &head, 1, validate_value_fn_ident, CheckSink::Collect);
    quote! {
        {
            let #head = &self.#field_ident_tok;
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
///
/// The declared type is the field's own tokens, both where it is returned and where it is checked,
/// so a type naming a lifetime needs that lifetime declared here — nothing of the struct's generics
/// reaches a free function. The check's parameter is typed rather than inferred: inference reaches
/// the walk before it reaches the return type, and the walk's own leaf call would otherwise settle
/// the helper's `T` as the validator's parameter type instead of the field's.
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
///
/// A path leaf differs only in how the checked value is reached: the validator takes the borrowed
/// path — which every wrap of the walk already ends at — and renders it once, so the checks below
/// are the very ones a `String` field is held to, over the string serde writes for that path.
///
/// Returns (`module_items`, `validate_body`) — both go into the schema module and `validate()` respectively.
#[cfg(feature = "serde")]
fn generate_string_validation_code(
    field_ident: &str,
    helper_stem: &str,
    meta: &ModelSchemaPropMeta,
    shape: &ConstrainedShape,
    field_ty: &syn::Type,
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

    // Build validation checks
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
        let pattern_lit = pattern.clone();
        checks.push(quote! {
            {
                use std::sync::LazyLock;
                static RE: LazyLock<regex::Regex> = LazyLock::new(|| {
                    regex::Regex::new(#pattern_lit).unwrap()
                });
                if !RE.is_match(value) {
                    return Err(format!(
                        "'{}' does not match pattern '{}'",
                        #field_name_lit, #pattern_lit
                    ));
                }
            }
        });
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

    let validate_body = build_field_validation(wraps, &field_ident_tok, &validate_value_fn_ident);

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

    let validate_body = build_field_validation(wraps, &field_ident_tok, &validate_value_fn_ident);

    FieldValidationCode {
        module_items,
        validate_body,
    }
}

/// Reads a field's type down to the value a constraint can land on, collecting the wrappers on the
/// way.
///
/// The three generated surfaces read the same collapsed field, so the constraint they render sits
/// on the innermost value however it was written; this is what lets the Rust validator land it in
/// the same place. Anything else — a sibling type, a map, a tuple, a multi-argument generic — has no
/// value for a length or a range to apply to and yields nothing.
#[cfg(feature = "serde")]
fn constrained_shape(ty: &syn::Type) -> Option<ConstrainedShape> {
    let mut wraps = Vec::new();
    let mut lifetimes: Vec<syn::Lifetime> = Vec::new();
    let mut current = ty;
    loop {
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

/// Rejects a named `Option` field whose serde attributes let a `None` reach the wire as `null`.
///
/// The generated contract renders such a field in the absent form — `T | undefined` and a
/// `z.union([T, z.undefined()]).prefault(undefined)` inside a `z.strictObject` — which admits a
/// missing key but never `null`. `is_optional` is the same signal that drives that rendering,
/// which keeps the guard and the contract from ever disagreeing. Positional fields are exempt: a
/// tuple slot cannot be omitted, so there `None` correctly renders as nullable.
///
/// The subject is the outermost `Option` and only that one, `is_optional` being the question asked
/// of that level. An `Option` written inside a sequence wrapper has no bare `null` to write: the
/// array around it is always written, so the key is always present and the `None` is an item, which
/// the field's own schema describes as nullable.
#[cfg(feature = "serde")]
fn check_optional_field_serialization(
    field: &Field,
    is_optional: bool,
    meta: &SerdeFieldMeta,
) -> Result<(), syn::Error> {
    let Some(ident) = field.ident.as_ref() else {
        return Ok(());
    };
    if !is_optional || meta.omits_none {
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
    positional_constraint_error: Option<proc_macro2::TokenStream>,
) -> Vec<proc_macro2::TokenStream> {
    positional_constraint_error
        .into_iter()
        .chain(serde_field_meta.cfg_attr_rejection.as_ref().map_or_else(
            || {
                check_optional_field_serialization(field, is_optional, serde_field_meta)
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

/// Every guard error the field violates: the two the written type earns — the `OsString` guard and
/// the map-key guard, neither of which any attribute can hide — then what the `model_schema_prop`
/// parser refused, then the unparseable `pattern`, then the serde-side guards when any fired.
///
/// Both `model_schema_prop` guards stand under every feature subset: an unread key and an
/// unparseable `pattern` alike reach the Rust validator, the Zod literal and the JSON schema, and
/// no toggle makes either one mean anything. The map-key guard stands under every subset that
/// emits a schema at all, which is the same set the registry it reads is populated under.
fn collect_field_guard_errors(
    field: &Field,
    field_def: &FieldDef,
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
        .chain(
            prop_meta
                .attr_rejection
                .as_ref()
                .map(|rejection| attr_guard_error(rejection, &label)),
        )
        .chain(
            prop_meta
                .pattern_rejection
                .as_ref()
                .map(|rejection| pattern_guard_error(rejection, &label)),
        )
        .chain(serde_guard_errors)
        .collect()
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
    rename_all: Option<&str>,
    field: &mut Field,
    schema_module_name: Option<&str>,
    variant_ident: Option<&str>,
    type_name: &str,
) -> (
    FieldDef,
    Option<proc_macro2::TokenStream>,
    Option<proc_macro2::TokenStream>,
    Vec<proc_macro2::TokenStream>,
) {
    let mut new_attrs = Vec::new();

    #[cfg(feature = "serde")]
    let serde_field_meta = parse_serde_field_attributes(&field.attrs);
    #[cfg(feature = "serde")]
    let field_rename = serde_field_meta.rename.clone();
    #[cfg(not(feature = "serde"))]
    let field_rename: Option<String> = None;

    // Get raw field ident (before renaming) for validation function name
    let raw_field_ident = field
        .ident
        .as_ref()
        .map(ToString::to_string)
        .unwrap_or_default();

    // Parse model_schema_prop attributes before filtering them out
    let model_schema_prop_meta = parse_model_schema_prop_attributes(&field.attrs);

    // Validate: cannot use both `as` and `preprocess` on the same field
    assert!(
        model_schema_prop_meta.as_type.is_none() || model_schema_prop_meta.preprocess.is_empty(),
        "Cannot use both `as` and `preprocess` on the same field in model_schema_prop"
    );

    // Filter out model_schema_prop attributes, and optionally inject serde deserialize_with
    for attr in &field.attrs {
        if !attr.path().is_ident("model_schema_prop") {
            new_attrs.push(attr.clone());
        }
    }

    // Generate validation code and inject serde attribute if serde feature is enabled
    #[cfg(feature = "serde")]
    let (validation_fn, validate_body, positional_constraint_error) = generate_field_validation(
        field,
        schema_module_name,
        &raw_field_ident,
        variant_ident,
        &model_schema_prop_meta,
        &mut new_attrs,
    );

    #[cfg(not(feature = "serde"))]
    let (validation_fn, validate_body): (
        Option<proc_macro2::TokenStream>,
        Option<proc_macro2::TokenStream>,
    ) = (None, None);
    #[cfg(not(feature = "serde"))]
    let _: &_ = &(schema_module_name, variant_ident);

    field.attrs = new_attrs;

    let field_type: &syn::Type = &field.ty;

    let final_name = get_final_field_name(&raw_field_ident, field_rename.as_deref(), rename_all);
    let field_docs = build_field_docs(field, &final_name);

    // Create the field definition and apply any model_schema_prop overrides
    let mut field_def = get_field_def(&final_name, field_type, &field_docs);

    // Resolve `Self` references to the concrete type name so recursive fields
    // (e.g. `Vec<Self>`) are treated exactly like `Vec<EnclosingType>`. Resolved before the guards
    // read the field so each one asks its question of the type the surfaces will render.
    field_def.resolve_self_references(type_name);

    #[cfg(feature = "serde")]
    let serde_guard_errors = field_guard_errors(
        field,
        &raw_field_ident,
        field_def.is_optional(),
        &serde_field_meta,
        positional_constraint_error,
    );
    #[cfg(not(feature = "serde"))]
    let serde_guard_errors: Vec<proc_macro2::TokenStream> = Vec::new();

    let guard_errors = collect_field_guard_errors(
        field,
        &field_def,
        &raw_field_ident,
        &model_schema_prop_meta,
        serde_guard_errors,
    );

    field_def.model_schema_prop_meta = (model_schema_prop_meta.as_type.is_some()
        || model_schema_prop_meta.literal.is_some()
        || model_schema_prop_meta.min_length.is_some()
        || model_schema_prop_meta.max_length.is_some()
        || model_schema_prop_meta.pattern.is_some()
        || model_schema_prop_meta.minimum.is_some()
        || model_schema_prop_meta.maximum.is_some()
        || model_schema_prop_meta.ts_optional
        || model_schema_prop_meta.as_number
        || !model_schema_prop_meta.preprocess.is_empty())
    .then_some(model_schema_prop_meta);

    let ts_optional_flag = field_def
        .model_schema_prop_meta
        .as_ref()
        .is_some_and(|m| m.ts_optional);
    // A failed assert here surfaces as a compile error at macro-expansion time.
    assert!(
        validate_ts_optional_flag(field_def.is_optional(), ts_optional_flag).is_ok(),
        "#[model_schema_prop(ts_optional)] requires an Option<T> field on field `{final_name}`"
    );

    let as_number_flag = field_def
        .model_schema_prop_meta
        .as_ref()
        .is_some_and(|m| m.as_number);
    assert!(
        validate_as_number_flag(&field_def.field_type, as_number_flag).is_ok(),
        "#[model_schema_prop(as_number)] requires a chrono DateTime<Tz> field on field `{final_name}`"
    );

    // Apply type overrides based on model_schema_prop attributes
    if let Some(meta) = &field_def.model_schema_prop_meta
        && let Some(literal) = &meta.literal
    {
        // If literal is specified, override the field type to StringLiteral
        field_def.field_type = FieldDefType::StringLiteral(literal.clone());
    }
    // TODO: Handle `as` parameter for type overrides in future implementation

    // Update field docs to include length/range constraint information
    apply_constraint_docs(&mut field_def, &final_name);

    (field_def, validation_fn, validate_body, guard_errors)
}

/// Whether the field needs a `#[serde(default)]` written for it alongside the `deserialize_with`.
///
/// serde reads a missing key as a `None` for any field that deserializes as an option — but only
/// while the field deserializes itself. A `deserialize_with` makes that same missing key a hard
/// error, and the generated schemas say the key may be left out, so such a field is given the
/// default that restores what its absence already meant: `None`, wrapped as it was written. A
/// field that already carries a default keeps the one it was written with.
///
/// A transparent wrapper hands its inner type the deserializer it was given, so a run of them
/// changes nothing about which key is optional; a sequence does not, and neither does a `None`
/// found beneath one. So the question is asked of the first wrapper that is not transparent, and
/// anything else is a field whose key was never optional — its absence stays the error it was.
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
/// constraints apply, injects the corresponding `#[serde(deserialize_with = ...)]` attribute — plus
/// the `#[serde(default)]` that keeps an optional key optional under one.
///
/// Returns (`module_items`, `validate_body`, `guard_error`).
#[cfg(feature = "serde")]
fn generate_field_validation(
    field: &Field,
    schema_module_name: Option<&str>,
    raw_field_ident: &str,
    variant_ident: Option<&str>,
    model_schema_prop_meta: &ModelSchemaPropMeta,
    new_attrs: &mut Vec<syn::Attribute>,
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
    let generated = match shape.leaf {
        ConstraintLeaf::Path | ConstraintLeaf::Str => has_string_constraints.then(|| {
            generate_string_validation_code(
                raw_field_ident,
                &helper_stem,
                model_schema_prop_meta,
                &shape,
                &field.ty,
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
            )
        }),
    };
    let Some(validation_code) = generated else {
        return (None, None, None);
    };

    let deserialize_with_path = format!("{module_name}::deserialize_{helper_stem}");
    let path_lit = syn::LitStr::new(&deserialize_with_path, proc_macro2::Span::call_site());
    new_attrs.push(syn::parse_quote! {
        #[serde(deserialize_with = #path_lit)]
    });
    if needs_injected_default(&shape.wraps, has_serde_default(&field.attrs)) {
        new_attrs.push(syn::parse_quote! {
            #[serde(default)]
        });
    }

    (
        Some(validation_code.module_items),
        Some(validation_code.validate_body),
        None,
    )
}

/// Builds the JSDoc-style doc string for a field from its doc comments (or a fallback).
fn build_field_docs(field: &Field, final_name: &str) -> String {
    get_field_docs(field).map_or_else(
        || {
            [final_name.to_owned(), String::new()]
                .into_iter()
                .map(|l| format!(" * {l}"))
                .collect::<Vec<_>>()
                .join("\n")
        },
        |doc_lines| {
            doc_lines
                .into_iter()
                .flat_map(|v| v.lines().map(ToOwned::to_owned).collect::<Vec<_>>())
                .chain(vec![String::new()])
                .map(|l| format!(" * {l}"))
                .collect::<Vec<_>>()
                .join("\n")
        },
    )
}

/// Appends length/range constraint information to a field's generated docs.
fn apply_constraint_docs(field_def: &mut FieldDef, final_name: &str) {
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
) -> proc_macro2::TokenStream {
    generate_struct_json_schema_method_impl(json_schema_fields, flatten_json_schemas, def_name)
}

/// What a struct's `#[serde(flatten)]` fields contribute to the object it writes.
#[cfg(feature = "jsonschema")]
fn flatten_merged_sources(flattened_fields: &[FieldDef]) -> Vec<MergedSource> {
    flattened_fields.iter().map(flatten_merged_source).collect()
}

/// What a value whose members join the object being written contributes to it, labelled with the
/// name the author gave it: the type for a named one, the field otherwise — a shape with no name of
/// its own is only ever pointed at through where it was written.
///
/// An `Option` around it travels too. What it wraps says which members the value contributes, and
/// the `Option` says whether it contributes them at all — the merge owes the object both answers,
/// and nothing below this point can still see the wrapper.
#[cfg(feature = "jsonschema")]
fn flatten_merged_source(fld: &FieldDef) -> MergedSource {
    if let FieldDefType::SiblingType(name, _) = &fld.field_type {
        MergedSource {
            label: name.clone(),
            optional: fld.is_optional(),
            value: sibling_json_schema_value(name, fld.type_span),
        }
    } else {
        MergedSource {
            label: fld.name.clone(),
            optional: fld.is_optional(),
            value: quote! { serde_json::json!({ "type": "object" }) },
        }
    }
}

#[cfg(feature = "typescript")]
/// Generates the TypeScript definition method (TypeScript types only, no Zod schema).
fn generate_ts_definition_method(
    docs: &str,
    item_name: &str,
    type_code: &str,
    fields_empty: bool,
    flatten_types: &[String],
) -> proc_macro2::TokenStream {
    let has_flatten = !flatten_types.is_empty();
    let intersection_only = flatten_types.join(" & ");
    let intersection_suffix: String = flatten_types.iter().fold(String::new(), |mut acc, t| {
        let _ = write!(acc, " & {t}");
        acc
    });

    // TypeScript type generation (only available when typescript feature is enabled)
    let typescript_type_gen = if fields_empty {
        if has_flatten {
            quote::quote! {
                format!("{}\n\nexport type {} = {};", docs, #item_name, #intersection_only)
            }
        } else {
            quote::quote! {
                format!(r#"/**\n{}\n**/\nexport type {} = Record<string, never>;"#, docs, #item_name)
            }
        }
    } else if has_flatten {
        quote::quote! {
            format!("{}\n\nexport type {} = {{\n{}\n}}{};", docs, #item_name, #type_code, #intersection_suffix)
        }
    } else {
        quote::quote! {
            format!("{}\n\nexport type {} = {{\n{}\n}};", docs, #item_name, #type_code)
        }
    };

    #[cfg(all(feature = "jsonschema", feature = "typescript"))]
    let json_docs_gen = generate_json_docs_part();

    #[cfg(not(feature = "jsonschema"))]
    let json_docs_gen = quote::quote! {
        let docs = format!("/**\n{docs}\n **/\n");
    };

    quote::quote! {
        pub fn ts_definition() -> String {
            let docs = #docs;
            #json_docs_gen
            #typescript_type_gen
        }
    }
}

/// A schema an intersection is built from, written so the name it carries is read when the
/// intersection is used rather than while the `const` holding it is being initialized.
///
/// A flattened base is rendered as the base's own exported `const`. Spliced in as it stands, that
/// name is read the moment the containing `const`'s initializer runs — which fails outright when
/// the base is declared below, and no declaration order puts each of two bases that flatten each
/// other above the other. Deferring the read is what makes every emitted module load whatever
/// order they are assembled in; a pair that does flatten each other then fails when something asks
/// it to validate, rather than at the import of everything declared alongside it.
#[cfg(feature = "zod")]
fn deferred_zod_operand(schema: &str) -> String {
    format!("z.lazy(() => {schema})")
}

#[cfg(feature = "zod")]
/// Generates the Zod schema method (Zod schemas only, no TypeScript types).
///
/// Each flattened base joins the intersection through [`deferred_zod_operand`]: a base names a
/// `const` of its own, and one macro invocation sees one type, so nothing here can know whether
/// that `const` is declared above the module this writes or below it.
fn generate_zod_schema_method(
    item_name: &str,
    schema_code: &str,
    show_opts: &str,
    flatten_schemas: &[String],
) -> proc_macro2::TokenStream {
    #[cfg_attr(not(feature = "zod"), allow(unused_variables))]
    let and_suffix: String = flatten_schemas.iter().fold(String::new(), |mut acc, s| {
        let _ = write!(acc, ".and({})", deferred_zod_operand(s));
        acc
    });

    #[cfg(feature = "zod")]
    {
        // When typescript feature is enabled, generate TypeScript-style Zod schema
        // Note: Example injection is handled by the delegating method on the type itself
        #[cfg(feature = "typescript")]
        {
            quote::quote! {
                pub fn zod_schema() -> String {
                    format!(r#"const {}$RawSchema = z.strictObject({{
{}
}}){}{};

export const {}$Schema: ZodType<{}> = {}$RawSchema;"#, #item_name, #schema_code, #show_opts, #and_suffix, #item_name, #item_name, #item_name)
                }
            }
        }

        // When typescript feature is disabled, generate JavaScript-style Zod schema
        #[cfg(not(feature = "typescript"))]
        {
            quote::quote! {
                pub fn zod_schema() -> String {
                    format!(r#"export const {}$Schema = z.strictObject({{
{}
}}){}{};"#, #item_name, #schema_code, #show_opts, #and_suffix)
                }
            }
        }
    }

    #[cfg(not(feature = "zod"))]
    {
        let _: &_ = &(item_name, schema_code, show_opts, flatten_schemas);
        quote::quote! {
            // Zod schema method not available - zod feature disabled
            // To enable: add "zod" to your features
            // Example: tixschema = { features = ["zod"] }
        }
    }
}

#[cfg(all(feature = "jsonschema", feature = "typescript"))]
fn generate_json_docs_part() -> proc_macro2::TokenStream {
    quote::quote! {
        let prettified = serde_json::to_string_pretty(&Self::json_schema()).unwrap().lines().map(|l| format!(" * {l}")).collect::<Vec<_>>().join("\n");
        let docs = format!("/**\n{docs}\n * JSON Schema:\n{prettified}\n **/\n");
    }
}

/// Binds the `docs` local that an enum's `ts_definition()` renders, enriched with the JSON
/// schema when this build of tixschema can produce one.
///
/// The enrichment reads `Self::json_schema()`, so it may only be emitted when tixschema's own
/// features put that method in the schema module — deciding here rather than emitting a `cfg`
/// keeps the reader and the method in agreement regardless of the consumer's feature table.
#[cfg(feature = "typescript")]
fn generate_enum_json_docs_part(docs: &str) -> proc_macro2::TokenStream {
    #[cfg(all(feature = "jsonschema", feature = "zod"))]
    {
        quote::quote! {
            let prettified = serde_json::to_string_pretty(&Self::json_schema()).unwrap().lines().map(|l| format!(" * {l}")).collect::<Vec<_>>().join("\n");
            let docs = format!("/**\n{}\n * JSON Schema:\n{}\n **/\n", #docs, prettified);
        }
    }

    #[cfg(not(all(feature = "jsonschema", feature = "zod")))]
    {
        quote::quote! {
            let docs = format!("/**\n{}\n**/\n", #docs);
        }
    }
}

#[cfg(feature = "jsonschema")]
/// Generates the JSON schema method for plain enums conditionally.
fn generate_plain_enum_json_schema_method(
    enumerated: &[proc_macro2::TokenStream],
    def_name: &str,
) -> proc_macro2::TokenStream {
    #[cfg(feature = "jsonschema")]
    {
        generate_plain_enum_json_schema_method_impl(enumerated, def_name)
    }

    #[cfg(not(feature = "jsonschema"))]
    {
        let _: &_ = &(enumerated, def_name); // Suppress unused variable warning
        quote::quote! {
            // JSON schema method not available - jsonschema feature disabled
            // To enable: add "jsonschema" to your features
            // Example: tixschema = { features = ["jsonschema"] }
        }
    }
}

#[cfg(feature = "typescript")]
/// Generates the TypeScript definition method for plain enums (TypeScript types only).
fn generate_plain_enum_ts_definition_method(
    docs: &str,
    item_name: &str,
    type_code: &str,
) -> proc_macro2::TokenStream {
    #[cfg(feature = "typescript")]
    {
        let json_docs_gen = generate_enum_json_docs_part(docs);

        // TypeScript type generation (only available when typescript feature is enabled)
        let typescript_type_gen = quote::quote! {
            format!("{}export type {} =\n{};", docs, #item_name, #type_code)
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
            // TypeScript definition method not available - typescript feature disabled
            // To enable: add "typescript" to your features
            // Example: tixschema = { features = ["typescript"] }
        }
    }
}

#[cfg(feature = "zod")]
/// Generates the Zod schema method for plain enums (Zod schemas only)
/// Note: Example injection is handled by the delegating method on the type itself.
fn generate_plain_enum_zod_schema_method(
    item_name: &str,
    schema_code: &str,
    description: &str,
) -> proc_macro2::TokenStream {
    #[cfg(feature = "zod")]
    {
        // When typescript feature is enabled, generate TypeScript-style Zod schema
        #[cfg(feature = "typescript")]
        {
            quote::quote! {
                pub fn zod_schema() -> String {
                    format!("const {}$RawSchema = z.enum([{}]).meta({{\n  description: \"{}\",\n}});\n\nexport const {}$Schema: ZodType<{}> = {}$RawSchema;", #item_name, #schema_code, #description, #item_name, #item_name, #item_name)
                }
            }
        }

        // When typescript feature is disabled, generate JavaScript-style Zod schema
        #[cfg(not(feature = "typescript"))]
        {
            quote::quote! {
                pub fn zod_schema() -> String {
                    format!("export const {}$Schema = z.enum([{}]).meta({{\n  description: \"{}\",\n}});", #item_name, #schema_code, #description)
                }
            }
        }
    }

    #[cfg(not(feature = "zod"))]
    {
        let _: &_ = &(item_name, schema_code, description);
        quote::quote! {
            // Zod schema method not available - zod feature disabled
            // To enable: add "zod" to your features
            // Example: tixschema = { features = ["zod"] }
        }
    }
}

#[cfg(feature = "jsonschema")]
/// Generates the JSON schema method for discriminated enums conditionally.
fn generate_discriminated_enum_json_schema_method(
    main_schema_code: &proc_macro2::TokenStream,
    def_name: &str,
) -> proc_macro2::TokenStream {
    json_schema_methods(def_name, &quote::quote! { { #main_schema_code } })
}

#[cfg(feature = "typescript")]
/// Generates the TypeScript definition method for discriminated enums (TypeScript types only).
fn generate_discriminated_enum_ts_definition_method(
    docs: &str,
    item_name: &str,
    type_code: &str,
) -> proc_macro2::TokenStream {
    #[cfg(feature = "typescript")]
    {
        let json_docs_gen = generate_enum_json_docs_part(docs);

        quote::quote! {
            pub fn ts_definition() -> String {
                #json_docs_gen
                let bundled_docs = docs;
                format!(r#"{bundled_docs}export type {} = {};"#, #item_name, #type_code)
            }
        }
    }

    #[cfg(not(feature = "typescript"))]
    {
        quote::quote! {
            // TypeScript definition method not available - typescript feature disabled
            // To enable: add "typescript" to your features
            // Example: tixschema = { features = ["typescript"] }
        }
    }
}

#[cfg(feature = "zod")]
/// Generates the Zod schema method for discriminated enums (Zod schemas only)
/// Note: Example injection is handled by the delegating method on the type itself.
fn generate_discriminated_enum_zod_schema_method(
    item_name: &str,
    schema_code: &str,
) -> proc_macro2::TokenStream {
    #[cfg(feature = "zod")]
    {
        // When typescript feature is enabled, generate TypeScript-style Zod schema
        #[cfg(feature = "typescript")]
        {
            quote::quote! {
                pub fn zod_schema() -> String {
                    format!("const {}$RawSchema = {};\n\nexport const {}$Schema: ZodType<{}> = {}$RawSchema;", #item_name, #schema_code, #item_name, #item_name, #item_name)
                }
            }
        }

        // When typescript feature is disabled, generate JavaScript-style Zod schema
        #[cfg(not(feature = "typescript"))]
        {
            quote::quote! {
                pub fn zod_schema() -> String {
                    format!(r#"export const {}$Schema = {};"#, #item_name, #schema_code)
                }
            }
        }
    }

    #[cfg(not(feature = "zod"))]
    {
        let _: &_ = &(item_name, schema_code);
        quote::quote! {
            // Zod schema method not available - zod feature disabled
            // To enable: add "zod" to your features
            // Example: tixschema = { features = ["zod"] }
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
        let docs_vec = get_item_docs(&alias.attrs)
            .unwrap_or_else(|| vec![export_name.to_owned(), String::new()]);
        let docs_formatted = format_docs_for_ts(&docs_vec, export_name);

        let generics: Vec<String> = alias
            .generics
            .params
            .iter()
            .filter_map(|param| {
                if let GenericParam::Type(tp) = param {
                    Some(safe_type_name(&tp.ident.to_string()))
                } else {
                    None
                }
            })
            .collect();

        generate_ts_alias_method(&docs_formatted, export_name, &generics, field_def)
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
    generics: &[String],
    field_def: &FieldDef,
) -> proc_macro2::TokenStream {
    let ts_generics = if generics.is_empty() {
        String::new()
    } else {
        format!("<{}>", generics.join(", "))
    };

    let alias_name_ts = format!("{export_name}{ts_generics}");
    let target_ts = field_def.typescript_typename();

    let docs_block = docs.to_owned();

    quote! {
        pub fn ts_definition() -> String {
            format!(
                "/**\n{}\n**/\nexport type {} = {};",
                #docs_block,
                #alias_name_ts,
                #target_ts
            )
        }
    }
}

/// The alias target as the JSON mapping reads it: every reference to one of the alias's own type
/// parameters replaced by the opaque type.
///
/// A parameter names no type until the alias is instantiated, and every position that references an
/// alias references it uninstantiated — a field written as `Pair<A, B>` carries the alias module's
/// one schema. So a parameter admits any value, as an opaque field does, while the shape around it
/// — arity, array-ness, a map's keys — is still described.
#[cfg(feature = "jsonschema")]
fn alias_json_schema_field_def(alias: &ItemType, field_def: &FieldDef) -> FieldDef {
    let parameters: Vec<String> = alias
        .generics
        .params
        .iter()
        .filter_map(|param| match param {
            syn::GenericParam::Type(tp) => Some(tp.ident.to_string()),
            syn::GenericParam::Const(_) | syn::GenericParam::Lifetime(_) => None,
        })
        .collect();
    let mut erased = field_def.clone();
    erased.erase_type_parameters(&parameters);
    erased
}

/// The diagnostic an alias whose target the dispatch cannot render emits, in place of the whole
/// `json_schema()` body.
///
/// It replaces the body rather than joining it: a schema left there would describe a type the
/// expansion has already rejected, and every slot naming the alias would carry it. The tokens are
/// the body's tail expression — no trailing semicolon, so the method's return type raises no second
/// error on top of the one the author can act on.
#[cfg(feature = "jsonschema")]
fn alias_json_schema_rejection(
    export_name: &str,
    rejection: &MapMemberRejection,
) -> proc_macro2::TokenStream {
    let message = map_member_rejection_message(&format!("type alias `{export_name}`"), rejection);
    quote! { compile_error!(#message) }
}

/// Builds the alias module's `json_schema()`, or nothing when `jsonschema` is off.
///
/// An alias names a type, so it publishes that type's own schema: the target `FieldDef` — the one
/// the TypeScript and Zod methods render from — through the dispatch a positional slot uses, that
/// being the dispatch total over the types the crate renders and the one whose `ObjectId` is the
/// field-position object. So the alias describes what a field written as the target describes. A
/// sibling target is carried by the shared reference, which resolves through the registry: an alias
/// of an alias lands on the type at the end of the chain, and an alias of a type that never
/// expanded fails on the module it names, exactly as a field of that type does.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn generate_alias_json_schema_method(
    alias: &ItemType,
    export_name: &str,
    field_def: &FieldDef,
) -> proc_macro2::TokenStream {
    #[cfg(feature = "jsonschema")]
    {
        let body = build_tuple_element_json_schema(&alias_json_schema_field_def(alias, field_def))
            .unwrap_or_else(|rejection| alias_json_schema_rejection(export_name, &rejection));
        json_schema_methods(export_name, &body)
    }
    #[cfg(not(feature = "jsonschema"))]
    {
        // Nothing in this build references an alias module's `json_schema()`; the sibling
        // reference that would (`flatten_field_json_schema_ref`) is itself jsonschema-gated.
        let _: &_ = &(alias, export_name, field_def);
        quote! {}
    }
}

#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn generate_alias_zod_method(export_name: &str, field_def: &FieldDef) -> proc_macro2::TokenStream {
    #[cfg(feature = "zod")]
    {
        // The alias's rendered Zod is its FieldDef expression (a tuple alias yields
        // the null-flavored `z.tuple([...])`, a scalar yields `z.string()`, a sibling
        // yields `Name$Schema`). Bind it to `$RawSchema` and re-export the annotated
        // `$Schema`, mirroring how struct/enum schemas expose their const.
        let schema_code = field_def.zod_type();
        quote! {
            pub fn zod_schema() -> String {
                format!(
                    "const {}$RawSchema = {};\n\nexport const {}$Schema: ZodType<{}> = {}$RawSchema;",
                    #export_name, #schema_code, #export_name, #export_name, #export_name
                )
            }
        }
    }
    #[cfg(not(feature = "zod"))]
    {
        // Without the `zod` feature, `FieldDef::zod_type` does not exist; nothing in
        // this build has zod enabled, so the schema method would be cfg'd out anyway.
        let _: &_ = &(export_name, field_def);
        quote! {}
    }
}

#[cfg(test)]
mod tests;

//! Dart type and JSON-codec generation.
//!
//! Emits one `dart_definition()` method per `#[model_schema]` item — a Dart class (or `enum`) plus
//! `fromJson`/`toJson`, generating the way the TypeScript backend generates. Fully independent of
//! the `typescript`/`zod`/`jsonschema` module-and-delegate machinery those three surfaces share:
//! Dart has real reified generics (a class stays literally generic, unlike Zod's runtime schema
//! factories) and resolves a reference across the whole library regardless of declaration order
//! (unlike a JavaScript module's top-to-bottom `const` evaluation), so none of the factory-cache or
//! forward-reference deferral machinery those three surfaces carry applies here.
//! `dart_schema_dispatch` is called directly from `exec_model_schema`, ahead of the
//! `process_struct`/`process_enum`/`process_type_alias` dispatch that consumes the item, and reads
//! its own borrow of it — the other three surfaces' emission is untouched by this module and this
//! module is untouched by them.

use core::cell::RefCell;
use core::fmt::Write as _;
use std::collections::HashMap;

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Fields, Ident, Item, ItemEnum, ItemStruct, ItemType, Variant};

use crate::features::model_schema_prop::parse_model_schema_prop_attributes;
use crate::features::serde::parse_serde_key_omission;
use crate::field_type::{
    FieldDef, FieldDefType, VariantKind, classify_variant, dart_from_json_argument,
    dart_lower_camel, dart_to_json_argument, get_field_def, is_plain_enum, is_sequence_wrapper,
};
use crate::rename_rule::{RenameRule, resolve_rename_rule};
use crate::utils::{
    MapKeyWire, compute_alias_export_name, compute_item_export_name, to_snake_case,
    type_parameters_in_scope,
};

#[cfg(feature = "serde")]
use crate::features::serde::{parse_serde_field_attributes, parse_serde_type_attributes};

/// One field this module has decided belongs on the wire: its Rust name (the Dart field/parameter
/// spelling — left as Rust wrote it, `snake_case` included, rather than re-cased to Dart's own
/// lower-camel convention), its wire name, whether the key always reaches the wire, whether it is a
/// `#[serde(flatten)]` source, and the `FieldDef` describing its type.
struct DartField {
    field_def: FieldDef,
    flatten: bool,
    required: bool,
    rust_name: String,
    wire_name: String,
}

/// The container attributes read off any enum, in the shape `process_enum` itself dispatches on —
/// `tag`/`content`/`untagged` default to "written none of them" without the `serde` feature, which
/// is what leaves an all-unit enum publishing the plain string-union shape regardless.
struct EnumTagAttrs {
    content: Option<String>,
    rename_all: Option<String>,
    rename_all_fields: Option<String>,
    tag: Option<String>,
    untagged: bool,
}

/// One tagged/untagged variant's payload, classified once so every builder below reads it the same
/// way. A `TupleMultiple` payload is folded into one `Tuple` [`FieldDef`], matching how a slot list
/// renders everywhere else in this module.
enum VariantPayload {
    Named(Vec<DartField>),
    Unit,
    Value(Box<FieldDef>),
}

/// The generic converter parameters a `fromJson`/`toJson` pair threads through, beyond its own
/// primary argument — one pair per type parameter, since a Dart function value cannot itself be
/// generic the way the class it decodes can be. Shared by every generic struct, branded newtype,
/// alias, and tagged or untagged enum: a subclass extending a generic sealed base is itself generic
/// at the same parameters, whether or not its own payload happens to use one, so it carries the
/// same converters even when its own body never calls them.
struct GenericCodec {
    /// `, tFromJson` for each parameter — appended, at a *call site*, after another `fromJson`'s own
    /// `json` argument, forwarding the same converters through.
    from_json_args: String,
    /// `, T Function(dynamic) tFromJson` for each parameter — appended, in a *declaration*, after
    /// `fromJson`'s own `json` parameter. Neither this nor [`Self::to_json_params`] ever repeats
    /// the class's own `<T>` — a constructor or method never does, the class it belongs to having
    /// already bound it.
    from_json_params: String,
    /// `dynamic Function(T) tToJson` for each parameter, joined by `, ` — `toJson`'s whole parameter
    /// list, in both a declaration and a call (`toJson` takes no other argument).
    to_json_params: String,
}

/// The tag and content keys a discriminated enum's own `fromJson`/`toJson` dispatch turns on: which
/// object the payload sits in, and how the wire's own tag string is read.
struct TagShape<'shape> {
    content_key: Option<&'shape str>,
    merge_tag_into_object: bool,
    tag_key: &'shape str,
}

/// The pieces of a Dart class body built from a set of [`DartField`]s, before they are joined into
/// a `toJson` method one way or wrapped into one the other way — see `class_body_content` (the
/// plain join) and `tagged_variant_tokens` (which wraps [`Self::to_json_map`] under a tag key
/// instead, for a `Named` payload under adjacent or external tagging).
struct ClassBodyParts {
    ctor_args: String,
    ctor_params: String,
    field_decls: String,
    /// The `{ 'a': a, 'b': b }` map literal a plain `toJson()` returns outright — kept apart from
    /// the method signature around it so a tagged variant can wrap it under a tag key instead.
    to_json_map: String,
}

impl ClassBodyParts {
    fn ctor_parameter_group(&self) -> String {
        if self.ctor_params.is_empty() {
            String::new()
        } else {
            format!("{{{}}}", self.ctor_params)
        }
    }
}

thread_local! {
    /// The Dart class/enum name each Rust ident publishes — the one thing a reference to a sibling
    /// item needs, since the reference's own field carries the type arguments (real Dart generics
    /// need no factory to bind them, unlike Zod). Independent of the three-surface `ALIAS_INFO`
    /// registry in `crate::utils`: Dart has no forward-reference or cycle problem to share
    /// bookkeeping over (whole-library resolution means declaration order never matters), so it
    /// stays its own small map.
    static DART_NAMES: RefCell<HashMap<String, String>> = RefCell::new(HashMap::new());
}

/// The Dart tokens `item` earns, given the `name = "..."` override an author declared on it (the
/// one `ModelSchemaArgs` field this module needs, passed by value rather than widening that
/// struct's visibility — Dart needs no `default_types`, having no JSON-Schema-style requirement for
/// one concrete filling). Dispatches on the item's own shape; an item this module has nothing to
/// say about (anything but a struct, an enum or a type alias) earns nothing.
pub fn dart_schema_dispatch(item: &Item, name_override: Option<&str>) -> TokenStream {
    if let Item::Struct(item_struct) = item {
        struct_dart_tokens(item_struct, name_override)
    } else if let Item::Enum(item_enum) = item {
        enum_dart_tokens(item_enum, name_override)
    } else if let Item::Type(item_type) = item {
        alias_dart_tokens(item_type, name_override)
    } else {
        TokenStream::new()
    }
}

/// The Dart name registered for `rust_ident`, or `None` for a type declared below the one asking —
/// which falls back to its own Rust ident, harmless since a renamed item always re-publishes that
/// ident too, as a `typedef`.
pub fn lookup_dart_name(rust_ident: &str) -> Option<String> {
    DART_NAMES.with(|names| names.borrow().get(rust_ident).cloned())
}

/// Records the Dart name `rust_ident` publishes under, for a later sibling reference to read back.
fn register_dart_name(rust_ident: &str, export_name: &str) {
    DART_NAMES.with(|names| {
        names
            .borrow_mut()
            .insert(rust_ident.to_owned(), export_name.to_owned());
    });
}

/// Whether `attrs` carries a bare `#[serde(transparent)]` — the same test `model_schema.rs` uses to
/// tell a branded newtype from an ordinary tuple struct, duplicated here (rather than reached
/// through a `pub(crate)` widening) since it is a dozen lines of plain `syn` parsing with no feature
/// dependency of its own.
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

/// The module `{ident}_dart` publishes `dart_definition()` from — never a direct inherent
/// `impl {ident}`. A Rust type alias is not a new type: `impl SlotAliasKey { .. }` for
/// `type SlotAliasKey = MetricSlot;` is really `impl MetricSlot { .. }`, which either collides
/// with `MetricSlot`'s own inherent `dart_definition()` (`E0592`, when the target is a declared
/// item in this crate) or is refused outright by the orphan rules (`E0116`) or the primitive-impl
/// restriction (`E0390`), when the target is a foreign or primitive type (`String`, `HashMap`,
/// `bool`, …) — which an alias's target very often is. A module name is never collapsed through an
/// alias the way an impl target is, so it is the one spelling every shape (struct, enum, branded
/// newtype, or alias) can publish `dart_definition()` under safely and uniformly.
fn dart_module_tokens(rust_ident: &str, span: proc_macro2::Span, dart_source: &str) -> TokenStream {
    let module_ident = Ident::new(&format!("{}_dart", to_snake_case(rust_ident)), span);
    quote! {
        pub mod #module_ident {
            pub fn dart_definition() -> String {
                #dart_source.to_owned()
            }
        }
    }
}

/// The `typedef {rust_ident}{generics} = {export_name}{generics};` a renamed item re-publishes
/// under its own Rust ident — the alias a reference declared above the rename still resolves
/// through. Empty for an item that already publishes under its own ident.
fn ident_typedef(rust_ident: &str, export_name: &str, generic_params: &str) -> String {
    if rust_ident == export_name {
        String::new()
    } else {
        format!("\n\ntypedef {rust_ident}{generic_params} = {export_name}{generic_params};")
    }
}

/// `<T, U>` for the type parameters `generics` declares, or the empty string for none.
fn dart_generic_params(generics: &syn::Generics) -> String {
    let parameters = type_parameters_in_scope(generics);
    if parameters.is_empty() {
        String::new()
    } else {
        format!("<{}>", parameters.join(", "))
    }
}

/// One field's `#[model_schema_prop(...)]` metadata, folded into the `FieldDef` `get_field_def`
/// built for it — `get_field_def` reads the Rust type alone, so a field carrying `nullable`,
/// `ts_optional` or `as_number` needs this filled in separately, exactly as `process_field` does
/// for the other three surfaces.
fn field_def_with_prop_meta(name: &str, ty: &syn::Type, attrs: &[syn::Attribute]) -> FieldDef {
    let mut field_def = get_field_def(name, ty, "");
    field_def.model_schema_prop_meta = Some(parse_model_schema_prop_attributes(attrs));
    field_def
}

/// The `#[serde(rename = "...")]` a field or variant earns, honored only where the `serde` feature
/// reads serde attributes at all.
#[cfg(feature = "serde")]
fn rename_override(attrs: &[syn::Attribute]) -> Option<String> {
    parse_serde_field_attributes(attrs).rename
}

#[cfg(not(feature = "serde"))]
const fn rename_override(_attrs: &[syn::Attribute]) -> Option<String> {
    None
}

/// Whether `attrs` carries `#[serde(flatten)]` — always `false` without the `serde` feature, since
/// nothing else reads what a serde attribute means in that build either.
#[cfg(feature = "serde")]
fn field_is_flatten(attrs: &[syn::Attribute]) -> bool {
    parse_serde_field_attributes(attrs).flatten
}

#[cfg(not(feature = "serde"))]
const fn field_is_flatten(_attrs: &[syn::Attribute]) -> bool {
    false
}

/// The wire name a field with Rust name `rust_name` and its own `rename` writes under, once
/// `rule` — the container's own `rename_all`, [`RenameRule::None`] without the `serde` feature —
/// has had its say. An explicit rename always wins over the container's rule, matching serde
/// itself.
fn wire_field_name(rust_name: &str, rename: Option<&str>, rule: RenameRule) -> String {
    rename.map_or_else(|| rule.apply_to_field(rust_name), ToOwned::to_owned)
}

/// A container's own `rename_all`, or [`RenameRule::None`] without the `serde` feature to read it
/// with — shared by a struct and an enum, whose variant names read the very same attribute.
#[cfg(feature = "serde")]
fn container_rename_rule(attrs: &[syn::Attribute]) -> RenameRule {
    let meta = parse_serde_type_attributes(attrs);
    resolve_rename_rule(meta.rename_all.as_deref())
}

#[cfg(not(feature = "serde"))]
fn container_rename_rule(_attrs: &[syn::Attribute]) -> RenameRule {
    resolve_rename_rule(None)
}

/// The container attributes an enum's own dispatch reads, or every field left at its default
/// without the `serde` feature to read one with.
#[cfg(feature = "serde")]
fn enum_tag_attrs(attrs: &[syn::Attribute]) -> EnumTagAttrs {
    let meta = parse_serde_type_attributes(attrs);
    EnumTagAttrs {
        content: meta.content,
        rename_all: meta.rename_all,
        rename_all_fields: meta.rename_all_fields,
        tag: meta.tag,
        untagged: meta.untagged,
    }
}

#[cfg(not(feature = "serde"))]
const fn enum_tag_attrs(_attrs: &[syn::Attribute]) -> EnumTagAttrs {
    EnumTagAttrs {
        content: None,
        rename_all: None,
        rename_all_fields: None,
        tag: None,
        untagged: false,
    }
}

/// Walks a named-field struct's or a struct-shaped enum variant's fields into [`DartField`]s,
/// dropping any field a serde attribute takes off the wire in both directions — the same field
/// `collect_struct_fields` drops for the other three surfaces.
fn collect_dart_fields(
    fields: &Fields,
    rule: RenameRule,
    type_parameters: &[String],
) -> Vec<DartField> {
    let Fields::Named(named) = fields else {
        return Vec::new();
    };
    let mut collected = Vec::new();
    for field in &named.named {
        let Some(ident) = field.ident.as_ref() else {
            continue;
        };
        let rust_name = ident.to_string();
        let omission = parse_serde_key_omission(&field.attrs);
        if omission.absent_from_wire() {
            continue;
        }
        let wire_name = wire_field_name(&rust_name, rename_override(&field.attrs).as_deref(), rule);
        let mut field_def = field_def_with_prop_meta(&rust_name, &field.ty, &field.attrs);
        field_def.erase_type_parameters(type_parameters);
        collected.push(DartField {
            required: !omission.omits_key,
            flatten: field_is_flatten(&field.attrs),
            field_def,
            rust_name,
            wire_name,
        });
    }
    collected
}

// Dart type and JSON-codec rendering, over `&FieldDef` — kept in this module rather than as
// methods on `FieldDef` itself (unlike `typescript_base`/`zod_type`) so `field_type.rs` carries no
// Dart-shaped knowledge at all; every other surface's own rendering stays exactly as it was.

/// Whether `field` carries `#[model_schema_prop(as_number)]` — the same question
/// `FieldDef::has_as_number` answers privately for the other surfaces, read here directly off the
/// public `model_schema_prop_meta` field since that private method is not reachable from this
/// module.
#[cfg(feature = "chrono")]
fn has_as_number(field: &FieldDef) -> bool {
    field
        .model_schema_prop_meta
        .as_ref()
        .is_some_and(|meta| meta.as_number)
}

/// The Dart type before the outer `?` an [`FieldDef::is_optional`] field carries: the scalar
/// match, then one `List<…>` per array level, an inner level written `?` where
/// [`FieldDef::is_nullable_at`] says so. Mirrors `typescript_base`.
fn dart_base(field: &FieldDef) -> String {
    let scalar = match &field.field_type {
        FieldDefType::Unknown => "dynamic".to_owned(),
        FieldDefType::TypeParam(name) => name.clone(),
        FieldDefType::Tuple(elements) => {
            let rendered = elements
                .iter()
                .map(dart_typename)
                .collect::<Vec<_>>()
                .join(", ");
            if elements.len() == 1 {
                format!("({rendered},)")
            } else {
                format!("({rendered})")
            }
        }
        FieldDefType::SiblingType(name, generics) => {
            if let [element] = generics.as_slice()
                && is_sequence_wrapper(name)
            {
                // Re-enters the whole rendering as the arrayed field the wrapper stands for,
                // carrying this field's own array levels with it — see `collection_element_field`.
                return dart_base(&field.collection_element_field(element));
            }
            let class_name = lookup_dart_name(name).unwrap_or_else(|| name.clone());
            if generics.is_empty() {
                class_name
            } else {
                let arguments = generics
                    .iter()
                    .map(dart_typename)
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{class_name}<{arguments}>")
            }
        }
        FieldDefType::Map(key, value) => {
            format!(
                "Map<{}, {}>",
                dart_map_key_typename(key),
                dart_typename(value)
            )
        }
        FieldDefType::Boolean => "bool".to_owned(),
        FieldDefType::Char | FieldDefType::String | FieldDefType::StringLiteral(_) => {
            "String".to_owned()
        }
        FieldDefType::BooleanLiteral(_) => "bool".to_owned(),
        FieldDefType::NumberLiteral(_) => "double".to_owned(),
        FieldDefType::U8
        | FieldDefType::U16
        | FieldDefType::U32
        | FieldDefType::U64
        | FieldDefType::I8
        | FieldDefType::I16
        | FieldDefType::I32
        | FieldDefType::I64
        | FieldDefType::Usize
        | FieldDefType::Isize => "int".to_owned(),
        FieldDefType::F32 | FieldDefType::F64 => "double".to_owned(),
        #[cfg(feature = "object_id")]
        FieldDefType::ObjectId => "ObjectId".to_owned(),
        #[cfg(feature = "chrono")]
        FieldDefType::NaiveDate | FieldDefType::NaiveTime | FieldDefType::NaiveDateTime => {
            "String".to_owned()
        }
        #[cfg(feature = "chrono")]
        FieldDefType::DateTime => {
            if has_as_number(field) {
                "int".to_owned()
            } else {
                "DateTime".to_owned()
            }
        }
    };
    (0..field.array_depth).fold(scalar, |wrapped, level| {
        let item = if field.is_nullable_at(level) {
            format!("{wrapped}?")
        } else {
            wrapped
        };
        format!("List<{item}>")
    })
}

/// The key type a `Map<…>` is written with — the key's own semantic Dart type, since (unlike
/// TypeScript's `Record`) a Dart `Map` places no restriction on its key type at all: a stringified
/// wire key (a bool, an integer, an enum, a timestamp) still reads back as its own real Dart type,
/// the string form living only in [`dart_map_key_decode`]/[`dart_map_key_encode`].
fn dart_map_key_typename(key: &FieldDef) -> String {
    if key.parameter_shape_name().is_some() {
        return "String".to_owned();
    }
    match key.map_key_wire() {
        MapKeyWire::Boolean => "bool".to_owned(),
        #[cfg(feature = "chrono")]
        MapKeyWire::Timestamp => "DateTime".to_owned(),
        MapKeyWire::Enumerated | MapKeyWire::Named => dart_base(key),
    }
}

/// The expression that reads one `Map` key back from the `String` JSON always writes it as — the
/// counterpart of [`dart_map_key_typename`]. `expr` names a `String`.
fn dart_map_key_decode(key: &FieldDef, expr: &str) -> String {
    if key.parameter_shape_name().is_some() {
        return expr.to_owned();
    }
    match key.map_key_wire() {
        MapKeyWire::Boolean => format!("({expr} == \"true\")"),
        #[cfg(feature = "chrono")]
        MapKeyWire::Timestamp => format!("DateTime.parse({expr})"),
        MapKeyWire::Enumerated => format!("{}.fromJson({expr})", dart_base(key)),
        MapKeyWire::Named => match &key.field_type {
            FieldDefType::U8
            | FieldDefType::U16
            | FieldDefType::U32
            | FieldDefType::U64
            | FieldDefType::I8
            | FieldDefType::I16
            | FieldDefType::I32
            | FieldDefType::I64
            | FieldDefType::Usize
            | FieldDefType::Isize => format!("int.parse({expr})"),
            FieldDefType::F32 | FieldDefType::F64 => format!("double.parse({expr})"),
            FieldDefType::Boolean
            | FieldDefType::Char
            | FieldDefType::String
            | FieldDefType::StringLiteral(_)
            | FieldDefType::BooleanLiteral(_)
            | FieldDefType::NumberLiteral(_)
            | FieldDefType::SiblingType(_, _)
            | FieldDefType::Map(_, _)
            | FieldDefType::Tuple(_)
            | FieldDefType::TypeParam(_)
            | FieldDefType::Unknown => dart_decode_expr(key, expr),
            #[cfg(feature = "object_id")]
            FieldDefType::ObjectId => dart_decode_expr(key, expr),
            #[cfg(feature = "chrono")]
            FieldDefType::NaiveDate
            | FieldDefType::NaiveTime
            | FieldDefType::NaiveDateTime
            | FieldDefType::DateTime => dart_decode_expr(key, expr),
        },
    }
}

/// The expression that writes one `Map` key as the `String` JSON always carries it as — the
/// counterpart of [`dart_map_key_decode`]. `expr` names a value of [`dart_map_key_typename`]'s own
/// type.
fn dart_map_key_encode(key: &FieldDef, expr: &str) -> String {
    if key.parameter_shape_name().is_some() {
        return expr.to_owned();
    }
    match key.map_key_wire() {
        MapKeyWire::Boolean => format!("({expr} ? \"true\" : \"false\")"),
        #[cfg(feature = "chrono")]
        MapKeyWire::Timestamp => format!("{expr}.toIso8601String()"),
        MapKeyWire::Enumerated => format!("({expr}).toJson()"),
        MapKeyWire::Named => match &key.field_type {
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
            | FieldDefType::F64 => format!("({expr}).toString()"),
            FieldDefType::Boolean
            | FieldDefType::Char
            | FieldDefType::String
            | FieldDefType::StringLiteral(_)
            | FieldDefType::BooleanLiteral(_)
            | FieldDefType::NumberLiteral(_)
            | FieldDefType::SiblingType(_, _)
            | FieldDefType::Map(_, _)
            | FieldDefType::Tuple(_)
            | FieldDefType::TypeParam(_)
            | FieldDefType::Unknown => {
                format!("({}) as String", dart_encode_expr(key, expr, false))
            }
            #[cfg(feature = "object_id")]
            FieldDefType::ObjectId => format!("({}) as String", dart_encode_expr(key, expr, false)),
            #[cfg(feature = "chrono")]
            FieldDefType::NaiveDate
            | FieldDefType::NaiveTime
            | FieldDefType::NaiveDateTime
            | FieldDefType::DateTime => {
                format!("({}) as String", dart_encode_expr(key, expr, false))
            }
        },
    }
}

/// The Dart type `field` renders as: [`dart_base`] plus the `?` an [`FieldDef::is_optional`] field
/// carries — Dart has one nullable spelling for all three of a bare `Option<T>`,
/// `#[model_schema_prop(ts_optional)]` and `#[model_schema_prop(nullable)]`, since a Dart
/// `Map<String, dynamic>` answers a dropped key and an explicit `null` alike.
///
/// `pub`, like [`lookup_dart_name`]: the `http_rest` Dart client
/// (`features::service_schema::dart_http_client`) reads the same type name for a message, a
/// success, an error and a `header_in`/`header_out` binding, rather than a second copy of this
/// dispatch.
pub fn dart_typename(field: &FieldDef) -> String {
    let base = dart_base(field);
    if field.is_optional() {
        format!("{base}?")
    } else {
        base
    }
}

/// The expression that decodes `field`'s value out of the raw, dynamically-typed JSON `expr`
/// names — the whole field including its outer optionality, which is asked once here rather than
/// by every caller.
fn dart_decode_expr(field: &FieldDef, expr: &str) -> String {
    let unwrapped = dart_decode_at(field, field.array_depth, expr);
    if field.is_optional() {
        format!("({expr} == null ? null : {unwrapped})")
    } else {
        unwrapped
    }
}

/// [`dart_decode_expr`] before the outer optionality: peels `level` `List<dynamic>` levels off
/// `expr`, an inner level's own nullability read off [`FieldDef::is_nullable_at`], then dispatches
/// on [`FieldDef::field_type`] at level `0`.
fn dart_decode_at(field: &FieldDef, level: u8, expr: &str) -> String {
    if level > 0 {
        let inner_level = level - 1;
        let inner = dart_decode_at(field, inner_level, "e");
        let mapper = if field.is_nullable_at(inner_level) {
            format!("(e) => (e == null ? null : {inner})")
        } else {
            format!("(e) => {inner}")
        };
        return format!("({expr} as List<dynamic>).map({mapper}).toList()");
    }
    match &field.field_type {
        FieldDefType::Unknown => expr.to_owned(),
        FieldDefType::TypeParam(name) => format!("{}({expr})", dart_from_json_argument(name)),
        FieldDefType::Tuple(elements) => {
            let rendered = elements
                .iter()
                .enumerate()
                .map(|(index, element)| {
                    dart_decode_expr(element, &format!("({expr} as List<dynamic>)[{index}]"))
                })
                .collect::<Vec<_>>()
                .join(", ");
            if elements.len() == 1 {
                format!("({rendered},)")
            } else {
                format!("({rendered})")
            }
        }
        FieldDefType::SiblingType(name, generics) => {
            if let [element] = generics.as_slice()
                && is_sequence_wrapper(name)
            {
                let combined = field.collection_element_field(element);
                return dart_decode_at(&combined, combined.array_depth, expr);
            }
            let class_name = lookup_dart_name(name).unwrap_or_else(|| name.clone());
            if generics.is_empty() {
                format!("{class_name}.fromJson({expr})")
            } else {
                let converters = generics
                    .iter()
                    .map(|argument| format!("(e) => {}", dart_decode_expr(argument, "e")))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{class_name}.fromJson({expr}, {converters})")
            }
        }
        FieldDefType::Map(key, value) => {
            let key_ty = dart_map_key_typename(key);
            let value_ty = dart_typename(value);
            let key_decode = dart_map_key_decode(key, "e.key");
            let value_decode = dart_decode_expr(value, "e.value");
            format!(
                "Map<{key_ty}, {value_ty}>.fromEntries(({expr} as Map<String, dynamic>).entries.map((e) => MapEntry({key_decode}, {value_decode})))"
            )
        }
        FieldDefType::Boolean => format!("{expr} as bool"),
        FieldDefType::Char | FieldDefType::String | FieldDefType::StringLiteral(_) => {
            format!("{expr} as String")
        }
        FieldDefType::BooleanLiteral(_) => format!("{expr} as bool"),
        FieldDefType::NumberLiteral(_) => format!("({expr} as num).toDouble()"),
        FieldDefType::U8
        | FieldDefType::U16
        | FieldDefType::U32
        | FieldDefType::U64
        | FieldDefType::I8
        | FieldDefType::I16
        | FieldDefType::I32
        | FieldDefType::I64
        | FieldDefType::Usize
        | FieldDefType::Isize => format!("{expr} as int"),
        FieldDefType::F32 | FieldDefType::F64 => format!("({expr} as num).toDouble()"),
        #[cfg(feature = "object_id")]
        FieldDefType::ObjectId => format!("ObjectId.fromJson({expr})"),
        #[cfg(feature = "chrono")]
        FieldDefType::NaiveDate | FieldDefType::NaiveTime | FieldDefType::NaiveDateTime => {
            format!("{expr} as String")
        }
        #[cfg(feature = "chrono")]
        FieldDefType::DateTime => {
            if has_as_number(field) {
                format!("{expr} as int")
            } else {
                format!("DateTime.parse({expr} as String)")
            }
        }
    }
}

/// The expression that encodes `field`'s Dart value `expr` into a `jsonEncode`-safe value (`null`,
/// `bool`, `num`, `String`, `List<dynamic>` or `Map<String, dynamic>`) — the whole field including
/// its outer optionality, mirroring [`dart_decode_expr`].
fn dart_encode_expr(field: &FieldDef, expr: &str, promoted: bool) -> String {
    let unwrapped = dart_encode_at(field, field.array_depth, expr);
    if !field.is_optional() {
        return unwrapped;
    }
    if promoted || unwrapped == expr {
        return format!("({expr} == null ? null : {unwrapped})");
    }
    let asserted = dart_encode_at(field, field.array_depth, &format!("{expr}!"));
    format!("({expr} == null ? null : {asserted})")
}

/// [`dart_encode_expr`] before the outer optionality — the encode counterpart of [`dart_decode_at`].
fn dart_encode_at(field: &FieldDef, level: u8, expr: &str) -> String {
    if level > 0 {
        let inner_level = level - 1;
        let inner = dart_encode_at(field, inner_level, "e");
        let mapper = if field.is_nullable_at(inner_level) {
            format!("(e) => (e == null ? null : {inner})")
        } else {
            format!("(e) => {inner}")
        };
        return format!("{expr}.map({mapper}).toList()");
    }
    match &field.field_type {
        FieldDefType::Unknown => expr.to_owned(),
        FieldDefType::TypeParam(name) => format!("{}({expr})", dart_to_json_argument(name)),
        FieldDefType::Tuple(elements) => {
            let rendered = elements
                .iter()
                .enumerate()
                .map(|(index, element)| {
                    dart_encode_expr(element, &format!("({expr}).${}", index + 1), false)
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!("[{rendered}]")
        }
        FieldDefType::SiblingType(name, generics) => {
            if let [element] = generics.as_slice()
                && is_sequence_wrapper(name)
            {
                let combined = field.collection_element_field(element);
                return dart_encode_at(&combined, combined.array_depth, expr);
            }
            if generics.is_empty() {
                format!("({expr}).toJson()")
            } else {
                let converters = generics
                    .iter()
                    .map(|argument| format!("(e) => {}", dart_encode_expr(argument, "e", true)))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("({expr}).toJson({converters})")
            }
        }
        FieldDefType::Map(key, value) => {
            let key_encode = dart_map_key_encode(key, "e.key");
            let value_encode = dart_encode_expr(value, "e.value", false);
            format!(
                "Map<String, dynamic>.fromEntries(({expr}).entries.map((e) => MapEntry({key_encode}, {value_encode})))"
            )
        }
        FieldDefType::Boolean
        | FieldDefType::Char
        | FieldDefType::String
        | FieldDefType::StringLiteral(_)
        | FieldDefType::BooleanLiteral(_)
        | FieldDefType::NumberLiteral(_)
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
        | FieldDefType::F64 => expr.to_owned(),
        #[cfg(feature = "object_id")]
        FieldDefType::ObjectId => format!("({expr}).toJson()"),
        #[cfg(feature = "chrono")]
        FieldDefType::NaiveDate | FieldDefType::NaiveTime | FieldDefType::NaiveDateTime => {
            expr.to_owned()
        }
        #[cfg(feature = "chrono")]
        FieldDefType::DateTime => {
            if has_as_number(field) {
                expr.to_owned()
            } else {
                format!("({expr}).toIso8601String()")
            }
        }
    }
}

fn generic_codec(parameters: &[String]) -> GenericCodec {
    let from_json_params = parameters.iter().fold(String::new(), |mut acc, parameter| {
        write!(
            acc,
            ", {parameter} Function(dynamic) {}",
            dart_from_json_argument(parameter)
        )
        .unwrap();
        acc
    });
    let from_json_args = parameters.iter().fold(String::new(), |mut acc, parameter| {
        write!(acc, ", {}", dart_from_json_argument(parameter)).unwrap();
        acc
    });
    let to_json_params = parameters
        .iter()
        .map(|parameter| {
            format!(
                "dynamic Function({parameter}) {}",
                dart_to_json_argument(parameter)
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    GenericCodec {
        from_json_args,
        from_json_params,
        to_json_params,
    }
}

/// The constructor, field declarations, and `fromJson`/`toJson` bodies for a set of [`DartField`]s
/// — shared by a named-field struct and a struct-shaped (`Named`) enum variant. `extra_to_json`
/// entries are written into the `toJson` map ahead of the fields (a discriminant key, say).
fn class_body_parts(fields: &[DartField], extra_to_json: &[String]) -> ClassBodyParts {
    let ctor_params: String = fields
        .iter()
        .map(|field| {
            if field.required || !field.field_def.is_optional() {
                format!("required this.{},", field.rust_name)
            } else {
                format!("this.{},", field.rust_name)
            }
        })
        .collect();
    let field_decls = fields.iter().fold(String::new(), |mut acc, field| {
        write!(
            acc,
            "final {} {};",
            dart_typename(&field.field_def),
            field.rust_name
        )
        .unwrap();
        acc
    });
    let ctor_args = fields.iter().fold(String::new(), |mut acc, field| {
        let source = if field.flatten {
            "json".to_owned()
        } else {
            format!("json['{}']", field.wire_name)
        };
        let decode = dart_decode_expr(&field.field_def, &source);
        write!(acc, "{}: {decode},", field.rust_name).unwrap();
        acc
    });
    let to_json_entries: String = fields
        .iter()
        .map(|field| {
            let encode = dart_encode_expr(&field.field_def, &field.rust_name, false);
            if field.flatten {
                if field.field_def.is_optional() {
                    format!(
                        "if ({} != null) ...({encode} as Map<String, dynamic>),",
                        field.rust_name
                    )
                } else {
                    format!("...({encode} as Map<String, dynamic>),")
                }
            } else if field.required || !field.field_def.is_optional() {
                format!("'{}': {encode},", field.wire_name)
            } else {
                format!(
                    "if ({} != null) '{}': {encode},",
                    field.rust_name, field.wire_name
                )
            }
        })
        .collect();
    let extra: String = extra_to_json.join(" ");
    ClassBodyParts {
        ctor_args,
        ctor_params,
        field_decls,
        to_json_map: format!("{{ {extra}{to_json_entries} }}"),
    }
}

/// The Dart class *body* (constructor, fields, `fromJson`, `toJson` — everything a class's own
/// braces hold, no `class X { … }` declaration of its own) for a set of [`DartField`]s — shared by
/// a named-field struct and a struct-shaped (`Named`) enum variant under internal tagging (whose
/// tag key merges into this very map, via `extra_to_json`) — see [`class_body_parts`]. A `Named`
/// variant under adjacent or external tagging does not use this: its `toJson` wraps
/// [`ClassBodyParts::to_json_map`] under a tag key instead of returning it outright, so
/// [`tagged_variant_tokens`] builds that class from [`class_body_parts`] directly.
fn class_body_content(
    class_name: &str,
    fields: &[DartField],
    extra_to_json: &[String],
    codec: &GenericCodec,
) -> String {
    let parts = class_body_parts(fields, extra_to_json);
    format!(
        "const {class_name}({ctor_group}); {field_decls} \
         factory {class_name}.fromJson(Map<String, dynamic> json{fp}) => {class_name}({ctor_args}); \
         Map<String, dynamic> toJson({tp}) => {to_json_map};",
        ctor_group = parts.ctor_parameter_group(),
        field_decls = parts.field_decls,
        ctor_args = parts.ctor_args,
        to_json_map = parts.to_json_map,
        fp = codec.from_json_params,
        tp = codec.to_json_params,
    )
}

/// [`class_body_content`], wrapped as the standalone `class X<T> { … }` a named-field struct
/// publishes (as opposed to a `Named` enum variant's subclass, which wraps the same content in its
/// own `extends` declaration instead — see [`tagged_variant_tokens`]).
fn class_body(
    class_name: &str,
    generic_params: &str,
    fields: &[DartField],
    codec: &GenericCodec,
) -> String {
    let content = class_body_content(class_name, fields, &[], codec);
    format!("class {class_name}{generic_params} {{ {content} }}")
}

/// The Dart tokens a named-field struct earns: a class, plus a `typedef` under its own Rust ident
/// when `name = "..."` moved its published name elsewhere.
fn struct_dart_tokens(item_struct: &ItemStruct, name_override: Option<&str>) -> TokenStream {
    let type_parameters = type_parameters_in_scope(&item_struct.generics);
    if has_serde_transparent(&item_struct.attrs) && is_single_slot(&item_struct.fields) {
        let value_field = single_slot_field(&item_struct.fields, &type_parameters);
        return value_wrapper_tokens(
            &item_struct.ident,
            &item_struct.generics,
            name_override,
            &value_field,
        );
    }
    if matches!(item_struct.fields, Fields::Unnamed(_)) {
        return tuple_struct_dart_tokens(item_struct, name_override);
    }

    let rust_ident = item_struct.ident.to_string();
    let export_name = compute_item_export_name(&rust_ident, name_override);
    register_dart_name(&rust_ident, &export_name);

    let rule = container_rename_rule(&item_struct.attrs);
    let fields = collect_dart_fields(&item_struct.fields, rule, &type_parameters);
    let generic_params = dart_generic_params(&item_struct.generics);
    let codec = generic_codec(&type_parameters);
    let body = class_body(&export_name, &generic_params, &fields, &codec);
    let typedef = ident_typedef(&rust_ident, &export_name, &generic_params);
    let dart_source = format!("{body}{typedef}");

    dart_module_tokens(&rust_ident, item_struct.ident.span(), &dart_source)
}

/// Whether `fields` is a tuple shape (unnamed) with exactly one slot — the shape a branded newtype
/// and a bare-value (non-branded) newtype struct share on the wire, both writing the slot's value
/// alone.
fn is_single_slot(fields: &Fields) -> bool {
    matches!(fields, Fields::Unnamed(unnamed) if unnamed.unnamed.len() == 1)
}

/// The `FieldDef` of a single-slot tuple shape's one field, its type parameters already erased.
/// Falls back to describing an empty value for a shape that is not actually single-slot — never
/// reached given [`is_single_slot`] gates every caller, but left panic-free regardless, since a
/// generator that panics on an input another guard has already refused is strictly worse than one
/// that answers with something harmless.
fn single_slot_field(fields: &Fields, type_parameters: &[String]) -> FieldDef {
    let Fields::Unnamed(unnamed) = fields else {
        return get_field_def("value", &syn::parse_quote!(()), "");
    };
    let Some(slot) = unnamed.unnamed.first() else {
        return get_field_def("value", &syn::parse_quote!(()), "");
    };
    let mut field_def = field_def_with_prop_meta("value", &slot.ty, &slot.attrs);
    field_def.erase_type_parameters(type_parameters);
    field_def
}

/// The Dart tokens for a value that carries no shape of its own on the wire beyond one wrapped
/// value: a branded newtype, a non-branded single-slot ("bare value") tuple struct, or a type
/// alias. All three publish the same way — a class wrapping `value`, `fromJson`/`toJson` delegating
/// to `value_field`'s own — which does give a type-alias reference slightly more nominal weight in
/// Dart than the bare structural alias TypeScript publishes for it; the wire codec the two languages
/// exchange is identical either way, only the Dart-side ergonomics differ from a perfect TypeScript
/// mirror for this one shape.
fn value_wrapper_tokens(
    ident: &Ident,
    generics: &syn::Generics,
    name_override: Option<&str>,
    value_field: &FieldDef,
) -> TokenStream {
    let rust_ident = ident.to_string();
    let export_name = compute_item_export_name(&rust_ident, name_override);
    register_dart_name(&rust_ident, &export_name);

    let generic_params = dart_generic_params(generics);
    let parameters = type_parameters_in_scope(generics);
    let codec = generic_codec(&parameters);
    let decode = dart_decode_expr(value_field, "json");
    let encode = dart_encode_expr(value_field, "value", false);
    let value_ty = dart_typename(value_field);
    let body = format!(
        "class {export_name}{generic_params} {{ const {export_name}(this.value); final {value_ty} value; \
         factory {export_name}.fromJson(dynamic json{fp}) => {export_name}({decode}); \
         dynamic toJson({tp}) => {encode}; }}",
        fp = codec.from_json_params,
        tp = codec.to_json_params,
    );
    let typedef = ident_typedef(&rust_ident, &export_name, &generic_params);
    let dart_source = format!("{body}{typedef}");

    dart_module_tokens(&rust_ident, ident.span(), &dart_source)
}

/// The Dart tokens for a non-branded tuple struct: the single-slot ("bare value") shape shares
/// [`value_wrapper_tokens`] with a branded newtype; a wider tuple struct wraps the fixed-size Dart
/// record its slots describe.
fn tuple_struct_dart_tokens(item_struct: &ItemStruct, name_override: Option<&str>) -> TokenStream {
    let type_parameters = type_parameters_in_scope(&item_struct.generics);
    let Fields::Unnamed(unnamed) = &item_struct.fields else {
        return TokenStream::new();
    };
    let slots: Vec<FieldDef> = unnamed
        .unnamed
        .iter()
        .filter(|slot| !parse_serde_key_omission(&slot.attrs).absent_from_wire())
        .map(|slot| {
            let mut field_def = field_def_with_prop_meta("slot", &slot.ty, &slot.attrs);
            field_def.erase_type_parameters(&type_parameters);
            field_def
        })
        .collect();
    let tuple_field = FieldDef {
        absent_from_wire: false,
        array_depth: 0,
        array_lengths: Vec::new(),
        docs: String::new(),
        field_type: FieldDefType::Tuple(slots),
        model_schema_prop_meta: None,
        name: "value".to_owned(),
        nullable_levels: Vec::new(),
        omits_value: false,
        #[cfg(feature = "jsonschema")]
        type_span: proc_macro2::Span::call_site(),
    };
    value_wrapper_tokens(
        &item_struct.ident,
        &item_struct.generics,
        name_override,
        &tuple_field,
    )
}

/// The Dart tokens a `type X = ...;` alias earns: a class wrapping the target's own value, exactly
/// like a branded newtype — see [`value_wrapper_tokens`].
fn alias_dart_tokens(item_type: &ItemType, name_override: Option<&str>) -> TokenStream {
    let export_name = compute_alias_export_name(&item_type.ident.to_string(), name_override);
    let type_parameters = type_parameters_in_scope(&item_type.generics);
    let mut target = get_field_def(&export_name, &item_type.ty, "");
    target.erase_type_parameters(&type_parameters);
    value_wrapper_tokens(
        &item_type.ident,
        &item_type.generics,
        Some(&export_name),
        &target,
    )
}

/// The Dart tokens an enum earns: a plain enhanced enum for an all-unit, untagged-by-default
/// declaration; a sealed class hierarchy for every tagged or untagged shape otherwise — mirroring
/// exactly the shape `process_enum` itself dispatches a declaration to.
fn enum_dart_tokens(item_enum: &ItemEnum, name_override: Option<&str>) -> TokenStream {
    let rust_ident = item_enum.ident.to_string();
    let export_name = compute_item_export_name(&rust_ident, name_override);
    register_dart_name(&rust_ident, &export_name);
    let tag_attrs = enum_tag_attrs(&item_enum.attrs);
    let writes_bare_variant_names =
        tag_attrs.tag.is_none() && tag_attrs.content.is_none() && !tag_attrs.untagged;
    let dart_source = if is_plain_enum(item_enum) && writes_bare_variant_names {
        plain_enum_dart_source(item_enum, &rust_ident, &export_name)
    } else if tag_attrs.untagged {
        untagged_enum_dart_source(item_enum, &rust_ident, &export_name, &tag_attrs)
    } else {
        tagged_enum_dart_source(item_enum, &rust_ident, &export_name, &tag_attrs)
    };
    dart_module_tokens(&rust_ident, item_enum.ident.span(), &dart_source)
}

/// One tagged/untagged variant's payload — see [`VariantPayload`]. A `TupleMultiple` payload is
/// folded into one `Tuple` [`FieldDef`], matching how a slot list renders everywhere else in this
/// module.
fn variant_payload(
    variant: &Variant,
    field_rule: RenameRule,
    type_parameters: &[String],
) -> VariantPayload {
    match classify_variant(variant) {
        VariantKind::Unit => VariantPayload::Unit,
        VariantKind::Named => VariantPayload::Named(collect_dart_fields(
            &variant.fields,
            field_rule,
            type_parameters,
        )),
        VariantKind::TupleSingle => {
            let slot = variant.fields.iter().next();
            VariantPayload::Value(Box::new(slot.map_or_else(
                || get_field_def("value", &syn::parse_quote!(()), ""),
                |field| {
                    let mut field_def = field_def_with_prop_meta("value", &field.ty, &field.attrs);
                    field_def.erase_type_parameters(type_parameters);
                    field_def
                },
            )))
        }
        VariantKind::TupleMultiple => {
            let slots: Vec<FieldDef> = variant
                .fields
                .iter()
                .map(|field| {
                    let mut field_def = field_def_with_prop_meta("slot", &field.ty, &field.attrs);
                    field_def.erase_type_parameters(type_parameters);
                    field_def
                })
                .collect();
            VariantPayload::Value(Box::new(FieldDef {
                absent_from_wire: false,
                array_depth: 0,
                array_lengths: Vec::new(),
                docs: String::new(),
                field_type: FieldDefType::Tuple(slots),
                model_schema_prop_meta: None,
                name: "value".to_owned(),
                nullable_levels: Vec::new(),
                omits_value: false,
                #[cfg(feature = "jsonschema")]
                type_span: proc_macro2::Span::call_site(),
            }))
        }
    }
}

/// The Dart expression reading a tagged variant's own payload slot out of the top-level `json` (or,
/// under external tagging, the `map` the base class's own dispatch already bound): the content key
/// under adjacent tagging, `json` itself under internal tagging (whose object merges the tag beside
/// the payload's own fields), or `map['{wire_tag}']` under external tagging, whose dispatch has
/// already read the wire tag as the object's one key.
fn tagged_payload_json_expr(wire_tag: &str, shape: &TagShape<'_>) -> String {
    shape.content_key.map_or_else(
        || {
            if shape.merge_tag_into_object {
                "json".to_owned()
            } else {
                format!("map['{wire_tag}']")
            }
        },
        |content| format!("json['{content}']"),
    )
}

/// The `toJson` this module writes for a variant whose own object does not carry the tag key beside
/// its fields: `{'{tag_key}': '{wire_tag}', '{content_key}': {own_encode}}` under adjacent tagging,
/// `{'{tag_key}': '{wire_tag}'}` for a `Unit` variant with no content key at all (real adjacent
/// tagging's own shape for one), or `'{wire_tag}': {own_encode}` under external tagging (no
/// `content_key`) — external's own `Unit` case (`own_encode` absent, no `content_key`) is the bare
/// wire string external tagging actually writes for one.
fn tagged_wrap_encode(shape: &TagShape<'_>, wire_tag: &str, own_encode: Option<&str>) -> String {
    match (shape.content_key, own_encode) {
        (Some(content), Some(encode)) => {
            format!(
                "{{ '{}': '{wire_tag}', '{content}': {encode} }}",
                shape.tag_key
            )
        }
        (Some(_), None) => format!("{{ '{}': '{wire_tag}' }}", shape.tag_key),
        (None, Some(encode)) => format!("{{ '{wire_tag}': {encode} }}"),
        (None, None) => format!("'{wire_tag}'"),
    }
}

/// The Dart tokens for one tagged-union variant: the subclass definition, and the `fromJson`
/// dispatch's own arm (everything after `case '{wire_tag}':`).
///
/// [`TagShape::merge_tag_into_object`] is internal tagging's own shape (a `Named` payload's object
/// carries the tag key beside its fields); every other combination (adjacent tagging, external
/// tagging, and a non-`Named` payload under internal tagging, which real serde refuses and this
/// module answers for only well enough not to panic) instead wraps the tag and the payload's own
/// encoding together at the call site — see [`tagged_wrap_encode`]/[`tagged_payload_json_expr`].
fn tagged_variant_tokens(
    export_name: &str,
    subclass_name: &str,
    generic_params: &str,
    codec: &GenericCodec,
    wire_tag: &str,
    payload: &VariantPayload,
    shape: &TagShape<'_>,
) -> (String, String) {
    let payload_expr = tagged_payload_json_expr(wire_tag, shape);
    let extends = format!("extends {export_name}{generic_params}");
    match payload {
        VariantPayload::Unit => {
            let own_encode = if shape.merge_tag_into_object {
                format!("{{ '{}': '{wire_tag}' }}", shape.tag_key)
            } else {
                tagged_wrap_encode(shape, wire_tag, None)
            };
            let class_text = format!(
                "class {subclass_name}{generic_params} {extends} {{ const {subclass_name}(); \
                 @override dynamic toJson({tp}) => {own_encode}; }}",
                tp = codec.to_json_params,
            );
            (class_text, format!("{subclass_name}{generic_params}()"))
        }
        VariantPayload::Named(fields) => {
            let class_text = if shape.merge_tag_into_object {
                let extra = vec![format!("'{}': '{wire_tag}',", shape.tag_key)];
                let content = class_body_content(subclass_name, fields, &extra, codec);
                format!("class {subclass_name}{generic_params} {extends} {{ {content} }}")
            } else {
                // Adjacent or external tagging: the variant's own field map carries no tag of its
                // own, so `toJson` wraps it under the tag (and, for adjacent tagging, the content
                // key) instead of returning it outright — the `Value` arm above wraps the same way,
                // through the same `tagged_wrap_encode`.
                let parts = class_body_parts(fields, &[]);
                let wrapped = tagged_wrap_encode(shape, wire_tag, Some(&parts.to_json_map));
                format!(
                    "class {subclass_name}{generic_params} {extends} {{ const {subclass_name}({ctor_group}); \
                     {field_decls} factory {subclass_name}.fromJson(Map<String, dynamic> json{fp}) => \
                     {subclass_name}({ctor_args}); @override dynamic toJson({tp}) => {wrapped}; }}",
                    ctor_group = parts.ctor_parameter_group(),
                    field_decls = parts.field_decls,
                    ctor_args = parts.ctor_args,
                    fp = codec.from_json_params,
                    tp = codec.to_json_params,
                )
            };
            let from_json = format!(
                "{subclass_name}{generic_params}.fromJson({payload_expr}{fa})",
                fa = codec.from_json_args,
            );
            (class_text, from_json)
        }
        VariantPayload::Value(field_def) => {
            let decode = dart_decode_expr(field_def, &payload_expr);
            let own_encode = dart_encode_expr(field_def, "value", false);
            let wrapped_encode = if shape.merge_tag_into_object {
                // Internal tagging with a non-`Named` payload: real serde refuses this unless the
                // payload is itself object-shaped (`#[serde(tag = "...")]` cannot carry a scalar or
                // an array beside a bare tag), so by the time this module sees one, the other
                // surfaces have already required an object here — the same guarantee a `Named`
                // payload's own fields carry for free. The tag merges into that object exactly as
                // it does there.
                format!(
                    "{{ '{}': '{wire_tag}', ...({own_encode} as Map<String, dynamic>) }}",
                    shape.tag_key
                )
            } else {
                tagged_wrap_encode(shape, wire_tag, Some(&own_encode))
            };
            let class_text = format!(
                "class {subclass_name}{generic_params} {extends} {{ const {subclass_name}(this.value); \
                 final {} value; @override dynamic toJson({tp}) => {wrapped_encode}; }}",
                dart_typename(field_def),
                tp = codec.to_json_params,
            );
            (
                class_text,
                format!("{subclass_name}{generic_params}({decode})"),
            )
        }
    }
}

/// The Dart tokens a tagged enum (internal, adjacent, or the default external form) earns: a
/// `sealed` base class with a `fromJson` dispatching on the tag, and one subclass per variant.
fn tagged_enum_dart_source(
    item_enum: &ItemEnum,
    rust_ident: &str,
    export_name: &str,
    tag_attrs: &EnumTagAttrs,
) -> String {
    let tag_key = tag_attrs.tag.as_deref().unwrap_or("type");
    let shape = TagShape {
        content_key: tag_attrs.content.as_deref(),
        merge_tag_into_object: tag_attrs.tag.is_some() && tag_attrs.content.is_none(),
        tag_key,
    };
    let variant_rule = resolve_rename_rule(tag_attrs.rename_all.as_deref());
    let field_rule = resolve_rename_rule(tag_attrs.rename_all_fields.as_deref());
    let type_parameters = type_parameters_in_scope(&item_enum.generics);
    let generic_params = dart_generic_params(&item_enum.generics);
    let codec = generic_codec(&type_parameters);

    let mut subclasses = Vec::new();
    let mut is_string_dispatch_arms = Vec::new();
    let mut map_dispatch_arms = Vec::new();
    // Internal and adjacent tagging both read the tag off a *named* key of the very object the
    // payload itself lives in (or beside), so both dispatch the same way; only external tagging —
    // no `tag`, no `content` — reads it off the object's one and only key instead, and answers a
    // bare wire string separately for the one shape (a `Unit` variant) that is not an object at
    // all.
    let is_external = shape.content_key.is_none() && !shape.merge_tag_into_object;
    for variant in &item_enum.variants {
        let variant_rust_name = variant.ident.to_string();
        let wire_tag = rename_override(&variant.attrs)
            .unwrap_or_else(|| variant_rule.apply_to_variant(&variant_rust_name));
        let subclass_name = format!("{export_name}{variant_rust_name}");
        let payload = variant_payload(variant, field_rule, &type_parameters);
        let (class_text, from_json) = tagged_variant_tokens(
            export_name,
            &subclass_name,
            &generic_params,
            &codec,
            &wire_tag,
            &payload,
            &shape,
        );
        subclasses.push(class_text);
        if is_external && matches!(payload, VariantPayload::Unit) {
            is_string_dispatch_arms.push(format!("case '{wire_tag}': return {from_json};"));
        } else {
            map_dispatch_arms.push(format!("case '{wire_tag}': return {from_json};"));
        }
    }

    let base_from_json = if is_external {
        format!(
            "factory {export_name}.fromJson(dynamic json{fp}) {{ if (json is String) {{ switch (json) \
             {{ {} default: throw ArgumentError('Unknown {export_name}: ' + json); }} }} \
             final map = json as Map<String, dynamic>; final tag = map.keys.first; switch (tag) \
             {{ {} default: throw ArgumentError('Unknown {export_name}: ' + tag); }} }}",
            is_string_dispatch_arms.join(" "),
            map_dispatch_arms.join(" "),
            fp = codec.from_json_params,
        )
    } else {
        format!(
            "factory {export_name}.fromJson(Map<String, dynamic> json{fp}) {{ final tag = json['{tag_key}'] \
             as String; switch (tag) {{ {} default: throw ArgumentError('Unknown {export_name}: ' + tag); }} }}",
            map_dispatch_arms.join(" "),
            fp = codec.from_json_params,
        )
    };

    let base = format!(
        "sealed class {export_name}{generic_params} {{ const {export_name}(); {base_from_json} \
         dynamic toJson({tp}); }}",
        tp = codec.to_json_params,
    );
    let typedef = ident_typedef(rust_ident, export_name, &generic_params);
    format!("{base} {}{typedef}", subclasses.join(" "))
}

/// The Dart tokens a `#[serde(untagged)]` enum earns: a `sealed` base class whose `fromJson` tries
/// each variant's own decode in turn (Dart has no runtime structural type inspection to dispatch on
/// ahead of time, unlike Zod's `z.union`), returning the first that does not throw.
fn untagged_enum_dart_source(
    item_enum: &ItemEnum,
    rust_ident: &str,
    export_name: &str,
    tag_attrs: &EnumTagAttrs,
) -> String {
    let field_rule = resolve_rename_rule(tag_attrs.rename_all_fields.as_deref());
    let type_parameters = type_parameters_in_scope(&item_enum.generics);
    let generic_params = dart_generic_params(&item_enum.generics);
    let codec = generic_codec(&type_parameters);
    let extends = format!("extends {export_name}{generic_params}");

    let mut subclasses = Vec::new();
    let mut attempts = Vec::new();
    for variant in &item_enum.variants {
        let variant_rust_name = variant.ident.to_string();
        let subclass_name = format!("{export_name}{variant_rust_name}");
        let payload = variant_payload(variant, field_rule, &type_parameters);
        let (class_text, from_json) = match &payload {
            VariantPayload::Named(fields) => {
                let content = class_body_content(&subclass_name, fields, &[], &codec);
                (
                    format!("class {subclass_name}{generic_params} {extends} {{ {content} }}"),
                    format!(
                        "{subclass_name}{generic_params}.fromJson(json as Map<String, dynamic>{fa})",
                        fa = codec.from_json_args,
                    ),
                )
            }
            VariantPayload::Value(field_def) => {
                let decode = dart_decode_expr(field_def, "json");
                let encode = dart_encode_expr(field_def, "value", false);
                (
                    format!(
                        "class {subclass_name}{generic_params} {extends} {{ const {subclass_name}(this.value); \
                         final {} value; @override dynamic toJson({tp}) => {encode}; }}",
                        dart_typename(field_def),
                        tp = codec.to_json_params,
                    ),
                    format!("{subclass_name}{generic_params}({decode})"),
                )
            }
            VariantPayload::Unit => {
                // Refused for every member of a real untagged enum (serde writes a unit variant as
                // a bare `null` there), so this arm exists only so the match stays exhaustive and
                // this module never panics on an input another guard has already refused.
                (
                    format!(
                        "class {subclass_name}{generic_params} {extends} {{ const {subclass_name}(); \
                         @override dynamic toJson({tp}) => null; }}",
                        tp = codec.to_json_params,
                    ),
                    format!("{subclass_name}{generic_params}()"),
                )
            }
        };
        subclasses.push(class_text);
        attempts.push(format!("try {{ return {from_json}; }} catch (_) {{}}"));
    }

    let base = format!(
        "sealed class {export_name}{generic_params} {{ const {export_name}(); \
         factory {export_name}.fromJson(dynamic json{fp}) \
         {{ {} throw ArgumentError('No variant of {export_name} matched'); }} dynamic toJson({tp}); }}",
        attempts.join(" "),
        fp = codec.from_json_params,
        tp = codec.to_json_params,
    );
    let typedef = ident_typedef(rust_ident, export_name, &generic_params);
    format!("{base} {}{typedef}", subclasses.join(" "))
}

/// One plain-enum variant's Dart member name (lower-camel of its Rust ident) and wire value.
fn plain_enum_member(variant: &Variant, rule: RenameRule) -> (String, String) {
    let rust_name = variant.ident.to_string();
    let member_name = dart_lower_camel(&rust_name);
    let wire = rename_override(&variant.attrs).unwrap_or_else(|| rule.apply_to_variant(&rust_name));
    (member_name, wire)
}

/// The Dart tokens a plain (all-unit, string-wire) enum earns: a `String`-backed enhanced enum.
fn plain_enum_dart_source(item_enum: &ItemEnum, rust_ident: &str, export_name: &str) -> String {
    let rule = container_rename_rule(&item_enum.attrs);
    let members: Vec<(String, String)> = item_enum
        .variants
        .iter()
        .map(|variant| plain_enum_member(variant, rule))
        .collect();
    let member_list = members
        .iter()
        .map(|(name, wire)| format!("{name}('{wire}')"))
        .collect::<Vec<_>>()
        .join(", ");
    let match_arms = members.iter().fold(String::new(), |mut acc, (name, wire)| {
        write!(acc, "'{wire}' => {export_name}.{name},").unwrap();
        acc
    });
    let body = format!(
        "enum {export_name} {{ {member_list}; const {export_name}(this.wireValue); final String wireValue; \
         static {export_name} fromJson(String json) => switch (json) {{ {match_arms} _ => throw \
         ArgumentError('Unknown {export_name}: ' + json) }}; String toJson() => wireValue; }}"
    );
    let typedef = ident_typedef(rust_ident, export_name, "");
    format!("{body}{typedef}")
}

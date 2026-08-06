use syn::{Fields, GenericArgument, Ident, ItemEnum, PathArguments, Type, Variant};

#[cfg(feature = "jsonschema")]
use proc_macro2::Span;
#[cfg(feature = "jsonschema")]
use syn::spanned::Spanned as _;

#[cfg(feature = "serde")]
use syn::Attribute;

use crate::features::model_schema_prop::ModelSchemaPropMeta;
use crate::utils::{lookup_alias_info, safe_type_name, written_type};

#[cfg(feature = "zod")]
use crate::utils::{
    ZodUnionMember, escape_js_regex_literal, publishes_zod_factory, zod_factory_argument,
};

#[cfg(all(feature = "serde", any(feature = "typescript", feature = "zod")))]
use crate::utils::FlattenVariant;

#[cfg(feature = "chrono")]
use crate::features::chrono;
#[cfg(feature = "object_id")]
use crate::features::object_id;

#[cfg(feature = "serde")]
use crate::features::serde::{
    parse_serde_field_attributes as parse_serde_field_attributes_impl,
    parse_serde_type_attributes as parse_serde_type_attributes_impl,
};
// Bring serde metadata types into scope (used by the serde parsing helpers below).
#[cfg(feature = "serde")]
use crate::features::serde::{SerdeFieldMeta, SerdeTypeMeta};

/// Classifies how an enum variant stores its data.
///
/// This is used to determine the correct TypeScript/Zod generation strategy
/// for discriminated union variants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VariantKind {
    /// Named struct fields: `Payment::Card { number: String }`
    /// Generates: `{ type: "Card", number: string }`.
    Named,
    /// Multiple tuple elements: `Value::Complex(String, i64)`
    /// Generates: `{ type: "Complex", value: [string, number] }`.
    TupleMultiple,
    /// Single tuple element: `Value::Text(String)`
    /// Generates: `{ type: "Text", value: string }`.
    TupleSingle,
    /// Unit variant with no fields: `Status::Active`
    /// Generates: `{ type: "Active" }`.
    Unit,
}

/// Enum representing the possible types a field can have in the schema generation system.
#[derive(Clone, Debug, PartialEq)]
pub enum FieldDefType {
    /// Boolean primitive - maps to boolean.
    Boolean,
    /// Boolean literal type — added via `model_schema_prop(literal = true)`.
    /// Maps to `true`/`false` in TS, `z.literal(true)`/`z.literal(false)` in Zod.
    BooleanLiteral(bool),
    /// `char` primitive - serde writes it as a one-character string and reads only that back, so
    /// it is described as one: TypeScript `string`, Zod `z.string().length(1)`, JSON Schema
    /// `{"type": "string", "minLength": 1, "maxLength": 1}`.
    Char,
    #[cfg(feature = "chrono")]
    /// Chrono `DateTime<Tz>` type - requires "`chrono`" feature.
    /// Maps to `string` in TS (ISO 8601 format: "2025-11-29T14:30:00Z").
    /// Zod: `z.string().datetime()`.
    /// JSON Schema: string with format "date-time".
    /// Note: the timezone type parameter is ignored for schema generation.
    DateTime,
    F32,
    F64,
    I16,
    I32,
    I64,
    I8,
    Isize,
    /// Map type (`HashMap`<K, V>) - only String keys supported per rules
    /// Boxed for recursion. Generates Partial<Record<K, V>> in TS.
    Map(Box<FieldDef>, Box<FieldDef>),
    #[cfg(feature = "chrono")]
    /// Chrono `NaiveDate` type - requires "`chrono`" feature.
    /// Maps to `string` in TS (ISO 8601 date format: "2025-11-29").
    /// Zod: `z.string().date()`.
    /// JSON Schema: string with format "date".
    NaiveDate,
    #[cfg(feature = "chrono")]
    /// Chrono `NaiveDateTime` type - requires "`chrono`" feature.
    /// Maps to `string` in TS (ISO 8601 format: "2025-11-29T14:30:00").
    /// Zod: `z.string().datetime({ local: true })`.
    /// JSON Schema: string with format "date-time".
    NaiveDateTime,
    #[cfg(feature = "chrono")]
    /// Chrono `NaiveTime` type - requires "`chrono`" feature.
    /// Maps to `string` in TS (format: "14:30:00").
    /// Zod: `z.string().time()`.
    /// JSON Schema: string with format "time".
    NaiveTime,
    /// Numeric literal type — added via `model_schema_prop(literal = 214)`.
    /// Maps to `214` in TS, `z.literal(214)` in Zod. Stored as `f64` regardless of the field's own
    /// integer or float type, so a whole value renders without the trailing `.0` `f64` carries.
    NumberLiteral(f64),
    #[cfg(feature = "object_id")]
    /// `MongoDB` `ObjectId` type - requires "`object_id`" feature.
    /// Maps to `ObjectId` interface in TS with `$oid: string`.
    /// Zod: `z.object({ $oid: z.string().regex(...) })`.
    /// JSON Schema: object with `$oid` string property.
    /// See `README.md` for serialization format and validation details.
    ObjectId,
    /// Reference to another struct/enum type, potentially with generics
    /// First String is the type name (without Json suffix in TS)
    /// Vec<FieldDef> holds generic parameters if any.
    SiblingType(String, Vec<FieldDef>),
    /// String primitive - maps to string.
    String,
    /// String literal type - for fixed string values
    /// Added via `model_schema_prop(literal` = "value")
    /// Maps to "value" in TS, z.literal("value") in Zod.
    StringLiteral(String), // For string literal types like "Tixena"
    /// Tuple type - generates anonymous object in TS/Zod.
    Tuple(Vec<FieldDef>),
    /// One of the enclosing item's own type parameters — `IdType` in `struct Wrapper<IdType>`.
    TypeParam(String),
    U16,
    U32,
    U64,
    U8,
    /// Unknown or unsupported type - generates 'unknown' in TS/Zod.
    Unknown,
    Usize,
}

impl FieldDefType {
    /// Whether this is one of the numeric primitives — every integer and float kind a
    /// `literal = N` may collapse into a [`Self::NumberLiteral`].
    pub const fn is_numeric(&self) -> bool {
        matches!(
            self,
            Self::U8
                | Self::U16
                | Self::U32
                | Self::U64
                | Self::I8
                | Self::I16
                | Self::I32
                | Self::I64
                | Self::Usize
                | Self::Isize
                | Self::F32
                | Self::F64
        )
    }
}

/// Struct representing a field's definition for schema generation.
#[derive(Clone, Debug)]
pub struct FieldDef {
    pub absent_from_wire: bool,
    pub array_depth: u8,
    pub array_lengths: Vec<(u8, usize)>,
    pub docs: String,
    pub field_type: FieldDefType,
    pub model_schema_prop_meta: Option<ModelSchemaPropMeta>,
    pub name: String,
    pub nullable_levels: Vec<u8>,
    pub omits_value: bool,
    #[cfg(feature = "jsonschema")]
    pub type_span: Span,
}

/// Two field defs are equal when they describe the same value on every surface: the same type, the
/// same array levels, the same fixed lengths and the same nullable levels.
///
/// What the author wrote *around* the value is left out. A name, a doc comment and a
/// `model_schema_prop` are the field's, not the type's, and two fields differing only in those
/// render the same schema — which is exactly the question `as = Type` asks of its target.
impl PartialEq for FieldDef {
    fn eq(&self, other: &Self) -> bool {
        self.array_depth == other.array_depth
            && self.array_lengths == other.array_lengths
            && self.nullable_levels == other.nullable_levels
            && self.field_type == other.field_type
    }
}

impl FieldDef {
    /// The enclosing item's own type parameter this field reaches *below* its own position — the
    /// `T` a `Later<T>` hands to the type it names, and the same `T` inside a tuple or a map.
    #[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
    pub fn argument_parameter_name(&self) -> Option<&str> {
        match &self.field_type {
            FieldDefType::SiblingType(_, arguments) => arguments
                .iter()
                .find_map(|argument| argument.parameter_or_argument_name()),
            FieldDefType::Map(key, value) => key
                .parameter_or_argument_name()
                .or_else(|| value.parameter_or_argument_name()),
            FieldDefType::Tuple(elements) => {
                elements.iter().find_map(Self::parameter_or_argument_name)
            }
            FieldDefType::TypeParam(_)
            | FieldDefType::Unknown
            | FieldDefType::StringLiteral(_)
            | FieldDefType::BooleanLiteral(_)
            | FieldDefType::NumberLiteral(_)
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

    /// The element of a collection wrapper, as the field the wrapper's own serialization makes it.
    pub fn collection_element_field(&self, element: &Self) -> Self {
        let mut arrayed = element.clone();
        arrayed.name.clone_from(&self.name);
        arrayed.array_depth = element.array_depth.saturating_add(1);
        for level in &self.nullable_levels {
            arrayed.mark_nullable_at(level.saturating_add(arrayed.array_depth));
        }
        for &(level, length) in &self.array_lengths {
            arrayed.mark_fixed_length_at(level.saturating_add(arrayed.array_depth), length);
        }
        arrayed.array_depth = arrayed.array_depth.saturating_add(self.array_depth);
        arrayed
            .model_schema_prop_meta
            .clone_from(&self.model_schema_prop_meta);
        arrayed.omits_value = self.omits_value;
        arrayed.absent_from_wire = self.absent_from_wire;
        arrayed
    }

    /// The shape this field renders when the values a bound could be spelled against are its
    /// members rather than the field itself.
    pub const fn composite_shape_name(&self) -> Option<&'static str> {
        match &self.field_type {
            FieldDefType::Map(_, _) => Some("a map"),
            FieldDefType::Tuple(_) => Some("a tuple"),
            #[cfg(feature = "object_id")]
            FieldDefType::ObjectId => None,
            #[cfg(feature = "chrono")]
            FieldDefType::NaiveDate
            | FieldDefType::NaiveTime
            | FieldDefType::NaiveDateTime
            | FieldDefType::DateTime => None,
            FieldDefType::SiblingType(_, _)
            | FieldDefType::TypeParam(_)
            | FieldDefType::Unknown
            | FieldDefType::StringLiteral(_)
            | FieldDefType::BooleanLiteral(_)
            | FieldDefType::NumberLiteral(_)
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
        }
    }

    /// Whether a length, a pattern or a range written on this field reaches no surface at all —
    /// the one question both the refusal and the docs are written from, so neither can come to
    /// answer it differently from the other.
    pub fn constraints_reach_nothing(&self) -> bool {
        self.fixed_shape_name().is_some()
            || self.composite_shape_name().is_some()
            || self.parameter_shape_name().is_some()
    }

    #[cfg(feature = "zod")]
    /// Checks if this field contains a reference to the given type name.
    pub fn contains_type_reference(&self, type_name: &str) -> bool {
        match &self.field_type {
            FieldDefType::SiblingType(name, generics) => {
                // Direct match (check both original name and stripped name)
                let stripped_name = safe_type_name(name);
                if name == type_name || stripped_name == type_name {
                    return true;
                }
                generics
                    .iter()
                    .any(|g| g.contains_type_reference(type_name))
            }
            FieldDefType::Map(k, v) => {
                k.contains_type_reference(type_name) || v.contains_type_reference(type_name)
            }
            FieldDefType::Tuple(elements) => elements
                .iter()
                .any(|e| e.contains_type_reference(type_name)),
            // Primitive and leaf types can't contain recursive references
            FieldDefType::TypeParam(_)
            | FieldDefType::Unknown
            | FieldDefType::StringLiteral(_)
            | FieldDefType::BooleanLiteral(_)
            | FieldDefType::NumberLiteral(_)
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
            | FieldDefType::F64 => false,
            #[cfg(feature = "object_id")]
            FieldDefType::ObjectId => false,
            #[cfg(feature = "chrono")]
            FieldDefType::NaiveDate
            | FieldDefType::NaiveTime
            | FieldDefType::NaiveDateTime
            | FieldDefType::DateTime => false,
        }
    }

    /// Rewrites every name that is one of the enclosing item's own type parameters into
    /// [`FieldDefType::TypeParam`], so the surfaces stop reading it as a reference to another
    /// generated type.
    pub fn erase_type_parameters(&mut self, parameters: &[String]) {
        if let FieldDefType::SiblingType(name, _) = &self.field_type
            && parameters.iter().any(|parameter| parameter == name)
        {
            self.field_type = FieldDefType::TypeParam(name.clone());
            return;
        }
        for nested in self.nested_type_positions() {
            nested.erase_type_parameters(parameters);
        }
    }

    /// The element count the array at `level` was written with, for a level written as a `[T; N]`
    /// whose `N` the expansion could read. `None` is every other level: serde writes as many items
    /// as it holds there, so nothing bounds it.
    #[cfg(any(feature = "jsonschema", feature = "zod"))]
    pub fn fixed_length_at(&self, level: u8) -> Option<usize> {
        self.array_lengths
            .iter()
            .find(|&&(at, _)| at == level)
            .map(|&(_, length)| length)
    }

    /// The name of the type this field renders as, when that type's schema is one the crate writes
    /// whole and a `model_schema_prop` bound has no place in.
    pub const fn fixed_shape_name(&self) -> Option<&'static str> {
        match &self.field_type {
            #[cfg(feature = "object_id")]
            FieldDefType::ObjectId => Some("ObjectId"),
            #[cfg(feature = "chrono")]
            FieldDefType::NaiveDate => Some("chrono::NaiveDate"),
            #[cfg(feature = "chrono")]
            FieldDefType::NaiveTime => Some("chrono::NaiveTime"),
            #[cfg(feature = "chrono")]
            FieldDefType::NaiveDateTime => Some("chrono::NaiveDateTime"),
            #[cfg(feature = "chrono")]
            FieldDefType::DateTime => Some("chrono::DateTime"),
            FieldDefType::SiblingType(_, _)
            | FieldDefType::Map(_, _)
            | FieldDefType::Tuple(_)
            | FieldDefType::TypeParam(_)
            | FieldDefType::Unknown
            | FieldDefType::StringLiteral(_)
            | FieldDefType::BooleanLiteral(_)
            | FieldDefType::NumberLiteral(_)
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
        }
    }

    /// What an object flattening this field joins for each variant of the externally tagged enum it
    /// names, as that enum recorded them, and nothing for a field that names no such enum.
    ///
    /// Answered under the same bound [`Self::zod_union_members`] is answered under, and for the same
    /// reason: what is spliced in the name's place has to be the whole of what the operand
    /// validates.
    #[cfg(all(feature = "serde", any(feature = "typescript", feature = "zod")))]
    pub fn flatten_variants(&self) -> Vec<FlattenVariant> {
        let wrapped = self
            .model_schema_prop_meta
            .as_ref()
            .is_some_and(|meta| !meta.preprocess.is_empty());
        if self.array_depth > 0 || wrapped {
            return Vec::new();
        }
        let FieldDefType::SiblingType(name, _) = &self.field_type else {
            return Vec::new();
        };
        lookup_alias_info(name).map_or_else(Vec::new, |info| info.flatten_variants)
    }

    #[cfg(feature = "chrono")]
    fn has_as_number(&self) -> bool {
        self.model_schema_prop_meta
            .as_ref()
            .is_some_and(|m| m.as_number)
    }

    /// Whether `#[model_schema_prop(nullable)]` was written on this field — an `Option<T>` at
    /// object-key position rendering `T | null` with the key required, instead of the coercing
    /// default. Validated elsewhere to sit only on an `Option<T>` field, so a caller reading it
    /// under [`Self::is_optional`] never needs to ask twice.
    fn has_nullable(&self) -> bool {
        self.model_schema_prop_meta
            .as_ref()
            .is_some_and(|m| m.nullable)
    }

    /// Whether the field describes an array at all — the question every surface asked of the
    /// boolean this depth replaced. Asked only where a schema is generated.
    #[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
    pub const fn is_array(&self) -> bool {
        self.array_depth > 0
    }

    /// Whether the value sitting at array level `level` was written as an `Option`.
    ///
    /// Levels count from the innermost value outward: level 0 is what `field_type` names, level
    /// `array_depth` is the field as a whole. Below the outermost, a `None` reaches the wire as a
    /// `null` among the items of the array one level up — the array itself is always written.
    pub fn is_nullable_at(&self, level: u8) -> bool {
        self.nullable_levels.contains(&level)
    }

    /// Whether the field as a whole is an `Option` — the outermost level, and the only one whose
    /// `None` is not written inside an array. What it costs is the position's to say: an absent key
    /// in struct-field position, a `null` in a slot that cannot be dropped.
    pub fn is_optional(&self) -> bool {
        self.is_nullable_at(self.array_depth)
    }

    /// Whether every payload serde writes carries a key for this field — what the JSON surface's
    /// `required` names, and the one question there that a field's `Option`-ness alone cannot
    /// answer. A `nullable` `Option<T>` is the one case where both hold at once: the key is always
    /// written and `null` is its empty value.
    #[cfg(feature = "jsonschema")]
    pub fn key_is_required(&self) -> bool {
        if self.is_optional() {
            self.has_nullable()
        } else {
            !self.omits_value
        }
    }

    /// Whether the object key this field writes may be absent — `field?: T` admits the payload
    /// with no such key, `field: T | undefined` demands it.
    ///
    /// The key-dropping serde attribute is deliberately not read for an `Option`: the
    /// `Option`-null guard requires one of every named `Option` field, so reading it here would
    /// leave `ts_optional` inert on every field that carries it.
    fn key_may_be_absent(&self) -> bool {
        if self.is_optional() {
            self.model_schema_prop_meta
                .as_ref()
                .is_some_and(|m| m.ts_optional)
        } else {
            self.omits_value
        }
    }

    fn mark_fixed_length_at(&mut self, level: u8, length: usize) {
        if !self.array_lengths.iter().any(|&(at, _)| at == level) {
            self.array_lengths.push((level, length));
        }
    }

    fn mark_nullable_at(&mut self, level: u8) {
        if !self.nullable_levels.contains(&level) {
            self.nullable_levels.push(level);
        }
    }

    /// Whether this field's rendering reads any other item's own module-scope Zod binding — a
    /// factory call or a bare `$Schema` const — at any depth, wrapped or named directly. Deferral
    /// asks a wider question than the direct-sibling fold gate does:
    /// `z.array(Tagged$SchemaFactory(z.string()))` reads a sibling `const` exactly as much as a
    /// bare `Tagged$SchemaFactory(z.string())` does.
    #[cfg(feature = "zod")]
    pub fn names_a_sibling_binding(&self) -> bool {
        match &self.field_type {
            FieldDefType::SiblingType(name, generics) => {
                !is_sequence_wrapper(name) || generics.iter().any(Self::names_a_sibling_binding)
            }
            FieldDefType::Map(_, value) => value.names_a_sibling_binding(),
            FieldDefType::Tuple(elements) => elements.iter().any(Self::names_a_sibling_binding),
            FieldDefType::TypeParam(_)
            | FieldDefType::Unknown
            | FieldDefType::StringLiteral(_)
            | FieldDefType::BooleanLiteral(_)
            | FieldDefType::NumberLiteral(_)
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
            | FieldDefType::F64 => false,
            #[cfg(feature = "object_id")]
            FieldDefType::ObjectId => false,
            #[cfg(feature = "chrono")]
            FieldDefType::NaiveDate
            | FieldDefType::NaiveTime
            | FieldDefType::NaiveDateTime
            | FieldDefType::DateTime => false,
        }
    }

    /// The defs written inside this one, which is every position a type parameter can be reached
    /// at below the top: a `SiblingType`'s generic arguments, a `Map`'s key and value, a `Tuple`'s
    /// elements. Every other variant names a type outright and holds no def.
    ///
    /// Listed exhaustively and in one place, so a variant that grows a nested position has to be
    /// classified here rather than silently escaping every walk that reads a parameter.
    fn nested_type_positions(&mut self) -> Vec<&mut Self> {
        match &mut self.field_type {
            FieldDefType::SiblingType(_, generics) => generics.iter_mut().collect(),
            FieldDefType::Map(key, value) => vec![key, value],
            FieldDefType::Tuple(elements) => elements.iter_mut().collect(),
            // Leaf types hold no def.
            FieldDefType::TypeParam(_)
            | FieldDefType::Unknown
            | FieldDefType::StringLiteral(_)
            | FieldDefType::BooleanLiteral(_)
            | FieldDefType::NumberLiteral(_)
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
            | FieldDefType::F64 => Vec::new(),
            #[cfg(feature = "object_id")]
            FieldDefType::ObjectId => Vec::new(),
            #[cfg(feature = "chrono")]
            FieldDefType::NaiveDate
            | FieldDefType::NaiveTime
            | FieldDefType::NaiveDateTime
            | FieldDefType::DateTime => Vec::new(),
        }
    }

    /// The `?` a member written with an absent-able key carries, and nothing for one whose key is
    /// always written.
    pub fn optional_key_marker(&self) -> &'static str {
        if self.key_may_be_absent() { "?" } else { "" }
    }

    /// The name of the first `OsString`/`OsStr` this field reaches, at any depth.
    pub fn os_string_name(&self) -> Option<&str> {
        match &self.field_type {
            FieldDefType::SiblingType(name, generics) => {
                if matches!(name.as_str(), "OsString" | "OsStr") {
                    return Some(name);
                }
                generics.iter().find_map(Self::os_string_name)
            }
            FieldDefType::Map(key, value) => {
                key.os_string_name().or_else(|| value.os_string_name())
            }
            FieldDefType::Tuple(elements) => elements.iter().find_map(Self::os_string_name),
            FieldDefType::TypeParam(_)
            | FieldDefType::Unknown
            | FieldDefType::StringLiteral(_)
            | FieldDefType::BooleanLiteral(_)
            | FieldDefType::NumberLiteral(_)
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

    /// The parameter this field is or reaches, which is what a nested position is asked for: below
    /// the top level the two questions have one answer.
    #[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
    fn parameter_or_argument_name(&self) -> Option<&str> {
        self.parameter_shape_name()
            .or_else(|| self.argument_parameter_name())
    }

    /// The enclosing item's own type parameter this field renders as, when a bound spelled against
    /// it names a value whose type the expansion never sees.
    pub fn parameter_shape_name(&self) -> Option<&str> {
        match &self.field_type {
            FieldDefType::TypeParam(name) => Some(name),
            FieldDefType::SiblingType(_, _)
            | FieldDefType::Map(_, _)
            | FieldDefType::Tuple(_)
            | FieldDefType::Unknown
            | FieldDefType::StringLiteral(_)
            | FieldDefType::BooleanLiteral(_)
            | FieldDefType::NumberLiteral(_)
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

    /// Whether this field reaches a type the registry does not hold yet — a `#[model_schema]` item
    /// declared below the one being expanded, whether or not that item declares a parameter of its
    /// own.
    #[cfg(feature = "zod")]
    pub fn reaches_a_type_declared_later(&self) -> bool {
        match &self.field_type {
            FieldDefType::SiblingType(name, generics) => {
                (!is_sequence_wrapper(name) && lookup_alias_info(name).is_none())
                    || generics.iter().any(Self::reaches_a_type_declared_later)
            }
            FieldDefType::Map(key, value) => {
                key.reaches_a_type_declared_later() || value.reaches_a_type_declared_later()
            }
            FieldDefType::Tuple(elements) => {
                elements.iter().any(Self::reaches_a_type_declared_later)
            }
            FieldDefType::TypeParam(_)
            | FieldDefType::Unknown
            | FieldDefType::StringLiteral(_)
            | FieldDefType::BooleanLiteral(_)
            | FieldDefType::NumberLiteral(_)
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
            | FieldDefType::F64 => false,
            #[cfg(feature = "object_id")]
            FieldDefType::ObjectId => false,
            #[cfg(feature = "chrono")]
            FieldDefType::NaiveDate
            | FieldDefType::NaiveTime
            | FieldDefType::NaiveDateTime
            | FieldDefType::DateTime => false,
        }
    }

    /// Rewrites any `Self` type reference to the concrete enclosing type name, at the parameters
    /// the enclosing item declares.
    pub fn resolve_self_references(&mut self, type_name: &str, parameters: &[String]) {
        match &mut self.field_type {
            FieldDefType::SiblingType(name, generics) => {
                if name == "Self" {
                    type_name.clone_into(name);
                    generics.extend(parameters.iter().map(|parameter| Self {
                        name: parameter.clone(),
                        field_type: FieldDefType::TypeParam(parameter.clone()),
                        array_depth: 0,
                        array_lengths: Vec::new(),
                        docs: String::new(),
                        model_schema_prop_meta: None,
                        nullable_levels: Vec::new(),
                        absent_from_wire: false,
                        omits_value: false,
                        #[cfg(feature = "jsonschema")]
                        type_span: Span::call_site(),
                    }));
                }
                for generic in generics.iter_mut() {
                    generic.resolve_self_references(type_name, parameters);
                }
            }
            FieldDefType::Map(key, value) => {
                key.resolve_self_references(type_name, parameters);
                value.resolve_self_references(type_name, parameters);
            }
            FieldDefType::Tuple(elements) => {
                for element in elements.iter_mut() {
                    element.resolve_self_references(type_name, parameters);
                }
            }
            // Leaf types cannot contain nested references.
            FieldDefType::TypeParam(_)
            | FieldDefType::Unknown
            | FieldDefType::StringLiteral(_)
            | FieldDefType::BooleanLiteral(_)
            | FieldDefType::NumberLiteral(_)
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
            | FieldDefType::F64 => {}
            #[cfg(feature = "object_id")]
            FieldDefType::ObjectId => {}
            #[cfg(feature = "chrono")]
            FieldDefType::NaiveDate
            | FieldDefType::NaiveTime
            | FieldDefType::NaiveDateTime
            | FieldDefType::DateTime => {}
        }
    }

    /// The members of the untagged union this field names as an object flattening it spells them,
    /// as the registry recorded them — and nothing for a field that names no such union, and for one
    /// whose members carry no exclusion the union's own name does not already describe.
    #[cfg(all(feature = "serde", feature = "typescript"))]
    pub fn ts_union_members(&self) -> Vec<String> {
        let wrapped = self
            .model_schema_prop_meta
            .as_ref()
            .is_some_and(|meta| !meta.preprocess.is_empty());
        if self.array_depth > 0 || wrapped {
            return Vec::new();
        }
        let FieldDefType::SiblingType(name, _) = &self.field_type else {
            return Vec::new();
        };
        lookup_alias_info(name).map_or_else(Vec::new, |info| info.ts_union_members)
    }

    /// Builds the TypeScript type before the outermost optional wrap: the type match plus one
    /// `Array<…>` per array level, each carrying the `| null` of the level it wraps. The outermost
    /// level's wrap lives in `typescript_typename` and `typescript_slot_typename`, which is where
    /// the position it sits in decides between `| undefined` and `| null`.
    fn typescript_base(&self) -> String {
        let result = match &self.field_type {
            FieldDefType::Unknown => "unknown".to_owned(),
            // The declaration binds this name, so the field is written under it.
            FieldDefType::TypeParam(name) => name.clone(),
            FieldDefType::Tuple(lst) => {
                let elements = lst
                    .iter()
                    .map(Self::typescript_slot_typename)
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("[{elements}]")
            }
            FieldDefType::SiblingType(name, lst) => {
                if let [element] = lst.as_slice()
                    && is_sequence_wrapper(name)
                {
                    // The element re-enters the whole per-type rendering as the arrayed field it
                    // stands for, so a set renders exactly as the `Vec` of that element does. It
                    // carries this field's own array levels with it, so the wrap below is its to
                    // apply and not this pass's.
                    return self.collection_element_field(element).typescript_base();
                } else if let Some(info) = lookup_alias_info(name) {
                    if lst.is_empty() {
                        info.export_name
                    } else {
                        format!(
                            "{}<{}>",
                            info.export_name,
                            lst.iter()
                                .map(Self::typescript_typename)
                                .collect::<Vec<_>>()
                                .join(", ")
                        )
                    }
                } else if lst.is_empty() {
                    name.clone()
                } else {
                    format!(
                        "{name}<{}>",
                        lst.iter()
                            .map(Self::typescript_typename)
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                }
            }
            FieldDefType::Map(k, v) => {
                format!(
                    "Partial<Record<{}, {}>>",
                    k.typescript_map_key_typename(),
                    v.typescript_slot_typename()
                )
            }
            FieldDefType::Boolean => "boolean".to_owned(),
            FieldDefType::Char | FieldDefType::String => "string".to_owned(),
            FieldDefType::StringLiteral(literal) => format!("\"{literal}\""),
            FieldDefType::BooleanLiteral(value) => value.to_string(),
            FieldDefType::NumberLiteral(value) => format_number_literal(*value),
            FieldDefType::U8
            | FieldDefType::U16
            | FieldDefType::U32
            | FieldDefType::U64
            | FieldDefType::I8
            | FieldDefType::I16
            | FieldDefType::I32
            | FieldDefType::I64
            | FieldDefType::Usize
            | FieldDefType::Isize => "number".to_owned(),
            FieldDefType::F32 | FieldDefType::F64 => "number".to_owned(),
            #[cfg(feature = "object_id")]
            FieldDefType::ObjectId => object_id::get_object_id_typescript_type(),
            #[cfg(feature = "chrono")]
            FieldDefType::NaiveDate => chrono::get_naive_date_typescript_type(),
            #[cfg(feature = "chrono")]
            FieldDefType::NaiveTime => chrono::get_naive_time_typescript_type(),
            #[cfg(feature = "chrono")]
            FieldDefType::NaiveDateTime => chrono::get_naive_datetime_typescript_type(),
            #[cfg(feature = "chrono")]
            FieldDefType::DateTime => {
                if self.has_as_number() {
                    chrono::get_datetime_number_typescript_type()
                } else {
                    chrono::get_datetime_typescript_type()
                }
            }
        };
        (0..self.array_depth).fold(result, |wrapped, level| {
            let item = if self.is_nullable_at(level) {
                format!("{wrapped} | null")
            } else {
                wrapped
            };
            format!("Array<{item}>")
        })
    }

    /// The key a `Partial<Record<…>>` is written with: the key's own type, except where the key is
    /// one of the enclosing item's type parameters, which states `string` — the same answer
    /// [`Self::zod_map_key_type`] gives, for the same reason.
    fn typescript_map_key_typename(&self) -> String {
        if self.parameter_shape_name().is_some() {
            return "string".to_owned();
        }
        self.typescript_typename()
    }

    /// What this field contributes to an object that writes its members beside its own, on the
    /// TypeScript surface: the value itself, with no answer for the outermost `Option`.
    #[cfg(feature = "typescript")]
    pub fn typescript_merged_typename(&self) -> String {
        self.typescript_base()
    }

    /// The TypeScript type for a value in a slot that cannot be dropped — a tuple element, a map
    /// entry, or the content key of a single-element tuple variant, which serde always writes. An
    /// `Option` there is null-flavored (`{base} | null`) rather than undefined-flavored: none of
    /// those positions can be omitted the way an object key can, so serde emits `null` for a
    /// `None` in each of them.
    pub fn typescript_slot_typename(&self) -> String {
        let base = self.typescript_base();
        if self.is_optional() {
            format!("{base} | null")
        } else {
            base
        }
    }

    /// Generates the TypeScript type name for this field.
    pub fn typescript_typename(&self) -> String {
        let pre_result = self.typescript_base();
        if self.is_optional() {
            if self.has_nullable() {
                format!("{pre_result} | null")
            } else if self.key_may_be_absent() {
                // An absent-able key renders as `field?: T`, so the `| undefined` is redundant —
                // and under `exactOptionalPropertyTypes` it would claim an explicit `undefined`
                // the key's omission is exactly what serde writes instead.
                pre_result
            } else {
                format!("{pre_result} | undefined")
            }
        } else {
            pre_result
        }
    }

    /// The value this field's wrappers hold, as a field of its own: the same type with the
    /// `Option`s and the array levels dropped.
    ///
    /// The wrappers are the field's to declare and the value under them is what a type name stands
    /// for, so this is what an `as = Type` is compared against when the target names a bare type —
    /// the spelling `as = String` on a `Vec<String>` uses.
    pub fn value_under_wrappers(&self) -> Self {
        let mut value = self.clone();
        value.array_depth = 0;
        value.array_lengths.clear();
        value.nullable_levels.clear();
        value
    }

    /// The type match plus one `z.array(…)` per array level, each carrying the `z.nullable(…)` of
    /// the level it wraps and the `.length(N)` of a level written as a fixed-size `[T; N]`, before
    /// the preprocess wrap.
    #[cfg(feature = "zod")]
    fn zod_array_base(&self) -> String {
        let result = match &self.field_type {
            FieldDefType::Unknown => "z.unknown()".to_owned(),
            // A `const` cannot be parameterised, so every generic publisher writes a factory and a
            // parameter composes the argument that factory binds for it — see
            // [`zod_factory_argument`].
            FieldDefType::TypeParam(name) => zod_factory_argument(name),
            FieldDefType::Tuple(lst) => {
                let elements = lst
                    .iter()
                    .map(Self::zod_slot_type)
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("z.tuple([{elements}])")
            }
            FieldDefType::SiblingType(name, lst) => {
                if let [element] = lst.as_slice()
                    && is_sequence_wrapper(name)
                {
                    // The element carries this field's own array levels, so it applies the wrap.
                    return self.collection_element_field(element).zod_array_base();
                } else if let Some(info) = lookup_alias_info(name) {
                    // What the named type published is what this can name: a factory where the
                    // type declares parameters, and the one schema it has where it declares none.
                    // Read off the registry rather than off the arguments written here, because a
                    // name carrying arguments says nothing about which of the two it published.
                    if publishes_zod_factory(name) {
                        zod_factory_call(&info.export_name, lst)
                    } else {
                        format!("{}$Schema", info.export_name)
                    }
                } else if lst.is_empty() {
                    format!("{name}$Schema")
                } else {
                    // A name the registry does not hold yet, written with arguments, is a generic
                    // type expanded after this one: only a factory can take them.
                    zod_factory_call(name, lst)
                }
            }
            FieldDefType::Map(k, v) => {
                format!("z.record({}, {})", k.zod_map_key_type(), v.zod_slot_type())
            }
            FieldDefType::Boolean => "z.boolean()".to_owned(),
            // serde writes a `char` as a one-character string and reads only that back, so the
            // length is fixed rather than read from `model_schema_prop` — a `char` field carries
            // none of those constraints.
            FieldDefType::Char => "z.string().length(1)".to_owned(),
            FieldDefType::String => self.zod_string_type(),
            FieldDefType::StringLiteral(literal) => format!("z.literal(\"{literal}\")"),
            FieldDefType::BooleanLiteral(value) => format!("z.literal({value})"),
            FieldDefType::NumberLiteral(value) => {
                format!("z.literal({})", format_number_literal(*value))
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
            | FieldDefType::Isize => self.zod_number_type("z.number().int()"),
            FieldDefType::F32 | FieldDefType::F64 => self.zod_number_type("z.number()"),
            #[cfg(feature = "object_id")]
            FieldDefType::ObjectId => object_id::get_object_id_zod_schema(),
            #[cfg(feature = "chrono")]
            FieldDefType::NaiveDate => chrono::get_naive_date_zod_schema(),
            #[cfg(feature = "chrono")]
            FieldDefType::NaiveTime => chrono::get_naive_time_zod_schema(),
            #[cfg(feature = "chrono")]
            FieldDefType::NaiveDateTime => chrono::get_naive_datetime_zod_schema(),
            #[cfg(feature = "chrono")]
            FieldDefType::DateTime => {
                if self.has_as_number() {
                    chrono::get_datetime_number_zod_schema()
                } else {
                    chrono::get_datetime_native_zod_schema()
                }
            }
        };
        (0..self.array_depth).fold(result, |wrapped, level| {
            let item = if self.is_nullable_at(level) {
                format!("z.nullable({wrapped})")
            } else {
                wrapped
            };
            let bound = self
                .fixed_length_at(level)
                .map_or_else(String::new, |length| format!(".length({length})"));
            format!("z.array({item}){bound}")
        })
    }

    /// Builds the Zod schema before the struct-field optional wrap: the type match, the
    /// array wraps, and the preprocess wrap. The
    /// `z.union([…, z.undefined()]).prefault(undefined)` wrap lives in `zod_type`.
    #[cfg(feature = "zod")]
    fn zod_base(&self) -> String {
        self.zod_preprocess_wrap(self.zod_array_base())
    }

    /// The key schema a `z.record(…)` is written with: the key's own, except where the key is one
    /// of the enclosing item's type parameters.
    #[cfg(feature = "zod")]
    fn zod_map_key_type(&self) -> String {
        if self.parameter_shape_name().is_some() {
            return "z.string()".to_owned();
        }
        self.zod_type()
    }

    /// The same value on the Zod surface, for the same reason: what a merged source validates, with
    /// the outermost `Option` left to whatever assembles the merge. See
    /// [`Self::typescript_merged_typename`].
    #[cfg(feature = "zod")]
    pub fn zod_merged_schema(&self) -> String {
        self.zod_base()
    }

    /// Builds the Zod schema string for a numeric field, applying any min/max constraints.
    #[cfg(feature = "zod")]
    fn zod_number_type(&self, base: &str) -> String {
        let mut result = base.to_owned();
        if let Some(meta) = &self.model_schema_prop_meta {
            if let Some(min) = meta.minimum {
                result = format!("{result}.min({min})");
            }
            if let Some(max) = meta.maximum {
                result = format!("{result}.max({max})");
            }
        }
        result
    }

    /// Wraps `schema` in one `z.preprocess(fn, …)` per function named by `preprocess`, innermost
    /// function first, or returns it unwrapped where none was written. Held apart from
    /// [`Self::zod_base`] so [`Self::zod_type`] can also wrap the whole `nullable` union rather
    /// than only the type match beneath it — coerce, then validate.
    #[cfg(feature = "zod")]
    fn zod_preprocess_wrap(&self, schema: String) -> String {
        let Some(meta) = &self.model_schema_prop_meta else {
            return schema;
        };
        meta.preprocess
            .iter()
            .rev()
            .fold(schema, |wrapped, fn_name| {
                format!("z.preprocess({fn_name}, {wrapped})")
            })
    }

    /// The Zod schema for a value in a slot that cannot be dropped — a tuple element, a map entry,
    /// or the content key of a single-element tuple variant, which serde always writes. An
    /// `Option` there is null-flavored (`z.nullable({base})`) rather than undefined-flavored: none
    /// of those positions can be omitted the way an object key can, so serde emits `null` for a
    /// `None` in each of them.
    #[cfg(feature = "zod")]
    pub fn zod_slot_type(&self) -> String {
        let base = self.zod_base();
        if self.is_optional() {
            format!("z.nullable({base})")
        } else {
            base
        }
    }

    /// Builds the Zod schema string for a string field, applying any length/pattern constraints.
    #[cfg(feature = "zod")]
    fn zod_string_type(&self) -> String {
        let mut result = "z.string()".to_owned();
        if let Some(meta) = &self.model_schema_prop_meta
            && let Some(min_len) = meta.min_length
        {
            result = format!("{result}.min({min_len})");
        }
        if let Some(meta) = &self.model_schema_prop_meta
            && let Some(max_len) = meta.max_length
        {
            result = format!("{result}.max({max_len})");
        }
        if let Some(meta) = &self.model_schema_prop_meta
            && let Some(pattern) = &meta.pattern
        {
            let literal_body = escape_js_regex_literal(pattern);
            result = format!("{result}.check(z.regex(/{literal_body}/))");
        }
        result
    }

    #[cfg(feature = "zod")]
    /// Generates the Zod schema string for this field (requires "zod" feature).
    pub fn zod_type(&self) -> String {
        if self.is_optional() {
            if self.has_nullable() {
                let union = format!("z.union([{}, z.null()])", self.zod_array_base());
                self.zod_preprocess_wrap(union)
            } else {
                let pre_result = self.zod_base();
                format!(
                    "z.union([z.null().transform(() => undefined), {pre_result}, z.undefined()]).prefault(undefined)"
                )
            }
        } else {
            let pre_result = self.zod_base();
            if self.key_may_be_absent() {
                format!("{pre_result}.optional()")
            } else {
                pre_result
            }
        }
    }

    /// The members of the untagged union this field names, as the registry recorded them, and
    /// nothing for a field that names no such union.
    #[cfg(feature = "zod")]
    pub fn zod_union_members(&self) -> Vec<ZodUnionMember> {
        let wrapped = self
            .model_schema_prop_meta
            .as_ref()
            .is_some_and(|meta| !meta.preprocess.is_empty());
        if self.array_depth > 0 || wrapped {
            return Vec::new();
        }
        let FieldDefType::SiblingType(name, _) = &self.field_type else {
            return Vec::new();
        };
        lookup_alias_info(name).map_or_else(Vec::new, |info| info.zod_union_members)
    }
}

/// The `f64` a `literal = N` was written with, formatted the way TypeScript and Zod read a numeric
/// literal type: a whole value renders without the trailing `.0` `f64`'s own `Display` carries.
pub fn format_number_literal(value: f64) -> String {
    if value.is_finite() && value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        value.to_string()
    }
}

/// A reference to a type that publishes a factory, as the call it has to be.
///
/// Each argument is rendered by the renderer that renders the reference itself, so an argument that
/// is a forwarded parameter, a primitive, a date, or another generic reference all reach the call
/// the same way — and one that is itself generic composes at whatever depth it was written at.
#[cfg(feature = "zod")]
fn zod_factory_call(name: &str, arguments: &[FieldDef]) -> String {
    format!(
        "{name}$SchemaFactory({})",
        arguments
            .iter()
            .map(FieldDef::zod_type)
            .collect::<Vec<_>>()
            .join(", ")
    )
}

/// The one list of std wrappers the crate renders as arrays, shared by every surface.
pub fn is_sequence_wrapper(name: &str) -> bool {
    matches!(
        name,
        "BTreeSet" | "BinaryHeap" | "HashSet" | "LinkedList" | "Vec" | "VecDeque"
    )
}

/// The number of leading type arguments a std container's wire form is written from, or `None` for
/// a name that is not one of them.
fn container_wire_arity(name: &str) -> Option<usize> {
    if name == "HashMap" || name == "BTreeMap" {
        Some(2)
    } else if is_sequence_wrapper(name) {
        Some(1)
    } else {
        None
    }
}

/// The one list of std wrappers the crate reads straight through to what they hold.
pub fn is_transparent_wrapper(name: &str) -> bool {
    matches!(name, "Arc" | "Box" | "Cow" | "Rc")
}

/// Classifies a `syn::Variant` into its `VariantKind`.
pub fn classify_variant(variant: &Variant) -> VariantKind {
    match &variant.fields {
        Fields::Unit => VariantKind::Unit,
        Fields::Named(_) => VariantKind::Named,
        Fields::Unnamed(fields) => {
            if fields.unnamed.is_empty() {
                // Empty tuple like `Foo()` - treat as unit
                VariantKind::Unit
            } else if fields.unnamed.len() == 1 {
                VariantKind::TupleSingle
            } else {
                VariantKind::TupleMultiple
            }
        }
    }
}

/// Main function to create `FieldDef` from `syn::Type`.
fn get_field_def_from_type_path(
    type_path: &syn::TypePath,
    safe_name: String,
    field_docs: &str,
) -> FieldDef {
    let Some(segment) = type_path.path.segments.last() else {
        return FieldDef {
            name: safe_name,
            field_type: FieldDefType::Unknown,
            array_depth: 0,
            array_lengths: Vec::new(),
            docs: field_docs.to_owned(),
            model_schema_prop_meta: None,
            nullable_levels: Vec::new(),
            absent_from_wire: false,
            omits_value: false,
            #[cfg(feature = "jsonschema")]
            type_span: type_path.span(),
        };
    };
    let ident = segment.ident.to_string();
    match &segment.arguments {
        PathArguments::None => FieldDef {
            name: safe_name,
            field_type: get_field_def_type_or_sibling(&ident),
            array_depth: 0,
            array_lengths: Vec::new(),
            docs: field_docs.to_owned(),
            model_schema_prop_meta: None,
            nullable_levels: Vec::new(),
            absent_from_wire: false,
            omits_value: false,
            // The name segment, not the whole path: it is what a generated module is named after,
            // so a reference the module cannot resolve is blamed on the name it was built from.
            #[cfg(feature = "jsonschema")]
            type_span: segment.ident.span(),
        },
        PathArguments::AngleBracketed(args) => {
            get_field_def_from_generic_type(&segment.ident, args, safe_name, field_docs)
        }
        // Function pointer types are unsupported; fall back to `unknown`.
        PathArguments::Parenthesized(_) => FieldDef {
            name: safe_name,
            field_type: FieldDefType::Unknown,
            array_depth: 0,
            array_lengths: Vec::new(),
            docs: field_docs.to_owned(),
            model_schema_prop_meta: None,
            nullable_levels: Vec::new(),
            absent_from_wire: false,
            omits_value: false,
            #[cfg(feature = "jsonschema")]
            type_span: segment.ident.span(),
        },
    }
}

/// Builds a `FieldDef` for a named type written with generic arguments.
fn get_field_def_from_generic_type(
    type_ident: &Ident,
    args: &syn::AngleBracketedGenericArguments,
    safe_name: String,
    field_docs: &str,
) -> FieldDef {
    let ident_name = type_ident.to_string();
    let ident = ident_name.as_str();
    let mut arg_types: Vec<FieldDef> = args
        .args
        .iter()
        .filter_map(|arg| {
            if let GenericArgument::Type(inner_ty) = arg {
                Some(get_field_def("", inner_ty, ""))
            } else {
                None
            }
        })
        .collect();
    // A container is claimed by its own name, so what it is written with past its wire form is
    // dropped before the arms below count arguments.
    if let Some(arity) = container_wire_arity(ident)
        && arg_types.len() > arity
    {
        arg_types.truncate(arity);
    }
    if arg_types.is_empty() {
        FieldDef {
            name: safe_name,
            field_type: FieldDefType::SiblingType(ident.to_owned(), vec![]),
            array_depth: 0,
            array_lengths: Vec::new(),
            docs: field_docs.to_owned(),
            model_schema_prop_meta: None,
            nullable_levels: Vec::new(),
            absent_from_wire: false,
            omits_value: false,
            #[cfg(feature = "jsonschema")]
            type_span: type_ident.span(),
        }
    } else if let [element] = arg_types.as_slice()
        && let Some(collapsed) = collapsed_wrapper_def(ident, element, &safe_name, field_docs)
    {
        collapsed
    } else if arg_types.len() == 2 && (ident == "HashMap" || ident == "BTreeMap") {
        log::trace!(
            "Creating HashMap Map type - key: {:?}, value: {:?}",
            arg_types[0],
            arg_types[1]
        );
        FieldDef {
            array_depth: 0,
            array_lengths: Vec::new(),
            name: safe_name,
            field_type: FieldDefType::Map(
                Box::new(arg_types[0].clone()),
                Box::new(arg_types[1].clone()),
            ),
            docs: field_docs.to_owned(),
            model_schema_prop_meta: None,
            nullable_levels: Vec::new(),
            absent_from_wire: false,
            omits_value: false,
            #[cfg(feature = "jsonschema")]
            type_span: type_ident.span(),
        }
    } else if arg_types.len() == 1 && is_datetime_generic_type(ident) {
        // The timezone type parameter says nothing about what is written.
        FieldDef {
            name: safe_name,
            field_type: datetime_field_type(),
            array_depth: 0,
            array_lengths: Vec::new(),
            docs: field_docs.to_owned(),
            model_schema_prop_meta: None,
            nullable_levels: Vec::new(),
            absent_from_wire: false,
            omits_value: false,
            #[cfg(feature = "jsonschema")]
            type_span: type_ident.span(),
        }
    } else {
        log::trace!("Creating SiblingType - name: {ident}, arg_types: {arg_types:?}");
        FieldDef {
            name: safe_name,
            field_type: FieldDefType::SiblingType(ident.to_owned(), arg_types),
            array_depth: 0,
            array_lengths: Vec::new(),
            docs: field_docs.to_owned(),
            model_schema_prop_meta: None,
            nullable_levels: Vec::new(),
            absent_from_wire: false,
            omits_value: false,
            #[cfg(feature = "jsonschema")]
            type_span: type_ident.span(),
        }
    }
}

/// The def a single-argument wrapper collapses onto, for the wrappers that collapse.
fn collapsed_wrapper_def(
    ident: &str,
    element: &FieldDef,
    safe_name: &str,
    field_docs: &str,
) -> Option<FieldDef> {
    let mut result = element.clone();
    match ident {
        "Option" => result.mark_nullable_at(result.array_depth),
        "Vec" => result.array_depth = result.array_depth.saturating_add(1),
        _ if is_transparent_wrapper(ident) => {}
        _ => return None,
    }
    safe_name.clone_into(&mut result.name);
    field_docs.clone_into(&mut result.docs);
    Some(result)
}

/// The element count a fixed-size array was written with, when the expansion can read it.
///
/// A literal is the whole of that. A const generic parameter, a `const` item and any computed
/// length each name a value only the compiler has, and the macro runs before there is one to ask
/// for; each therefore describes as the unbounded array every other sequence spelling describes as,
/// which is the honest answer when the count is unknown.
fn literal_array_length(len: &syn::Expr) -> Option<usize> {
    let syn::Expr::Lit(syn::ExprLit {
        lit: syn::Lit::Int(literal),
        ..
    }) = len
    else {
        return None;
    };
    literal.base10_parse::<usize>().ok()
}

/// Debug logging: Set `RUST_LOG=trace` to see HashMap/SiblingType creation.
pub fn get_field_def(name: &str, ty: &Type, field_docs: &str) -> FieldDef {
    let safe_name = safe_type_name(name);
    let written = written_type(ty);
    if let Type::Path(type_path) = written {
        get_field_def_from_type_path(type_path, safe_name, field_docs)
    } else if let Type::Reference(type_ref) = written {
        // let lifetime = type_ref
        //     .lifetime
        //     .as_ref()
        //     .map_or("".to_string(), |l| format!("'{}", l.ident));
        get_field_def(name, type_ref.elem.as_ref(), field_docs)
    } else if let Type::Array(type_array) = written {
        let mut def = get_field_def(name, &type_array.elem, field_docs);
        // The array this spelling adds is the level the element's own depth counts up to, and the
        // length is that level's — not the field's, which may sit under further wrappers.
        let level = def.array_depth;
        def.array_depth = def.array_depth.saturating_add(1);
        if let Some(length) = literal_array_length(&type_array.len) {
            def.mark_fixed_length_at(level, length);
        }
        def
    } else if let Type::Slice(type_slice) = written {
        let mut def = get_field_def(name, &type_slice.elem, field_docs);
        def.array_depth = def.array_depth.saturating_add(1);
        def
    } else if let Type::Tuple(type_tuple) = written {
        let elements: Vec<FieldDef> = type_tuple
            .elems
            .iter()
            .enumerate()
            .map(|(idx, v)| get_field_def(&format!("element_{idx}"), v, field_docs))
            .collect();
        FieldDef {
            name: safe_name,
            field_type: FieldDefType::Tuple(elements),
            array_depth: 0,
            array_lengths: Vec::new(),
            docs: field_docs.to_owned(),
            model_schema_prop_meta: None,
            nullable_levels: Vec::new(),
            absent_from_wire: false,
            omits_value: false,
            #[cfg(feature = "jsonschema")]
            type_span: written.span(),
        }
    } else {
        // Fallback for BareFn, ImplTrait, etc.
        FieldDef {
            name: safe_name,
            field_type: FieldDefType::Unknown,
            array_depth: 0,
            array_lengths: Vec::new(),
            docs: field_docs.to_owned(),
            model_schema_prop_meta: None,
            nullable_levels: Vec::new(),
            absent_from_wire: false,
            omits_value: false,
            #[cfg(feature = "jsonschema")]
            type_span: written.span(),
        }
    }
}

/// Helper to map Rust type name strings to `FieldDefType`.
fn get_field_def_type_or_sibling(t_name: &str) -> FieldDefType {
    if lookup_alias_info(t_name).is_some() {
        return FieldDefType::SiblingType(t_name.to_owned(), vec![]);
    }
    match t_name {
        "bool" => FieldDefType::Boolean,
        "char" => FieldDefType::Char,
        // `str` and `Path` are the borrowed forms of `String` and `PathBuf`, and each writes the
        // same JSON string its owned form does. Both are reachable only behind a wrapper or a
        // reference, and the parser reads through either to land here. `OsString`/`OsStr` are
        // deliberately absent: serde writes them as an externally tagged enum, not a string, so
        // they fall through to `SiblingType` and are rejected by `os_string_name`.
        "String" | "PathBuf" | "str" | "Path" => FieldDefType::String,
        "Value" => FieldDefType::Unknown,
        "u8" => FieldDefType::U8,
        "u16" => FieldDefType::U16,
        "u32" => FieldDefType::U32,
        "u64" => FieldDefType::U64,
        "i8" => FieldDefType::I8,
        "i16" => FieldDefType::I16,
        "i32" => FieldDefType::I32,
        "i64" => FieldDefType::I64,
        "usize" => FieldDefType::Usize,
        "isize" => FieldDefType::Isize,
        "f32" => FieldDefType::F32,
        "f64" => FieldDefType::F64,
        #[cfg(feature = "object_id")]
        "ObjectId" => {
            if object_id::should_handle_as_object_id(t_name) {
                FieldDefType::ObjectId
            } else {
                FieldDefType::SiblingType(t_name.to_owned(), vec![])
            }
        }
        #[cfg(not(feature = "object_id"))]
        "ObjectId" => {
            // When object_id feature is disabled, warn user and treat as regular type
            eprintln!("warning: ObjectId type detected but 'object_id' feature is not enabled");
            eprintln!(
                "         ObjectId will be treated as a custom type (may cause compilation errors)"
            );
            eprintln!("         Enable the object_id feature: features = [\"object_id\"]");
            eprintln!("         Or add the required ObjectId type definition to your code");
            FieldDefType::SiblingType(t_name.to_owned(), vec![])
        }
        #[cfg(feature = "chrono")]
        "NaiveDate" => FieldDefType::NaiveDate,
        #[cfg(feature = "chrono")]
        "NaiveTime" => FieldDefType::NaiveTime,
        #[cfg(feature = "chrono")]
        "NaiveDateTime" => FieldDefType::NaiveDateTime,
        type_name_json if type_name_json.ends_with("Json") => {
            FieldDefType::SiblingType(safe_type_name(type_name_json), vec![])
        }
        type_name => FieldDefType::SiblingType(type_name.to_owned(), vec![]),
    }
}

/// Whether a name is one the language reserves for a primitive type that
/// [`get_field_def_type_or_sibling`] has no arm for.
#[cfg(feature = "jsonschema")]
pub fn is_undescribable_primitive(name: &str) -> bool {
    matches!(name, "i128" | "u128" | "f16" | "f128")
}

/// Parses serde attributes from struct/enum attributes (requires "serde" feature).
///
/// Delegates to features/serde.rs for actual parsing.
/// Used in `model_schema.rs` to get type-level serde metadata like `rename_all`.
///
/// Without "serde" feature: No attribute processing, uses Rust names as-is.
#[cfg(feature = "serde")]
pub fn parse_serde_type_attributes(attrs: &[Attribute]) -> SerdeTypeMeta {
    parse_serde_type_attributes_impl(attrs)
}

/// Parses serde attributes from field attributes (requires "serde" feature).
///
/// Similar to `parse_serde_type_attributes` but for individual fields.
/// Handles field rename, `skip_serializing_if`, etc.
///
/// Integrates with FieldDef.name if rename present.
#[cfg(feature = "serde")]
pub fn parse_serde_field_attributes(attrs: &[Attribute]) -> SerdeFieldMeta {
    parse_serde_field_attributes_impl(attrs)
}

/// Utility to check if an enum is a plain unit enum (no fields in variants).
pub fn is_plain_enum(item_enum: &ItemEnum) -> bool {
    item_enum
        .variants
        .iter()
        .all(|variant| matches!(variant.fields, Fields::Unit))
}

/// Check if a type name is a `DateTime` generic type (chrono feature).
/// Returns true only when chrono feature is enabled and type is `DateTime`.
#[cfg(feature = "chrono")]
fn is_datetime_generic_type(type_name: &str) -> bool {
    type_name == "DateTime"
}

#[cfg(not(feature = "chrono"))]
const fn is_datetime_generic_type(_type_name: &str) -> bool {
    false
}

/// What a `DateTime<Tz>` field carries: the chrono type, the timezone parameter saying nothing
/// about what is written.
#[cfg(feature = "chrono")]
const fn datetime_field_type() -> FieldDefType {
    FieldDefType::DateTime
}

/// Unreachable without chrono — `is_datetime_generic_type` answers `false` there — and named as
/// any other unknown type would be.
#[cfg(not(feature = "chrono"))]
fn datetime_field_type() -> FieldDefType {
    FieldDefType::SiblingType("DateTime".to_owned(), vec![])
}

#[cfg(test)]
mod tests;

use syn::{Fields, GenericArgument, Ident, ItemEnum, PathArguments, Type, Variant};

#[cfg(feature = "jsonschema")]
use proc_macro2::Span;
#[cfg(feature = "jsonschema")]
use syn::spanned::Spanned as _;

#[cfg(feature = "serde")]
use syn::Attribute;

use crate::features::model_schema_prop::ModelSchemaPropMeta;
use crate::utils::{lookup_alias_info, safe_type_name};

#[cfg(feature = "zod")]
use crate::utils::{ZodUnionMember, escape_js_regex_literal, zod_factory_argument};

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
///
/// This enum is central to tixschema's type mapping system. It categorizes Rust types into
/// categories that can be translated to TypeScript types and Zod schemas. The variants cover
/// primitive types, complex structures, and special cases like `ObjectId` (feature-dependent).
///
/// Key points from crate documentation:
/// - Primitives map to TS equivalents (e.g., String -> string, numbers -> number)
/// - Collections like Vec<T> become Array<T> (handled via `FieldDef`'s `array_depth` count)
/// - `HashMap`<String, T> becomes Partial<Record<string, T>> (Map variant)
/// - Enums and nested structs use `SiblingType`
/// - All types must follow naming conventions (e.g., end with Json suffix for structs)
///
/// Feature dependencies:
/// - "`object_id"`: Enables `ObjectId` variant for `MongoDB` support
/// - Without features, falls back to basic mappings
///
/// Equality is the question `as = Type` asks: do two written types describe the same value on every
/// surface? The variants a spelling collapses onto answer it — `Path` reaching `String` is the same
/// type here as `PathBuf` is — and the nested defs answer it through [`FieldDef`]'s own `PartialEq`.
#[derive(Clone, Debug, PartialEq)]
pub enum FieldDefType {
    /// Boolean primitive - maps to boolean.
    Boolean,
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
    ///
    /// Held apart from [`FieldDefType::SiblingType`] because the three surfaces answer for it
    /// differently, and only one of them can name it: TypeScript binds the parameter for real, so
    /// it renders as the name it was written with, while the two validating surfaces render the
    /// opaque value. [`FieldDef::erase_type_parameters`] is where that rule is written down and
    /// where a written name becomes this.
    TypeParam(String),
    U16,
    U32,
    U64,
    U8,
    /// Unknown or unsupported type - generates 'unknown' in TS/Zod.
    Unknown,
    Usize,
}

/// Struct representing a field's definition for schema generation.
///
/// This is the core data structure used to analyze and generate schemas for each field
/// in a struct or enum variant. It's created by `get_field_def()` and used in
/// `model_schema.rs` to build the full type definitions.
///
/// Fields:
/// - `nullable_levels`: which array levels the field was written with an `Option` at. Level 0 is
///   what `field_type` names and level `array_depth` is the field as a whole, so `Vec<Option<T>>`
///   records level 0 and `Option<Vec<T>>` records level 1 — two different values on the wire
///   (`[null]` against `null`) that a single flag could not tell apart. `is_optional` asks it for
///   the outermost level, the only one whose `None` is not written inside an array and so the only
///   one whose flavor depends on where the field sits: a dropped or `| undefined`-valued key /
///   `z.union([type, z.undefined()])` in struct-field position — `omits_value` deciding which of
///   the two the key gets — and `| null` / `z.nullable(type)` in a tuple element or map value,
///   neither of which can be dropped the way an object key can.
/// - name: Safe field name (uses serde rename if feature enabled)
/// - docs: Doc comments from Rust - included in generated TS as `JSDoc`
/// - `field_type`: The core type classification (see `FieldDefType`)
/// - `array_depth`: How many array levels wrap the type — one per `Vec`/slice/array/set the field
///   was written under, so `Vec<Vec<T>>` is 2. Zero is a bare value, which is what `is_array` asks.
/// - `array_lengths`: the element count of every level written as a fixed-size `[T; N]` with a
///   literal `N`, one entry per such level and numbered the way `nullable_levels` numbers them. A
///   level absent from the list holds however many items were written — what every other sequence
///   spelling holds, and what a non-literal `N` records, the expansion having no value to read a
///   const generic or a computed length from. Only the two validating surfaces spend it: the JSON
///   schema pins the level with `minItems`/`maxItems` and Zod with `.length(N)`, so both reject the
///   wrong-length payload serde itself refuses to deserialize. TypeScript keeps `Array<T>` — the
///   fixed-length form its type system has is the N-element tuple, written out element by element,
///   which stops being readable long before `N` stops being legal.
/// - `model_schema_prop_meta`: Optional metadata from #[`model_schema_prop`] attribute
///   - Used for overrides like literals, minLength, etc.
///   - See Phase 5 in `notes/20250707_field_features.md` for minLength details
/// - `omits_value`: whether the serde attributes on a *named* field leave its value out of the
///   serialized output entirely rather than writing it — a `None` under a
///   `skip_serializing_if = "Option::is_none"`, an empty `Vec` under a `Vec::is_empty`, every
///   value at all under a bare `skip`. Only a named field has a key to drop: a positional one is
///   written by its place in a tuple, where nothing can be omitted. A field carrying it is one
///   whose key serde may never write, which is what the object surfaces ask before choosing
///   between the two spellings of an optional member — the key-optional `field?: T` for a key
///   that may be absent, `field: T | undefined` for one that is always written — and what takes
///   the field out of the JSON surface's `required`. Read in every build: the attribute stating
///   it is on the field whatever the features say, and one declaration describes one wire.
/// - `absent_from_wire`: whether the serde attributes on a *named* field take its key out of both
///   directions at once — nothing serde writes carries it and nothing serde reads keeps what a
///   payload put under it. A bare `skip` says so, and so do `skip_serializing` and
///   `skip_deserializing` written side by side, which is the same wire spelled out. A field
///   carrying it is one no surface describes at all: the surfaces describe the payload serde
///   writes, and there is no such key in any of them. Read in every build, for the same reason
///   `omits_value` is.
/// - `type_span`: where the type this field carries was written — the name segment of a path, the
///   whole type otherwise, and the innermost name under a wrapper, so `Vec<Inner>` points at
///   `Inner`. The JSON schema is the one surface that emits a Rust path built from a type name, so
///   it is the one that can fail to resolve, and this is what its reference carries so the failure
///   is reported at the user's type instead of at `#[model_schema()]`. Present only under
///   `jsonschema` for that reason.
///
/// Usage notes:
/// - Created recursively for nested types
/// - Handles Option<T> by recording its array level in `nullable_levels`
/// - For `HashMap`, only String keys allowed
/// - Feature "serde" affects name (rename attributes)
/// - Feature "zod" enables `zod_type()` method
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
    /// The element of a collection wrapper, as the field the wrapper's own serialization makes it.
    ///
    /// A `Vec<T>` and a set of `T` both write a JSON array of `T`, so the element carries the
    /// array-ness and answers for what the array holds. The field's own constraints ride along:
    /// they have nothing but the element to land on, exactly as on the `Vec` spelling, which the
    /// parser hands over already collapsed onto its element.
    ///
    /// Every array level survives the move: the element's own, the one this wrapper adds, and the
    /// ones the wrapper itself sits under. The result therefore stands for the whole field, and a
    /// caller must not re-apply the field's own levels on top of it. Each level keeps what it was
    /// written with — its nullability, and the fixed length of a `[T; N]` — renumbered where it
    /// moved: the element's levels are the innermost and keep their numbers, while the wrapper's
    /// own sit above the array it adds.
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
    ///
    /// A map writes its keys and its values, a tuple writes each of its elements, and every surface
    /// builds those from the inner field defs — which carry no meta, the field's own sitting on the
    /// outer def none of them reads. A length, a pattern or a range names one value, and
    /// `model_schema_prop` has no way to say which member it meant, so a bound written here reaches
    /// nothing on any surface; the caller turns that into a guard error naming the field.
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
    ///
    /// This is used to detect recursive types where a type references itself.
    /// For example, `Vec<DynamicValue>` inside `DynamicValue` would return true.
    ///
    /// The check is recursive, looking into:
    /// - `SiblingType` direct references
    /// - `Map` key and value types
    /// - `Tuple` element types
    /// - array wrappers (Vec<T>)
    /// - `is_optional` wrappers (Option<T>)
    pub fn contains_type_reference(&self, type_name: &str) -> bool {
        match &self.field_type {
            FieldDefType::SiblingType(name, generics) => {
                // Direct match (check both original name and stripped name)
                let stripped_name = safe_type_name(name);
                if name == type_name || stripped_name == type_name {
                    return true;
                }
                // Also check generic arguments
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
    ///
    /// This is the one rule the two validating surfaces read a type parameter under, and the one
    /// place they part company with TypeScript over it. A parameter names no type at expansion —
    /// the instantiation names one, and one schema is written for every instantiation. So on those
    /// two surfaces a parameter describes as an opaque value does (`{}` in JSON Schema,
    /// `z.unknown()` in Zod), which leaves the shape it sits in — a tuple's arity, an array, a
    /// map's keys — described. TypeScript needs no such rule: it is a type surface, and
    /// `export type Wrapper<IdType> = { id: IdType }` binds the parameter for real, so it renders
    /// the name.
    ///
    /// Zod is why the rule cannot stop at JSON. Zod publishes *values*, and a `const` cannot be
    /// parameterised, so a parameter left to render as the `Name$Schema` binding every unresolved
    /// type is named after would reference a binding no emitted module declares — the consumer
    /// pasting the output gets a `ReferenceError` before a payload is read. That is why only the
    /// enclosing item's *own* parameters are rewritten: a genuinely unresolved sibling type keeps
    /// its `$Schema` reference, because the type it names does publish that binding.
    ///
    /// The rewrite carries into what a schema surface may then claim about the value. An opaque
    /// value takes no string checks — Zod 4's `z.unknown()` carries no `.min`/`.max`, and
    /// `.brand()` hands back the very same instance rather than a wrapper that could — so a
    /// branded newtype constraining one of its own parameters is refused, through the opaque arm
    /// of `non_string_inner_shape` this rewrite puts it in front of.
    ///
    /// Recurses through `SiblingType` generics, `Map` keys/values, and `Tuple` elements. Applied
    /// after the field guards have read the field, so every guard asks its question of the type
    /// the author wrote — and in every build, features or none, because which of the two a name is
    /// is a fact about the declaration rather than about the surfaces reading it.
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
    ///
    /// Each of these renders a shape fixed here rather than the plain string or number a length, a
    /// pattern or a range is spelled against: a chrono value as its own ISO spelling, an `ObjectId`
    /// as the `{"$oid": …}` object serde writes it as. None of the three surfaces reads a bound for
    /// them and neither does the Rust validator, so a bound written on one reaches nothing; the
    /// caller turns that into a guard error naming the field instead of dropping it.
    ///
    /// Only the field's own rendering is asked about. A map or a tuple holding one of these
    /// describes its members separately, and a bound on the field around them is
    /// [`Self::composite_shape_name`]'s to answer for.
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
        }
    }

    #[cfg(feature = "chrono")]
    fn has_as_number(&self) -> bool {
        self.model_schema_prop_meta
            .as_ref()
            .is_some_and(|m| m.as_number)
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
    /// answer.
    ///
    /// An `Option` in struct-field position may write no key. A serde attribute that omits the
    /// value means the same thing for a field of any type. Either one is enough to take the field
    /// out of `required`, because `required` is a claim about every payload and one payload
    /// without the key is enough to falsify it.
    #[cfg(feature = "jsonschema")]
    pub fn key_is_required(&self) -> bool {
        !self.is_optional() && !self.omits_value
    }

    /// Whether the object key this field writes may be absent, which is the question the two
    /// spellings of an optional member answer differently — `field?: T` admits the payload with no
    /// such key, `field: T | undefined` demands the key and lets its value be `undefined`.
    ///
    /// Two things say it, and either alone is enough. `ts_optional` says it for the TypeScript
    /// surface on the author's word, which only an `Option` field may give. A serde attribute that
    /// omits the value says it off the wire, whatever the field is written as: the payload serde
    /// writes simply has no key there. That second one asks nothing about `Option`-ness on purpose
    /// — a `Vec` behind a `Vec::is_empty` predicate has no `None` anywhere in it and its key still
    /// goes missing, so a rule keyed on `Option` would describe a payload serde does not write.
    ///
    /// The two overlap on every field that carries both, which leaves the flag deciding this only
    /// where no such attribute was written — a field the `serde` feature's `Option`-null guard
    /// refuses, so the flag's own answer is one a build with that feature off is the only place to
    /// read.
    fn key_may_be_absent(&self) -> bool {
        self.omits_value
            || (self.is_optional()
                && self
                    .model_schema_prop_meta
                    .as_ref()
                    .is_some_and(|m| m.ts_optional))
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

    #[cfg(feature = "zod")]
    fn opaque_type_parameters(&mut self) {
        if matches!(self.field_type, FieldDefType::TypeParam(_)) {
            self.field_type = FieldDefType::Unknown;
            return;
        }
        for nested in self.nested_type_positions() {
            nested.opaque_type_parameters();
        }
    }

    /// The `?` a member written with an absent-able key carries, and nothing for one whose key is
    /// always written.
    pub fn optional_key_marker(&self) -> &'static str {
        if self.key_may_be_absent() { "?" } else { "" }
    }

    /// The name of the first `OsString`/`OsStr` this field reaches, at any depth.
    ///
    /// Neither has a primitive mapping and neither can get one: serde writes them as an externally
    /// tagged enum whose variant is the target platform — `{"Unix":[u8, …]}` or
    /// `{"Windows":[u16, …]}` — so the same Rust field has two wire forms and no schema can
    /// describe both. Left unmapped they read as an ordinary `SiblingType`, and the generated code
    /// would reference a schema module the user never wrote; the caller turns this into a guard
    /// error naming the field instead.
    ///
    /// Recurses through `SiblingType` generics, `Map` keys/values, and `Tuple` elements.
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

    /// The enclosing item's own type parameter this field renders as, when a bound spelled against
    /// it names a value whose type the expansion never sees.
    ///
    /// A parameter names no type at expansion — the instantiation names one, and one schema is
    /// written for every instantiation. So the two validating surfaces describe the value as the
    /// opaque one, which takes no length, no pattern and no range, while the generated validator
    /// and serde read whatever type the instantiation supplied and hold it to nothing written here.
    /// A bound therefore reaches nothing on any surface, exactly as one written beside a map's
    /// members does; the caller turns that into a guard error naming the parameter.
    ///
    /// Only a field already read through [`Self::erase_type_parameters`] answers here: which of
    /// the two a written name is — the item's own parameter or a reference to another type — is
    /// that rewrite's question, and asking it twice is how the two answers come to differ.
    pub fn parameter_shape_name(&self) -> Option<&str> {
        match &self.field_type {
            FieldDefType::TypeParam(name) => Some(name),
            FieldDefType::SiblingType(_, _)
            | FieldDefType::Map(_, _)
            | FieldDefType::Tuple(_)
            | FieldDefType::Unknown
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

    /// Rewrites any `Self` type reference to the concrete enclosing type name.
    ///
    /// A recursive type may refer to itself with the `Self` keyword
    /// (e.g. `Array(Vec<Self>)`). The macro detects recursion and renders
    /// references by comparing type names, so `Self` must be resolved to the
    /// actual type name before that logic runs; afterwards `Vec<Self>` is
    /// treated exactly like `Vec<EnclosingType>`.
    ///
    /// Recurses through `SiblingType` generics, `Map` keys/values, and `Tuple`
    /// elements.
    pub fn resolve_self_references(&mut self, type_name: &str) {
        match &mut self.field_type {
            FieldDefType::SiblingType(name, generics) => {
                if name == "Self" {
                    type_name.clone_into(name);
                }
                for generic in generics.iter_mut() {
                    generic.resolve_self_references(type_name);
                }
            }
            FieldDefType::Map(key, value) => {
                key.resolve_self_references(type_name);
                value.resolve_self_references(type_name);
            }
            FieldDefType::Tuple(elements) => {
                for element in elements.iter_mut() {
                    element.resolve_self_references(type_name);
                }
            }
            // Leaf types cannot contain nested references.
            FieldDefType::TypeParam(_)
            | FieldDefType::Unknown
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
                    k.typescript_typename(),
                    v.typescript_slot_typename()
                )
            }
            FieldDefType::Boolean => "boolean".to_owned(),
            FieldDefType::String => "string".to_owned(),
            FieldDefType::StringLiteral(literal) => format!("\"{literal}\""),
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

    /// What this field contributes to an object that writes its members beside its own, on the
    /// TypeScript surface: the value itself, with no answer for the outermost `Option`.
    ///
    /// A merged source has no key of its own, so that `Option` is not the question
    /// [`Self::typescript_typename`] answers. There, a `None` is a key the object leaves out; here
    /// it is every one of the source's keys left out at once — the object writes its own members
    /// merged with the source's or writes its own alone, and a choice between two key sets belongs
    /// where the merge is assembled rather than on the operand it is assembled from.
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
    ///
    /// This method is the core of TypeScript type generation. It recursively builds
    /// the TS type string based on `field_type`, `array_depth`, and `nullable_levels`.
    ///
    /// Process:
    /// 1. Match on `field_type` to get base type
    /// 2. Wrap in Array<...> once per `array_depth` level, adding `| null` inside the wrap at each
    ///    level written as an `Option` — the array is always written, so the `None` is an item
    /// 3. If `is_optional`, which is that question asked of the outermost level: struct-field
    ///    position adds `| undefined`, or nothing at all where the key itself may be absent and
    ///    [`Self::optional_key_marker`] writes the `?` that says so; tuple-element and map-value
    ///    positions add `| null` (neither can be omitted like an object key, so a `None` there
    ///    serializes as `null`)
    ///
    /// Feature notes:
    /// - "`object_id"`: Uses special `ObjectId` type
    /// - Ignores `model_schema_prop_meta` currently (except implicitly through `field_type`)
    /// - For `StringLiteral`, generates quoted string literal
    /// - All Rust numbers map to 'number' in TS
    ///
    /// See generation/typescript.rs for how this is used in full type defs.
    /// Examples in README.md show generated output.
    pub fn typescript_typename(&self) -> String {
        let pre_result = self.typescript_base();
        if self.is_optional() {
            // An absent-able key renders as `field?: T`, so the `| undefined` is redundant — and
            // under `exactOptionalPropertyTypes` it would claim an explicit `undefined` the key's
            // omission is exactly what serde writes instead.
            if self.key_may_be_absent() {
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

    /// The same def with every [`FieldDefType::TypeParam`] written back as the opaque value.
    ///
    /// Zod names a parameter through the argument the enclosing factory binds for it, which only a
    /// type that publishes a factory has. An alias and a branded newtype publish a plain `const`,
    /// so there is no argument for a parameter inside either to name and the opaque value is still
    /// all either can write — the answer [`Self::erase_type_parameters`] describes, kept for the
    /// two surfaces that have not moved off it.
    #[cfg(feature = "zod")]
    #[must_use]
    pub fn with_opaque_type_parameters(mut self) -> Self {
        self.opaque_type_parameters();
        self
    }

    /// The type match plus one `z.array(…)` per array level, each carrying the `z.nullable(…)` of
    /// the level it wraps and the `.length(N)` of a level written as a fixed-size `[T; N]`, before
    /// the preprocess wrap.
    ///
    /// Zod is a validator, so it says what the JSON schema says: serde reads a `[T; N]` back only
    /// from an array of exactly `N` items, and `.length` is that constraint spelled directly. The
    /// TypeScript surface takes the other answer and stays `Array<T>` — see `array_lengths`.
    ///
    /// A set's element re-enters the rendering here rather than at `zod_base`: it carries a copy of
    /// the field's own metadata, and the preprocess wrap belongs once, outside the array — where
    /// the `Vec` spelling of the same field puts it.
    #[cfg(feature = "zod")]
    fn zod_array_base(&self) -> String {
        let result = match &self.field_type {
            FieldDefType::Unknown => "z.unknown()".to_owned(),
            // A `const` cannot be parameterised, so a generic type publishes a factory and a
            // parameter composes the argument that factory binds for it — see
            // [`zod_factory_argument`]. A surface with no factory to bind one opaques the
            // parameter first, through [`FieldDef::with_opaque_type_parameters`].
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
                    // What the named type published is what this can name. A generic struct or
                    // enum published a factory, so the arguments written here are the call's; an
                    // alias and a branded newtype published the one `const` they have whatever
                    // they were declared with, and that `const` is the whole of what a reference
                    // to either can say.
                    if info.publishes_zod_factory {
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
            FieldDefType::String => self.zod_string_type(),
            FieldDefType::StringLiteral(literal) => format!("z.literal(\"{literal}\")"),
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
        let array_result = self.zod_array_base();

        // Wrap with preprocess if specified
        if let Some(meta) = &self.model_schema_prop_meta
            && !meta.preprocess.is_empty()
        {
            let mut wrapped = array_result;
            for fn_name in meta.preprocess.iter().rev() {
                wrapped = format!("z.preprocess({fn_name}, {wrapped})");
            }
            wrapped
        } else {
            array_result
        }
    }

    /// The key schema a `z.record(…)` is written with: the key's own, except where the key is one
    /// of the enclosing item's type parameters.
    ///
    /// A record key has to produce string keys, and the opaque value a parameter describes as
    /// everywhere else declines to say anything at all about them — the one position where saying
    /// nothing says less than the wire already guarantees. serde is what guarantees it: an
    /// instantiation either writes this map's object keys as strings or refuses the whole map at
    /// serialization with `key must be a string`, with no fallback form, so `z.string()` holds for
    /// every instantiation that serializes at all. It is also the answer the JSON surface reads off
    /// the same key through its own classification, which is what keeps the two saying one thing.
    ///
    /// Only a bare key reaches the parameter arm: a key written under a sequence or an `Option`
    /// wrapper is refused before any surface renders the map.
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
        // Add min length validation if specified
        if let Some(meta) = &self.model_schema_prop_meta
            && let Some(min_len) = meta.min_length
        {
            result = format!("{result}.min({min_len})");
        }
        // Add max length validation if specified
        if let Some(meta) = &self.model_schema_prop_meta
            && let Some(max_len) = meta.max_length
        {
            result = format!("{result}.max({max_len})");
        }
        // Add pattern validation if specified
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
    ///
    /// Similar to `typescript_typename()` but generates Zod validation schema.
    /// Uses z.* functions appropriate to the type.
    ///
    /// Additional logic:
    /// - For String: Adds .`min(min_len)` if `model_schema_prop_meta` has `min_length`
    /// - For literals: Uses z.literal(...)
    /// - For `ObjectId`: Uses regex validation for hex string
    /// - Wraps with `z.array()` once per `array_depth` level, wrapping `z.nullable(…)` inside it
    ///   at each level written as an `Option` and appending `.length(N)` at each level written as a
    ///   fixed-size `[T; N]`
    /// - If `is_optional`, which is that question asked of the outermost level: struct-field
    ///   position wraps `z.union([type, z.undefined()])`; tuple-element and map-value positions wrap
    ///   `z.nullable(type)` (neither can be omitted like an object key, so a `None` there
    ///   serializes as `null`)
    /// - Otherwise, if the key may still go missing — a serde attribute dropping the value of a
    ///   field that is no `Option` — appends `.optional()`. The value under the key is unchanged,
    ///   so the type is left as it stands and only its presence is relaxed. Recorded against zod
    ///   4.4.3: inside a `z.strictObject`, that member admits the payload with the key absent and
    ///   the payload carrying it, still rejects `null`, and still rejects an unrecognized key.
    ///
    /// Requires Zod v4 in frontend - generates v4-compatible syntax.
    /// See `notes/20250706_features.md` for Zod feature details.
    pub fn zod_type(&self) -> String {
        let pre_result = self.zod_base();
        if self.is_optional() {
            format!("z.union([{pre_result}, z.undefined()]).prefault(undefined)")
        } else if self.key_may_be_absent() {
            format!("{pre_result}.optional()")
        } else {
            pre_result
        }
    }

    /// The members of the untagged union this field names, as the registry recorded them, and
    /// nothing for a field that names no such union.
    ///
    /// A merge cannot join a union as one operand — an intersection recognizes exactly the keys its
    /// operands name, and a `z.union` names none — so it joins one member per branch instead, and
    /// these are the branches. The members are recorded already multiplied out, so a member that is
    /// itself a union has already contributed its own.
    ///
    /// Answered only for a field whose whole Zod spelling *is* the name the registry keys — an
    /// array level or a preprocess wrap is part of what the operand validates, and a member spliced
    /// in the name's place would drop it. The outermost `Option` is not one of those: it is what
    /// [`Self::zod_merged_schema`] already leaves to the merge.
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
///
/// Membership is decided on the wire and nothing else: serde writes each of these as a JSON array
/// of its single element type, so each describes as the `Vec` of that element does. The maps are
/// absent because they write objects; `Vec` is listed even though the parser collapses it onto its
/// element as an array level long before anything asks a wrapper's name, so that a `Vec` written
/// where a wrapper name is read still takes the wrapper path.
pub fn is_sequence_wrapper(name: &str) -> bool {
    matches!(
        name,
        "BTreeSet" | "BinaryHeap" | "HashSet" | "LinkedList" | "Vec" | "VecDeque"
    )
}

/// The number of leading type arguments a std container's wire form is written from, or `None` for
/// a name that is not one of them.
///
/// std's hashed maps and sets carry a hasher past the types they write, and each of these
/// containers takes an allocator beside it. Neither reaches the wire — serde writes the same bytes
/// whichever is named — so neither is part of what the container describes as, and an argument past
/// this count is dropped rather than read as a type of its own.
///
/// The count is what the container is claimed on, not what it is written with: a spelling carrying
/// fewer arguments than this is not the container at all, and is left to fall through as the
/// sibling it was written as, where the schema module it names is reported unresolvable against the
/// author's own type. The one-argument list is the surfaces' shared answer for what writes a JSON
/// array, so a wrapper cannot be read here as a container and there as a name of its own.
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
///
/// Membership is decided on the wire and nothing else: serde writes each of these as its inner
/// value, with nothing of its own around it, so a field written under one is the very field its
/// inner type is. The parser therefore collapses them onto that inner field and no surface ever
/// learns a wrapper was written — which is what keeps a wrapper name out of the generated output,
/// where none of the three surfaces has any meaning for it.
///
/// `Cow` is covered on the same terms despite carrying a lifetime beside its type: the lifetime is
/// not a type argument and never reaches the collected arguments, so a `Cow` arrives with the one
/// argument a `Box` arrives with. The interior-mutability wrappers are absent — that is a wider
/// list than this defect was measured over, not a claim that they write anything else.
pub fn is_transparent_wrapper(name: &str) -> bool {
    matches!(name, "Arc" | "Box" | "Cow" | "Rc")
}

/// Classifies a `syn::Variant` into its `VariantKind`.
///
/// This determines how the variant should be rendered in TypeScript/Zod:
/// - Unit → no content field
/// - Named → individual named fields
/// - `TupleSingle` → flattened `value: T`
/// - `TupleMultiple` → tuple `value: [T1, T2, ...]`
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
///
/// This is the entry point for field analysis. It recursively parses the Rust type
/// syntax tree to build the `FieldDef` structure.
///
/// Handles:
/// - Primitives and simple types (via `get_field_def_type_or_sibling`)
/// - Generics (Option<T>, Vec<T>, `HashMap`<K,V>)
/// - Tuples, arrays, slices, references
/// - Falls back to Unknown for unsupported types
///
/// Key behaviors:
/// - Strips references (&T -> T)
/// - Counts an `array_depth` level for each Vec, slice, array
/// - Records a nullable level for each Option, at the array level it was written at
/// - Only supports `HashMap`<String, T> (panics or errors otherwise per rules)
/// - Uses `safe_type_name()` to strip Json suffix
///
/// Called from `process_field()` in `model_schema.rs` for each struct field.
///
/// Builds a `FieldDef` for a `Type::Path` (named type, possibly with generic arguments).
///
/// Handles `Option<T>`, `Vec<T>`, `HashMap`/`BTreeMap`, `DateTime<Tz>`, and falls back to
/// `SiblingType`/`Unknown` for everything else.
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
///
/// Handles `Option<T>`, `Vec<T>`, the transparent wrappers, `HashMap`/`BTreeMap` and
/// `DateTime<Tz>`, and falls back to `SiblingType` for everything else.
///
/// Only the type arguments are read. A lifetime writes nothing, so a type carrying one arrives here
/// with the arguments it would have without it — which is what lets `Cow<'a, T>` be answered for by
/// the same single-argument arms as `Box<T>`.
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
///
/// A collapse keeps the field and drops only the wrapper. The docs were written where the field
/// was, not around what it holds, so they cross onto the element the field's own name crosses onto
/// — the element was parsed with none of its own to lose. Everything a wrapper could sit around or
/// inside is already on that element and rides along untouched: `Option` lifts its optionality onto
/// the outermost level, a sequence counts one more level, and a wrapper that is not on the wire at
/// all adds nothing for the field to carry and hands the element over whole.
///
/// `None` for every other name, which is a wrapper this does not answer for rather than one that
/// collapses to nothing — the caller goes on to its own arms.
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
    if let Type::Path(type_path) = ty {
        get_field_def_from_type_path(type_path, safe_name, field_docs)
    } else if let Type::Reference(type_ref) = ty {
        // let lifetime = type_ref
        //     .lifetime
        //     .as_ref()
        //     .map_or("".to_string(), |l| format!("'{}", l.ident));
        get_field_def(name, type_ref.elem.as_ref(), field_docs)
    } else if let Type::Array(type_array) = ty {
        let mut def = get_field_def(name, &type_array.elem, field_docs);
        // The array this spelling adds is the level the element's own depth counts up to, and the
        // length is that level's — not the field's, which may sit under further wrappers.
        let level = def.array_depth;
        def.array_depth = def.array_depth.saturating_add(1);
        if let Some(length) = literal_array_length(&type_array.len) {
            def.mark_fixed_length_at(level, length);
        }
        def
    } else if let Type::Slice(type_slice) = ty {
        let mut def = get_field_def(name, &type_slice.elem, field_docs);
        def.array_depth = def.array_depth.saturating_add(1);
        def
    } else if let Type::Tuple(type_tuple) = ty {
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
            type_span: ty.span(),
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
            type_span: ty.span(),
        }
    }
}

/// Helper to map Rust type name strings to `FieldDefType`.
///
/// Used in `get_field_def` for types without generics.
///
/// Mapping rules:
/// - Built-in primitives to their variants
/// - Types ending with "Json" to `SiblingType` (strips suffix in TS)
/// - Other types to `SiblingType`
/// - `ObjectId`: Special handling if feature enabled, else `SiblingType` with warning
///
/// Feature "`object_id"`:
/// - Enables special `ObjectId` variant
/// - Without: Prints compile-time warning and treats as custom type
fn get_field_def_type_or_sibling(t_name: &str) -> FieldDefType {
    if lookup_alias_info(t_name).is_some() {
        return FieldDefType::SiblingType(t_name.to_owned(), vec![]);
    }
    match t_name {
        "bool" => FieldDefType::Boolean,
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
///
/// Used in `model_schema.rs` to distinguish plain enums (union types) from
/// tagged enums (discriminated unions).
///
/// Plain enums generate string unions in TS/Zod.
/// See enum examples in README.md.
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

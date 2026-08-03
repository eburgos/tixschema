use syn::{Fields, GenericArgument, ItemEnum, PathArguments, Type, Variant};

#[cfg(feature = "serde")]
use syn::Attribute;

use crate::features::model_schema_prop::ModelSchemaPropMeta;
use crate::utils::{lookup_alias_info, safe_type_name};

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
#[derive(Clone, Debug)]
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
///   one whose flavor depends on where the field sits: `| undefined` /
///   `z.union([type, z.undefined()])` in struct-field position, `| null` / `z.nullable(type)` in a
///   tuple element or map value, neither of which can be dropped the way an object key can.
/// - name: Safe field name (uses serde rename if feature enabled)
/// - docs: Doc comments from Rust - included in generated TS as `JSDoc`
/// - `field_type`: The core type classification (see `FieldDefType`)
/// - `array_depth`: How many array levels wrap the type — one per `Vec`/slice/array/set the field
///   was written under, so `Vec<Vec<T>>` is 2. Zero is a bare value, which is what `is_array` asks.
/// - `array_num`: Unused currently (future fixed-size array support?)
/// - `model_schema_prop_meta`: Optional metadata from #[`model_schema_prop`] attribute
///   - Used for overrides like literals, minLength, etc.
///   - See Phase 5 in `notes/20250707_field_features.md` for minLength details
///
/// Usage notes:
/// - Created recursively for nested types
/// - Handles Option<T> by recording its array level in `nullable_levels`
/// - For `HashMap`, only String keys allowed
/// - Feature "serde" affects name (rename attributes)
/// - Feature "zod" enables `zod_type()` method
#[derive(Clone, Debug)]
pub struct FieldDef {
    pub array_depth: u8,
    pub array_num: Option<u16>,
    pub docs: String,
    pub field_type: FieldDefType,
    pub model_schema_prop_meta: Option<ModelSchemaPropMeta>,
    pub name: String,
    pub nullable_levels: Vec<u8>,
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
    /// caller must not re-apply the field's own levels on top of it. Each level keeps the
    /// nullability it was written with, renumbered where it moved: the element's levels are the
    /// innermost and keep their numbers, while the wrapper's own sit above the array it adds.
    pub fn collection_element_field(&self, element: &Self) -> Self {
        let mut arrayed = element.clone();
        arrayed.name.clone_from(&self.name);
        arrayed.array_depth = element.array_depth.saturating_add(1);
        for level in &self.nullable_levels {
            arrayed.mark_nullable_at(level.saturating_add(arrayed.array_depth));
        }
        arrayed.array_depth = arrayed.array_depth.saturating_add(self.array_depth);
        arrayed
            .model_schema_prop_meta
            .clone_from(&self.model_schema_prop_meta);
        arrayed
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

    /// Replaces every reference to one of the enclosing item's type parameters with the opaque
    /// type.
    ///
    /// A parameter names no type at expansion — the instantiation names one, and the schema is
    /// written once for every instantiation. So a parameter describes as an opaque value does,
    /// which leaves the shape it sits in — a tuple's arity, an array, a map's keys — described.
    ///
    /// Recurses through `SiblingType` generics, `Map` keys/values, and `Tuple` elements.
    #[cfg(feature = "jsonschema")]
    pub fn erase_type_parameters(&mut self, parameters: &[String]) {
        if let FieldDefType::SiblingType(name, _) = &self.field_type
            && parameters.iter().any(|parameter| parameter == name)
        {
            self.field_type = FieldDefType::Unknown;
            return;
        }
        match &mut self.field_type {
            FieldDefType::SiblingType(_, generics) => {
                for generic in generics.iter_mut() {
                    generic.erase_type_parameters(parameters);
                }
            }
            FieldDefType::Map(key, value) => {
                key.erase_type_parameters(parameters);
                value.erase_type_parameters(parameters);
            }
            FieldDefType::Tuple(elements) => {
                for element in elements.iter_mut() {
                    element.erase_type_parameters(parameters);
                }
            }
            // Leaf types name no parameter.
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

    #[cfg(feature = "chrono")]
    fn has_as_number(&self) -> bool {
        self.model_schema_prop_meta
            .as_ref()
            .is_some_and(|m| m.as_number)
    }

    fn has_ts_optional(&self) -> bool {
        self.is_optional()
            && self
                .model_schema_prop_meta
                .as_ref()
                .is_some_and(|m| m.ts_optional)
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

    fn mark_nullable_at(&mut self, level: u8) {
        if !self.nullable_levels.contains(&level) {
            self.nullable_levels.push(level);
        }
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

    pub fn ts_optional_key_marker(&self) -> &'static str {
        if self.has_ts_optional() { "?" } else { "" }
    }

    /// Builds the TypeScript type before the outermost optional wrap: the type match plus one
    /// `Array<…>` per array level, each carrying the `| null` of the level it wraps. The outermost
    /// level's wrap lives in `typescript_typename` and `typescript_slot_typename`, which is where
    /// the position it sits in decides between `| undefined` and `| null`.
    fn typescript_base(&self) -> String {
        let result = match &self.field_type {
            FieldDefType::Unknown => "unknown".to_owned(),
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

    /// The TypeScript type for a value in a slot that cannot be dropped — a tuple element or a
    /// map entry. An `Option` there is null-flavored (`{base} | null`) rather than
    /// undefined-flavored: only an object key can be omitted, so serde emits `null` for a `None`
    /// in either position.
    fn typescript_slot_typename(&self) -> String {
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
    ///    position adds `| undefined`; tuple-element and map-value positions add `| null` (neither
    ///    can be omitted like an object key, so a `None` there serializes as `null`)
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
            // `ts_optional` renders the key as `field?: T`, so the `| undefined` is redundant.
            if self.has_ts_optional() {
                pre_result
            } else {
                format!("{pre_result} | undefined")
            }
        } else {
            pre_result
        }
    }

    /// The type match plus one `z.array(…)` per array level, each carrying the `z.nullable(…)` of
    /// the level it wraps, before the preprocess wrap.
    ///
    /// A set's element re-enters the rendering here rather than at `zod_base`: it carries a copy of
    /// the field's own metadata, and the preprocess wrap belongs once, outside the array — where
    /// the `Vec` spelling of the same field puts it.
    #[cfg(feature = "zod")]
    fn zod_array_base(&self) -> String {
        let result = match &self.field_type {
            FieldDefType::Unknown => "z.unknown()".to_owned(),
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
                    // Always reference the $Schema, regardless of generic params.
                    // For branded wrappers like DocumentTypeId<String>, the Zod
                    // schema is defined on the wrapper itself.
                    format!("{}$Schema", info.export_name)
                } else if lst.is_empty() {
                    format!("{name}$Schema")
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
                format!("z.record({}, {})", k.zod_type(), v.zod_slot_type())
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
            format!("z.array({item})")
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

    /// The Zod schema for a value in a slot that cannot be dropped — a tuple element or a map
    /// entry. An `Option` there is null-flavored (`z.nullable({base})`) rather than
    /// undefined-flavored: only an object key can be omitted, so serde emits `null` for a `None`
    /// in either position.
    #[cfg(feature = "zod")]
    fn zod_slot_type(&self) -> String {
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
            result = format!("{result}.check(z.regex(/{pattern}/))");
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
    ///   at each level written as an `Option`
    /// - If `is_optional`, which is that question asked of the outermost level: struct-field
    ///   position wraps `z.union([type, z.undefined()])`; tuple-element and map-value positions wrap
    ///   `z.nullable(type)` (neither can be omitted like an object key, so a `None` there
    ///   serializes as `null`)
    ///
    /// Requires Zod v4 in frontend - generates v4-compatible syntax.
    /// See `notes/20250706_features.md` for Zod feature details.
    pub fn zod_type(&self) -> String {
        let pre_result = self.zod_base();
        if self.is_optional() {
            format!("z.union([{pre_result}, z.undefined()]).prefault(undefined)")
        } else {
            pre_result
        }
    }
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
            array_num: None,
            docs: field_docs.to_owned(),
            model_schema_prop_meta: None,
            nullable_levels: Vec::new(),
        };
    };
    let ident = segment.ident.to_string();
    match &segment.arguments {
        PathArguments::None => FieldDef {
            name: safe_name,
            field_type: get_field_def_type_or_sibling(&ident),
            array_depth: 0,
            array_num: None,
            docs: field_docs.to_owned(),
            model_schema_prop_meta: None,
            nullable_levels: Vec::new(),
        },
        PathArguments::AngleBracketed(args) => {
            get_field_def_from_generic_type(&ident, args, safe_name, field_docs)
        }
        // Function pointer types are unsupported; fall back to `unknown`.
        PathArguments::Parenthesized(_) => FieldDef {
            name: safe_name,
            field_type: FieldDefType::Unknown,
            array_depth: 0,
            array_num: None,
            docs: field_docs.to_owned(),
            model_schema_prop_meta: None,
            nullable_levels: Vec::new(),
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
    ident: &str,
    args: &syn::AngleBracketedGenericArguments,
    safe_name: String,
    field_docs: &str,
) -> FieldDef {
    let arg_types: Vec<FieldDef> = args
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
    if arg_types.is_empty() {
        FieldDef {
            name: safe_name,
            field_type: FieldDefType::SiblingType(ident.to_owned(), vec![]),
            array_depth: 0,
            array_num: None,
            docs: field_docs.to_owned(),
            model_schema_prop_meta: None,
            nullable_levels: Vec::new(),
        }
    } else if arg_types.len() == 1 && ident == "Option" {
        let mut result = arg_types[0].clone();
        result.name = safe_name;
        result.mark_nullable_at(result.array_depth);
        result
    } else if arg_types.len() == 1 && ident == "Vec" {
        let mut result = arg_types[0].clone();
        result.name = safe_name;
        result.array_depth = result.array_depth.saturating_add(1);
        result
    }
    // A wrapper that is not on the wire adds nothing for the field to carry: the inner field stands
    // in whole, under the wrapped field's own name and its own docs, which belong to where the
    // field was written rather than to what it was written under. Everything a wrapper could sit
    // around or inside — the optionality an `Option` lifts, the levels a sequence counts — is
    // already on that field and rides along untouched.
    else if arg_types.len() == 1 && is_transparent_wrapper(ident) {
        let mut result = arg_types[0].clone();
        result.name = safe_name;
        field_docs.clone_into(&mut result.docs);
        result
    } else if arg_types.len() == 2 && (ident == "HashMap" || ident == "BTreeMap") {
        log::trace!(
            "Creating HashMap Map type - key: {:?}, value: {:?}",
            arg_types[0],
            arg_types[1]
        );
        FieldDef {
            array_depth: 0,
            array_num: None,
            name: safe_name,
            field_type: FieldDefType::Map(
                Box::new(arg_types[0].clone()),
                Box::new(arg_types[1].clone()),
            ),
            docs: field_docs.to_owned(),
            model_schema_prop_meta: None,
            nullable_levels: Vec::new(),
        }
    } else if arg_types.len() == 1 && is_datetime_generic_type(ident) {
        // The timezone type parameter says nothing about what is written.
        handle_datetime_generic_type(safe_name, field_docs)
    } else {
        log::trace!("Creating SiblingType - name: {ident}, arg_types: {arg_types:?}");
        FieldDef {
            name: safe_name,
            field_type: FieldDefType::SiblingType(ident.to_owned(), arg_types),
            array_depth: 0,
            array_num: None,
            docs: field_docs.to_owned(),
            model_schema_prop_meta: None,
            nullable_levels: Vec::new(),
        }
    }
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
        def.array_depth = def.array_depth.saturating_add(1);
        def.array_num = None; // type_array.len;
        def
    } else if let Type::Slice(type_slice) = ty {
        let mut def = get_field_def(name, &type_slice.elem, field_docs);
        def.array_depth = def.array_depth.saturating_add(1);
        def.array_num = None; // type_array.len;
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
            array_num: None,
            docs: field_docs.to_owned(),
            model_schema_prop_meta: None,
            nullable_levels: Vec::new(),
        }
    } else {
        // Fallback for BareFn, ImplTrait, etc.
        FieldDef {
            name: safe_name,
            field_type: FieldDefType::Unknown,
            array_depth: 0,
            array_num: None,
            docs: field_docs.to_owned(),
            model_schema_prop_meta: None,
            nullable_levels: Vec::new(),
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

/// Handle `DateTime<Tz>` generic type (chrono feature).
/// Creates a `FieldDef` with `DateTime` type, ignoring the timezone parameter.
#[cfg(feature = "chrono")]
fn handle_datetime_generic_type(safe_name: String, field_docs: &str) -> FieldDef {
    FieldDef {
        name: safe_name,
        field_type: FieldDefType::DateTime,
        array_depth: 0,
        array_num: None,
        docs: field_docs.to_owned(),
        model_schema_prop_meta: None,
        nullable_levels: Vec::new(),
    }
}

#[cfg(not(feature = "chrono"))]
fn handle_datetime_generic_type(safe_name: String, field_docs: &str) -> FieldDef {
    // Fallback - should never be called since is_datetime_generic_type returns false
    FieldDef {
        name: safe_name,
        field_type: FieldDefType::SiblingType("DateTime".to_owned(), vec![]),
        array_depth: 0,
        array_num: None,
        docs: field_docs.to_owned(),
        model_schema_prop_meta: None,
        nullable_levels: Vec::new(),
    }
}

#[cfg(test)]
mod tests;

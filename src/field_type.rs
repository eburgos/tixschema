use syn::{Fields, GenericArgument, ItemEnum, PathArguments, Type, Variant};

#[cfg(feature = "serde")]
use syn::Attribute;

use crate::utils::{lookup_alias_info, safe_type_name};

/// Classifies how an enum variant stores its data.
///
/// This is used to determine the correct TypeScript/Zod generation strategy
/// for discriminated union variants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VariantKind {
    /// Unit variant with no fields: `Status::Active`
    /// Generates: `{ type: "Active" }`
    Unit,
    /// Named struct fields: `Payment::Card { number: String }`
    /// Generates: `{ type: "Card", number: string }`
    Named,
    /// Single tuple element: `Value::Text(String)`
    /// Generates: `{ type: "Text", value: string }`
    TupleSingle,
    /// Multiple tuple elements: `Value::Complex(String, i64)`
    /// Generates: `{ type: "Complex", value: [string, number] }`
    TupleMultiple,
}

/// Classifies a syn::Variant into its VariantKind.
///
/// This determines how the variant should be rendered in TypeScript/Zod:
/// - Unit → no content field
/// - Named → individual named fields
/// - TupleSingle → flattened `value: T`
/// - TupleMultiple → tuple `value: [T1, T2, ...]`
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

/// Enum representing the possible types a field can have in the schema generation system.
///
/// This enum is central to tixschema's type mapping system. It categorizes Rust types into
/// categories that can be translated to TypeScript types and Zod schemas. The variants cover
/// primitive types, complex structures, and special cases like `ObjectId` (feature-dependent).
///
/// Key points from crate documentation:
/// - Primitives map to TS equivalents (e.g., String -> string, numbers -> number)
/// - Collections like Vec<T> become Array<T> (handled via `FieldDef`'s `is_array` flag)
/// - `HashMap`<String, T> becomes Partial<Record<string, T>> (Map variant)
/// - Enums and nested structs use `SiblingType`
/// - All types must follow naming conventions (e.g., end with Json suffix for structs)
///
/// Feature dependencies:
/// - "`object_id"`: Enables `ObjectId` variant for `MongoDB` support
/// - Without features, falls back to basic mappings
#[derive(Clone, Debug)]
pub enum FieldDefType {
    /// Unknown or unsupported type - generates 'unknown' in TS/Zod
    Unknown,
    /// Reference to another struct/enum type, potentially with generics
    /// First String is the type name (without Json suffix in TS)
    /// Vec<FieldDef> holds generic parameters if any
    SiblingType(String, Vec<FieldDef>),
    /// Map type (`HashMap`<K, V>) - only String keys supported per rules
    /// Boxed for recursion. Generates Partial<Record<K, V>> in TS
    Map(Box<FieldDef>, Box<FieldDef>),
    /// Tuple type - generates anonymous object in TS/Zod
    Tuple(Vec<FieldDef>),
    /// Boolean primitive - maps to boolean
    Boolean,
    /// String primitive - maps to string
    String,
    /// String literal type - for fixed string values
    /// Added via `model_schema_prop(literal` = "value")
    /// Maps to "value" in TS, z.literal("value") in Zod
    StringLiteral(String), // For string literal types like "Tixena"
    U8,
    U16,
    U32,
    U64,
    I8,
    I16,
    I32,
    I64,
    Usize,
    Isize,
    F32,
    F64,

    #[cfg(feature = "object_id")]
    /// `MongoDB` `ObjectId` type - requires "`object_id`" feature
    /// Maps to `ObjectId` interface in TS with $oid: string
    /// Zod: z.object({ $oid: z.string().regex(...) })
    /// JSON Schema: object with $oid string property
    /// See README.md for serialization format and validation details
    ObjectId,

    #[cfg(feature = "chrono")]
    /// Chrono `NaiveDate` type - requires "`chrono`" feature
    /// Maps to `string` in TS (ISO 8601 date format: "2025-11-29")
    /// Zod: z.string().date()
    /// JSON Schema: string with format "date"
    NaiveDate,

    #[cfg(feature = "chrono")]
    /// Chrono `NaiveTime` type - requires "`chrono`" feature
    /// Maps to `string` in TS (format: "14:30:00")
    /// Zod: z.string().time()
    /// JSON Schema: string with format "time"
    NaiveTime,

    #[cfg(feature = "chrono")]
    /// Chrono `NaiveDateTime` type - requires "`chrono`" feature
    /// Maps to `string` in TS (ISO 8601 format: "2025-11-29T14:30:00")
    /// Zod: z.string().datetime({ local: true })
    /// JSON Schema: string with format "date-time"
    NaiveDateTime,

    #[cfg(feature = "chrono")]
    /// Chrono `DateTime<Tz>` type - requires "`chrono`" feature
    /// Maps to `string` in TS (ISO 8601 format: "2025-11-29T14:30:00Z")
    /// Zod: z.string().datetime()
    /// JSON Schema: string with format "date-time"
    /// Note: The timezone type parameter is ignored for schema generation
    DateTime,
}

/// Struct representing a field's definition for schema generation.
///
/// This is the core data structure used to analyze and generate schemas for each field
/// in a struct or enum variant. It's created by `get_field_def()` and used in
/// `model_schema.rs` to build the full type definitions.
///
/// Fields:
/// - `is_optional`: If true, adds | undefined in TS and z.union([type, `z.undefined()`]) in Zod (v4 syntax)
/// - name: Safe field name (uses serde rename if feature enabled)
/// - docs: Doc comments from Rust - included in generated TS as `JSDoc`
/// - `field_type`: The core type classification (see `FieldDefType`)
/// - `is_array`: If true, wraps type in Array<...> (for Vec<T>, slices, arrays)
/// - `array_num`: Unused currently (future fixed-size array support?)
/// - `model_schema_prop_meta`: Optional metadata from #[`model_schema_prop`] attribute
///   - Used for overrides like literals, minLength, etc.
///   - See Phase 5 in `notes/20250707_field_features.md` for minLength details
///
/// Usage notes:
/// - Created recursively for nested types
/// - Handles Option<T> by setting `is_optional=true` on inner type
/// - For `HashMap`, only String keys allowed
/// - Feature "serde" affects name (rename attributes)
/// - Feature "zod" enables `zod_type()` method
#[derive(Clone, Debug)]
pub struct FieldDef {
    pub is_optional: bool,
    pub name: String,
    pub docs: String,
    pub field_type: FieldDefType,
    pub is_array: bool,
    pub array_num: Option<u16>,
    pub model_schema_prop_meta: Option<crate::features::model_schema_prop::ModelSchemaPropMeta>,
}

// Re-export serde types conditionally based on feature
#[cfg(feature = "serde")]
/// Re-exports from features/serde.rs - only available with "serde" feature
/// Provides metadata structures for serde attribute parsing
pub use crate::features::serde::{SerdeFieldMeta, SerdeTypeMeta};

impl FieldDef {
    /// Generates the TypeScript type name for this field.
    ///
    /// This method is the core of TypeScript type generation. It recursively builds
    /// the TS type string based on `field_type`, `is_array`, and `is_optional`.
    ///
    /// Process:
    /// 1. Match on `field_type` to get base type
    /// 2. If `is_array`, wrap in Array<...>
    /// 3. If `is_optional`, add | undefined
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
        let result = match &self.field_type {
            FieldDefType::Unknown => "unknown".to_string(),
            FieldDefType::Tuple(lst) => {
                let elements = lst
                    .iter()
                    .map(|v| format!("{}: {}", v.name, v.typescript_typename()))
                    .collect::<Vec<_>>()
                    .join("; ");
                format!("{{ {elements} }}")
            }
            FieldDefType::SiblingType(name, lst) => {
                if let Some(info) = lookup_alias_info(name) {
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
                    name.to_string()
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
                    v.typescript_typename()
                )
            }
            FieldDefType::Boolean => "boolean".to_string(),
            FieldDefType::String => "string".to_string(),
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
            | FieldDefType::Isize => "number".to_string(),
            FieldDefType::F32 | FieldDefType::F64 => "number".to_string(),
            #[cfg(feature = "object_id")]
            FieldDefType::ObjectId => crate::features::object_id::get_object_id_typescript_type(),
            #[cfg(feature = "chrono")]
            FieldDefType::NaiveDate => crate::features::chrono::get_naive_date_typescript_type(),
            #[cfg(feature = "chrono")]
            FieldDefType::NaiveTime => crate::features::chrono::get_naive_time_typescript_type(),
            #[cfg(feature = "chrono")]
            FieldDefType::NaiveDateTime => {
                crate::features::chrono::get_naive_datetime_typescript_type()
            }
            #[cfg(feature = "chrono")]
            FieldDefType::DateTime => crate::features::chrono::get_datetime_typescript_type(),
        };
        let pre_result = if self.is_array {
            format!("Array<{result}>")
        } else {
            result
        };

        if self.is_optional {
            format!("{pre_result} | undefined")
        } else {
            pre_result
        }
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
    /// - Wraps with `z.array()` if `is_array`
    /// - Adds z.union([type, `z.undefined()`]) if `is_optional` (Zod v4 syntax)
    ///
    /// Requires Zod v4 in frontend - generates v4-compatible syntax.
    /// See `notes/20250706_features.md` for Zod feature details.
    pub fn zod_type(&self) -> String {
        let result = match &self.field_type {
            FieldDefType::Unknown => "z.unknown()".to_string(),
            FieldDefType::Tuple(lst) => {
                let elements = lst
                    .iter()
                    .map(|v| format!("{}: {}", v.name, v.zod_type()))
                    .collect::<Vec<_>>()
                    .join("; ");
                format!("{{ {elements} }}")
            }
            FieldDefType::SiblingType(name, lst) => {
                if let Some(info) = lookup_alias_info(name) {
                    if lst.is_empty() {
                        // Use the exported schema name (e.g., "ContactInfo$Schema")
                        format!("{}$Schema", info.export_name)
                    } else {
                        format!(
                            "{name}<{}>",
                            lst.iter()
                                .map(Self::typescript_typename)
                                .collect::<Vec<_>>()
                                .join(", ")
                        )
                    }
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
                format!("z.record({}, {})", k.zod_type(), v.zod_type())
            }
            FieldDefType::Boolean => "z.boolean()".to_string(),
            FieldDefType::String => {
                let mut result = "z.string()".to_string();
                // Add min length validation if specified
                if let Some(ref meta) = self.model_schema_prop_meta
                    && let Some(min_len) = meta.min_length
                {
                    result = format!("{result}.min({min_len})");
                }
                // Add pattern validation if specified
                if let Some(ref meta) = self.model_schema_prop_meta
                    && let Some(ref pattern) = meta.pattern
                {
                    result = format!("{result}.check(z.regex(/{pattern}/))");
                }
                result
            }
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
            | FieldDefType::Isize => "z.number().int()".to_string(),
            FieldDefType::F32 | FieldDefType::F64 => "z.number()".to_string(),
            #[cfg(feature = "object_id")]
            FieldDefType::ObjectId => crate::features::object_id::get_object_id_zod_schema(),
            #[cfg(feature = "chrono")]
            FieldDefType::NaiveDate => crate::features::chrono::get_naive_date_zod_schema(),
            #[cfg(feature = "chrono")]
            FieldDefType::NaiveTime => crate::features::chrono::get_naive_time_zod_schema(),
            #[cfg(feature = "chrono")]
            FieldDefType::NaiveDateTime => crate::features::chrono::get_naive_datetime_zod_schema(),
            #[cfg(feature = "chrono")]
            FieldDefType::DateTime => crate::features::chrono::get_datetime_zod_schema(),
        };
        let pre_result = if self.is_array {
            format!("z.array({result})")
        } else {
            result
        };

        // Wrap with preprocess if specified
        let pre_result = if let Some(ref meta) = self.model_schema_prop_meta
            && !meta.preprocess.is_empty()
        {
            let mut wrapped = pre_result;
            for fn_name in meta.preprocess.iter().rev() {
                wrapped = format!("z.preprocess({fn_name}, {wrapped})");
            }
            wrapped
        } else {
            pre_result
        };

        if self.is_optional {
            format!("z.union([{pre_result}, z.undefined()])")
        } else {
            pre_result
        }
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
    /// - `is_array` wrappers (Vec<T>)
    /// - `is_optional` wrappers (Option<T>)
    pub fn contains_type_reference(&self, type_name: &str) -> bool {
        match &self.field_type {
            FieldDefType::SiblingType(name, generics) => {
                // Direct match (check both original name and stripped name)
                let stripped_name = crate::utils::safe_type_name(name);
                if name == type_name || stripped_name == type_name {
                    return true;
                }
                // Also check generic arguments
                generics.iter().any(|g| g.contains_type_reference(type_name))
            }
            FieldDefType::Map(k, v) => {
                k.contains_type_reference(type_name) || v.contains_type_reference(type_name)
            }
            FieldDefType::Tuple(elements) => {
                elements.iter().any(|e| e.contains_type_reference(type_name))
            }
            // Primitive types can't contain recursive references
            _ => false,
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
/// - Sets `is_array` for Vec, slices, arrays
/// - Sets `is_optional` for Option
/// - Only supports `HashMap`<String, T> (panics or errors otherwise per rules)
/// - Uses `safe_type_name()` to strip Json suffix
///
/// Called from `process_field()` in `model_schema.rs` for each struct field.
///
/// Debug logging: Set `RUST_LOG=trace` to see HashMap/SiblingType creation.
pub fn get_field_def(name: &str, ty: &Type, field_docs: &str) -> FieldDef {
    let safe_name = safe_type_name(name);
    match ty {
        Type::Path(type_path) => {
            if let Some(segment) = type_path.path.segments.last() {
                let ident = segment.ident.to_string();
                match &segment.arguments {
                    PathArguments::None => FieldDef {
                        is_optional: false,
                        name: safe_name,
                        field_type: get_field_def_type_or_sibling(&ident),
                        is_array: false,
                        array_num: None,
                        docs: field_docs.to_string(),
                        model_schema_prop_meta: None,
                    },
                    PathArguments::AngleBracketed(args) => {
                        let arg_types: Vec<FieldDef> = args
                            .args
                            .iter()
                            .filter_map(|arg| {
                                match arg {
                                    GenericArgument::Type(inner_ty) => {
                                        Some(get_field_def("", inner_ty, ""))
                                    }
                                    _ => None, // Ignore lifetimes, const generics, etc.
                                }
                            })
                            .collect();
                        if arg_types.is_empty() {
                            FieldDef {
                                is_optional: false,
                                name: safe_name,
                                field_type: FieldDefType::SiblingType(ident, vec![]),
                                is_array: false,
                                array_num: None,
                                docs: field_docs.to_string(),
                                model_schema_prop_meta: None,
                            }
                        } else if arg_types.len() == 1 && &ident == "Option" {
                            let mut result = arg_types[0].clone();
                            result.name = safe_name;
                            result.is_optional = true;
                            result
                        } else if arg_types.len() == 1 && &ident == "Vec" {
                            let mut result = arg_types[0].clone();
                            result.name = safe_name;
                            result.is_array = true;
                            result
                        } else if arg_types.len() == 2 && &ident == "HashMap" {
                            // Debug print to see what's happening
                            if std::env::var("RUST_LOG") == Ok(String::from("trace")) {
                                println!(
                                    "Creating HashMap Map type - key: {:?}, value: {:?}",
                                    arg_types[0], arg_types[1]
                                );
                            }
                            FieldDef {
                                is_array: false,
                                is_optional: false,
                                array_num: None,
                                name: safe_name,
                                field_type: FieldDefType::Map(
                                    Box::new(arg_types[0].clone()),
                                    Box::new(arg_types[1].clone()),
                                ),
                                docs: field_docs.to_string(),
                                model_schema_prop_meta: None,
                            }
                        } else if arg_types.len() == 1 && is_datetime_generic_type(&ident) {
                            // Handle DateTime<Tz> - the timezone type parameter is ignored
                            handle_datetime_generic_type(safe_name, field_docs)
                        }
                        // Fall through to SiblingType for other generic types
                        else {
                            // Debug print to see what's happening with SiblingType
                            if std::env::var("RUST_LOG") == Ok(String::from("trace")) {
                                println!(
                                    "Creating SiblingType - name: {ident}, arg_types: {arg_types:?}"
                                );
                            }
                            FieldDef {
                                is_optional: false,
                                name: safe_name,
                                field_type: FieldDefType::SiblingType(ident, arg_types),
                                is_array: false,
                                array_num: None,
                                docs: field_docs.to_string(),
                                model_schema_prop_meta: None,
                            }
                        }
                    }
                    PathArguments::Parenthesized(_) => panic!("Unsupported field type"), //format!("({})", ident), // Function pointer types
                }
            } else {
                FieldDef {
                    name: safe_name,
                    is_optional: false,
                    field_type: FieldDefType::Unknown,
                    is_array: false,
                    array_num: None,
                    docs: field_docs.to_string(),
                    model_schema_prop_meta: None,
                }
            }
        }
        Type::Reference(type_ref) => {
            // let lifetime = type_ref
            //     .lifetime
            //     .as_ref()
            //     .map_or("".to_string(), |l| format!("'{}", l.ident));
            get_field_def(name, type_ref.elem.as_ref(), field_docs)
        }
        Type::Array(type_array) => {
            let mut def = get_field_def(name, &type_array.elem, field_docs);
            def.is_array = true;
            def.array_num = None; // type_array.len;
            def
        }
        Type::Slice(type_slice) => {
            let mut def = get_field_def(name, &type_slice.elem, field_docs);
            def.is_array = true;
            def.array_num = None; // type_array.len;
            def
        }
        Type::Tuple(type_tuple) => {
            let elements: Vec<FieldDef> = type_tuple
                .elems
                .iter()
                .enumerate()
                .map(|(idx, v)| get_field_def(&format!("element_{idx}"), v, field_docs))
                .collect();
            FieldDef {
                name: safe_name,
                is_optional: false,
                field_type: FieldDefType::Tuple(elements),
                is_array: false,
                array_num: None,
                docs: field_docs.to_string(),
                model_schema_prop_meta: None,
            }
        }
        _ => FieldDef {
            name: safe_name,
            is_optional: false,
            field_type: FieldDefType::Unknown,
            is_array: false,
            array_num: None,
            docs: field_docs.to_string(),
            model_schema_prop_meta: None,
        }, // Fallback for BareFn, ImplTrait, etc.
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
        return FieldDefType::SiblingType(t_name.to_string(), vec![]);
    }
    match t_name {
        "bool" => FieldDefType::Boolean,
        "String" => FieldDefType::String,
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
            if crate::features::object_id::should_handle_as_object_id(t_name) {
                FieldDefType::ObjectId
            } else {
                FieldDefType::SiblingType(t_name.to_string(), vec![])
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
            FieldDefType::SiblingType(t_name.to_string(), vec![])
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
        type_name => FieldDefType::SiblingType(type_name.to_string(), vec![]),
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
    crate::features::serde::parse_serde_type_attributes(attrs)
}

/// Parses serde attributes from field attributes (requires "serde" feature).
///
/// Similar to `parse_serde_type_attributes` but for individual fields.
/// Handles field rename, `skip_serializing_if`, etc.
///
/// Integrates with FieldDef.name if rename present.
#[cfg(feature = "serde")]
pub fn parse_serde_field_attributes(attrs: &[Attribute]) -> SerdeFieldMeta {
    crate::features::serde::parse_serde_field_attributes(attrs)
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

/// Check if a type name is a DateTime generic type (chrono feature)
/// Returns true only when chrono feature is enabled and type is DateTime
#[cfg(feature = "chrono")]
fn is_datetime_generic_type(type_name: &str) -> bool {
    type_name == "DateTime"
}

#[cfg(not(feature = "chrono"))]
fn is_datetime_generic_type(_type_name: &str) -> bool {
    false
}

/// Handle DateTime<Tz> generic type (chrono feature)
/// Creates a FieldDef with DateTime type, ignoring the timezone parameter
#[cfg(feature = "chrono")]
fn handle_datetime_generic_type(safe_name: String, field_docs: &str) -> FieldDef {
    FieldDef {
        is_optional: false,
        name: safe_name,
        field_type: FieldDefType::DateTime,
        is_array: false,
        array_num: None,
        docs: field_docs.to_string(),
        model_schema_prop_meta: None,
    }
}

#[cfg(not(feature = "chrono"))]
fn handle_datetime_generic_type(safe_name: String, field_docs: &str) -> FieldDef {
    // Fallback - should never be called since is_datetime_generic_type returns false
    FieldDef {
        is_optional: false,
        name: safe_name,
        field_type: FieldDefType::SiblingType("DateTime".to_string(), vec![]),
        is_array: false,
        array_num: None,
        docs: field_docs.to_string(),
        model_schema_prop_meta: None,
    }
}

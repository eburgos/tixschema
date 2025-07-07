use syn::{
    Fields, GenericArgument, ItemEnum, PathArguments, Type,
};

#[cfg(feature = "serde")]
use syn::Attribute;

use crate::safe_type_name;

#[derive(Clone, Debug)]
pub(crate) enum FieldDefType {
    Unknown,
    SiblingType(String, Vec<FieldDef>),
    Map(Box<FieldDef>, Box<FieldDef>),
    Tuple(Vec<FieldDef>),
    Boolean,
    String,
    StringLiteral(String),  // For string literal types like "ProDoctivity"
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
    ObjectId,
}

#[derive(Clone, Debug)]
pub(crate) struct FieldDef {
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
pub(crate) use crate::features::serde::{SerdeTypeMeta, SerdeFieldMeta};



impl FieldDef {
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
                if lst.is_empty() {
                    name.to_string()
                } else {
                    format!(
                        "{name}<{}>",
                        lst.iter()
                            .map(|v| v.typescript_typename())
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
            FieldDefType::StringLiteral(literal) => format!("\"{}\"", literal),
            FieldDefType::U8 | FieldDefType::U16 | FieldDefType::U32 | FieldDefType::U64 
                | FieldDefType::I8 | FieldDefType::I16 | FieldDefType::I32 | FieldDefType::I64 
                | FieldDefType::Usize | FieldDefType::Isize => "number".to_string(),
            FieldDefType::F32 | FieldDefType::F64 => "number".to_string(),
            #[cfg(feature = "object_id")]
            FieldDefType::ObjectId => crate::features::object_id::get_object_id_typescript_type(),
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
                if lst.is_empty() {
                    format!("{name}$Schema")
                } else {
                    format!(
                        "{name}<{}>",
                        lst.iter()
                            .map(|v| v.typescript_typename())
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
                if let Some(ref meta) = self.model_schema_prop_meta {
                    if let Some(min_len) = meta.min_length {
                        result = format!("{}.min({})", result, min_len);
                    }
                }
                result
            },
            FieldDefType::StringLiteral(literal) => format!("z.literal(\"{}\")", literal),
            FieldDefType::U8 | FieldDefType::U16 | FieldDefType::U32 | FieldDefType::U64 
                | FieldDefType::I8 | FieldDefType::I16 | FieldDefType::I32 | FieldDefType::I64 
                | FieldDefType::Usize | FieldDefType::Isize => {
                "z.number().int()".to_string()
            }
            FieldDefType::F32 | FieldDefType::F64 => "z.number()".to_string(),
            #[cfg(feature = "object_id")]
            FieldDefType::ObjectId => crate::features::object_id::get_object_id_zod_schema(),
        };
        let pre_result = if self.is_array {
            format!("z.array({result})")
        } else {
            result
        };

        if self.is_optional {
            format!("{pre_result}.or(z.undefined())")
        } else {
            pre_result
        }
    }

    
}

pub(crate) fn get_field_def(name: &str, ty: &Type, field_docs: &str) -> FieldDef {
    let safe_name = safe_type_name(name);
    match ty {
        Type::Path(type_path) => {
            if let Some(segment) = type_path.path.segments.last() {
                let ident = segment.ident.to_string();
                match &segment.arguments {
                    PathArguments::None => FieldDef {
                        is_optional: false,
                        name: safe_name,
                        field_type: get_field_def_type_or_sibling(&ident.to_string()),
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
                                field_type: FieldDefType::SiblingType(ident.to_string(), vec![]),
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
                                println!("Creating HashMap Map type - key: {:?}, value: {:?}", arg_types[0], arg_types[1]);
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
                        } else {
                            // Debug print to see what's happening with SiblingType
                            if std::env::var("RUST_LOG") == Ok(String::from("trace")) {
                                println!("Creating SiblingType - name: {ident}, arg_types: {arg_types:?}");
                            }
                            FieldDef {
                                is_optional: false,
                                name: safe_name,
                                field_type: FieldDefType::SiblingType(ident.to_string(), arg_types),
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

fn get_field_def_type_or_sibling(t_name: &str) -> FieldDefType {
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
            eprintln!("         ObjectId will be treated as a custom type (may cause compilation errors)");
            eprintln!("         Enable the object_id feature: features = [\"object_id\"]");
            eprintln!("         Or add the required ObjectId type definition to your code");
            FieldDefType::SiblingType(t_name.to_string(), vec![])
        }
        type_name_json if type_name_json.ends_with("Json") => {
            FieldDefType::SiblingType(safe_type_name(type_name_json), vec![])
        }
        type_name => FieldDefType::SiblingType(type_name.to_string(), vec![]),
    }
}

/// Parses serde attributes from a struct or enum.
#[cfg(feature = "serde")]
pub(crate) fn parse_serde_type_attributes(attrs: &[Attribute]) -> SerdeTypeMeta {
    crate::features::serde::parse_serde_type_attributes(attrs)
}

/// Parses serde attributes from a field.
#[cfg(feature = "serde")]
pub(crate) fn parse_serde_field_attributes(attrs: &[Attribute]) -> SerdeFieldMeta {
    crate::features::serde::parse_serde_field_attributes(attrs)
}

pub(crate) fn is_plain_enum(item_enum: &ItemEnum) -> bool {
    item_enum
        .variants
        .iter()
        .all(|variant| matches!(variant.fields, Fields::Unit))
}

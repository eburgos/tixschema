use proc_macro2::TokenTree;
use syn::{
    Attribute, Fields, GenericArgument, ItemEnum, LitStr, Meta, MetaList, PathArguments, Type,
};

use crate::safe_type_name;

#[derive(Clone, Debug)]
pub(crate) enum FieldDefType {
    Unknown,
    SiblingType(String, Vec<FieldDef>),
    Map(Box<FieldDef>, Box<FieldDef>),
    Tuple(Vec<FieldDef>),
    Boolean,
    String,
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
}

#[derive(Clone, Debug)]
pub(crate) struct FieldDef {
    pub is_optional: bool,
    pub name: String,
    pub docs: String,
    pub field_type: FieldDefType,
    pub is_array: bool,
    pub array_num: Option<u16>,
}

/// Metadata for serde attributes applied to a struct or enum.
pub(crate) struct SerdeTypeMeta {
    pub tag: Option<String>,        // e.g., "behaviorType"
    pub rename_all: Option<String>, // e.g., "camelCase"
}

/// Metadata for serde attributes applied to a field.
pub(crate) struct SerdeFieldMeta {
    pub rename: Option<String>, // e.g., "new_name"
    pub skip: bool,             // Whether to skip the field
}

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
            FieldDefType::U8 | FieldDefType::U16 | FieldDefType::U32 | FieldDefType::U64 
                | FieldDefType::I8 | FieldDefType::I16 | FieldDefType::I32 | FieldDefType::I64 
                | FieldDefType::Usize | FieldDefType::Isize => "number".to_string(),
            FieldDefType::F32 | FieldDefType::F64 => "number".to_string(),
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
            FieldDefType::String => "z.string()".to_string(),
            FieldDefType::U8 | FieldDefType::U16 | FieldDefType::U32 | FieldDefType::U64 
                | FieldDefType::I8 | FieldDefType::I16 | FieldDefType::I32 | FieldDefType::I64 
                | FieldDefType::Usize | FieldDefType::Isize => {
                "z.number().int()".to_string()
            }
            FieldDefType::F32 | FieldDefType::F64 => "z.number()".to_string(),
        };
        let pre_result = if self.is_array {
            format!("z.array({result})")
        } else {
            result
        };

        if self.is_optional {
            format!("{pre_result}.optional()")
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
                            }
                        } else {
                            // Debug print to see what's happening with SiblingType
                            if std::env::var("RUST_LOG") == Ok(String::from("trace")) {
                                println!("Creating SiblingType - name: {}, arg_types: {:?}", ident, arg_types);
                            }
                            FieldDef {
                                is_optional: false,
                                name: safe_name,
                                field_type: FieldDefType::SiblingType(ident.to_string(), arg_types),
                                is_array: false,
                                array_num: None,
                                docs: field_docs.to_string(),
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
            }
        }
        _ => FieldDef {
            name: safe_name,
            is_optional: false,
            field_type: FieldDefType::Unknown,
            is_array: false,
            array_num: None,
            docs: field_docs.to_string(),
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
        type_name_json if type_name_json.ends_with("Json") => {
            FieldDefType::SiblingType(safe_type_name(type_name_json), vec![])
        }
        type_name => FieldDefType::SiblingType(type_name.to_string(), vec![]),
    }
}

/// Parses serde attributes from a struct or enum.
pub(crate) fn parse_serde_type_attributes(attrs: &[Attribute]) -> SerdeTypeMeta {
    let mut meta = SerdeTypeMeta {
        tag: None,
        rename_all: None,
    };

    for attr in attrs {
        if attr.path().is_ident("serde") {
            attr.parse_nested_meta(|nested| {
                // Handle `tag = "value"`
                if nested.path.is_ident("tag") {
                    let value = nested.value()?;
                    let lit: LitStr = value.parse()?;
                    meta.tag = Some(lit.value());
                }
                // Handle `rename_all = "value"`
                else if nested.path.is_ident("rename_all") {
                    let value = nested.value()?;
                    let lit: LitStr = value.parse()?;
                    meta.rename_all = Some(lit.value());
                }
                Ok(())
            })
            .unwrap_or_else(|e| {
                log::error!("Failed to parse serde type attribute: {e}");
            });
        }
    }

    meta
}
/// Parses serde attributes from a field.
pub fn parse_serde_field_attributes(attrs: &[Attribute]) -> SerdeFieldMeta {
    let mut meta = SerdeFieldMeta {
        rename: None,
        skip: false,
    };

    for attr in attrs {
        if attr.path().is_ident("serde") {
            match &attr.meta {
                Meta::List(MetaList {
                    path: _path,
                    delimiter: _delimiter,
                    tokens,
                }) => {
                    /*
                    meta_list: Path { leading_colon: None, segments: [PathSegment { ident: Ident { ident: "serde", span: #0 bytes(34250..34255) }, arguments: PathArguments::None }] } - MacroDelimiter::Paren(Paren) -
                    TokenStream [
                        Ident { ident: "rename", span: #0 bytes(34256..34262) },
                        Punct { ch: '=', spacing: Alone, span: #0 bytes(34263..34264) },
                        Literal { kind: Str, symbol: "trialPeriodDays", suffix: None, span: #0 bytes(34265..34282) }, Punct { ch: ',', spacing: Alone, span: #0 bytes(34282..34283)
                        },
                        Ident { ident: "skip_serializing_if", span: #0 bytes(34284..34303) }, Punct { ch: '=', spacing: Alone, span: #0 bytes(34304..34305) }, Literal { kind: Str, symbol: "Option::is_none", suffix: None, span: #0 bytes(34306..34323) }]
                     */

                    let tokens_vec: Vec<TokenTree> = {
                        let mut vec = Vec::new();
                        for token in tokens.clone() {
                            vec.push(token);
                        }
                        vec
                    };
                    let mut t = 0;
                    let len = tokens_vec.len();
                    while t < len {
                        match &tokens_vec[t] {
                            TokenTree::Ident(ident) if *ident == "rename" => {
                                if t + 2 < len {
                                    if let TokenTree::Literal(lit) = &tokens_vec[t + 2] {
                                        let lit_str = lit.to_string();
                                        if lit_str.starts_with("\"") && lit_str.ends_with("\"") {
                                            meta.rename =
                                                Some(lit_str[1..lit_str.len() - 1].to_string());
                                        }
                                    }
                                    t += 3;
                                } else {
                                    t += 1;
                                }
                            }
                            TokenTree::Ident(ident) if *ident == "skip" => {
                                if t + 2 < len {
                                    if let TokenTree::Ident(lit) = &tokens_vec[t + 2]
                                        && *lit == "true"
                                    {
                                        meta.skip = true;
                                    }
                                    t += 3;
                                } else {
                                    t += 1;
                                }
                            }
                            _ => {
                                t += 1;
                            }
                        }
                    }
                }
                Meta::NameValue(meta_name_value) => {
                    log::error!("meta_name_value: {meta_name_value:?}");
                }
                _ => {}
            };
        }
    }

    meta
}

pub(crate) fn is_plain_enum(item_enum: &ItemEnum) -> bool {
    item_enum
        .variants
        .iter()
        .all(|variant| matches!(variant.fields, Fields::Unit))
}

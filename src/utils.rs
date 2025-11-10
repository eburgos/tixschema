use core::cell::RefCell;
use std::collections::HashMap;
use syn::{Attribute, Expr, Field, Lit, Meta, Variant};

#[cfg(any(feature = "typescript", feature = "zod"))]
use syn::{ItemEnum, ItemStruct};

#[derive(Clone)]
pub struct AliasInfo {
    pub export_name: String,
    pub module_name: String,
}

thread_local! {
    static ALIAS_INFO: RefCell<HashMap<String, AliasInfo>> = RefCell::new(HashMap::new());
}

pub fn register_alias_info(rust_ident: &str, export_name: &str, module_name: String) {
    ALIAS_INFO.with(|map| {
        map.borrow_mut().insert(
            rust_ident.to_string(),
            AliasInfo {
                export_name: export_name.to_string(),
                module_name,
            },
        );
    });
}

pub fn lookup_alias_info(rust_ident: &str) -> Option<AliasInfo> {
    ALIAS_INFO.with(|map| map.borrow().get(rust_ident).cloned())
}

pub fn safe_type_name(key: &str) -> String {
    if key.ends_with("Json") {
        key.strip_suffix("Json")
            .map(ToString::to_string)
            .expect("Failed to strip Json suffix")
    } else {
        key.to_string()
    }
}

pub fn compute_alias_export_name(rust_ident: &str, override_name: Option<String>) -> String {
    match override_name {
        Some(name) if name.trim().is_empty() => format!("{rust_ident}Type"),
        Some(name) if name == rust_ident => format!("{rust_ident}Type"),
        Some(name) => name,
        None => format!("{}Type", safe_type_name(rust_ident)),
    }
}

#[cfg(any(feature = "typescript", feature = "zod"))]
pub fn format_docs_for_ts(docs: &[String], fallback_name: &str) -> String {
    if docs.is_empty() {
        format!(" * {fallback_name}\n * ")
    } else {
        docs.iter()
            .map(|line| format!(" * {line}"))
            .chain(core::iter::once(" * ".to_string()))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[cfg(any(feature = "typescript", feature = "zod"))]
/// Extracts and concatenates documentation comments from a `syn::ItemStruct`.
///
/// # Arguments
///
/// * `item_struct` - A reference to the `syn::ItemStruct` to process.
///
/// # Returns
///
/// An `Option<String>` containing the concatenated documentation,
/// or `None` if no doc comments are found. Returns an empty string
/// if doc comments exist but are empty.
pub fn get_struct_docs(item_struct: &ItemStruct) -> Option<Vec<String>> {
    collect_doc_lines(&item_struct.attrs)
}

#[cfg(any(feature = "typescript", feature = "zod"))]
pub fn get_enum_docs(item_enum: &ItemEnum) -> Option<Vec<String>> {
    collect_doc_lines(&item_enum.attrs)
}

pub fn get_variant_docs(variant: &Variant) -> Option<Vec<String>> {
    collect_doc_lines(&variant.attrs)
}

pub fn get_field_docs(field: &Field) -> Option<Vec<String>> {
    collect_doc_lines(&field.attrs)
}

#[cfg(any(feature = "typescript", feature = "zod"))]
pub fn get_item_docs(attrs: &[Attribute]) -> Option<Vec<String>> {
    collect_doc_lines(attrs)
}

fn collect_doc_lines(attrs: &[Attribute]) -> Option<Vec<String>> {
    let mut doc_lines = Vec::new();

    for attr in attrs {
        if attr.path().is_ident("doc")
            && let Meta::NameValue(meta_name_value) = &attr.meta
            && let Expr::Lit(syn::ExprLit {
                lit: Lit::Str(lit_str),
                ..
            }) = &meta_name_value.value
        {
            doc_lines.push(lit_str.value().trim().to_string());
        }
    }

    if doc_lines.is_empty() {
        None
    } else {
        Some(doc_lines)
    }
}

pub fn to_snake_case(name: &str) -> String {
    let mut result = String::new();
    let mut prev_lower = false;
    for ch in name.chars() {
        if ch.is_ascii_uppercase() {
            if prev_lower {
                result.push('_');
            }
            result.push(ch.to_ascii_lowercase());
            prev_lower = true;
        } else {
            result.push(ch);
            prev_lower = ch.is_ascii_lowercase();
        }
    }
    result
}

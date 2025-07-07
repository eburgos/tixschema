use syn::{Expr, Field, Lit, Meta, Variant};

#[cfg(feature = "typescript")]
use syn::{ItemStruct, ItemEnum};

pub fn safe_type_name(key: &str) -> String {
    if key.ends_with("Json") {
        key.strip_suffix("Json")
            .map_or_else(|| panic!("Error stripping Json"), ToString::to_string)
    } else {
        key.to_string()
    }
}


#[cfg(feature = "typescript")]
/// Extracts and concatenates documentation comments from a syn::ItemStruct.
///
/// # Arguments
///
/// * `item_struct` - A reference to the syn::ItemStruct to process.
///
/// # Returns
///
/// An `Option<String>` containing the concatenated documentation,
/// or `None` if no doc comments are found. Returns an empty string
/// if doc comments exist but are empty.
pub(crate) fn get_struct_docs(item_struct: &ItemStruct) -> Option<Vec<String>> {
    let mut doc_lines = Vec::new();

    // 1. Iterate through attributes
    for attr in &item_struct.attrs {
        // 2. Filter for `#[doc = ...]` attributes
        if attr.path().is_ident("doc") {
            // 3. Extract the string literal
            if let Meta::NameValue(meta_name_value) = &attr.meta
                && let Expr::Lit(syn::ExprLit {
                    lit: Lit::Str(lit_str),
                    ..
                }) = &meta_name_value.value
            {
                // Trim leading/trailing whitespace common in doc comments
                doc_lines.push(lit_str.value().trim().to_string());
            }
        }
    }

    // 4. Combine lines if any were found
    if doc_lines.is_empty() {
        None // No doc comments found
    } else {
        Some(doc_lines) // Join lines with newline characters
    }
}

#[cfg(feature = "typescript")]
pub(crate) fn get_enum_docs(item_enum: &ItemEnum) -> Option<Vec<String>> {
    let mut doc_lines = Vec::new();

    // 1. Iterate through attributes
    for attr in &item_enum.attrs {
        // 2. Filter for `#[doc = ...]` attributes
        if attr.path().is_ident("doc") {
            // 3. Extract the string literal
            if let Meta::NameValue(meta_name_value) = &attr.meta
                && let Expr::Lit(syn::ExprLit {
                    lit: Lit::Str(lit_str),
                    ..
                }) = &meta_name_value.value
            {
                // Trim leading/trailing whitespace common in doc comments
                doc_lines.push(lit_str.value().trim().to_string());
            }
        }
    }

    // 4. Combine lines if any were found
    if doc_lines.is_empty() {
        None // No doc comments found
    } else {
        Some(doc_lines) // Join lines with newline characters
    }
}

pub(crate) fn get_variant_docs(variant: &Variant) -> Option<Vec<String>> {
    let mut doc_lines = Vec::new();

    for attr in &variant.attrs {
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
        None // No doc comments found
    } else {
        Some(doc_lines) // Join lines with newline characters
    }
}

pub(crate) fn get_field_docs(field: &Field) -> Option<Vec<String>> {
    let mut doc_lines = Vec::new();

    for attr in &field.attrs {
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
        None // No doc comments found
    } else {
        Some(doc_lines) // Join lines with newline characters
    }
}

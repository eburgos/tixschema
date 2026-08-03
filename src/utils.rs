use core::cell::RefCell;
#[cfg(feature = "typescript")]
use core::iter;
use std::collections::HashMap;
use syn::{Attribute, Expr, Field, Lit, Meta, Variant};

#[cfg(any(feature = "typescript", feature = "zod"))]
use syn::{ItemEnum, ItemStruct};

#[derive(Clone)]
pub struct AliasInfo {
    pub export_name: String,
    #[cfg(feature = "jsonschema")]
    pub module_name: String,
}

thread_local! {
    static ALIAS_INFO: RefCell<HashMap<String, AliasInfo>> = RefCell::new(HashMap::new());
}

#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
pub fn register_alias_info(rust_ident: &str, export_name: &str, module_name: &str) {
    #[cfg(not(feature = "jsonschema"))]
    let _: &_ = &module_name;
    ALIAS_INFO.with(|map| {
        map.borrow_mut().insert(
            rust_ident.to_owned(),
            AliasInfo {
                export_name: export_name.to_owned(),
                #[cfg(feature = "jsonschema")]
                module_name: module_name.to_owned(),
            },
        );
    });
}

pub fn lookup_alias_info(rust_ident: &str) -> Option<AliasInfo> {
    ALIAS_INFO.with(|map| map.borrow().get(rust_ident).cloned())
}

pub fn safe_type_name(key: &str) -> String {
    if key.ends_with("Json") {
        key.strip_suffix("Json").map(str::to_owned).unwrap()
    } else {
        key.to_owned()
    }
}

/// The export name is what `register_alias_info` stores and what the alias schema module is
/// named after, so every feature that references an alias needs it — not just `typescript`.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
pub fn compute_alias_export_name(rust_ident: &str, override_name: Option<String>) -> String {
    match override_name {
        Some(name) if name.trim().is_empty() => format!("{rust_ident}Type"),
        Some(name) => name,
        None => format!("{}Type", safe_type_name(rust_ident)),
    }
}

#[cfg(feature = "typescript")]
pub fn format_docs_for_ts(docs: &[String], fallback_name: &str) -> String {
    if docs.is_empty() {
        format!(" * {fallback_name}\n * ")
    } else {
        docs.iter()
            .map(|line| format!(" * {line}"))
            .chain(iter::once(" * ".to_owned()))
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

#[cfg(feature = "typescript")]
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
            let value = lit_str.value();
            // Split on newlines to handle block comments (/** */)
            // which may come as a single string with embedded \n
            for line in value.lines() {
                doc_lines.push(line.trim().to_owned());
            }
        }
    }

    if doc_lines.is_empty() {
        None
    } else {
        Some(doc_lines)
    }
}

#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
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

/// Extracts the first Rust code example from documentation comments.
///
/// Looks for a code fence with the format ` ```rust example` and extracts
/// all code until the closing ` ``` `. If multiple examples are found,
/// only the first one is returned.
///
/// # Arguments
///
/// * `docs` - A slice of documentation comment lines.
///
/// # Returns
///
/// An `Option<String>` containing the example code if found, or `None` if
/// no example fence is present.
#[cfg(feature = "zod")]
pub fn extract_example_from_docs(docs: &[String]) -> Option<String> {
    let mut in_example_block = false;
    let mut example_lines = Vec::new();

    for line in docs {
        let trimmed = line.trim();
        // Strip leading asterisk from block-style comments
        let cleaned = trimmed.strip_prefix('*').unwrap_or(trimmed).trim();

        // Check for opening fence
        if cleaned == "```rust example" {
            if !example_lines.is_empty() {
                // Already found one example, return it
                break;
            }
            in_example_block = true;
            continue;
        }

        // Check for closing fence
        if in_example_block && cleaned == "```" {
            // Found complete example
            break;
        }

        // Collect example lines
        if in_example_block {
            example_lines.push(line.clone());
        }
    }

    if example_lines.is_empty() {
        None
    } else {
        // Apply regex transformations to make doctest-compatible code work for schema_example
        let code = example_lines.join("\n");
        Some(transform_example_code(&code))
    }
}

/// Transforms doctest-compatible example code to be suitable for `schema_example()`.
///
/// Applies regex transformations to convert code that returns () (for doctest)
/// into code that returns the actual value (for schema serialization).
///
/// Current transformations:
/// - Strips `use` statements (type is already in scope in impl block)
/// - `println!("...", value);` → `value`
/// - `let _: Type = value;` → `value`
#[cfg(feature = "zod")]
fn transform_example_code(code: &str) -> String {
    let mut result = code.to_owned();

    // Pattern 0: Strip use statements
    // Remove lines starting with "use " (they're not needed in the impl block context)
    let re_use = regex::Regex::new(r"(?m)^\s*use\s+[^;]+;\s*\n?").unwrap();
    result = re_use.replace_all(&result, "").to_string();

    // Pattern 1: println!("...", variable); → variable
    // Matches: println!("anything", value); or println!("format {}", value);
    let re = regex::Regex::new(r"println!\s*\([^,)]+,\s*([^)]+)\)\s*;?\s*$").unwrap();
    if let Some(captures) = re.captures(&result)
        && let Some(variable) = captures.get(1)
    {
        result = re.replace(&result, variable.as_str()).to_string();
    }

    // Pattern 2: let _: Type = value; → value
    // Matches: let _: SomeType = value; or let _ = value;
    let re2 = regex::Regex::new(r"let\s+_(?:\s*:\s*[^=]+)?\s*=\s*([^;]+)\s*;?\s*$").unwrap();
    if let Some(captures) = re2.captures(&result)
        && let Some(variable) = captures.get(1)
    {
        result = re2.replace(&result, variable.as_str()).to_string();
    }

    result.trim().to_owned()
}

/// Strips example code blocks from documentation lines.
///
/// This is used for descriptions to avoid including example code in the description field.
#[cfg(any(feature = "typescript", feature = "zod"))]
pub fn strip_examples_from_docs(docs: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    let mut in_example_block = false;

    for line in docs {
        let trimmed = line.trim();
        // Strip leading asterisk from block-style comments
        let cleaned = trimmed.strip_prefix('*').unwrap_or(trimmed).trim();

        // Check for opening fence
        if cleaned == "```rust example" {
            in_example_block = true;
            continue;
        }

        // Check for closing fence
        if in_example_block && cleaned == "```" {
            in_example_block = false;
            continue;
        }

        // Skip lines inside example blocks
        if in_example_block {
            continue;
        }

        result.push(line.clone());
    }

    result
}

#[cfg(test)]
#[cfg(any(feature = "typescript", feature = "zod"))]
mod tests;

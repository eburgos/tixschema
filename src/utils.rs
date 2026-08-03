use core::cell::RefCell;
#[cfg(feature = "typescript")]
use core::iter;
use std::collections::HashMap;
use syn::{Attribute, Expr, Field, Lit, LitStr, Meta, Variant};

#[cfg(any(feature = "typescript", feature = "zod"))]
use syn::{ItemEnum, ItemStruct};

/// Whether a registered Rust ident, *written as a type path*, resolves to something carrying an
/// inherent `enum_members()` — the enumeration the JSON-schema map-key expansion calls. Only a
/// plain unit enum gets that method, and a type path sees straight through an alias, so an alias
/// answers for whatever it targets rather than for itself.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AliasKind {
    /// A plain unit enum, or an alias chain ending in one.
    EnumMembers,
    /// Provably has none: a struct, a branded newtype, a non-plain enum, or an alias whose target
    /// is a primitive, a collection, or one of those.
    NoEnumMembers,
    /// Undecidable at this expansion — an alias naming a type that was not registered before it.
    Unknown,
}

#[derive(Clone)]
pub struct AliasInfo {
    pub export_name: String,
    #[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
    pub kind: AliasKind,
    #[cfg(feature = "jsonschema")]
    pub module_name: String,
}

thread_local! {
    static ALIAS_INFO: RefCell<HashMap<String, AliasInfo>> = RefCell::new(HashMap::new());
}

#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
pub fn register_alias_info(
    rust_ident: &str,
    export_name: &str,
    module_name: &str,
    kind: AliasKind,
) {
    #[cfg(not(feature = "jsonschema"))]
    let _: &_ = &module_name;
    ALIAS_INFO.with(|map| {
        map.borrow_mut().insert(
            rust_ident.to_owned(),
            AliasInfo {
                export_name: export_name.to_owned(),
                kind,
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

#[cfg(any(
    feature = "typescript",
    feature = "zod",
    feature = "jsonschema",
    feature = "serde"
))]
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

/// The escape body a JavaScript regex literal spells a line terminator with, i.e. what follows
/// the backslash. The literal grammar excludes a raw line terminator outright, both on its own
/// and as the character a backslash escapes, so these are the only spellings available.
#[cfg(feature = "zod")]
const fn js_line_terminator_escape(ch: char) -> Option<&'static str> {
    match ch {
        '\n' => Some("n"),
        '\r' => Some("r"),
        '\u{2028}' => Some("u2028"),
        '\u{2029}' => Some("u2029"),
        _ => None,
    }
}

/// The `regex` crate's own rejection of a `pattern` attribute value, spanned on the literal the
/// author wrote, or `None` when the pattern parses.
///
/// Both splice points hand the string to `regex::Regex::new(...).unwrap()` inside the generated
/// validator, so a pattern that does not parse here is a panic at the first validation there. The
/// macro holds the string and links `regex`, so the parse happens at expansion instead.
pub fn regex_rejection(lit: &LitStr) -> Option<syn::Error> {
    regex::Regex::new(&lit.value())
        .err()
        .map(|err| syn::Error::new_spanned(lit, err))
}

/// Escapes a regex pattern for splicing between the `/` delimiters of a JavaScript regex literal.
///
/// The pattern is already a regex, so what needs work is what the literal syntax alone gives a
/// meaning to: the `/` delimiter, which becomes `\/`, and a raw line terminator, which the literal
/// cannot carry at all and so becomes its escape. A backslash escape is consumed whole, which keeps
/// an authored `\/` from gaining a second backslash and keeps a literal `\\` from being read as the
/// escape for the `/` that follows it. A backslash before a raw line terminator is an identity
/// escape, and the escape form denotes that same character.
#[cfg(feature = "zod")]
pub fn escape_js_regex_literal(pattern: &str) -> String {
    let mut result = String::with_capacity(pattern.len());
    let mut chars = pattern.chars();
    while let Some(ch) = chars.next() {
        match ch {
            '\\' => {
                result.push('\\');
                if let Some(escaped) = chars.next() {
                    match js_line_terminator_escape(escaped) {
                        Some(escape) => result.push_str(escape),
                        None => result.push(escaped),
                    }
                }
            }
            '/' => result.push_str("\\/"),
            _ => match js_line_terminator_escape(ch) {
                Some(escape) => {
                    result.push('\\');
                    result.push_str(escape);
                }
                None => result.push(ch),
            },
        }
    }
    result
}

#[cfg(test)]
#[cfg(any(feature = "typescript", feature = "zod"))]
mod tests;

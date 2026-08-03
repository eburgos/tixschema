//! Model schema property feature module
//!
//! This module handles parsing of model_schema_prop attributes for field-level customization
//! of TypeScript type and Zod schema generation.

use syn::meta::ParseNestedMeta;
use syn::{Attribute, LitStr, Type};

use crate::utils::regex_rejection;

/// Every key the parser reads, in the order it tries them, as the unknown-key rejection names them.
///
/// A key added to [`parse_prop_key`] belongs here too: `no_key_the_parser_reads_is_rejected` walks
/// this list back through the parser and fails on any name here it does not read.
const KNOWN_KEYS: &[&str] = &[
    "as",
    "literal",
    "minLength",
    "maxLength",
    "minimum",
    "maximum",
    "pattern",
    "preprocess",
    "ts_optional",
    "as_number",
];

/// Metadata for `model_schema_prop` attributes applied to a field.
///
/// # Supported attributes
///
/// ## String constraints
///
/// - `pattern = "regex"` — validates the string matches the regex pattern.
///   - Zod: `.check(z.regex(/regex/))`
///   - JSON Schema: `"pattern"`
///   - Rust: auto-generates a `deserialize_{field}` serde hook and a `validate_{field}_value()` static function
///
/// - `minLength = N` — minimum string length (inclusive).
///   - Zod: `.min(N)`
///   - JSON Schema: `"minLength"`
///   - Rust: serde validator
///
/// - `maxLength = N` — maximum string length (inclusive).
///   - Zod: `.max(N)`
///   - JSON Schema: `"maxLength"`
///   - Rust: serde validator
///
/// ## Numeric constraints
///
/// - `minimum = N` — minimum value for numeric fields (integer or float).
///   - Zod: `.min(N)`
///   - JSON Schema: `"minimum"`
///   - Rust: serde validator
///
/// - `maximum = N` — maximum value for numeric fields (integer or float).
///   - Zod: `.max(N)`
///   - JSON Schema: `"maximum"`
///   - Rust: serde validator
///
/// ## Type overrides
///
/// - `as = Type` — override the TypeScript/Zod type emitted for this field.
/// - `literal = "value"` — emit as a string literal type instead of `string`.
/// - `ts_optional` — for an `Option<T>` field, emit `field?: T` instead of `field: T | undefined` (TypeScript only; a non-`Option` field is a compile error).
///
/// ## Zod preprocessing
///
/// - `preprocess = ["fn1", "fn2"]` — wrap the Zod schema with `z.preprocess()` calls (Zod-only, no Rust-side effect).
///   Multiple functions are nested: `z.preprocess(fn1, z.preprocess(fn2, innerSchema))`.
///
/// # Example
///
/// ```rust
/// use tixschema::{model_schema, model_schema_prop};
/// use serde::{Deserialize, Serialize};
///
/// #[model_schema()]
/// #[derive(Serialize, Deserialize)]
/// pub struct User {
///     #[model_schema_prop(minLength = 3, maxLength = 50, pattern = "^[a-z]+$")]
///     pub username: String,
///
///     #[model_schema_prop(minimum = 0, maximum = 120)]
///     pub age: u32,
///
///     #[model_schema_prop(pattern = "^[0-9a-fA-F]{24}$")]
///     pub id: String,
/// }
/// ```
#[derive(Clone, Debug, Default)]
pub struct ModelSchemaPropMeta {
    pub as_number: bool, // DateTime<Tz>: epoch-number + codegen coercer instead of the native Date default
    pub as_type: Option<String>, // e.g., "String" from as = String
    /// The parser's refusal of the attribute — a key it does not read, or a value it cannot read —
    /// spanned on the tokens that earned it.
    pub attr_rejection: Option<syn::Error>,
    pub literal: Option<String>, // e.g., "Tixena" from literal = "Tixena"
    pub max_length: Option<usize>, // e.g., 50 from maxLength = 50
    pub maximum: Option<f64>,    // e.g., 100.0 from maximum = 100
    pub min_length: Option<usize>, // e.g., 1 from minLength = 1
    pub minimum: Option<f64>,    // e.g., 0.0 from minimum = 0
    pub pattern: Option<String>, // e.g., "^[0-9a-fA-F]{24}$" from pattern = "^[0-9a-fA-F]{24}$"
    /// The `regex` crate's rejection of `pattern`, spanned on the literal it was written as.
    pub pattern_rejection: Option<syn::Error>,
    pub preprocess: Vec<String>, // e.g., ["epoch_to_date", "trim"] from preprocess = ["epoch_to_date", "trim"]
    pub ts_optional: bool,
}

/// Parses `model_schema_prop` attributes from a field.
///
/// What the parser cannot read is recorded as [`ModelSchemaPropMeta::attr_rejection`] rather than
/// dropped: this attribute is read here and nowhere else, so a key or value that stops at this
/// parser reaches no emitter, and the field it was written to constrain is emitted as though the
/// attribute had been left off.
pub fn parse_model_schema_prop_attributes(attrs: &[Attribute]) -> ModelSchemaPropMeta {
    let mut meta = ModelSchemaPropMeta::default();

    for attr in attrs {
        if attr.path().is_ident("model_schema_prop")
            && let Err(rejection) =
                attr.parse_nested_meta(|nested| parse_prop_key(&nested, &mut meta))
        {
            meta.attr_rejection.get_or_insert(rejection);
        }
    }

    meta
}

/// Reads one `key` or `key = value` of a `model_schema_prop` attribute into `meta`.
fn parse_prop_key(nested: &ParseNestedMeta, meta: &mut ModelSchemaPropMeta) -> syn::Result<()> {
    if nested.path.is_ident("as") {
        let ty: Type = nested.value()?.parse()?;
        meta.as_type = Some(quote::quote!(#ty).to_string());
    } else if nested.path.is_ident("literal") {
        let lit: LitStr = nested.value()?.parse()?;
        meta.literal = Some(lit.value());
    } else if nested.path.is_ident("minLength") {
        meta.min_length = Some(nested.value()?.parse::<syn::LitInt>()?.base10_parse()?);
    } else if nested.path.is_ident("maxLength") {
        meta.max_length = Some(nested.value()?.parse::<syn::LitInt>()?.base10_parse()?);
    } else if nested.path.is_ident("minimum") {
        meta.minimum = Some(numeric_bound(nested, "minimum")?);
    } else if nested.path.is_ident("maximum") {
        meta.maximum = Some(numeric_bound(nested, "maximum")?);
    } else if nested.path.is_ident("pattern") {
        let lit: LitStr = nested.value()?.parse()?;
        meta.pattern_rejection = regex_rejection(&lit);
        meta.pattern = Some(lit.value());
    } else if nested.path.is_ident("preprocess") {
        let arr: syn::ExprArray = nested.value()?.parse()?;
        meta.preprocess = arr
            .elems
            .iter()
            .map(preprocess_fn_name)
            .collect::<syn::Result<_>>()?;
    } else if nested.path.is_ident("ts_optional") {
        meta.ts_optional = true;
    } else if nested.path.is_ident("as_number") {
        meta.as_number = true;
    } else {
        return Err(unknown_key_rejection(nested));
    }
    Ok(())
}

/// The `f64` a numeric bound was written as.
///
/// Both bounds reach a numeric comparison in the Rust validator and a numeric literal in the Zod
/// and JSON schemas, so a value that is not a number is one no surface can carry.
fn numeric_bound(nested: &ParseNestedMeta, key: &str) -> syn::Result<f64> {
    let lit: syn::Lit = nested.value()?.parse()?;
    if let syn::Lit::Int(int) = &lit {
        int.base10_parse()
    } else if let syn::Lit::Float(float) = &lit {
        float.base10_parse()
    } else {
        Err(syn::Error::new_spanned(
            &lit,
            format!("`model_schema_prop` key `{key}` takes an integer or float literal"),
        ))
    }
}

/// The name one `preprocess` element carries: the function is spliced into the emitted Zod schema
/// by name, so a string literal is the only element that names one.
fn preprocess_fn_name(elem: &syn::Expr) -> syn::Result<String> {
    if let syn::Expr::Lit(expr_lit) = elem
        && let syn::Lit::Str(name) = &expr_lit.lit
    {
        Ok(name.value())
    } else {
        Err(syn::Error::new_spanned(
            elem,
            "`model_schema_prop` key `preprocess` takes an array of string literals, each naming a \
             function to wrap the Zod schema with",
        ))
    }
}

/// Rejects a key the parser does not read, spanned on the name as written.
fn unknown_key_rejection(nested: &ParseNestedMeta) -> syn::Error {
    let path = &nested.path;
    nested.error(format!(
        "unknown `model_schema_prop` key `{}`. This attribute is this crate's own, so a key it \
         does not read reaches no emitter: the field would be written as though the key had been \
         left off, unconstrained on every surface. Valid keys: {}",
        quote::quote!(#path),
        KNOWN_KEYS.join(", ")
    ))
}

#[cfg(test)]
mod tests;

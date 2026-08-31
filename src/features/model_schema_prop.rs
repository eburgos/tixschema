//! Model schema property feature module
//!
//! This module handles parsing of model_schema_prop attributes for field-level customization
//! of TypeScript type and Zod schema generation.
//!
//! The attribute sits on a named field or a tuple slot, of a struct or of an enum variant. The one
//! position it may not sit on is the slot of a `#[serde(transparent)]` single-field tuple struct: a
//! brand publishes its inner's own schema, so no key written there reaches a surface, and one
//! written there is refused at expansion.

use syn::meta::ParseNestedMeta;
use syn::{Attribute, Lit, LitStr, Type};

use crate::utils::{constraining_pattern, emittable_pattern, portable_pattern};

/// Every key the parser reads, in the order the unknown-key rejection names them. Add a new key to
/// [`parse_prop_key`], add it here too, or `no_key_the_parser_reads_is_rejected` fails.
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
    "nullable",
];

/// The value a `literal` key was written with, kept in the kind it was written as. The kind decides
/// which [`crate::field_type::FieldDefType`] the field collapses to and which Rust type may carry
/// it.
#[derive(Clone, Debug, PartialEq)]
pub enum LiteralValue {
    Bool(bool),
    Number(f64),
    Str(String),
}

/// Metadata for `model_schema_prop` attributes applied to a field.
///
/// # Supported attributes
///
/// ## String constraints
///
/// Each applies to a field that renders a plain string — `String`, `str`, `PathBuf`, `Path`, and
/// those under any number of `Option`, sequence and transparent wrappers. A type whose schema this
/// crate writes whole (`ObjectId`, the chrono types) renders no bound and reading one is a compile
/// error rather than a silent drop.
///
/// - `pattern = "regex"` — validates the string matches the regex pattern. Must be a regex all
///   three engines read the same way; see [`crate::utils::portable_pattern`] for what that rules
///   out and what it rewrites. It must also turn some value away: a pattern every string satisfies
///   is refused where it is written rather than published as a check that checks nothing — see
///   [`crate::utils::constraining_pattern`].
///   - Zod: `.check(z.regex(/regex/))`
///   - JSON Schema: `"pattern"`
///   - Rust: `validate()` — see "Where a constraint is checked" below
///
/// - `minLength = N` — minimum string length (inclusive).
///   - Zod: `.min(N)`
///   - JSON Schema: `"minLength"`
///   - Rust: `validate()`
///
/// - `maxLength = N` — maximum string length (inclusive).
///   - Zod: `.max(N)`
///   - JSON Schema: `"maxLength"`
///   - Rust: `validate()`
///
/// ## Numeric constraints
///
/// - `minimum = N` — minimum value for numeric fields (integer or float).
///   - Zod: `.min(N)`
///   - JSON Schema: `"minimum"`
///   - Rust: `validate()`
///
/// - `maximum = N` — maximum value for numeric fields (integer or float).
///   - Zod: `.max(N)`
///   - JSON Schema: `"maximum"`
///   - Rust: `validate()`
///
/// ## Where a constraint is checked
///
/// By the `validate()` the type publishes, and not as the payload is read.
///
/// A constraint describes the value, not the shape, so a payload carrying a value it rejects is
/// still structurally the message it claims to be — every key present, every value of its field's
/// declared type. Checking it on the read makes the two indistinguishable to whoever receives the
/// failure: "I could not parse this at all" and "I parsed it and the value broke a rule" are
/// different sentences that send a caller looking in different places. So the read admits the
/// value and `validate()` refuses it, naming the field.
///
/// A field still generates both helpers into the schema module — `validate_{field}_value()`, which
/// `validate()` calls, and `deserialize_{field}`, a serde hook an author may hang on a field of
/// their own accord. Only one position is hung with that hook automatically: a member of an
/// `#[serde(untagged)]` enum, where whether the member is admissible is what chooses which variant
/// the payload is. There the check is part of reading the value rather than part of judging it,
/// exactly as it is under `anyOf` and `z.union` on the two schema surfaces the same type
/// publishes, and `validate()` cannot stand in for it — by the time it runs the variant has
/// already been chosen.
///
/// A constrained *brand* is the remaining exception and is enforced on the read. A message holding
/// a branded *named field* reaches it through the walk below as well, so the two overlap there; the
/// read is what still covers the positions the walk does not reach — a positional slot, which has
/// no name for a violation to be reported under, and a field written under a wrapper the walk does
/// not read through.
///
/// ## What a validator reaches
///
/// Its own constrained fields, and then whatever each of its other fields holds.
///
/// A bound declared on a nested message is a bound on the message that carries it. The Zod schema
/// the same declaration publishes is composed — validating the outer schema validates the inner one
/// with it — so a `validate()` stopping at the top level would make the two ends of one declaration
/// disagree about whether a payload is valid.
///
/// Every field bottoming out in a declared type is walked, through the same wrappers a constraint
/// is reached through, and the value's own `validate()` is run; a field bottoming out in a
/// primitive is not, its bounds being declared on the field and run there. A type publishing no
/// `validate()` answers `Ok(())` through a fallback written inside the walk.
///
/// A violation is reported under the path it was reached through —
/// `'account.claims.jti' is too short: minimum length is 1, got 0` — so the field a fault names is
/// the key a caller looks for in the payload it sent. A `#[serde(flatten)]` hop writes no key and
/// contributes no segment. A violation naming no field of its own, as a constrained brand's does,
/// is reported under the field the walk reached it through.
///
/// A type declaring no bound of its own publishes a `validate()` when something beneath it does; a
/// type holding nothing but primitives publishes none.
///
/// ## Type overrides
///
/// - `as = Type` — name the type emitted for this field. The target must be the field's own type
///   or the value under its wrappers (`as = String` on a `Vec<String>`); any other target is a
///   compile error. Cannot be written beside `preprocess`.
/// - `literal = "value"` — emit as a literal type instead of the field's own primitive. Takes a
///   string, boolean, integer or float literal; the kind written must match what the field's Rust
///   type can carry (a boolean literal on a `bool` field, a numeric literal on a numeric field, a
///   string literal on a `String` field) or the attribute is a compile error naming the mismatch.
/// - `ts_optional` — for an `Option<T>` field, emit `field?: T` instead of `field: T | undefined`
///   (TypeScript only; a non-`Option` field is a compile error). It decides the key only on a field
///   no serde key-omission attribute speaks for, since such an attribute already writes the
///   optional key off the wire in every build. Under the `serde` feature that field does not
///   compile — the `Option`-null guard refuses it — so the one shape the flag is live on is a build
///   with the feature off.
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
    /// The type named by `as`, kept as written so the guard that answers for it can build the field
    /// the target would render and compare that against the field's own.
    pub as_type: Option<Type>, // e.g., `String` from as = String
    /// The parser's refusal of the attribute — a key it does not read, or a value it cannot read —
    /// spanned on the tokens that earned it.
    pub attr_rejection: Option<syn::Error>,
    pub literal: Option<LiteralValue>, // e.g., Str("Tixena") from literal = "Tixena"
    pub max_length: Option<usize>,     // e.g., 50 from maxLength = 50
    pub maximum: Option<f64>,          // e.g., 100.0 from maximum = 100
    pub min_length: Option<usize>,     // e.g., 1 from minLength = 1
    pub minimum: Option<f64>,          // e.g., 0.0 from minimum = 0
    pub nullable: bool, // Option<T> at object-key position renders `T | null` with the key required
    /// `pattern` in the spelling every surface reads the same way, or as it was written when it
    /// earned a [`Self::pattern_rejection`].
    pub pattern: Option<String>, // e.g., "^[0-9a-fA-F]{24}$" from pattern = "^[0-9a-fA-F]{24}$"
    /// What keeps `pattern` off the surfaces it was written for -- a regex the `regex` crate
    /// cannot parse, a construct a JavaScript regex literal cannot carry, a shape that admits
    /// every value and so says nothing on any of them, or a lone look-around the emitted regex
    /// cannot carry lint-free -- spanned on the literal it was written as.
    pub pattern_rejection: Option<syn::Error>,
    pub preprocess: Vec<String>, // e.g., ["epoch_to_date", "trim"] from preprocess = ["epoch_to_date", "trim"]
    pub ts_optional: bool,
}

/// What the parser cannot read is recorded as [`ModelSchemaPropMeta::attr_rejection`] rather than
/// dropped, and the field is emitted as though the attribute had been left off.
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
        meta.as_type = Some(nested.value()?.parse::<Type>()?);
    } else if nested.path.is_ident("literal") {
        meta.literal = Some(literal_prop_value(nested)?);
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
        // A refused pattern is recorded as written: the guards that answer for what a `pattern`
        // may sit on read that one was given, not what it says.
        match portable_pattern(&lit)
            .and_then(|portable| constraining_pattern(&lit, portable))
            .and_then(|constraining| emittable_pattern(&lit, constraining))
        {
            Ok(pattern) => meta.pattern = Some(pattern),
            Err(rejection) => {
                meta.pattern_rejection = Some(rejection);
                meta.pattern = Some(lit.value());
            }
        }
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
    } else if nested.path.is_ident("nullable") {
        meta.nullable = true;
    } else {
        return Err(unknown_key_rejection(nested));
    }
    Ok(())
}

/// The `f64` a numeric bound was written as: both bounds reach a numeric comparison in the Rust
/// validator and a numeric literal in the Zod and JSON schemas, so a non-number is one no surface
/// can carry.
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

/// The [`LiteralValue`] a `literal` key was written as, kept in whichever of the four kinds the
/// author wrote — the kind [`crate::model_schema`](macro@crate::model_schema)'s own guard then
/// measures against the field's declared Rust type.
fn literal_prop_value(nested: &ParseNestedMeta) -> syn::Result<LiteralValue> {
    let lit: Lit = nested.value()?.parse()?;
    if let Lit::Str(str_lit) = &lit {
        Ok(LiteralValue::Str(str_lit.value()))
    } else if let Lit::Bool(bool_lit) = &lit {
        Ok(LiteralValue::Bool(bool_lit.value()))
    } else if let Lit::Int(int_lit) = &lit {
        Ok(LiteralValue::Number(int_lit.base10_parse()?))
    } else if let Lit::Float(float_lit) = &lit {
        Ok(LiteralValue::Number(float_lit.base10_parse()?))
    } else {
        Err(syn::Error::new_spanned(
            &lit,
            "`model_schema_prop` key `literal` takes a string, boolean, integer or float literal",
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

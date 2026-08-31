//! The sentence one violated bound is reported in, built once and handed to both emitters.
//!
//! A field declares a bound once and `#[model_schema]` emits two checks for it — the Rust
//! validator, and the Zod check the TypeScript side runs. A caller that sends the same bad value
//! to either implementation of one service is owed the same sentence, so the words are built here
//! and given to both rather than left to each: the Rust half would otherwise say what it wrote
//! itself and the Zod half whatever the version of zod in the consumer's lockfile says, and the
//! two would drift with no test in either language able to see it.
//!
//! The words name no field. Both sides already name it the same way and neither does it inside the
//! sentence: the generated validator writes `'{field}': ` in front of what it gets from here, and
//! the generated dispatcher writes `'${issue.path.join(".")}': ` in front of what Zod reports. A
//! brand names none at all on either side, the brand being the value rather than a field of
//! anything — the field it is held in supplies the name.

#[cfg(feature = "zod")]
use crate::utils::escape_js_double_quoted;
#[cfg(feature = "serde")]
use quote::quote;

/// A bound a declaration can carry, at the one value it was written with.
#[cfg(any(feature = "serde", feature = "zod"))]
#[derive(Clone, Copy)]
pub enum Bound<'pattern> {
    MaxLength(usize),
    Maximum(f64),
    MinLength(usize),
    Minimum(f64),
    Pattern(&'pattern str),
}

/// How a sentence measures the value that broke the bound, for the bounds that quote one back.
#[cfg(any(feature = "serde", feature = "zod"))]
#[derive(Clone, Copy)]
enum Observed {
    /// The value's length, which is what a length bound measured.
    Length,
    /// The value itself, which is what a numeric bound compared.
    Value,
}

#[cfg(any(feature = "serde", feature = "zod"))]
impl Bound<'_> {
    /// What the sentence quotes back, or `None` for a bound whose report names only itself.
    const fn observed(self) -> Option<Observed> {
        match self {
            Self::MaxLength(_) | Self::MinLength(_) => Some(Observed::Length),
            Self::Maximum(_) | Self::Minimum(_) => Some(Observed::Value),
            Self::Pattern(_) => None,
        }
    }

    /// The bound in words, without the value that broke it. Every number here is rendered at
    /// expansion time, by the same `Display` that writes the bound into the Zod check beside it,
    /// so the two spell it identically without either having to know how the other did.
    fn stated(self) -> String {
        match self {
            Self::MaxLength(len) => format!("too long: maximum length is {len}"),
            Self::Maximum(bound) => format!("too large: maximum is {bound}"),
            Self::MinLength(len) => format!("too short: minimum length is {len}"),
            Self::Minimum(bound) => format!("too small: minimum is {bound}"),
            Self::Pattern(pattern) => format!("does not match pattern '{pattern}'"),
        }
    }
}

/// The `format!` a generated validator pushes onto its report for `bound`.
///
/// `field` is the member the bound was declared on, or `None` where the checked value is not a
/// member of anything — a brand, whose name is written by whatever holds it. `observed` is the
/// expression the sentence quotes back, read in the validator's own scope: the length of the value
/// it measured, or the value it compared.
#[cfg(feature = "serde")]
pub fn rust_violation(
    bound: Bound<'_>,
    field: Option<&str>,
    observed: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let stated = bound.stated();
    match (field, bound.observed()) {
        (Some(named), Some(_)) => quote! {
            format!("'{}': {}, got {}", #named, #stated, #observed)
        },
        (Some(named), None) => quote! {
            format!("'{}': {}", #named, #stated)
        },
        (None, Some(_)) => quote! {
            format!("{}, got {}", #stated, #observed)
        },
        (None, None) => quote! {
            #stated.to_owned()
        },
    }
}

/// The parameter object a generated Zod check carries for `bound`, which is what makes it report
/// the sentence [`rust_violation`] writes instead of zod's own.
///
/// A bound that quotes the value back takes a function, that being the only form zod hands the
/// value to. The value arrives as `unknown` and goes through `String` rather than straight into the
/// template, so the expression type-checks under `--strict` whatever the schema was handed.
#[cfg(feature = "zod")]
pub fn zod_error_arg(bound: Bound<'_>) -> String {
    let stated = bound.stated();
    // Only a `pattern` carries text somebody else wrote, and it is the one rendered into a quoted
    // string; the rest are built here out of a fixed vocabulary and digits.
    match bound.observed() {
        Some(Observed::Length) => {
            format!("{{ error: (issue) => `{stated}, got ${{String(issue.input).length}}` }}")
        }
        Some(Observed::Value) => {
            format!("{{ error: (issue) => `{stated}, got ${{String(issue.input)}}` }}")
        }
        None => format!("{{ error: \"{}\" }}", escape_js_double_quoted(&stated)),
    }
}

#[cfg(test)]
#[cfg(any(feature = "serde", feature = "zod"))]
mod tests;

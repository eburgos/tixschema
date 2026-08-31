//! What the one sentence looks like in each of the two spellings it is handed out in.
//!
//! The proof that the two *agree* is not here — it takes a declaration, a value, and both emitters,
//! and lives in `tests/language_parity_tests`. What is here is the shape of each rendering, so a
//! change to either is a change somebody wrote down.

use super::Bound;
#[cfg(feature = "serde")]
use super::rust_violation;
#[cfg(feature = "zod")]
use super::zod_error_arg;

#[cfg(feature = "serde")]
use quote::quote;

/// The tokens `rust_violation` builds, as one line with the spacing `quote!` prints.
#[cfg(feature = "serde")]
fn rendered(bound: Bound<'_>, field: Option<&str>, observed: &proc_macro2::TokenStream) -> String {
    rust_violation(bound, field, observed).to_string()
}

#[cfg(feature = "serde")]
#[test]
fn a_bound_that_quotes_the_value_back_names_the_field_and_the_measure() {
    assert_eq!(
        rendered(
            Bound::MinLength(3),
            Some("organization_id"),
            &quote! { value.len() }
        ),
        r#"format ! ("'{}': {}, got {}" , "organization_id" , "too short: minimum length is 3" , value . len ())"#
    );
    assert_eq!(
        rendered(Bound::Maximum(120.0), Some("age"), &quote! { value }),
        r#"format ! ("'{}': {}, got {}" , "age" , "too large: maximum is 120" , value)"#
    );
}

#[cfg(feature = "serde")]
#[test]
fn a_bound_that_quotes_nothing_back_states_only_itself() {
    assert_eq!(
        rendered(
            Bound::Pattern("^[a-z]+$"),
            Some("slug"),
            &quote! { value.len() }
        ),
        r#"format ! ("'{}': {}" , "slug" , "does not match pattern '^[a-z]+$'")"#
    );
}

/// A brand is the value rather than a member of anything, so nothing writes a field into its
/// report — the field it is held in does that, on both sides of the wire.
#[cfg(feature = "serde")]
#[test]
fn a_report_with_no_field_to_name_names_none() {
    assert_eq!(
        rendered(Bound::MinLength(3), None, &quote! { value.len() }),
        r#"format ! ("{}, got {}" , "too short: minimum length is 3" , value . len ())"#
    );
    assert_eq!(
        rendered(Bound::Pattern("^/"), None, &quote! { value.len() }),
        r#""does not match pattern '^/'" . to_owned ()"#
    );
}

#[cfg(feature = "zod")]
#[test]
fn a_length_bound_reads_its_measure_off_the_issue() {
    assert_eq!(
        zod_error_arg(Bound::MinLength(3)),
        "{ error: (issue) => `too short: minimum length is 3, got ${String(issue.input).length}` }"
    );
    assert_eq!(
        zod_error_arg(Bound::MaxLength(50)),
        "{ error: (issue) => `too long: maximum length is 50, got ${String(issue.input).length}` }"
    );
}

#[cfg(feature = "zod")]
#[test]
fn a_numeric_bound_reads_the_value_itself_off_the_issue() {
    assert_eq!(
        zod_error_arg(Bound::Minimum(0.0)),
        "{ error: (issue) => `too small: minimum is 0, got ${String(issue.input)}` }"
    );
    assert_eq!(
        zod_error_arg(Bound::Maximum(120.0)),
        "{ error: (issue) => `too large: maximum is 120, got ${String(issue.input)}` }"
    );
}

#[cfg(feature = "zod")]
#[test]
fn a_pattern_states_itself_and_takes_no_function() {
    assert_eq!(
        zod_error_arg(Bound::Pattern("^[a-z]+$")),
        r#"{ error: "does not match pattern '^[a-z]+$'" }"#
    );
}

/// The one bound carrying text somebody else wrote is the one rendered into a quoted string, so
/// what a pattern spells cannot end that string early or eat the escape after it.
#[cfg(feature = "zod")]
#[test]
fn a_pattern_reaches_the_quoted_string_escaped() {
    assert_eq!(
        zod_error_arg(Bound::Pattern(r#"^\d+"x"$"#)),
        r#"{ error: "does not match pattern '^\\d+\"x\"$'" }"#
    );
}

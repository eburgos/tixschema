//! What `#[service_schema(...)]`'s own arguments are read into, and every refusal they earn.
//!
//! The refusals are read off `parse_transports` rather than off rendered `compile_error!` tokens,
//! so an assertion compares the text the compiler shows against the text the design specifies,
//! character for character.

use super::{Transport, parse_transports};
use proc_macro2::TokenStream;

/// The transports `args` asks for, named as the service wrote them.
fn asked_for(args: &str) -> Vec<&'static str> {
    parse_transports(written(args))
        .unwrap()
        .iter()
        .map(|known| known.name())
        .collect()
}

fn refusal(args: &str) -> syn::Error {
    parse_transports(written(args)).unwrap_err()
}

/// Attribute arguments carrying file locations, so a refusal's span can be read back to the text it
/// points at.
fn written(args: &str) -> TokenStream {
    syn::parse_str(args).unwrap()
}

/// A service that says nothing about transports asks for none, and an attribute written with empty
/// parentheses is the same declaration — the macro is handed no tokens either way.
#[test]
fn an_attribute_carrying_no_arguments_asks_for_no_transport() {
    assert!(parse_transports(TokenStream::new()).unwrap().is_empty());
}

/// The written empty list is the same answer said out loud, and is not a refusal.
#[test]
fn an_empty_written_list_asks_for_no_transport() {
    assert!(asked_for("transports = []").is_empty());
}

#[test]
fn a_named_transport_is_read_into_the_list() {
    assert_eq!(asked_for(r#"transports = ["amqp_rpc"]"#), ["amqp_rpc"]);
}

/// The list is the service's, in its order: nothing sorts it and nothing dedupes it. Written over
/// every known transport and over that list reversed, so a second transport makes the two runs
/// differ.
#[test]
fn the_written_order_of_the_list_is_the_order_it_is_read_into() {
    let known: Vec<&str> = Transport::KNOWN.iter().map(|each| each.name()).collect();
    let reversed: Vec<&str> = known.iter().rev().copied().collect();
    for order in [&known, &reversed] {
        let list = order
            .iter()
            .map(|name| format!("\"{name}\""))
            .collect::<Vec<_>>()
            .join(", ");
        assert_eq!(asked_for(&format!("transports = [{list}]")), *order);
    }
}

/// The refusal names the value the service wrote and lists what it could have written instead.
#[test]
fn an_unknown_transport_names_itself_and_what_is_known() {
    assert_eq!(
        refusal(r#"transports = ["grpc"]"#).to_string(),
        "service_schema: `grpc` is not a transport this version knows\n       \
         known transports: `amqp_rpc`"
    );
}

/// The caret sits under the name that is wrong rather than under the whole attribute, so a list of
/// several points at the one the service has to change.
#[test]
fn an_unknown_transport_refusal_is_spanned_on_the_name() {
    let refused = refusal(r#"transports = ["amqp_rpc", "grpc"]"#);
    assert_eq!(refused.span().source_text().as_deref(), Some("\"grpc\""));
}

/// A name is written as a string, so a bare ident is refused with the shape rather than read as
/// one.
#[test]
fn a_list_of_anything_but_strings_says_what_shape_was_expected() {
    for args in ["transports = [amqp_rpc]", "transports = [1]"] {
        assert_eq!(
            refusal(args).to_string(),
            "service_schema: `transports` takes a bracketed list of transport names\n       \
             write `transports = [\"amqp_rpc\"]`, or `transports = []` for none",
            "for {args}"
        );
    }
}

/// One transport written without the brackets is a list of one written wrong, not a shorthand.
#[test]
fn a_list_written_without_brackets_says_what_shape_was_expected() {
    let refused = refusal(r#"transports = "amqp_rpc""#);
    assert_eq!(
        refused.to_string(),
        "service_schema: `transports` takes a bracketed list of transport names\n       \
         write `transports = [\"amqp_rpc\"]`, or `transports = []` for none"
    );
    assert_eq!(
        refused.span().source_text().as_deref(),
        Some("\"amqp_rpc\"")
    );
}

/// The singular spelling is the mistake this refusal exists for: it is not an argument the
/// attribute takes, and reading it as one would let a service name transports the macro never saw.
#[test]
fn an_argument_that_is_not_transports_says_what_the_one_argument_is() {
    assert_eq!(
        refusal(r#"transport = ["amqp_rpc"]"#).to_string(),
        "service_schema: unknown `service_schema` argument\n       \
         the one argument is `transports`, written `transports = [\"amqp_rpc\"]`"
    );
}

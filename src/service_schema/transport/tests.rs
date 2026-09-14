//! What `#[service_schema(...)]`'s own arguments are read into, and every refusal they earn.
//!
//! The refusals are read off `parse_arguments` rather than off rendered `compile_error!` tokens,
//! so an assertion compares the text the compiler shows against the text the design specifies,
//! character for character.

use super::{ServiceArguments, Transport, parse_arguments};
use proc_macro2::TokenStream;

/// The transports `args` asks for, named as the service wrote them.
fn asked_for(args: &str) -> Vec<&'static str> {
    parse_arguments(written(args))
        .unwrap()
        .transports
        .iter()
        .map(|known| known.name())
        .collect()
}

fn refusal(args: &str) -> syn::Error {
    parse_arguments(written(args)).unwrap_err()
}

/// Attribute arguments carrying file locations, so a refusal's span can be read back to the text it
/// points at.
fn written(args: &str) -> TokenStream {
    syn::parse_str(args).unwrap()
}

/// A service that says nothing about transports asks for none, and an attribute written with empty
/// parentheses is the same declaration — the macro is handed no tokens either way. Nothing written
/// is the default `ServiceArguments`: no transport, exhaustive.
#[test]
fn an_attribute_carrying_no_arguments_reads_the_default() {
    assert_eq!(
        parse_arguments(TokenStream::new()).unwrap(),
        ServiceArguments::default()
    );
}

/// The written empty list is the same answer said out loud, and is not a refusal.
#[test]
fn an_empty_written_list_asks_for_no_transport() {
    assert_eq!(asked_for("transports = []"), Vec::<&str>::new());
}

#[test]
fn a_named_transport_is_read_into_the_list() {
    assert_eq!(asked_for(r#"transports = ["amqp_rpc"]"#), ["amqp_rpc"]);
}

#[test]
fn a_named_ws_rpc_transport_is_read_into_the_list() {
    assert_eq!(asked_for(r#"transports = ["ws_rpc"]"#), ["ws_rpc"]);
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
         known transports: `amqp_rpc`, `http_rest`, `ws_rpc`"
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

/// The singular spelling is not an argument the attribute takes, and the refusal names both of the
/// arguments it does take rather than just the one this service happened to reach for.
#[test]
fn an_unknown_argument_names_both_arguments() {
    assert_eq!(
        refusal(r#"transport = ["amqp_rpc"]"#).to_string(),
        "service_schema: unknown `service_schema` argument\n       \
         the arguments are `transports`, written `transports = [\"amqp_rpc\"]`,\n       \
         and `non_exhaustive`, written bare"
    );
}

/// The flag alone, with no transport, is read into `ServiceArguments` with an empty transport list.
#[test]
fn the_flag_alone_asks_for_no_transport_and_reads_non_exhaustive() {
    assert_eq!(
        parse_arguments(written("non_exhaustive")).unwrap(),
        ServiceArguments {
            non_exhaustive: true,
            transports: Vec::new(),
        }
    );
}

/// `#[service_schema(non_exhaustive)]` with no `transports` is accepted and asks for no transport —
/// the two arguments are independent, and neither is required for the other to be written.
#[test]
fn the_flag_composes_with_a_transport_list() {
    assert_eq!(
        parse_arguments(written(r#"non_exhaustive, transports = ["amqp_rpc"]"#)).unwrap(),
        ServiceArguments {
            non_exhaustive: true,
            transports: vec![Transport::AmqpRpc],
        }
    );
}

/// Argument order is free: the flag before the list reads the same as the list before the flag.
#[test]
fn argument_order_does_not_change_what_is_read() {
    let forward = parse_arguments(written(r#"non_exhaustive, transports = ["amqp_rpc"]"#)).unwrap();
    let reversed =
        parse_arguments(written(r#"transports = ["amqp_rpc"], non_exhaustive"#)).unwrap();
    assert_eq!(forward, reversed);
}

/// `non_exhaustive` is a bare flag: writing it as `= true` is refused rather than read as a second
/// spelling of the same request.
#[test]
fn non_exhaustive_written_with_a_value_says_it_takes_none() {
    assert_eq!(
        refusal("non_exhaustive = true").to_string(),
        "service_schema: `non_exhaustive` is a bare flag and takes no value\n       \
         write `non_exhaustive`, and leave it out for the exhaustive generated types"
    );
}

/// The refusal is spanned on the flag itself rather than on the whole argument list, so the caret
/// points at the one word that has to lose its `= true`.
#[test]
fn non_exhaustive_written_with_a_value_is_spanned_on_the_flag() {
    let refused = refusal("non_exhaustive = true");
    assert_eq!(
        refused.span().source_text().as_deref(),
        Some("non_exhaustive")
    );
}

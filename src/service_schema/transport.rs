//! The transports a service can ask for, and the one place a name is bound to one.
//!
//! `#[service_schema(transports = ["amqp_rpc"])]` is where a service says which of them it wants.
//! [`parse_transports`] reads that list, and [`Transport`] is the vocabulary it reads against —
//! written with an underscore rather than a hyphen, because a transport's name reaches a generated
//! macro name and a hyphen cannot.
//!
//! # Adding one
//!
//! A module beside [`amqp_rpc`], and one variant here. The variant makes the match in [`emit`]
//! incomplete until the new one is answered for, so the emitter each transport contributes is bound
//! in this file and nowhere else, and no existing transport's module is touched to add another.
//!
//! # What a named transport contributes
//!
//! Three `#[macro_export] macro_rules!` per transport the service asked for, named
//! `{service}_{transport}_dispatcher`, `{service}_{transport}_client` and
//! `{service}_{transport}_server`, and emitted at the trait's own scope. Nothing inside any of them
//! is compiled where the service is declared, and a service that named no transport is emitted
//! nothing here at all.
//!
//! Three macros rather than one, because the halves of a service usually live in different crates —
//! a crate that calls the service can see the contract but has no business seeing the server's
//! backend, and a crate that only wants `dispatch` itself (a hand-rolled adapter, or a test with no
//! broker in reach) has no business seeing the server macro's `lapin`, `tokio` and `futures`
//! either. Each is invoked and placed by the half that wants it, and none drags in another.
//!
//! [`emit`] walks [`Transport::KNOWN`] rather than the list as written, so a transport named twice
//! contributes one pair rather than two definitions of one exported name.

mod amqp_rpc;

use super::parse::ServiceDef;
use crate::rename_rule::RenameRule;
use proc_macro2::TokenStream;
use quote::format_ident;
use syn::spanned::Spanned as _;
use syn::{Expr, ExprLit, Ident, Lit, meta::parser, parse::Parser as _};

const TRANSPORTS_ARGUMENT: &str = "transports";

const UNKNOWN_ARGUMENT_MESSAGE: &str = concat!(
    "service_schema: unknown `service_schema` argument\n",
    "       the one argument is `transports`, written `transports = [\"amqp_rpc\"]`"
);

const WRITTEN_SHAPE_MESSAGE: &str = concat!(
    "service_schema: `transports` takes a bracketed list of transport names\n",
    "       write `transports = [\"amqp_rpc\"]`, or `transports = []` for none"
);

/// One transport a service asks for by name.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Transport {
    AmqpRpc,
}

impl Transport {
    /// Every transport this version knows, in the order a refusal lists them.
    pub const KNOWN: &'static [Self] = &[Self::AmqpRpc];

    fn from_name(written: &str) -> Option<Self> {
        Self::KNOWN
            .iter()
            .copied()
            .find(|known| known.name() == written)
    }

    /// The name a service writes for it.
    pub const fn name(self) -> &'static str {
        match self {
            Self::AmqpRpc => "amqp_rpc",
        }
    }
}

/// The name a transport's client macro publishes under: `{service}_{transport}_client`.
pub fn client_macro_ident(service: &ServiceDef, transport: Transport) -> Ident {
    macro_ident(service, transport, "client")
}

/// The name a transport's dispatcher macro publishes under: `{service}_{transport}_dispatcher`.
pub fn dispatcher_macro_ident(service: &ServiceDef, transport: Transport) -> Ident {
    macro_ident(service, transport, "dispatcher")
}

/// The name a transport's server macro publishes under: `{service}_{transport}_server`.
pub fn server_macro_ident(service: &ServiceDef, transport: Transport) -> Ident {
    macro_ident(service, transport, "server")
}

/// Either half's macro name, spelled in one place so the two cannot drift apart.
///
/// `#[macro_export]` places each at the declaring crate's root whatever module it was written in,
/// so the service name is what keeps two services in one crate from claiming one name.
fn macro_ident(service: &ServiceDef, transport: Transport, half: &str) -> Ident {
    format_ident!(
        "{}_{}_{}",
        RenameRule::SnakeCase.apply_to_variant(&service.ident.to_string()),
        transport.name(),
        half,
        span = service.ident.span()
    )
}

/// What every transport the service asked for contributes, in the registry's own order.
///
/// Each contributes at most once however many times it was named: what it publishes is
/// `#[macro_export]`ed, and one name at a crate root can be defined once.
pub fn emit(service: &ServiceDef, asked: &[Transport]) -> TokenStream {
    Transport::KNOWN
        .iter()
        .filter(|known| asked.contains(known))
        .map(|known| match *known {
            Transport::AmqpRpc => amqp_rpc::emit(service, *known),
        })
        .collect()
}

/// Reads `#[service_schema(...)]`'s own arguments into the transports the service asked for, in the
/// order it wrote them.
///
/// A bare `#[service_schema]` and an empty `#[service_schema()]` ask for none, and so does
/// `transports = []` — the same list, said out loud. Anything else the attribute carries is
/// refused rather than dropped, so a service cannot ask for a transport this version does not have
/// and be handed silence.
///
/// # An unknown name is refused, under the name itself
///
/// The service below asks for a transport that does not exist:
///
/// ```rust,compile_fail
/// use tixschema::service_schema;
///
/// #[derive(serde::Deserialize, serde::Serialize)]
/// pub struct BalanceResponse;
///
/// #[derive(serde::Deserialize, serde::Serialize)]
/// pub enum BalanceError {
///     DbError,
/// }
///
/// #[service_schema(transports = ["grpc"])]
/// pub trait UsageService<Ctx> {
///     async fn sweep(&self, ctx: &Ctx) -> Result<BalanceResponse, BalanceError>;
/// }
///
/// fn main() {}
/// ```
///
/// A `compile_fail` doctest asserts only that *something* was refused, so the file above was
/// compiled standalone and the diagnostic read off that run, verbatim. It was the only error the
/// file earned, and the caret sits under the name rather than under the attribute:
///
/// ```text
/// error: service_schema: `grpc` is not a transport this version knows
///               known transports: `amqp_rpc`
///   --> tests/zz_probe.rs:11:32
///    |
/// 11 | #[service_schema(transports = ["grpc"])]
///    |                                ^^^^^^
///
/// error: could not compile `tixschema` (test "zz_probe") due to 1 previous error
/// ```
///
/// # A list written without brackets is refused, and so is an argument that is not `transports`
///
/// The same service with the brackets left off earns a sentence naming the shape that was
/// expected:
///
/// ```rust,compile_fail
/// use tixschema::service_schema;
///
/// #[derive(serde::Deserialize, serde::Serialize)]
/// pub struct BalanceResponse;
///
/// #[derive(serde::Deserialize, serde::Serialize)]
/// pub enum BalanceError {
///     DbError,
/// }
///
/// #[service_schema(transports = "amqp_rpc")]
/// pub trait UsageService<Ctx> {
///     async fn sweep(&self, ctx: &Ctx) -> Result<BalanceResponse, BalanceError>;
/// }
///
/// fn main() {}
/// ```
///
/// ```text
/// error: service_schema: `transports` takes a bracketed list of transport names
///               write `transports = ["amqp_rpc"]`, or `transports = []` for none
///   --> tests/zz_probe.rs:11:31
///    |
/// 11 | #[service_schema(transports = "amqp_rpc")]
///    |                               ^^^^^^^^^^
///
/// error: could not compile `tixschema` (test "zz_probe") due to 1 previous error
/// ```
///
/// And the singular spelling is a name the attribute does not take, rather than a second way of
/// saying the same thing:
///
/// ```rust,compile_fail
/// use tixschema::service_schema;
///
/// #[derive(serde::Deserialize, serde::Serialize)]
/// pub struct BalanceResponse;
///
/// #[derive(serde::Deserialize, serde::Serialize)]
/// pub enum BalanceError {
///     DbError,
/// }
///
/// #[service_schema(transport = ["amqp_rpc"])]
/// pub trait UsageService<Ctx> {
///     async fn sweep(&self, ctx: &Ctx) -> Result<BalanceResponse, BalanceError>;
/// }
///
/// fn main() {}
/// ```
///
/// ```text
/// error: service_schema: unknown `service_schema` argument
///               the one argument is `transports`, written `transports = ["amqp_rpc"]`
///   --> tests/zz_probe.rs:11:18
///    |
/// 11 | #[service_schema(transport = ["amqp_rpc"])]
///    |                  ^^^^^^^^^
///
/// error: could not compile `tixschema` (test "zz_probe") due to 1 previous error
/// ```
///
/// Each of the three is the service below with exactly one thing changed — the name, the
/// brackets, the argument — so the refusal can only be what was changed. This one asks for the
/// transport this version does know, and compiles:
///
/// ```rust
/// use tixschema::service_schema;
///
/// #[derive(serde::Deserialize, serde::Serialize)]
/// pub struct BalanceResponse;
///
/// #[derive(serde::Deserialize, serde::Serialize)]
/// pub enum BalanceError {
///     DbError,
/// }
///
/// #[service_schema(transports = ["amqp_rpc"])]
/// pub trait UsageService<Ctx> {
///     async fn sweep(&self, ctx: &Ctx) -> Result<BalanceResponse, BalanceError>;
/// }
///
/// fn main() {}
/// ```
pub fn parse_transports(args: TokenStream) -> Result<Vec<Transport>, syn::Error> {
    let mut asked = Vec::new();
    let reader = parser(|meta| {
        if !meta.path.is_ident(TRANSPORTS_ARGUMENT) {
            return Err(meta.error(UNKNOWN_ARGUMENT_MESSAGE));
        }
        let written = meta.value()?.parse::<Expr>()?;
        let Expr::Array(listed) = written else {
            return Err(syn::Error::new(written.span(), WRITTEN_SHAPE_MESSAGE));
        };
        for element in &listed.elems {
            asked.push(transport_written(element)?);
        }
        Ok(())
    });
    reader.parse2(args)?;
    Ok(asked)
}

/// One element of the written list, which is a string naming a transport this version has.
fn transport_written(element: &Expr) -> Result<Transport, syn::Error> {
    let Expr::Lit(ExprLit {
        lit: Lit::Str(named),
        ..
    }) = element
    else {
        return Err(syn::Error::new(element.span(), WRITTEN_SHAPE_MESSAGE));
    };
    let written = named.value();
    Transport::from_name(&written)
        .ok_or_else(|| syn::Error::new(named.span(), unknown_transport_message(&written)))
}

fn unknown_transport_message(written: &str) -> String {
    let known = Transport::KNOWN
        .iter()
        .map(|transport| format!("`{}`", transport.name()))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "service_schema: `{written}` is not a transport this version knows\n       \
         known transports: {known}"
    )
}

#[cfg(test)]
mod tests;

//! The transports a service can ask for, and the one place a name is bound to one.
//!
//! `#[service_schema(transports = ["amqp_rpc"])]` is where a service says which of them it wants.
//! [`parse_transports`] reads that list, and [`Transport`] is the vocabulary it reads against —
//! written with an underscore rather than a hyphen, because a transport's name reaches a generated
//! macro name and a hyphen cannot.
//!
//! # Adding one
//!
//! A module beside [`amqp_rpc`], and one variant here. The variant makes every match on a transport
//! incomplete until the new one is answered for, so the emitter each transport contributes is bound
//! in this file and nowhere else, and no existing transport's module is touched to add another.
//!
//! Nothing consumes the parsed list yet: the emitters that will read it are landed by their own
//! tasks, and until then the list is read for the refusals it earns.

mod amqp_rpc;

use proc_macro2::TokenStream;
use syn::spanned::Spanned as _;
use syn::{Expr, ExprLit, Lit, meta::parser, parse::Parser as _};

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

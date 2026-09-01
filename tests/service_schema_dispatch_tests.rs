//! The dispatcher `#[service_schema]` generates, driven end to end: a probe service, a probe reply
//! handle that writes down how each message was settled, and a payload for every path through an
//! arm.
//!
//! Gated on the `serde` feature, which `#[service_schema]` requires: a build without it is
//! refused at the declaration, so a harness declaring a service would not compile at all. The
//! refusal itself is read in the crate's own unit tests, which run in every combination. Nothing
//! here is gated any further — what the construct emits carries no other surface a feature writes.
//!
//! The one thing that does depend on a further feature is whether a message publishes a
//! `validate()` at all, and the module that reads validation says so itself.
//!
//! # Why the placements below are shaped the way they are
//!
//! A `macro_rules!` body is linted under the levels of the crate that *invokes* it, so this file
//! is a consumer of the dispatcher macro as much as a test of it. Four properties keep it clean,
//! and a tidy-up that drops any of them turns `just lint` red for reasons that look nothing like
//! the change that caused them:
//!
//! 1. every module is `#[path]`-attributed and lives in a file of its own, never `mod x { … }`,
//!    which is what `clippy::inline_modules` refuses;
//! 2. the `mod` declarations sit above the `use` items, which is the grouping
//!    `clippy::arbitrary_source_item_ordering` asks for;
//! 3. everything is private, so no generated item is *exported* — a `pub` module here would
//!    publish the proc macro's own message and support types along with it;
//! 4. the tests build an `IncomingMessage` and call `dispatch` in every placement, so nothing the
//!    macro emits is dead. `dead_code` is an error under `-D warnings`, and it is spanned on the
//!    `#[service_schema]` attribute rather than on anything the placement wrote.

#[cfg(test)]
#[macro_use]
#[path = "service_schema_dispatch_tests/tests.rs"]
mod tests;

#[cfg(all(test, feature = "serde"))]
#[path = "service_schema_dispatch_tests/amqp_transport.rs"]
mod amqp_transport;

/// The same macro again, in a second module of its own: two dispatchers for one service in one
/// crate, which is what the macro emitting bare items rather than a module of its own is for.
#[cfg(all(test, feature = "serde"))]
#[path = "service_schema_dispatch_tests/second_amqp_transport.rs"]
mod second_amqp_transport;

#[cfg(all(
    test,
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[path = "service_schema_dispatch_tests/gate_amqp_transport.rs"]
mod gate_amqp_transport;

// A transport's dispatcher reaches what the service declared through `$crate`, which is this
// binary's root: a service written in a submodule is named here for the expansion to resolve.
#[cfg(all(
    test,
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
use tests::a_message_annotated_with_a_constraint::{GateService, gate_service_schema};
#[cfg(all(test, feature = "serde"))]
use tests::{ProbeService, probe_service_schema};

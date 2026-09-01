//! The client `#[service_schema]` generates, driven over transports written by hand: one that
//! hands out prepared answers and records what it was asked to send, and one that loops straight
//! back into the generated dispatcher so both halves of the seam are read against each other.
//!
//! Gated on the `serde` feature, which `#[service_schema]` requires: a build without it is
//! refused at the declaration, so a harness declaring a service would not compile at all. The
//! refusal itself is read in the crate's own unit tests, which run in every combination. Nothing
//! here is gated any further — what the construct emits carries no other surface a feature writes.
//!
//! Both halves a transport contributes are placed here, in modules of their own, which is the one
//! harness that needs both: everything else about the client is read against a transport that
//! never dispatches, and the crate beside this one places a client with no dispatcher at all.
//!
//! The `use` at the foot is what `$crate` reaches. Either macro body names what the declaration
//! generated from the declaring crate's *root* — the messages beside the trait, and the fault and
//! the call error inside the service's own module — and the declarations sit in the module beside
//! this file. Importing them here puts those names where an expansion looks for them. Private,
//! because nothing outside this test binary reads them.

#![cfg(feature = "serde")]

#[cfg(test)]
#[macro_use]
#[path = "service_schema_client_tests/tests.rs"]
mod tests;

#[cfg(all(test, feature = "serde"))]
#[path = "service_schema_client_tests/amqp_transport.rs"]
mod amqp_transport;

#[cfg(all(test, feature = "serde"))]
#[path = "service_schema_client_tests/amqp_client.rs"]
mod amqp_client;

/// The same client macro a second time, in a module of its own: two clients for one service in one
/// crate, which is what the macro emitting bare items rather than a module of its own is for.
#[cfg(all(test, feature = "serde"))]
#[path = "service_schema_client_tests/spare_amqp_client.rs"]
mod spare_amqp_client;

#[cfg(all(
    test,
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[path = "service_schema_client_tests/enrol_amqp_transport.rs"]
mod enrol_amqp_transport;

#[cfg(all(
    test,
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
#[path = "service_schema_client_tests/enrol_amqp_client.rs"]
mod enrol_amqp_client;

// Both halves reach what the service declared through `$crate`, which is this binary's root: a
// service written in a submodule is named here for either expansion to resolve. The messages the
// macro declared are named beside the trait, and the client builds one per operation that declared
// no message of its own.
#[cfg(all(
    test,
    feature = "serde",
    any(feature = "typescript", feature = "zod", feature = "jsonschema")
))]
use tests::a_bound_the_fields_own_type_declares::{EnrolService, enrol_service_schema};
#[cfg(all(test, feature = "serde"))]
use tests::{ExpireCreditRequest, ProbeService, SweepRequest, probe_service_schema};

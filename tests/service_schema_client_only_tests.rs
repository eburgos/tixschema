//! A crate that places a client and no dispatcher, which is the half a caller of a service wants.
//!
//! The two halves a transport contributes are separate macros because they usually live in
//! separate crates: a crate that calls the service can see the contract but has no business seeing
//! the server's backend. That this binary compiles at all is the assertion — nothing here names
//! `dispatch`, `IncomingMessage` or `Reply`, and nothing here reaches `tracing` — and the calls
//! beneath it are what says the half that was placed works without the one that was not.
//!
//! Gated on the `serde` feature, which `#[service_schema]` requires: a build without it is refused
//! at the declaration, so a harness declaring a service would not compile at all.

#![cfg(feature = "serde")]

#[cfg(test)]
#[macro_use]
#[path = "service_schema_client_only_tests/tests.rs"]
mod tests;

#[cfg(all(test, feature = "serde"))]
#[path = "service_schema_client_only_tests/amqp_client.rs"]
mod amqp_client;

// The client reaches what the service declared through `$crate`, which is this binary's root: the
// service's own module, and the message the macro declared for the operation that named none. The
// trait is not among them — naming the contract is the dispatcher's business, not a caller's.
#[cfg(all(test, feature = "serde"))]
use tests::{SweepRequest, call_service_schema};

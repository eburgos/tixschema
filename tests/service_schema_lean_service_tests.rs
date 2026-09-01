//! Two services shaped so that most of what a transport contributes has nothing to do: one whose
//! every operation expects no reply, and one that declares no operation at all.
//!
//! Both halves are placed for each, so what the macros emit for them is *compiled* rather than
//! merely read off a token stream. That is the assertion: a fault mirror kept for a service with
//! no reply to read one out of, or a panic guard kept for a service with no arm to call one
//! behind, is dead code in whatever module the consumer placed - and `dead_code` is an error in
//! plenty of consumers' builds, unfixable from where they stand. Gating those items out is only
//! safe if what remains still compiles and still works, which is what runs here.
//!
//! Gated on the `serde` feature, which `#[service_schema]` requires: a build without it is refused
//! at the declaration, so a harness declaring a service would not compile at all.

#![cfg(feature = "serde")]

#[cfg(test)]
#[macro_use]
#[path = "service_schema_lean_service_tests/tests.rs"]
mod tests;

#[cfg(all(test, feature = "serde"))]
#[path = "service_schema_lean_service_tests/bare_amqp_client.rs"]
mod bare_amqp_client;

#[cfg(all(test, feature = "serde"))]
#[path = "service_schema_lean_service_tests/bare_amqp_transport.rs"]
mod bare_amqp_transport;

#[cfg(all(test, feature = "serde"))]
#[path = "service_schema_lean_service_tests/note_amqp_client.rs"]
mod note_amqp_client;

#[cfg(all(test, feature = "serde"))]
#[path = "service_schema_lean_service_tests/note_amqp_transport.rs"]
mod note_amqp_transport;

// Every half reaches what the service declared through `$crate`, which is this binary's root: the
// traits, and the services' own modules, which every message either half handles is reached
// through.
#[cfg(all(test, feature = "serde"))]
use tests::{BareService, NoteService, bare_service_schema, note_service_schema};

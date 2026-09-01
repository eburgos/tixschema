//! A dispatcher written by hand, for a service that named no transport.
//!
//! The service is declared in one module and answered in another, so the module that dispatches
//! expands no tixschema macro at all and reaches nothing but what the contract half publishes. That
//! it compiles here is half the assertion: this is a separate crate, so a name it resolves is
//! genuinely public rather than merely `pub` in the expansion.
//!
//! Gated on the `serde` feature, which `#[service_schema]` requires: a build without it is refused
//! at the declaration, so a harness declaring a service would not compile at all.

#![cfg(feature = "serde")]

#[cfg(test)]
#[path = "service_schema_hand_written_dispatcher_tests/declarations.rs"]
mod declarations;

#[cfg(test)]
#[path = "service_schema_hand_written_dispatcher_tests/dispatcher.rs"]
mod dispatcher;

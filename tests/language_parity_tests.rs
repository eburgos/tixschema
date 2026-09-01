#[cfg(test)]
#[macro_use]
#[path = "language_parity_tests/tests.rs"]
mod tests;

#[cfg(all(test, feature = "serde", feature = "typescript", feature = "zod"))]
#[path = "language_parity_tests/amqp_transport.rs"]
mod amqp_transport;

// A transport's dispatcher reaches what the service declared through `$crate`, which is this
// binary's root: a service written in a submodule is named here for the expansion to resolve.
#[cfg(all(test, feature = "serde", feature = "typescript", feature = "zod"))]
use tests::refusals::{GateService, gate_service_schema};

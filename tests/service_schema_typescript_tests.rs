//! The TypeScript a `#[service_schema]` service publishes, and the bundle one registration line
//! per artifact produces.
//!
//! Deliberately **not** feature-gated at the harness level. The probe service and the messages
//! declared for it must expand in every feature combination; what the published TypeScript then
//! says is read inside the gated module, since only a build that writes TypeScript writes any.

#[cfg(test)]
#[path = "service_schema_typescript_tests/tests.rs"]
mod tests;

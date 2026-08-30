//! Spike: the two things the `#[service_schema]` design rests on and nobody had run.
//!
//! Deliberately **not** feature-gated at the harness level. The probe trait and the message the
//! macro declares for it must expand in every feature combination — what the surfaces then say is
//! read inside the gated modules, since only those are compiled when the feature that writes them
//! is on.

#[cfg(test)]
#[path = "service_schema_spike_tests/tests.rs"]
mod tests;

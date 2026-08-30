//! The messages `#[service_schema]` declares for the operations that named none, read off the
//! three surfaces a client on the far side needs and off the wire itself.
//!
//! Deliberately **not** feature-gated at the harness level. The probe trait and the messages
//! declared for it must expand in every feature combination — what the describing surfaces then
//! say is read inside the gated modules, since only those are compiled when the feature that
//! writes them is on.

#[cfg(test)]
#[path = "service_schema_message_tests/tests.rs"]
mod tests;

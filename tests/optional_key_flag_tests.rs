//! Where the `ts_optional` flag is what decides the key, and where something else already has.
//!
//! TypeScript is the only surface it touches, so the module is gated on that feature. Both build
//! flavours are covered from here — the flag's whole subject is a field the `serde` feature's
//! `Option`-null guard refuses, so the shape it decides can only be declared with that feature off.

#[cfg(test)]
#[cfg(feature = "typescript")]
#[path = "optional_key_flag_tests/tests.rs"]
mod tests;

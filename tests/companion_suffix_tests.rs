//! Tests for the companion-type suffix: a declaration named `XData` publishes every generated name
//! as `X`, while one named `XJson` publishes as itself. The three surfaces are covered where each
//! writes a type's own name — the `export type` line, the schema consts, and the `$defs` key a
//! self-referential document hoists under — alongside the reference resolution that has to agree
//! with all three.

#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[cfg(test)]
#[path = "companion_suffix_tests/tests.rs"]
mod tests;

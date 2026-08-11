//! Tests for the name a declaration publishes under: its own Rust ident, unless
//! `#[model_schema(name = "...")]` names another one. No spelling has any suffix taken off it. The
//! three surfaces are covered where each writes a type's own name — the `export type` line, the
//! schema consts, and the `$defs` key a self-referential document hoists under — alongside the
//! reference resolution that has to agree with all three.

#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
#[cfg(test)]
#[path = "published_name_tests/tests.rs"]
mod tests;

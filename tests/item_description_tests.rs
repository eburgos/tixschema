//! Tests for the description an item falls back to when it carries no doc comment — the `JSDoc`
//! header a `TypeScript` definition opens with and the `description` a Zod schema publishes.
//!
//! The fallback names the item as it is **exported**, so it agrees with the `export type` written
//! one line under it. An export name parts from the Rust ident two ways: a `name = "…"` override,
//! and the `Json` suffix stripped off a declared ident. Both are covered here, alongside the
//! un-suffixed un-renamed items whose output this must leave exactly where it was.

#[cfg(any(feature = "typescript", feature = "zod"))]
#[cfg(test)]
#[path = "item_description_tests/tests.rs"]
mod tests;

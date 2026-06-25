//! Zod v4 schema generation support.
//!
//! This module provides functionality for generating Zod v4 validation schemas
//! from Rust types when the `zod` feature is enabled.
//!
//! ## Features:
//! - Generates Zod v4 validation schemas from structs and enums
//! - Supports all primitive types, collections, and custom types
//! - Respects Serde attributes for field renaming
//! - Integrates with TypeScript type annotations when both features are enabled
//! - Uses z.union([type, `z.undefined()`]) for optional fields
//!
//! ## Tests:
//! Comprehensive tests for this feature are located in `tests/zod_tests.rs`.

//! Feature detection and conditional compilation utilities for tixschema.
//!
//! This module provides compile-time feature detection and utilities for handling
//! different feature combinations in the macro expansion process.

// Not gated on the `serde` feature: the attributes that decide whether a key reaches the wire are
// written on the item under every toggle, and the surfaces describe that wire in every build. What
// the feature gates is everything else the module reads — renaming, tagging, and the guards.
pub mod serde;

#[cfg(feature = "zod")]
pub mod zod;

#[cfg(feature = "jsonschema")]
pub mod jsonschema;

#[cfg(feature = "object_id")]
pub mod object_id;

#[cfg(feature = "chrono")]
pub mod chrono;

/// Module for parsing model_schema_prop attributes
pub mod model_schema_prop;

#[cfg(feature = "typescript")]
pub mod service_schema;

/// Feature detection utilities.
#[cfg(test)]
pub struct Features;

#[cfg(test)]
impl Features {
    /// Get a description of enabled features for debugging.
    pub fn enabled_features() -> Vec<&'static str> {
        let mut features = Vec::new();

        if Self::has_serde() {
            features.push("serde");
        }
        if Self::has_zod() {
            features.push("zod");
        }
        if Self::has_jsonschema() {
            features.push("jsonschema");
        }
        if Self::has_object_id() {
            features.push("object_id");
        }
        if Self::has_typescript() {
            features.push("typescript");
        }
        if Self::has_chrono() {
            features.push("chrono");
        }

        if features.is_empty() {
            features.push("minimal");
        }

        features
    }

    /// Check if `chrono` feature is enabled.
    pub const fn has_chrono() -> bool {
        cfg!(feature = "chrono")
    }

    /// Check if jsonschema feature is enabled.
    pub const fn has_jsonschema() -> bool {
        cfg!(feature = "jsonschema")
    }

    /// Check if `object_id` feature is enabled.
    pub const fn has_object_id() -> bool {
        cfg!(feature = "object_id")
    }

    /// Check if serde feature is enabled.
    pub const fn has_serde() -> bool {
        cfg!(feature = "serde")
    }

    /// Check if typescript feature is enabled.
    pub const fn has_typescript() -> bool {
        cfg!(feature = "typescript")
    }

    /// Check if zod feature is enabled.
    pub const fn has_zod() -> bool {
        cfg!(feature = "zod")
    }
}

// Note: Proc-macro crates cannot export macro_rules! macros
// Instead, we use cfg attributes directly where needed

#[cfg(test)]
mod tests;

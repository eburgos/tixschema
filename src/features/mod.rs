//! Feature detection and conditional compilation utilities for core_model_macros
//! 
//! This module provides compile-time feature detection and utilities for handling
//! different feature combinations in the macro expansion process.

#[cfg(feature = "serde")]
pub mod serde;

#[cfg(feature = "zod")]
pub mod zod;

#[cfg(feature = "jsonschema")]
pub mod jsonschema;

#[cfg(feature = "object_id")]
pub mod object_id;

/// Feature detection utilities
pub struct Features;

impl Features {
    /// Check if serde feature is enabled
    pub const fn has_serde() -> bool {
        cfg!(feature = "serde")
    }

    /// Check if zod feature is enabled
    pub const fn has_zod() -> bool {
        cfg!(feature = "zod")
    }

    /// Check if jsonschema feature is enabled
    pub const fn has_jsonschema() -> bool {
        cfg!(feature = "jsonschema")
    }

    /// Check if object_id feature is enabled
    pub const fn has_object_id() -> bool {
        cfg!(feature = "object_id")
    }

    /// Get a description of enabled features for debugging
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
        
        if features.is_empty() {
            features.push("minimal");
        }
        
        features
    }

    /// Check if we have minimal configuration (no features)
    pub const fn is_minimal() -> bool {
        !Self::has_serde() && !Self::has_zod() && !Self::has_jsonschema() && !Self::has_object_id()
    }

    /// Check if we have a TypeScript-only configuration (no zod, no jsonschema)
    pub const fn is_typescript_only() -> bool {
        !Self::has_zod() && !Self::has_jsonschema()
    }
}

// Note: Proc-macro crates cannot export macro_rules! macros
// Instead, we use cfg attributes directly where needed

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature_detection() {
        // Test that we can detect features at compile time
        let enabled = Features::enabled_features();
        println!("Enabled features: {:?}", enabled);
        
        // In default configuration, all features should be enabled
        #[cfg(all(feature = "serde", feature = "zod", feature = "jsonschema", feature = "object_id"))]
        {
            assert!(Features::has_serde());
            assert!(Features::has_zod());
            assert!(Features::has_jsonschema());
            assert!(Features::has_object_id());
            assert!(!Features::is_minimal());
            assert!(!Features::is_typescript_only());
        }
    }
} 
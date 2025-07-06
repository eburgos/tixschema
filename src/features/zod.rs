//! Zod schema generation feature module
//! 
//! This module handles Zod schema generation when the "zod" feature is enabled.

/// Generates the optional fields suffix for Zod schemas
pub fn show_optionals(opts: &[String]) -> String {
    if opts.is_empty() {
        String::new()
    } else {
        // In Zod v4, we don't need transform functions since we use .or(z.undefined())
        String::new()
    }
}

/// Check if we should generate Zod schemas
pub fn should_generate_zod_schema() -> bool {
    true // Always true when this module is compiled (feature is enabled)
}

/// Generates the Zod import statement for TypeScript output
pub fn get_zod_import() -> String {
    "import { z } from \"zod\";\n\n".to_string()
}

/// Wraps a field schema with array handling if needed
pub fn wrap_with_array_if_needed(schema: &str, is_array: bool) -> String {
    if is_array {
        format!("z.array({})", schema)
    } else {
        schema.to_string()
    }
}

/// Wraps a field schema with optional handling if needed
pub fn wrap_with_optional_if_needed(schema: &str, is_optional: bool) -> String {
    if is_optional {
        format!("{}.or(z.undefined())", schema)
    } else {
        schema.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_show_optionals() {
        assert_eq!(show_optionals(&[]), "");
        assert_eq!(show_optionals(&["field1".to_string()]), "");
        assert_eq!(show_optionals(&["field1".to_string(), "field2".to_string()]), "");
    }

    #[test]
    fn test_array_wrapping() {
        assert_eq!(wrap_with_array_if_needed("z.string()", false), "z.string()");
        assert_eq!(wrap_with_array_if_needed("z.string()", true), "z.array(z.string())");
    }

    #[test]
    fn test_optional_wrapping() {
        assert_eq!(wrap_with_optional_if_needed("z.string()", false), "z.string()");
        assert_eq!(wrap_with_optional_if_needed("z.string()", true), "z.string().or(z.undefined())");
    }
} 
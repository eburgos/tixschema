//! Code generation utilities and modules
//! 
//! This module contains the core code generation logic separated by functionality.

pub mod typescript;

#[cfg(test)]
use crate::field_type::FieldDef;


/// Common utilities for code generation
#[cfg(test)]
pub struct GenerationUtils;

#[cfg(test)]
impl GenerationUtils {
    /// Formats documentation for TypeScript comments
    pub fn format_docs(docs: &str) -> String {
        if docs.is_empty() {
            String::new()
        } else {
            docs.lines()
                .map(|line| format!(" * {}", line))
                .collect::<Vec<_>>()
                .join("\n")
        }
    }

    /// Formats a field for TypeScript type definition
    pub fn format_typescript_field(fld: &FieldDef) -> String {
        format!(
            "  /**\n{}\n**/\n  {}: {};",
            Self::format_docs(&fld.docs),
            fld.name,
            fld.typescript_typename()
        )
    }

    // /// Formats a field for Zod schema definition
    // #[cfg(feature = "zod")]
    // pub fn format_zod_field(fld: &FieldDef) -> String {
    //     format!("  {}: {},", fld.name, fld.zod_type())
    // }

    /// Generates safe type name by removing Json suffix for TypeScript
    pub fn safe_typescript_name(rust_name: &str) -> String {
        crate::utils::safe_type_name(rust_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field_type::{FieldDef, FieldDefType};

    #[test]
    fn test_format_docs() {
        assert_eq!(GenerationUtils::format_docs(""), "");
        assert_eq!(GenerationUtils::format_docs("Simple doc"), " * Simple doc");
        assert_eq!(
            GenerationUtils::format_docs("Line 1\nLine 2"),
            " * Line 1\n * Line 2"
        );
    }

    #[test]
    fn test_typescript_field_formatting() {
        let field = FieldDef {
            is_optional: false,
            name: "test_field".to_string(),
            docs: "Test documentation".to_string(),
            field_type: FieldDefType::String,
            is_array: false,
            array_num: None,
        };

        let formatted = GenerationUtils::format_typescript_field(&field);
        assert!(formatted.contains("test_field"));
        assert!(formatted.contains("string"));
        assert!(formatted.contains("Test documentation"));
    }
} 
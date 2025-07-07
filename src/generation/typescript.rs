//! TypeScript type generation module
//! 
//! This module handles the core TypeScript type generation that is always available
//! regardless of feature flags.
#[cfg(test)]
use crate::field_type::FieldDef;

#[cfg(test)]
use super::GenerationUtils;

#[cfg(test)]
/// TypeScript type generator
pub struct TypeScriptGenerator;

#[cfg(test)]
impl TypeScriptGenerator {
    /// Generates TypeScript type definition for a struct
    pub fn generate_struct_type(
        type_name: &str,
        fields: &[FieldDef],
        docs: &str,
    ) -> String {
        let fields_empty = fields.is_empty();
        let item_name = GenerationUtils::safe_typescript_name(type_name);
        
        if fields_empty {
            format!(
                "/**\n{}\n**/\nexport type {} = Record<string, never>;",
                GenerationUtils::format_docs(docs),
                item_name
            )
        } else {
            let type_code = fields
                .iter()
                .map(|fld| GenerationUtils::format_typescript_field(fld))
                .collect::<Vec<_>>()
                .join("\n");
            
            format!(
                "/**\n{}\n**/\nexport type {} = {{\n{}\n}};",
                GenerationUtils::format_docs(docs),
                item_name,
                type_code
            )
        }
    }

    /// Generates TypeScript type definition for a plain enum
    pub fn generate_plain_enum_type(
        type_name: &str,
        enum_options: &[String],
        docs: &str,
    ) -> String {
        let item_name = GenerationUtils::safe_typescript_name(type_name);
        let type_code = enum_options
            .iter()
            .map(|v| format!("\"{}\"", v))
            .collect::<Vec<_>>()
            .join(" | ");

        format!(
            "/**\n{}\n**/\nexport type {} = {};",
            GenerationUtils::format_docs(docs),
            item_name,
            type_code
        )
    }

    // /// Generates TypeScript type definition for a discriminated enum
    // pub fn generate_discriminated_enum_type(
    //     type_name: &str,
    //     variants: &[(String, Vec<FieldDef>, String)], // (variant_name, fields, docs)
    //     tag_name: &str,
    //     docs: &str,
    // ) -> String {
    //     let item_name = GenerationUtils::safe_typescript_name(type_name);
        
    //     let type_code_items: Vec<String> = variants
    //         .iter()
    //         .map(|(variant_name, fields, variant_docs)| {
    //             let mut variant_type_code = format!(
    //                 "{{  /**\n{}\n**/\n  {}: \"{}\";",
    //                 GenerationUtils::format_docs(variant_docs),
    //                 tag_name,
    //                 variant_name
    //             );

    //             for fld in fields {
    //                 variant_type_code.push_str(&format!(
    //                     "\n  /**\n{}\n**/\n  {}: {};",
    //                     GenerationUtils::format_docs(&fld.docs),
    //                     fld.name,
    //                     fld.typescript_typename()
    //                 ));
    //             }

    //             variant_type_code.push('}');
    //             variant_type_code
    //         })
    //         .collect();

    //     let type_code = type_code_items.join(" | ");

    //     format!(
    //         "/**\n{}\n**/\nexport type {} = {};",
    //         GenerationUtils::format_docs(docs),
    //         item_name,
    //         type_code
    //     )
    // }

    // /// Generates a complete TypeScript definition with optional imports
    // pub fn generate_complete_definition(
    //     typescript_type: &str,
    //     includes_imports: bool,
    // ) -> String {
    //     if includes_imports {
    //         #[cfg(feature = "zod")]
    //         {
    //             format!("import {{ z }} from \"zod\";\n\n{}", typescript_type)
    //         }
    //         #[cfg(not(feature = "zod"))]
    //         {
    //             typescript_type.to_string()
    //         }
    //     } else {
    //         typescript_type.to_string()
    //     }
    // }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field_type::{FieldDef, FieldDefType};

    #[test]
    fn test_generate_struct_type_empty() {
        let result = TypeScriptGenerator::generate_struct_type("TestJson", &[], "Test docs");
        assert!(result.contains("export type Test"));
        assert!(result.contains("Record<string, never>"));
        assert!(result.contains("Test docs"));
    }

    #[test]
    fn test_generate_struct_type_with_fields() {
        let fields = vec![
            FieldDef {
                is_optional: false,
                name: "id".to_string(),
                docs: "ID field".to_string(),
                field_type: FieldDefType::String,
                is_array: false,
                array_num: None,
                model_schema_prop_meta: None,
            },
            FieldDef {
                is_optional: true,
                name: "name".to_string(),
                docs: "Name field".to_string(),
                field_type: FieldDefType::String,
                is_array: false,
                array_num: None,
                model_schema_prop_meta: None,
            },
        ];

        let result = TypeScriptGenerator::generate_struct_type("UserJson", &fields, "User struct");
        assert!(result.contains("export type User"));
        assert!(result.contains("id: string"));
        assert!(result.contains("name: string | undefined"));
        assert!(result.contains("User struct"));
    }

    #[test]
    fn test_generate_plain_enum_type() {
        let options = vec!["active".to_string(), "inactive".to_string()];
        let result = TypeScriptGenerator::generate_plain_enum_type("StatusJson", &options, "Status enum");
        
        assert!(result.contains("export type Status"));
        assert!(result.contains("\"active\" | \"inactive\""));
        assert!(result.contains("Status enum"));
    }
} 
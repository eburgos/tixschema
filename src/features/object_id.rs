//! MongoDB ObjectId feature module
//! 
//! This module handles ObjectId type detection and generates appropriate
//! TypeScript and schema code when the "object_id" feature is enabled.

use crate::field_type::FieldDefType;

/// Detects if a type name represents a MongoDB ObjectId
pub fn is_object_id_type(type_name: &str) -> bool {
    type_name == "ObjectId"
}

/// Gets the FieldDefType for ObjectId
pub fn get_object_id_field_type() -> FieldDefType {
    FieldDefType::ObjectId
}

/// Generates TypeScript type name for ObjectId
pub fn get_object_id_typescript_type() -> String {
    "ObjectId".to_string()
}

/// Generates Zod schema for ObjectId with regex validation
pub fn get_object_id_zod_schema() -> String {
    "z.object({ $oid: z.string().regex(/^[a-f\\d]{24}$/i, { message: \"Invalid ObjectId\" }) })".to_string()
}

/// Generates JSON schema for ObjectId fields
pub fn get_object_id_json_schema() -> proc_macro2::TokenStream {
    quote::quote! {
        serde_json::json!({
            "type": "object",
            "properties": { "$oid": { "type": "string" } },
            "required": ["$oid"],
            "additionalProperties": false
        })
    }
}

/// Generates JSON schema for ObjectId arrays
pub fn get_object_id_array_json_schema() -> proc_macro2::TokenStream {
    let object_id_schema = get_object_id_json_schema();
    quote::quote! {
        serde_json::json!({
            "type": "array",
            "items": #object_id_schema
        })
    }
}

/// Check if we should handle this type as ObjectId
pub fn should_handle_as_object_id(type_name: &str) -> bool {
    is_object_id_type(type_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_object_id_detection() {
        assert!(is_object_id_type("ObjectId"));
        assert!(!is_object_id_type("String"));
        assert!(!is_object_id_type("UserId"));
    }

    #[test]
    fn test_object_id_typescript_type() {
        assert_eq!(get_object_id_typescript_type(), "ObjectId");
    }

    #[test]
    fn test_object_id_zod_schema() {
        let schema = get_object_id_zod_schema();
        assert!(schema.contains("$oid"));
        assert!(schema.contains("regex"));
        assert!(schema.contains("24"));
    }
} 
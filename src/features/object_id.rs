//! `MongoDB` `ObjectId` feature module.
//!
//! This module handles `ObjectId` type detection and generates appropriate
//! TypeScript and schema code when the "`object_id`" feature is enabled.

/// Detects if a type name represents a `MongoDB` `ObjectId`.
pub fn is_object_id_type(type_name: &str) -> bool {
    type_name == "ObjectId"
}

/// Generates TypeScript type name for `ObjectId`.
pub fn get_object_id_typescript_type() -> String {
    "ObjectId".to_owned()
}

#[cfg(all(feature = "object_id", any(test, feature = "zod")))]
pub fn get_object_id_zod_schema() -> String {
    "z.object({ $oid: z.string().regex(/^[a-f\\d]{24}$/i, { message: \"Invalid ObjectId\" }) })"
        .to_owned()
}

/// Check if we should handle this type as `ObjectId`.
pub fn should_handle_as_object_id(type_name: &str) -> bool {
    is_object_id_type(type_name)
}

#[cfg(test)]
mod tests;

//! `MongoDB` `ObjectId` feature module.
//!
//! This module handles `ObjectId` type detection and generates appropriate
//! TypeScript and schema code when the "`object_id`" feature is enabled.

/// The 24-character hex an `ObjectId`'s `$oid` member holds, as the regex every surface constrains
/// it by. Written once so no position can describe the same string a different way: the JSON
/// Schema `pattern` keyword and the Zod literal both read it from here.
#[cfg(any(test, feature = "zod", feature = "jsonschema"))]
pub const OBJECT_ID_HEX_PATTERN: &str = "^[a-f0-9]{24}$";

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
    get_object_id_zod_schema_with("")
}

/// The `$oid` object's Zod schema with `hex_checks` appended to the hex string it holds.
#[cfg(all(feature = "object_id", any(test, feature = "zod")))]
pub fn get_object_id_zod_schema_with(hex_checks: &str) -> String {
    format!(
        "z.object({{ $oid: z.string().regex(/{OBJECT_ID_HEX_PATTERN}/, {{ message: \"Invalid ObjectId\" }}){hex_checks} }})"
    )
}

/// Check if we should handle this type as `ObjectId`.
pub fn should_handle_as_object_id(type_name: &str) -> bool {
    is_object_id_type(type_name)
}

#[cfg(test)]
mod tests;

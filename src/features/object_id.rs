//! `MongoDB` `ObjectId` feature module.
//!
//! This module handles `ObjectId` type detection and generates appropriate
//! TypeScript and schema code when the "`object_id`" feature is enabled.

/// The 24-character hex an `ObjectId`'s `$oid` member holds, as the regex every surface constrains
/// it by. Written once so no position can describe the same string a different way: the JSON
/// Schema `pattern` keyword and the Zod literal both read it from here.
///
/// Spelled in the members both engines agree on rather than with a `\d`, for the same reason the
/// guard refuses an author one. The `regex` crate reads that class as the Unicode digits and a
/// flagless JavaScript literal reads the ASCII ones, so `[a-f\d]` admits twenty-four ARABIC-INDIC
/// digits in the generated Rust validator and refuses them wherever the schema is loaded. Hex is
/// ASCII, so writing `0-9` out leaves the value set this constant is for exactly where it was and
/// leaves one contract in place of two.
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
///
/// String checks belong on that member and never on the object around it: `$oid` is the only
/// string an `ObjectId` writes, and a `z.object` has no string check to take.
#[cfg(all(feature = "object_id", any(test, feature = "zod")))]
pub fn get_object_id_zod_schema_with(hex_checks: &str) -> String {
    format!(
        "z.object({{ $oid: z.string().regex(/{OBJECT_ID_HEX_PATTERN}/i, {{ message: \"Invalid ObjectId\" }}){hex_checks} }})"
    )
}

/// Check if we should handle this type as `ObjectId`.
pub fn should_handle_as_object_id(type_name: &str) -> bool {
    is_object_id_type(type_name)
}

#[cfg(test)]
mod tests;

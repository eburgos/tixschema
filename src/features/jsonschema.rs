//! JSON Schema generation feature module
//! 
//! This module handles JSON schema generation when the "jsonschema" feature is enabled.

/// Check if we should generate JSON schema methods
pub fn should_generate_json_schema() -> bool {
    true // Always true when this module is compiled (feature is enabled)
}

/// Generates the JSON schema method implementation for structs
pub fn generate_struct_json_schema_method(
    json_schema_fields: &[proc_macro2::TokenStream],
) -> proc_macro2::TokenStream {
    quote::quote! {
        pub fn json_schema() -> serde_json::Value {
            let mut schema_obj = serde_json::Map::new();
            schema_obj.insert("type".to_string(), serde_json::Value::String("object".to_string()));
            schema_obj.insert("additionalProperties".to_string(), serde_json::Value::Bool(false));
            let mut properties = serde_json::Map::new();
            let mut required = Vec::new();

            #(#json_schema_fields)*

            schema_obj.insert(
                "properties".to_string(),
                serde_json::Value::Object(properties),
            );

            schema_obj.insert("required".to_string(), serde_json::Value::Array(required));

            serde_json::Value::Object(schema_obj)
        }
    }
}

/// Generates the JSON schema method implementation for plain enums
pub fn generate_plain_enum_json_schema_method() -> proc_macro2::TokenStream {
    quote::quote! {
        pub fn json_schema() -> serde_json::Value {
            let mut schema_obj = serde_json::Map::new();
            schema_obj.insert("type".to_string(), serde_json::Value::String("string".to_string()));
            schema_obj.insert("enum".to_string(), serde_json::Value::Array(Self::enum_members().into_iter().map(|v| serde_json::Value::String(v)).collect()));

            serde_json::Value::Object(schema_obj)
        }
    }
}

/// Generates the JSON schema method implementation for discriminated enums
pub fn generate_discriminated_enum_json_schema_method(
    json_schema_variants: &[proc_macro2::TokenStream],
) -> proc_macro2::TokenStream {
    quote::quote! {
        pub fn json_schema() -> serde_json::Value {
            let mut schema_obj = serde_json::Map::new();
            schema_obj.insert("type".to_string(), serde_json::Value::String("object".to_string()));
            schema_obj.insert("oneOf".to_string(), {
                let result: Vec<serde_json::Value> = vec![
                    #(#json_schema_variants), *
                ];

                serde_json::Value::Array(result)
            });

            serde_json::Value::Object(schema_obj)
        }
    }
}

/// Generates JSON schema documentation for TypeScript comments
pub fn generate_json_schema_docs() -> String {
    r#"let prettified = serde_json::to_string_pretty(&Self::json_schema()).unwrap().lines().map(|l| format!(" * {l}")).collect::<Vec<_>>().join("\n");"#.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_generate_json_schema() {
        assert!(should_generate_json_schema());
    }

    #[test]
    fn test_json_schema_method_generation() {
        let fields = vec![];
        let method = generate_struct_json_schema_method(&fields);
        let method_str = method.to_string();
        
        assert!(method_str.contains("json_schema"));
        assert!(method_str.contains("serde_json"));
        assert!(method_str.contains("properties"));
        assert!(method_str.contains("required"));
    }
} 
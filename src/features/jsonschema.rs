//! JSON Schema generation feature module
//!
//! This module handles JSON schema generation when the "jsonschema" feature is enabled.

/// Check if we should generate JSON schema methods
#[cfg(test)]
pub const fn should_generate_json_schema() -> bool {
    true // Always true when this module is compiled (feature is enabled)
}

/// Generates the JSON schema method implementation for structs.
///
/// When the struct has `#[serde(flatten)]` fields, the base properties are
/// distributed into each branch of the flattened types' schemas (cross-product
/// over any `oneOf`), producing a strict closed schema that validates the base
/// fields and the flattened union together.
pub fn generate_struct_json_schema_method(
    json_schema_fields: &[proc_macro2::TokenStream],
    flatten_json_schemas: &[proc_macro2::TokenStream],
) -> proc_macro2::TokenStream {
    if flatten_json_schemas.is_empty() {
        return quote::quote! {
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
        };
    }

    quote::quote! {
        pub fn json_schema() -> serde_json::Value {
            fn merge_object_schemas(
                a: &serde_json::Map<String, serde_json::Value>,
                b: &serde_json::Map<String, serde_json::Value>,
            ) -> serde_json::Map<String, serde_json::Value> {
                let mut out = serde_json::Map::new();
                out.insert("type".to_string(), serde_json::Value::String("object".to_string()));
                let mut properties = serde_json::Map::new();
                for src in [a, b] {
                    if let Some(p) = src.get("properties").and_then(serde_json::Value::as_object) {
                        for (k, v) in p {
                            properties.insert(k.clone(), v.clone());
                        }
                    }
                }
                out.insert("properties".to_string(), serde_json::Value::Object(properties));
                let mut required: Vec<serde_json::Value> = Vec::new();
                for src in [a, b] {
                    if let Some(r) = src.get("required").and_then(serde_json::Value::as_array) {
                        for item in r {
                            if !required.contains(item) {
                                required.push(item.clone());
                            }
                        }
                    }
                }
                out.insert("required".to_string(), serde_json::Value::Array(required));
                out.insert("additionalProperties".to_string(), serde_json::Value::Bool(false));
                out
            }

            fn branches_of(
                schema: &serde_json::Value,
            ) -> Vec<serde_json::Map<String, serde_json::Value>> {
                if let Some(one_of) = schema.get("oneOf").and_then(serde_json::Value::as_array) {
                    one_of.iter().filter_map(|b| b.as_object().cloned()).collect()
                } else if let Some(obj) = schema.as_object() {
                    vec![obj.clone()]
                } else {
                    Vec::new()
                }
            }

            let mut schema_obj = serde_json::Map::new();
            schema_obj.insert("type".to_string(), serde_json::Value::String("object".to_string()));
            schema_obj.insert("additionalProperties".to_string(), serde_json::Value::Bool(false));
            let mut properties = serde_json::Map::new();
            let mut required = Vec::new();

            #(#json_schema_fields)*

            schema_obj.insert("properties".to_string(), serde_json::Value::Object(properties));
            schema_obj.insert("required".to_string(), serde_json::Value::Array(required));

            let flattened: Vec<serde_json::Value> = vec![ #(#flatten_json_schemas),* ];

            let mut branches: Vec<serde_json::Map<String, serde_json::Value>> = vec![schema_obj];
            for fs in &flattened {
                let fs_branches = branches_of(fs);
                if fs_branches.is_empty() {
                    continue;
                }
                let mut next = Vec::new();
                for base in &branches {
                    for fb in &fs_branches {
                        next.push(merge_object_schemas(base, fb));
                    }
                }
                branches = next;
            }

            if branches.len() == 1 {
                serde_json::Value::Object(branches.swap_remove(0))
            } else {
                let mut out = serde_json::Map::new();
                out.insert("type".to_string(), serde_json::Value::String("object".to_string()));
                out.insert(
                    "oneOf".to_string(),
                    serde_json::Value::Array(
                        branches.into_iter().map(serde_json::Value::Object).collect(),
                    ),
                );
                serde_json::Value::Object(out)
            }
        }
    }
}

/// Generates the JSON schema method implementation for plain enums
pub fn generate_plain_enum_json_schema_method(
    enumerated: &[proc_macro2::TokenStream],
) -> proc_macro2::TokenStream {
    quote::quote! {
        pub fn json_schema() -> serde_json::Value {
            let mut schema_obj = serde_json::Map::new();
            schema_obj.insert("type".to_string(), serde_json::Value::String("string".to_string()));
            schema_obj.insert("enum".to_string(), serde_json::Value::Array(
                [#(#enumerated),*].into_iter().map(|v: &str| serde_json::Value::String(v.to_string())).collect()
            ));

            serde_json::Value::Object(schema_obj)
        }
    }
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
        let method = generate_struct_json_schema_method(&fields, &[]);
        let method_str = method.to_string();

        assert!(method_str.contains("json_schema"));
        assert!(method_str.contains("serde_json"));
        assert!(method_str.contains("properties"));
        assert!(method_str.contains("required"));
    }

    #[test]
    fn test_json_schema_method_flatten_emits_merge() {
        let fields = vec![];
        let no_flatten = generate_struct_json_schema_method(&fields, &[]).to_string();
        let with_flatten = generate_struct_json_schema_method(
            &fields,
            &[quote::quote! { serde_json::json!({ "type": "object" }) }],
        )
        .to_string();

        assert!(!no_flatten.contains("merge_object_schemas"));
        assert!(with_flatten.contains("merge_object_schemas"));
        assert!(with_flatten.contains("oneOf"));
    }
}

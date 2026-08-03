//! JSON Schema generation feature module.
//!
//! This module handles JSON schema generation when the "jsonschema" feature is enabled.

/// What every pointer the crate writes opens with; what follows it is the name being deferred.
const DEFS_PREFIX: &str = "#/$defs/";

/// Check if we should generate JSON schema methods.
#[cfg(test)]
pub const fn should_generate_json_schema() -> bool {
    true // Always true when this module is compiled (feature is enabled)
}

/// Where a document holds the definition of `def_name`.
///
/// The crate writes draft 2020-12 (`prefixItems` with `"items": false` is that draft's fixed-arity
/// array, and the draft before it spells the same array with `items`/`additionalItems`), whose
/// deferred schema is a `$ref` into the document's own `$defs`.
fn defs_pointer(def_name: &str) -> String {
    format!("{DEFS_PREFIX}{def_name}")
}

/// The pair of JSON-schema methods every schema module publishes.
///
/// `json_schema` is the document a caller asks for. `json_schema_within` is that same description
/// written into a document already being built, and carries the two things that have to travel
/// with it: the names whose descriptions are still being written, and the definitions the
/// document's root must hold.
///
/// A cycle is only knowable there. A type names another by inlining it, and no expansion can see
/// the cycle it is part of — the other type may not have been expanded yet, and one that has been
/// cannot be revisited. So the name is recognized while the description runs: a name re-entered
/// while still in flight describes as a `$ref` into `$defs`, and the frame that put it in flight
/// hoists its body to the root that pointer resolves against.
pub fn json_schema_methods(
    def_name: &str,
    body: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let pointer = defs_pointer(def_name);
    quote::quote! {
        pub fn json_schema() -> serde_json::Value {
            let mut in_flight: Vec<&'static str> = Vec::new();
            let mut hoisted_defs = serde_json::Map::new();
            let described = Self::json_schema_within(&mut in_flight, &mut hoisted_defs);
            if hoisted_defs.is_empty() {
                return described;
            }
            // The pointers into them are from the root, so the definitions join it — ahead of the
            // description, which is the rest of the document. Every description the crate writes
            // is an object, which is what can take them as a member.
            let mut rooted = serde_json::Map::new();
            rooted.insert("$defs".to_string(), serde_json::Value::Object(hoisted_defs));
            if let serde_json::Value::Object(members) = described {
                rooted.extend(members);
            }
            serde_json::Value::Object(rooted)
        }

        pub fn json_schema_within(
            in_flight: &mut Vec<&'static str>,
            hoisted_defs: &mut serde_json::Map<String, serde_json::Value>,
        ) -> serde_json::Value {
            if in_flight.contains(&#def_name) {
                // Reserved rather than written: the frame that put this name in flight is still
                // writing the body, and fills the entry in once it has one.
                hoisted_defs.entry(#def_name).or_insert(serde_json::Value::Null);
                return serde_json::json!({ "$ref": #pointer });
            }
            in_flight.push(#def_name);
            let described = #body;
            in_flight.pop();
            if hoisted_defs.contains_key(#def_name) {
                hoisted_defs.insert(#def_name.to_string(), described);
                return serde_json::json!({ "$ref": #pointer });
            }
            described
        }
    }
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
    def_name: &str,
) -> proc_macro2::TokenStream {
    let body = if flatten_json_schemas.is_empty() {
        closed_object_body(json_schema_fields)
    } else {
        flattened_object_body(json_schema_fields, flatten_json_schemas)
    };
    json_schema_methods(def_name, &body)
}

/// The struct's own fields as one closed object.
fn closed_object_body(json_schema_fields: &[proc_macro2::TokenStream]) -> proc_macro2::TokenStream {
    quote::quote! {
        {
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

/// The struct's own fields distributed into each branch of the flattened types' schemas.
fn flattened_object_body(
    json_schema_fields: &[proc_macro2::TokenStream],
    flatten_json_schemas: &[proc_macro2::TokenStream],
) -> proc_macro2::TokenStream {
    let defs_prefix = DEFS_PREFIX;
    quote::quote! {
        {
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

            // A flattened base that names itself describes as a reference into the definitions
            // being hoisted, and a reference is the one thing with no properties to merge. The
            // body it points at is written by then — the frame that deferred the name fills the
            // entry in before it returns — so the merge reads it back. The entry is still only
            // reserved when the flatten is itself what came back around, and a base that holds
            // itself has no closed object to contribute.
            fn deferred_body<'doc>(
                schema: &'doc serde_json::Value,
                defs: &'doc serde_json::Map<String, serde_json::Value>,
            ) -> Option<&'doc serde_json::Value> {
                let pointer = schema.get("$ref")?.as_str()?;
                let name = pointer.strip_prefix(#defs_prefix)?;
                defs.get(name).filter(|body| body.is_object())
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
                let fs_body = deferred_body(fs, hoisted_defs).unwrap_or(fs);
                let fs_branches = branches_of(fs_body);
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

/// Generates the JSON schema method implementation for plain enums.
pub fn generate_plain_enum_json_schema_method(
    enumerated: &[proc_macro2::TokenStream],
    def_name: &str,
) -> proc_macro2::TokenStream {
    let body = quote::quote! {
        {
            let mut schema_obj = serde_json::Map::new();
            schema_obj.insert("type".to_string(), serde_json::Value::String("string".to_string()));
            schema_obj.insert("enum".to_string(), serde_json::Value::Array(
                [#(#enumerated),*].into_iter().map(|v: &str| serde_json::Value::String(v.to_string())).collect()
            ));

            serde_json::Value::Object(schema_obj)
        }
    };
    json_schema_methods(def_name, &body)
}

#[cfg(test)]
mod tests;

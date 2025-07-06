use std::fmt::Write;
use std::{collections::HashMap, env};

use proc_macro::TokenStream;
use quote::quote;
use syn::{Field, Item, parse_macro_input};

use crate::{
    field_type::{
        FieldDef, FieldDefType, SerdeFieldMeta, SerdeTypeMeta, get_field_def, is_plain_enum,
        parse_serde_field_attributes, parse_serde_type_attributes,
    },
    safe_type_name,
    utils::{get_enum_docs, get_field_docs, get_struct_docs, get_variant_docs},
};

/// Executes the model_schema macro processing to generate TypeScript and Zod schema definitions.
///
/// This function is the main entry point for the model_schema macro and handles both struct and enum types.
pub(crate) fn exec_model_schema(_args: TokenStream, input: TokenStream) -> TokenStream {
    let item = parse_macro_input!(input as Item);
    match item {
        Item::Struct(item_struct) => process_struct(item_struct),
        Item::Enum(item_enum) => process_enum(item_enum),
        _ => panic!("Unsupported target for model_schema"),
    }
}

/// Processes a struct item and generates TypeScript and Zod schema definitions for it.
fn process_struct(mut item_struct: syn::ItemStruct) -> TokenStream {
    let name = &item_struct.ident;
    let serde_type_meta = parse_serde_type_attributes(&item_struct.attrs);
    let item_name = safe_type_name(&name.to_string());

    // Process all fields in the struct
    let mut field_defs = Vec::new();
    for field in &mut item_struct.fields {
        let f_def = process_field(&serde_type_meta, field);
        field_defs.push(f_def);
    }

    // Generate TypeScript type and Zod schema code
    let mut type_code = String::new();
    let mut schema_code = String::new();
    let mut opts = Vec::new();
    let mut json_schema_fields: Vec<proc_macro2::TokenStream> = Vec::new();

    for fld in field_defs {
        write_field_type_and_schema(&mut type_code, &mut schema_code, &fld);

        if fld.is_optional {
            opts.push(fld.name.to_string());
        }

        json_schema_fields.push(build_field_schema(&fld));
    }

    let fields_empty = json_schema_fields.is_empty();
    let show_opts = show_optionals(&opts);

    let docs = match get_struct_docs(&item_struct) {
        Some(doc_lines) => doc_lines
            .into_iter()
            .flat_map(|v| v.lines().map(|l| l.to_owned()).collect::<Vec<_>>())
            .chain(vec!["".to_string()])
            .map(|l| format!(" * {l}"))
            .collect::<Vec<_>>()
            .join("\n"),
        None => [name.to_string(), "".to_string()]
            .into_iter()
            .map(|l| format!(" * {l}"))
            .collect::<Vec<_>>()
            .join("\n"),
    };

    // Generate the final output
    let output = quote! {
        #item_struct

        impl #name {
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

            pub fn ts_definition() -> String {
                let docs = #docs;
                let prettified = serde_json::to_string_pretty(&Self::json_schema()).unwrap().lines().map(|l| format!(" * {l}")).collect::<Vec<_>>().join("\n");
                let bundled_docs = format!("/**\n{docs}\n * JSON Schema:\n{prettified}\n **/\n");
                let type_def = {
                    match #fields_empty {
                        true => {
                            format!(r#"{bundled_docs}export type {} = Record<string, never>;"#, #item_name)
                        },
                        false => {
                            format!("{bundled_docs}export type {} = {{\n{}}};", #item_name, #type_code)
                        }
                    }
                };
                let schema_def = format!(r#"export const {}$Schema: z.Schema<{}, z.ZodTypeDef, unknown> = z.strictObject({{
{}
}}){};"#, #item_name, #item_name, #schema_code, #show_opts);
                format!("{type_def}\n\n{schema_def}")
            }
        }
    };

    if env::var("RUST_LOG") == Ok(String::from("trace")) {
        let output_str = output.to_string();
        println!("{output_str}");
    }

    TokenStream::from(output)
}

/// Processes an enum item and generates TypeScript and Zod schema definitions for it.
fn process_enum(item_enum: syn::ItemEnum) -> TokenStream {
    let name = item_enum.ident.clone();
    let serde_type_meta = parse_serde_type_attributes(&item_enum.attrs);
    let item_name = safe_type_name(&name.to_string());

    if is_plain_enum(&item_enum) {
        process_plain_enum(item_enum, &name, &serde_type_meta, &item_name)
    } else {
        process_discriminated_enum(item_enum, &name, &serde_type_meta, &item_name)
    }
}

/// Processes a plain enum (simple string enum in TypeScript) and generates its definitions.
fn process_plain_enum(
    mut item_enum: syn::ItemEnum,
    name: &syn::Ident,
    serde_type_meta: &SerdeTypeMeta,
    item_name: &str,
) -> TokenStream {
    let mut enum_options = Vec::new();

    for item in &mut item_enum.variants {
        let serde_field_meta = parse_serde_field_attributes(&item.attrs);
        let final_name = get_final_name(item.ident.to_string(), &serde_field_meta, serde_type_meta);
        enum_options.push(final_name);
    }

    // Generate TypeScript and Zod schema representations
    let type_code = enum_options
        .iter()
        .map(|v| format!("\"{v}\""))
        .collect::<Vec<_>>()
        .join(" | ");

    let schema_code = enum_options
        .iter()
        .map(|v| format!("\"{v}\""))
        .collect::<Vec<_>>()
        .join(", ");

    // Enumerate the strings with indices
    let enumerated = enum_options.iter().map(|v| {
        quote! { #v }
    });

    let docs = match get_enum_docs(&item_enum) {
        Some(doc_lines) => doc_lines
            .into_iter()
            .flat_map(|v| v.lines().map(|l| l.to_owned()).collect::<Vec<_>>())
            .chain(vec!["".to_string()])
            .map(|l| format!(" * {l}"))
            .collect::<Vec<_>>()
            .join("\n"),
        None => [name.to_string(), "".to_string()]
            .into_iter()
            .map(|l| format!(" * {l}"))
            .collect::<Vec<_>>()
            .join("\n"),
    };

    let output = quote! {
        #item_enum

        impl #name {
            pub fn json_schema() -> serde_json::Value {
                let mut schema_obj = serde_json::Map::new();
                schema_obj.insert("type".to_string(), serde_json::Value::String("string".to_string()));
                schema_obj.insert("enum".to_string(), serde_json::Value::Array(Self::enum_members().into_iter().map(|v| serde_json::Value::String(v)).collect()));

                serde_json::Value::Object(schema_obj)
            }

            pub fn ts_definition() -> String {
                let prettified = serde_json::to_string_pretty(&Self::json_schema()).unwrap().lines().map(|l| format!(" * {l}")).collect::<Vec<_>>().join("\n");
                let docs = #docs;
                let bundled_docs = format!("/**\n{docs}\n * JSON Schema:\n{prettified}\n **/\n");
                let type_def = format!(r#"{bundled_docs}export type {} = {};"#, #item_name, #type_code);
                let schema_def = format!(r#"export const {}$Schema: z.Schema<{}> = z.enum([{}]);"#, #item_name, #item_name, #schema_code);
                format!("{type_def}\n\n{schema_def}")
            }

            pub fn enum_members() -> Vec<String> {
                [
                    #(#enumerated),*
                ].iter().map(|v| v.to_string()).collect::<Vec<_>>()
            }
        }
    };

    if env::var("RUST_LOG") == Ok(String::from("trace")) {
        let output_str = output.to_string();
        println!("{output_str}");
    }

    TokenStream::from(output)
}

/// Processes a discriminated enum (tagged union in TypeScript) and generates its definitions.
fn process_discriminated_enum(
    mut item_enum: syn::ItemEnum,
    name: &syn::Ident,
    serde_type_meta: &SerdeTypeMeta,
    item_name: &str,
) -> TokenStream {
    let mut discriminator_field_defs: HashMap<String, Vec<FieldDef>> = HashMap::new();
    let mut discriminator_field_docs: HashMap<String, String> = HashMap::new();
    let mut json_schema_variants: Vec<proc_macro2::TokenStream> = Vec::new();

    let tag_name = serde_type_meta
        .tag
        .as_ref()
        .map_or_else(|| "type".to_string(), Clone::clone);

    // Process each variant in the enum
    for item in &mut item_enum.variants {
        let serde_field_meta = parse_serde_field_attributes(&item.attrs);
        let final_name = get_final_name(item.ident.to_string(), &serde_field_meta, serde_type_meta);

        let mut field_defs: Vec<FieldDef> = Vec::new();
        let mut json_schema_fields: Vec<proc_macro2::TokenStream> = Vec::new();

        for field in &mut item.fields {
            let f_def = process_field(serde_type_meta, field);
            json_schema_fields.push(build_field_schema(&f_def));
            field_defs.push(f_def);
        }

        discriminator_field_defs.insert(final_name.clone(), field_defs);
        let discriminator_docs = match get_variant_docs(item) {
            Some(doc_lines) => doc_lines
                .into_iter()
                .flat_map(|v| v.lines().map(|l| l.to_owned()).collect::<Vec<_>>())
                .chain(vec!["".to_string()])
                .map(|l| format!(" * {l}"))
                .collect::<Vec<_>>()
                .join("\n"),
            None => [final_name.to_string(), "".to_string()]
                .into_iter()
                .map(|l| format!(" * {l}"))
                .collect::<Vec<_>>()
                .join("\n"),
        };
        discriminator_field_docs.insert(final_name, discriminator_docs);
    }

    let mut type_code_items = Vec::new();
    let mut schema_code_items = Vec::new();

    // Generate TypeScript and Zod schema for each variant
    for (discriminator_value, field_defs) in discriminator_field_defs {
        let (variant_type_code, variant_schema_code, optional_fields, json_schema_variant) =
            generate_variant_code(
                &tag_name,
                &discriminator_value,
                field_defs,
                &discriminator_field_docs[&discriminator_value],
            );

        type_code_items.push(variant_type_code);
        schema_code_items.push((variant_schema_code, optional_fields));
        json_schema_variants.push(json_schema_variant);
    }

    let main_schema_code = quote! {
        let mut schema_obj = serde_json::Map::new();
        schema_obj.insert("type".to_string(), serde_json::Value::String("object".to_string()));
        schema_obj.insert("oneOf".to_string(), {
            let result: Vec<serde_json::Value> = vec![
                #(#json_schema_variants), *
            ];

            serde_json::Value::Array(result)
        });

        serde_json::Value::Object(schema_obj)
    };

    let type_code = type_code_items.join(" | ");
    let schema_code = format!(
        "z.discriminatedUnion(\"{tag_name}\", [{}])",
        schema_code_items
            .iter()
            .map(|(v, opts)| format!("z.strictObject({}){}", v, show_optionals(opts)))
            .collect::<Vec<_>>()
            .join(", ")
    );

    let docs = match get_enum_docs(&item_enum) {
        Some(doc_lines) => doc_lines
            .into_iter()
            .flat_map(|v| v.lines().map(|l| l.to_owned()).collect::<Vec<_>>())
            .chain(vec!["".to_string()])
            .map(|l| format!(" * {l}"))
            .collect::<Vec<_>>()
            .join("\n"),
        None => [name.to_string(), "".to_string()]
            .into_iter()
            .map(|l| format!(" * {l}"))
            .collect::<Vec<_>>()
            .join("\n"),
    };

    let output = quote! {
        #item_enum

        impl #name {
            pub fn json_schema() -> serde_json::Value {
                #main_schema_code
            }

            pub fn ts_definition() -> String {
                let prettified = serde_json::to_string_pretty(&Self::json_schema()).unwrap().lines().map(|l| format!(" * {l}")).collect::<Vec<_>>().join("\n");
                let docs = #docs;
                let bundled_docs = format!("/**\n{docs}\n * JSON Schema:\n{prettified}\n **/\n");
                let type_def = format!(r#"{bundled_docs}export type {} = {};"#, #item_name, #type_code);
                let schema_def = format!(r#"export const {}$Schema: z.Schema<{}, z.ZodTypeDef, unknown> = {};"#, #item_name, #item_name, #schema_code);
                format!("{type_def}\n\n{schema_def}")
            }
        }
    };

    if env::var("RUST_LOG") == Ok(String::from("trace")) {
        let output_str = output.to_string();
        println!("{output_str}");
    }

    TokenStream::from(output)
}

fn generate_type_schema(
    fld: &FieldDef,
    field_name_str: &str,
    type_json_schema: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    if fld.is_array {
        quote! {
            properties.insert(#field_name_str.to_string(), {
                serde_json::json!({
                    "type": "array",
                    "items": #type_json_schema
                })
            });
        }
    } else {
        quote! {
            properties.insert(#field_name_str.to_string(), #type_json_schema);
        }
    }
}

/// Generates TypeScript and Zod schema code for a discriminated enum variant.
fn generate_variant_code(
    tag_name: &str,
    discriminator_value: &str,
    field_defs: Vec<FieldDef>,
    discriminator_docs: &str,
) -> (String, String, Vec<String>, proc_macro2::TokenStream) {
    // Generate TypeScript type code
    let mut variant_type_code =
        format!("{{  /**\n{discriminator_docs}\n**/\n  {tag_name}: \"{discriminator_value}\";\n");

    // Generate Zod schema code
    let mut variant_schema_code =
        format!("{{\n  {tag_name}: z.literal(\"{discriminator_value}\"),\n");

    let mut optional_fields = Vec::new();
    let mut json_schema_variant_fields = Vec::new();

    // Process each field in the variant
    for fld in &field_defs {
        // Add TypeScript type definition
        if let Err(err) = writeln!(
            variant_type_code,
            "  /**\n{}\n**/\n  {}: {};",
            fld.docs,
            fld.name,
            fld.typescript_typename()
        ) {
            panic!("Failed to write TypeScript type: {err}");
        }

        // Add Zod schema definition
        if let Err(err) = writeln!(variant_schema_code, "  {}: {},", fld.name, fld.zod_type()) {
            panic!("Failed to write Zod schema: {err}");
        }

        if fld.name != tag_name {
            json_schema_variant_fields.push(build_field_schema(fld));
        }

        if fld.is_optional {
            optional_fields.push(fld.name.to_string());
        }
    }

    // Complete the type and schema code
    variant_type_code.push('}');
    variant_schema_code.push('}');

    // Create JSON schema for this variant
    let discriminator_value_str = discriminator_value.to_string();
    let tag_name_str = tag_name.to_string();

    let json_schema_variant = quote! {
        {
            let mut schema_obj = serde_json::Map::new();
            schema_obj.insert(
                "additionalProperties".to_string(),
                serde_json::Value::Bool(false),
            );
            let mut properties = serde_json::Map::new();
            let mut required = Vec::new();

            properties.insert(
                #tag_name_str.to_string(),
                serde_json::json!({
                    "type": "string",
                    "const": #discriminator_value_str,
                }),
            );
            required.push(serde_json::Value::String(#tag_name_str.to_string()));

            #(#json_schema_variant_fields)*

            schema_obj.insert(
                "properties".to_string(),
                serde_json::Value::Object(properties),
            );

            schema_obj.insert("required".to_string(), serde_json::Value::Array(required));

            serde_json::Value::Object(schema_obj)
        }
    };

    (
        variant_type_code,
        variant_schema_code,
        optional_fields,
        json_schema_variant,
    )
}

/// Builds JSON schema for a field.
fn build_field_schema(fld: &FieldDef) -> proc_macro2::TokenStream {
    let field_name = &fld.name;
    let field_name_str = field_name.to_string();
    let field_type = &fld.field_type;

    let schema_code = match field_type {
        FieldDefType::String => {
            if fld.is_array {
                quote! {
                    properties.insert(#field_name_str.to_string(), {
                        serde_json::json!({
                            "type": "array",
                            "items": serde_json::json!({ "type": "string" })
                        })
                    });
                }
            } else {
                quote! {
                    properties.insert(#field_name_str.to_string(), {
                        serde_json::json!({
                            "type": "string",
                        })
                    });
                }
            }
        }
        FieldDefType::U32 | FieldDefType::U16 | FieldDefType::U8 => {
            if fld.is_array {
                quote! {
                    properties.insert(#field_name_str.to_string(), {
                        serde_json::json!({
                            "type": "array",
                            "items": serde_json::json!({ "type": "integer" })
                        })
                    });
                }
            } else {
                quote! {
                    properties.insert(#field_name_str.to_string(), {
                        serde_json::json!({
                            "type": "integer",
                        })
                    });
                }
            }
        }
        FieldDefType::F32 => {
            if fld.is_array {
                quote! {
                    properties.insert(#field_name_str.to_string(), {
                        serde_json::json!({
                            "type": "array",
                            "items": serde_json::json!({ "type": "number" })
                        })
                    });
                }
            } else {
                quote! {
                    properties.insert(#field_name_str.to_string(), {
                        serde_json::json!({
                            "type": "number",
                        })
                    });
                }
            }
        }
        FieldDefType::Boolean => {
            if fld.is_array {
                quote! {
                    properties.insert(#field_name_str.to_string(), {
                        serde_json::json!({
                            "type": "array",
                            "items": serde_json::json!({ "type": "boolean" })
                        })
                    });
                }
            } else {
                quote! {
                    properties.insert(#field_name_str.to_string(), {
                        serde_json::json!({
                            "type": "boolean",
                        })
                    });
                }
            }
        }
        FieldDefType::SiblingType(name, lst) => {
            if env::var("RUST_LOG") == Ok(String::from("trace")) {
                println!("SiblingType => name: {name}, lst: {lst:?}");
            }
            if (name == "Vec" || name == "HashSet") && lst.len() == 1 {
                quote! {
                    properties.insert(#field_name_str.to_string(), {
                        serde_json::json!({
                            "type": "array",
                            "items": {
                                "type": "string", // This would need to be mapped based on inner_type
                            }
                        })
                    });
                }
            } else if (name == "HashMap" || name == "BTreeMap") && lst.len() == 2 {
                if env::var("RUST_LOG") == Ok(String::from("trace")) {
                    println!("HashMap => field_name: {field_name_str}, lst: {lst:?}");
                }
                quote! {
                    properties.insert(#field_name_str.to_string(), {
                        serde_json::json!({
                            "type": "object",
                            "additionalProperties": true
                        })
                    });
                }
            } else if lst.is_empty() {
                let name_ident = proc_macro2::Ident::new(
                    format!("{name}Json").as_str(),
                    proc_macro2::Span::call_site(),
                );
                let type_json_schema = quote! { #name_ident::json_schema() };

                generate_type_schema(fld, &field_name_str, type_json_schema)
            } else {
                panic!("Unsupported generic type: {name} - {lst:?}");
            }
        }
        FieldDefType::Map(key, value) => {
            if env::var("RUST_LOG") == Ok(String::from("trace")) {
                println!("Map => field_name: {field_name_str}, key: {key:?}, value: {value:?}");
            }

            match &key.field_type {
                FieldDefType::String => match &value.field_type {
                    FieldDefType::String => {
                        quote! {
                            properties.insert(#field_name_str.to_string(), {
                                serde_json::json!({
                                    "type": "object",
                                    "additionalProperties": {
                                        "type": "string"
                                    }
                                })
                            });
                        }
                    }
                    _ => {
                        panic!("Unsupported map key type: {:?}", value.field_type);
                    }
                },
                FieldDefType::SiblingType(key_type_name, lst) if lst.is_empty() => {
                    let key_type_name_ident = proc_macro2::Ident::new(
                        format!("{key_type_name}Json").as_str(),
                        proc_macro2::Span::call_site(),
                    );

                    let value_schema_code = match &value.field_type {
                        FieldDefType::SiblingType(value_type_name, lst) if lst.is_empty() => {
                            let value_type_name_ident = proc_macro2::Ident::new(
                                format!("{value_type_name}Json").as_str(),
                                proc_macro2::Span::call_site(),
                            );
                            quote! { let value_schema = #value_type_name_ident::json_schema(); }
                        }
                        _ => {
                            panic!("Unsupported map value type: {:?}", value.field_type);
                        }
                    };

                    quote! {
                        let mut map_properties = serde_json::Map::new();

                        #value_schema_code

                        for enum_key in #key_type_name_ident::enum_members() {
                            map_properties.insert(enum_key.to_string(), value_schema.clone());
                        }

                        let mut json_schema_def = serde_json::json!({
                            "type": "object",
                            "properties": map_properties,
                            "additionalProperties": false
                        });

                        properties.insert(#field_name_str.to_string(), {
                            json_schema_def
                        });
                    }
                }

                _ => {
                    if env::var("RUST_LOG") == Ok(String::from("trace")) {
                        println!("Map Key Type {:?}", key.field_type);
                    }

                    quote! {
                        properties.insert(#field_name_str.to_string(), {
                            serde_json::json!({
                                "type": "object",
                                "additionalProperties": true
                            })
                        });
                    }
                    // panic!("Unsupported map key type: {:?}", key.field_type);
                }
            }
        }
        fld_def => {
            if env::var("RUST_LOG") == Ok(String::from("trace")) {
                println!("Other => field_name: {field_name_str}, fld_def: {fld_def:?}");
            }
            let name = &fld.name;
            let name_ident = proc_macro2::Ident::new(
                format!("{name}Json").as_str(),
                proc_macro2::Span::call_site(),
            );
            let type_json_schema = quote! { #name_ident::json_schema() };
            quote! {
                properties.insert(#field_name_str.to_string(), #type_json_schema);
            }
        }
    };

    let required_code = if !fld.is_optional {
        quote! {
            required.push(serde_json::Value::String(#field_name_str.to_string()));
        }
    } else {
        quote! {}
    };

    quote! {
        #schema_code
        #required_code
    }
}

/// Writes the TypeScript type and Zod schema for a field to the provided buffers.
fn write_field_type_and_schema(type_code: &mut String, schema_code: &mut String, fld: &FieldDef) {
    if let Err(err) = writeln!(
        type_code,
        "  /**\n{}\n**/\n  {}: {};",
        fld.docs,
        fld.name,
        fld.typescript_typename()
    ) {
        panic!("Failed to write TypeScript type: {err}");
    }

    if let Err(err) = writeln!(schema_code, "  {}: {},", fld.name, fld.zod_type()) {
        panic!("Failed to write Zod schema: {err}");
    }
}

/// Generates optional fields transformation code for Zod schema.
fn show_optionals(opts: &[String]) -> String {
    if opts.is_empty() {
        String::new()
    } else {
        format!(
            r#".transform(args => Object.assign(args, {{
  {}
}}))"#,
            opts.iter()
                .map(|v| format!("{v}: args.{v}"))
                .collect::<Vec<_>>()
                .join(
                    r#",
  "#
                )
        )
    }
}

/// Processes a field and returns its definition.
fn process_field(serde_type_meta: &SerdeTypeMeta, field: &mut Field) -> FieldDef {
    let mut new_attrs = Vec::new();
    let serde_field_meta = parse_serde_field_attributes(&field.attrs);

    // Filter out model_schema_prop attributes
    for attr in &field.attrs {
        if !attr.path().is_ident("model_schema_prop") {
            new_attrs.push(attr.clone());
        }
    }

    field.attrs = new_attrs;

    let field_type: &syn::Type = &field.ty;
    let name = field
        .ident
        .as_ref()
        .map(ToString::to_string)
        .into_iter()
        .collect::<String>();

    let final_name = get_final_name(name, &serde_field_meta, serde_type_meta);
    let field_docs = match get_field_docs(field) {
        Some(doc_lines) => doc_lines
            .into_iter()
            .flat_map(|v| v.lines().map(|l| l.to_owned()).collect::<Vec<_>>())
            .chain(vec!["".to_string()])
            .map(|l| format!(" * {l}"))
            .collect::<Vec<_>>()
            .join("\n"),
        None => [final_name.to_string(), "".to_string()]
            .into_iter()
            .map(|l| format!(" * {l}"))
            .collect::<Vec<_>>()
            .join("\n"),
    };
    get_field_def(&final_name, field_type, &field_docs)
}

/// Gets the final name for a field or enum variant, considering serde attributes.
fn get_final_name(
    name: String,
    serde_field_meta: &SerdeFieldMeta,
    serde_type_meta: &SerdeTypeMeta,
) -> String {
    if let Some(rename) = &serde_field_meta.rename {
        rename.clone()
    } else if serde_type_meta.rename_all == Some("camelCase".to_string()) {
        snake_to_camel(&name)
    } else if serde_type_meta.rename_all == Some("lowercase".to_string()) {
        name.to_lowercase()
    } else {
        name
    }
}

/// Converts a snake_case string to camelCase.
fn snake_to_camel(s: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = false;

    for (i, c) in s.chars().enumerate() {
        if c == '_' {
            capitalize_next = true;
        } else if i == 0 {
            // Force the first character to lowercase
            result.push(c.to_lowercase().next().unwrap());
        } else if capitalize_next {
            // Capitalize after an underscore
            result.push(c.to_uppercase().next().unwrap());
            capitalize_next = false;
        } else {
            // Keep other characters as is
            result.push(c);
        }
    }

    result
}

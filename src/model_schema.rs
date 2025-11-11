use core::fmt::Write;
use std::{collections::HashMap, env};

use proc_macro::TokenStream;
use quote::quote;
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::{Field, Ident, Item, ItemType, MetaNameValue, Token, parse_macro_input};

#[cfg(feature = "typescript")]
use syn::GenericParam;

use crate::{
    field_type::{FieldDef, FieldDefType, get_field_def, is_plain_enum},
    safe_type_name,
    utils::{extract_example_from_docs, get_field_docs, get_variant_docs, lookup_alias_info},
};

#[cfg(feature = "serde")]
use crate::field_type::{parse_serde_field_attributes, parse_serde_type_attributes};

#[cfg(any(feature = "typescript", feature = "zod"))]
use crate::utils::{
    compute_alias_export_name, format_docs_for_ts, get_enum_docs, get_item_docs, get_struct_docs,
    register_alias_info, strip_examples_from_docs, to_snake_case,
};

#[derive(Default, Clone)]
struct ModelSchemaArgs {
    name_override: Option<String>,
}

fn parse_model_schema_args(args: TokenStream) -> ModelSchemaArgs {
    let mut result = ModelSchemaArgs::default();

    if args.is_empty() {
        return result;
    }

    let parser = Punctuated::<MetaNameValue, Token![,]>::parse_terminated;
    if let Ok(parsed) = parser.parse(args) {
        for meta in parsed {
            if meta.path.is_ident("name")
                && let syn::Expr::Lit(expr_lit) = meta.value
                && let syn::Lit::Str(lit_str) = expr_lit.lit
            {
                result.name_override = Some(lit_str.value());
            }
        }
    }

    result
}

/// Executes the `model_schema` macro processing to generate TypeScript and Zod schema definitions.
///
/// This function is the main entry point for the `model_schema` macro and handles both struct and enum types.
pub fn exec_model_schema(args: TokenStream, input: TokenStream) -> TokenStream {
    let parsed_args = parse_model_schema_args(args);
    let item = parse_macro_input!(input as Item);
    match item {
        Item::Struct(item_struct) => process_struct(item_struct),
        Item::Enum(item_enum) => process_enum(item_enum),
        Item::Type(item_type) => process_type_alias(item_type, &parsed_args),
        _ => panic!("Unsupported target for model_schema"),
    }
}

#[cfg(feature = "typescript")]
fn process_type_alias(item_type: ItemType, args: &ModelSchemaArgs) -> TokenStream {
    let mut alias = item_type;
    alias
        .attrs
        .retain(|attr| !attr.path().is_ident("model_schema"));

    let rust_ident = alias.ident.clone();
    let rust_ident_str = rust_ident.to_string();
    let export_name = compute_alias_export_name(&rust_ident_str, args.name_override.clone());
    let module_name = format!("{}_schema", to_snake_case(&export_name));
    let module_ident = Ident::new(&module_name, rust_ident.span());

    let docs_vec =
        get_item_docs(&alias.attrs).unwrap_or_else(|| vec![export_name.clone(), String::new()]);
    let docs_formatted = format_docs_for_ts(&docs_vec, &export_name);

    register_alias_info(&rust_ident_str, &export_name, module_name);

    let generics: Vec<String> = alias
        .generics
        .params
        .iter()
        .filter_map(|param| match param {
            GenericParam::Type(tp) => Some(crate::safe_type_name(&tp.ident.to_string())),
            _ => None,
        })
        .collect();

    let alias_field_def = get_field_def(export_name.as_str(), &alias.ty, "");

    let ts_method =
        generate_ts_alias_method(&docs_formatted, &export_name, &generics, &alias_field_def);
    let json_schema_method = generate_alias_json_schema_stub();
    let zod_method = generate_alias_zod_stub();

    let output = quote! {
        #alias

        pub mod #module_ident {
            use super::*;

            pub struct Schema;

            impl Schema {
                #ts_method
                #json_schema_method
                #zod_method
            }
        }
    };

    TokenStream::from(output)
}

#[cfg(not(feature = "typescript"))]
fn process_type_alias(item_type: ItemType, _args: &ModelSchemaArgs) -> TokenStream {
    let mut alias = item_type;
    alias
        .attrs
        .retain(|attr| !attr.path().is_ident("model_schema"));
    TokenStream::from(quote! { #alias })
}

/// Processes a struct item and generates TypeScript and Zod schema definitions for it.
fn process_struct(mut item_struct: syn::ItemStruct) -> TokenStream {
    let name = &item_struct.ident;

    #[cfg(feature = "serde")]
    let rename_all = parse_serde_type_attributes(&item_struct.attrs).rename_all;
    #[cfg(not(feature = "serde"))]
    let rename_all = None;

    #[cfg(any(feature = "typescript", feature = "zod"))]
    let item_name = safe_type_name(&name.to_string());

    // Extract docs early for example extraction
    #[cfg(any(feature = "typescript", feature = "zod"))]
    let docs_vec = get_struct_docs(&item_struct);

    #[cfg(feature = "zod")]
    let example_code = docs_vec
        .as_ref()
        .and_then(|docs| extract_example_from_docs(docs));

    // Process all fields in the struct
    let mut field_defs = Vec::new();
    for field in &mut item_struct.fields {
        let f_def = process_field(&rename_all, field);
        field_defs.push(f_def);
    }

    // Generate TypeScript type and Zod schema code
    let mut type_code = String::new();
    let mut schema_code = String::new();

    // TODO: Consider this when we add optionals to TypeScript instead of `| undefined`
    // let mut opts = Vec::new();

    let mut json_schema_fields: Vec<proc_macro2::TokenStream> = Vec::new();

    for fld in field_defs {
        write_field_type_and_schema(&mut type_code, &mut schema_code, &fld);

        // if fld.is_optional {
        //     opts.push(fld.name.to_string());
        // }

        json_schema_fields.push(build_field_schema(&fld));
    }

    #[cfg(feature = "typescript")]
    let fields_empty = json_schema_fields.is_empty();

    #[cfg(feature = "zod")]
    let show_opts = "";

    #[cfg(feature = "typescript")]
    let docs = match docs_vec.as_ref() {
        Some(doc_lines) => doc_lines
            .iter()
            .flat_map(|v| {
                v.lines()
                    .map(std::borrow::ToOwned::to_owned)
                    .collect::<Vec<_>>()
            })
            .chain(vec![String::new()])
            .map(|l| format!(" * {l}"))
            .collect::<Vec<_>>()
            .join("\n"),
        None => [name.to_string(), String::new()]
            .into_iter()
            .map(|l| format!(" * {l}"))
            .collect::<Vec<_>>()
            .join("\n"),
    };

    // Generate the final output with conditional compilation
    #[cfg(feature = "jsonschema")]
    let json_schema_method = generate_json_schema_method(&json_schema_fields);

    #[cfg(feature = "typescript")]
    let ts_definition_method =
        generate_ts_definition_method(&docs, &item_name, &type_code, fields_empty);

    #[cfg(feature = "zod")]
    let has_example = example_code.is_some();

    #[cfg(feature = "zod")]
    let zod_schema_method = generate_zod_schema_method(&item_name, &schema_code, show_opts, has_example);

    #[cfg(feature = "zod")]
    let example_method = example_code.as_ref().map(|code| {
        let code_tokens: proc_macro2::TokenStream = code.parse().unwrap();
        quote! {
            #[cfg(feature = "zod")]
            pub fn schema_example() -> serde_json::Value {
                let value: #name = {
                    #code_tokens
                };
                serde_json::to_value(&value).unwrap()
            }
        }
    });

    #[cfg(feature = "zod")]
    let impl_items: Vec<proc_macro2::TokenStream> = vec![
        #[cfg(feature = "jsonschema")]
        json_schema_method,
        #[cfg(feature = "typescript")]
        ts_definition_method,
        #[cfg(feature = "zod")]
        zod_schema_method,
    ]
    .into_iter()
    .chain(example_method.into_iter())
    .collect();

    #[cfg(not(feature = "zod"))]
    let impl_items: Vec<proc_macro2::TokenStream> = vec![
        #[cfg(feature = "jsonschema")]
        json_schema_method,
        #[cfg(feature = "typescript")]
        ts_definition_method,
    ];

    let output = quote! {
        #item_struct

        impl #name {
            #(#impl_items) *
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

    #[cfg(feature = "serde")]
    let serde_type_meta = parse_serde_type_attributes(&item_enum.attrs);

    let item_name = safe_type_name(&name.to_string());

    if is_plain_enum(&item_enum) {
        #[cfg(feature = "serde")]
        let rename_all = &serde_type_meta.rename_all;

        #[cfg(not(feature = "serde"))]
        let rename_all = &None;

        process_plain_enum(item_enum, &name, rename_all, &item_name)
    } else {
        #[cfg(feature = "serde")]
        let (tag_name, rename_all) = (
            serde_type_meta
                .tag
                .as_ref()
                .map_or_else(|| "type".to_string(), Clone::clone),
            serde_type_meta.rename_all,
        );

        #[cfg(not(feature = "serde"))]
        let (tag_name, rename_all) = ("type".to_string(), None);

        process_discriminated_enum(item_enum, &name, &tag_name, &rename_all, &item_name)
    }
}

/// Processes a plain enum (simple string enum in TypeScript) and generates its definitions.
fn process_plain_enum(
    mut item_enum: syn::ItemEnum,
    name: &syn::Ident,
    rename_all: &Option<String>,
    item_name: &str,
) -> TokenStream {
    // Extract docs early for example extraction
    #[cfg(any(feature = "typescript", feature = "zod"))]
    let docs_vec = get_enum_docs(&item_enum);

    #[cfg(feature = "zod")]
    let example_code = docs_vec
        .as_ref()
        .and_then(|docs| extract_example_from_docs(docs));

    let mut enum_options = Vec::new();
    #[cfg(feature = "typescript")]
    let mut enum_variant_docs = Vec::new();

    for item in &mut item_enum.variants {
        #[cfg(feature = "serde")]
        let field_rename = parse_serde_field_attributes(&item.attrs).rename;
        #[cfg(not(feature = "serde"))]
        let field_rename = None;

        let final_name = get_final_name(item.ident.to_string(), &field_rename, rename_all);
        enum_options.push(final_name);

        // Collect variant documentation
        #[cfg(feature = "typescript")]
        {
            let variant_docs = match get_variant_docs(item) {
                Some(doc_lines) => doc_lines.join("\n"),
                None => String::new(),
            };
            enum_variant_docs.push(variant_docs);
        }
    }

    #[cfg(feature = "typescript")]
    let type_code = enum_options
        .iter()
        .enumerate()
        .map(|(idx, v)| {
            let docs = &enum_variant_docs[idx];
            if docs.is_empty() {
                format!("  | \"{v}\"")
            } else {
                let formatted_docs = docs
                    .lines()
                    .map(|line| {
                        let trimmed = line.trim();
                        // Strip leading asterisk if present (from block comments)
                        let content = trimmed.strip_prefix('*').unwrap_or(trimmed).trim();
                        if content.is_empty() {
                            "  *".to_string()
                        } else {
                            format!("  * {content}")
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                format!("  /*\n{formatted_docs}\n  */\n  | \"{v}\"")
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    #[cfg(feature = "zod")]
    let schema_code = enum_options
        .iter()
        .map(|v| format!("\"{v}\""))
        .collect::<Vec<_>>()
        .join(", ");

    // Enumerate the strings with indices
    let enumerated: Vec<proc_macro2::TokenStream> = enum_options
        .iter()
        .map(|v| {
            quote! { #v }
        })
        .collect();

    #[cfg(any(feature = "typescript", feature = "zod"))]
    let (docs, plain_description) = if let Some(ref doc_lines) = docs_vec {
        // Strip example blocks from docs
        let doc_lines_without_examples = strip_examples_from_docs(doc_lines);

        let plain_lines: Vec<String> = doc_lines_without_examples
            .iter()
            .flat_map(|v| {
                v.lines()
                    .map(|line| {
                        // Strip leading asterisk from block-style doc comments
                        let trimmed = line.trim();
                        trimmed
                            .strip_prefix('*')
                            .unwrap_or(trimmed)
                            .trim()
                            .to_string()
                    })
                    .collect::<Vec<_>>()
            })
            .filter(|s| !s.is_empty())
            .collect();

        let docs_formatted = plain_lines
            .iter()
            .map(|l| format!(" * {l}"))
            .chain(vec![" * ".to_string()])
            .collect::<Vec<_>>()
            .join("\n");

        // Escape double quotes in description
        let description = plain_lines.join("\\n").replace("\"", "\\\"");
        (docs_formatted, description)
    } else {
        let docs_formatted = [name.to_string(), String::new()]
            .into_iter()
            .map(|l| format!(" * {l}"))
            .collect::<Vec<_>>()
            .join("\n");
        (docs_formatted, name.to_string())
    };

    // Generate conditional methods
    #[cfg(feature = "jsonschema")]
    let json_schema_method = generate_plain_enum_json_schema_method(&enumerated);

    #[cfg(feature = "typescript")]
    let ts_definition_method =
        generate_plain_enum_ts_definition_method(&docs, item_name, &type_code);
    #[cfg(feature = "zod")]
    let has_example = example_code.is_some();

    #[cfg(feature = "zod")]
    let zod_schema_method =
        generate_plain_enum_zod_schema_method(item_name, &schema_code, &plain_description, has_example);

    #[cfg(feature = "zod")]
    let example_method = example_code.as_ref().map(|code| {
        let code_tokens: proc_macro2::TokenStream = code.parse().unwrap();
        quote! {
            #[cfg(feature = "zod")]
            pub fn schema_example() -> serde_json::Value {
                let value: #name = {
                    #code_tokens
                };
                serde_json::to_value(&value).unwrap()
            }
        }
    });

    #[cfg(not(any(feature = "typescript", feature = "zod")))]
    let _ = item_name;

    #[cfg(feature = "zod")]
    let impl_items: Vec<proc_macro2::TokenStream> = vec![
        #[cfg(feature = "jsonschema")]
        json_schema_method,
        #[cfg(feature = "typescript")]
        ts_definition_method,
        #[cfg(feature = "zod")]
        zod_schema_method,
    ]
    .into_iter()
    .chain(example_method.into_iter())
    .collect();

    #[cfg(not(feature = "zod"))]
    let impl_items: Vec<proc_macro2::TokenStream> = vec![
        #[cfg(feature = "jsonschema")]
        json_schema_method,
        #[cfg(feature = "typescript")]
        ts_definition_method,
    ];

    // Use the enumerated values in the quote! macro
    let enum_values = &enumerated;

    let output = quote! {
        #item_enum

        impl #name {
            #(#impl_items) *

            pub fn enum_members() -> Vec<String> {
                [
                    #(#enum_values),*
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
    tag_name: &str,
    rename_all: &Option<String>,
    item_name: &str,
) -> TokenStream {
    // Extract docs early for example extraction
    #[cfg(any(feature = "typescript", feature = "zod"))]
    let docs_vec = get_enum_docs(&item_enum);

    #[cfg(feature = "zod")]
    let example_code = docs_vec
        .as_ref()
        .and_then(|docs| extract_example_from_docs(docs));

    let mut discriminator_field_defs: HashMap<String, Vec<FieldDef>> = HashMap::new();
    let mut discriminator_field_docs: HashMap<String, String> = HashMap::new();
    let mut json_schema_variants: Vec<proc_macro2::TokenStream> = Vec::new();

    // Process each variant in the enum
    for item in &mut item_enum.variants {
        #[cfg(feature = "serde")]
        let field_rename = parse_serde_field_attributes(&item.attrs).rename;
        #[cfg(not(feature = "serde"))]
        let field_rename = None;

        let final_name = get_final_name(item.ident.to_string(), &field_rename, rename_all);

        let mut field_defs: Vec<FieldDef> = Vec::new();
        // let mut json_schema_fields: Vec<proc_macro2::TokenStream> = Vec::new();

        for field in &mut item.fields {
            let f_def = process_field(rename_all, field);
            // json_schema_fields.push(build_field_schema(&f_def));
            field_defs.push(f_def);
        }

        discriminator_field_defs.insert(final_name.clone(), field_defs);
        let discriminator_docs = match get_variant_docs(item) {
            Some(doc_lines) => doc_lines
                .into_iter()
                .flat_map(|v| {
                    v.lines()
                        .map(std::borrow::ToOwned::to_owned)
                        .collect::<Vec<_>>()
                })
                .chain(vec![String::new()])
                .map(|l| format!(" * {l}"))
                .collect::<Vec<_>>()
                .join("\n"),
            None => [final_name.to_string(), String::new()]
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
                tag_name,
                &discriminator_value,
                field_defs,
                &discriminator_field_docs[&discriminator_value],
            );

        type_code_items.push(variant_type_code);
        schema_code_items.push((variant_schema_code, optional_fields));
        json_schema_variants.push(json_schema_variant);
    }

    #[cfg(feature = "jsonschema")]
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

    #[cfg(feature = "typescript")]
    let type_code = type_code_items.join(" | ");

    // Generate Zod schema conditionally
    #[cfg(feature = "zod")]
    let schema_code = format!(
        "z.discriminatedUnion(\"{tag_name}\", [{}])",
        schema_code_items
            .iter()
            .map(|(v, _opts)| format!("z.strictObject({}){}", v, ""))
            .collect::<Vec<_>>()
            .join(", ")
    );

    #[cfg(feature = "typescript")]
    let docs = match docs_vec.as_ref() {
        Some(doc_lines) => doc_lines
            .iter()
            .flat_map(|v| {
                v.lines()
                    .map(std::borrow::ToOwned::to_owned)
                    .collect::<Vec<_>>()
            })
            .chain(vec![String::new()])
            .map(|l| format!(" * {l}"))
            .collect::<Vec<_>>()
            .join("\n"),
        None => [name.to_string(), String::new()]
            .into_iter()
            .map(|l| format!(" * {l}"))
            .collect::<Vec<_>>()
            .join("\n"),
    };

    #[cfg(feature = "jsonschema")]
    let json_schema_method = generate_discriminated_enum_json_schema_method(&main_schema_code);

    #[cfg(feature = "typescript")]
    let ts_definition_method =
        generate_discriminated_enum_ts_definition_method(&docs, item_name, &type_code);

    #[cfg(feature = "zod")]
    let has_example = example_code.is_some();

    #[cfg(feature = "zod")]
    let zod_schema_method = generate_discriminated_enum_zod_schema_method(item_name, &schema_code, has_example);

    #[cfg(feature = "zod")]
    let example_method = example_code.as_ref().map(|code| {
        let code_tokens: proc_macro2::TokenStream = code.parse().unwrap();
        quote! {
            #[cfg(feature = "zod")]
            pub fn schema_example() -> serde_json::Value {
                let value: #name = {
                    #code_tokens
                };
                serde_json::to_value(&value).unwrap()
            }
        }
    });

    #[cfg(not(any(feature = "typescript", feature = "zod")))]
    let _ = item_name;

    #[cfg(feature = "zod")]
    let impl_items: Vec<proc_macro2::TokenStream> = vec![
        #[cfg(feature = "jsonschema")]
        json_schema_method,
        #[cfg(feature = "typescript")]
        ts_definition_method,
        #[cfg(feature = "zod")]
        zod_schema_method,
    ]
    .into_iter()
    .chain(example_method.into_iter())
    .collect();

    #[cfg(not(feature = "zod"))]
    let impl_items: Vec<proc_macro2::TokenStream> = vec![
        #[cfg(feature = "jsonschema")]
        json_schema_method,
        #[cfg(feature = "typescript")]
        ts_definition_method,
    ];

    let output = quote! {
        #item_enum

        impl #name {
            #(#impl_items) *
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

        // Add Zod schema definition - conditionally
        #[cfg(feature = "zod")]
        {
            let zod_field_type = fld.zod_type();
            if let Err(err) = writeln!(variant_schema_code, "  {}: {},", fld.name, zod_field_type) {
                panic!("Failed to write Zod schema: {err}");
            }
        }

        #[cfg(not(feature = "zod"))]
        {
            // When zod feature is disabled, don't write to variant_schema_code
            let _ = &variant_schema_code; // Suppress unused variable warning
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
                // Check if minLength is specified for array items
                if let Some(ref meta) = fld.model_schema_prop_meta {
                    if let Some(min_len) = meta.min_length {
                        quote! {
                            properties.insert(#field_name_str.to_string(), {
                                serde_json::json!({
                                    "type": "array",
                                    "items": serde_json::json!({ "type": "string", "minLength": #min_len })
                                })
                            });
                        }
                    } else {
                        quote! {
                            properties.insert(#field_name_str.to_string(), {
                                serde_json::json!({
                                    "type": "array",
                                    "items": serde_json::json!({ "type": "string" })
                                })
                            });
                        }
                    }
                } else {
                    quote! {
                        properties.insert(#field_name_str.to_string(), {
                            serde_json::json!({
                                "type": "array",
                                "items": serde_json::json!({ "type": "string" })
                            })
                        });
                    }
                }
            } else {
                // Check if minLength is specified
                if let Some(ref meta) = fld.model_schema_prop_meta {
                    if let Some(min_len) = meta.min_length {
                        quote! {
                            properties.insert(#field_name_str.to_string(), {
                                serde_json::json!({
                                    "type": "string",
                                    "minLength": #min_len
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
        }
        FieldDefType::StringLiteral(literal) => {
            if fld.is_array {
                quote! {
                    properties.insert(#field_name_str.to_string(), {
                        serde_json::json!({
                            "type": "array",
                            "items": serde_json::json!({ "type": "string", "const": #literal })
                        })
                    });
                }
            } else {
                quote! {
                    properties.insert(#field_name_str.to_string(), {
                        serde_json::json!({
                            "type": "string",
                            "const": #literal
                        })
                    });
                }
            }
        }
        FieldDefType::U32
        | FieldDefType::U16
        | FieldDefType::U8
        | FieldDefType::U64
        | FieldDefType::I8
        | FieldDefType::I16
        | FieldDefType::I32
        | FieldDefType::I64
        | FieldDefType::Usize
        | FieldDefType::Isize => {
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
        FieldDefType::F32 | FieldDefType::F64 => {
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
        #[cfg(feature = "object_id")]
        FieldDefType::ObjectId => {
            if fld.is_array {
                quote! {
                    properties.insert(#field_name_str.to_string(), {
                        serde_json::json!({
                            "type": "array",
                            "items": serde_json::json!({
                                "type": "object",
                                "properties": {
                                    "$oid": { "type": "string" }
                                },
                                "required": ["$oid"],
                                "additionalProperties": false
                            })
                        })
                    });
                }
            } else {
                quote! {
                    properties.insert(#field_name_str.to_string(), {
                        serde_json::json!({
                            "type": "object",
                            "properties": {
                                "$oid": { "type": "string" }
                            },
                            "required": ["$oid"],
                            "additionalProperties": false
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
                if let Some(alias) = lookup_alias_info(name) {
                    let module_ident = proc_macro2::Ident::new(
                        alias.module_name.as_str(),
                        proc_macro2::Span::call_site(),
                    );
                    let type_json_schema = quote! { #module_ident::Schema::json_schema() };
                    generate_type_schema(fld, &field_name_str, type_json_schema)
                } else {
                    let name_ident = proc_macro2::Ident::new(
                        format!("{name}Json").as_str(),
                        proc_macro2::Span::call_site(),
                    );
                    let type_json_schema = quote! { #name_ident::json_schema() };

                    generate_type_schema(fld, &field_name_str, type_json_schema)
                }
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
                        if value.is_array {
                            quote! {
                                properties.insert(#field_name_str.to_string(), {
                                    serde_json::json!({
                                        "type": "object",
                                        "additionalProperties": {
                                            "type": "array",
                                            "items": { "type": "string" }
                                        }
                                    })
                                });
                            }
                        } else {
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
                    }
                    FieldDefType::U8
                    | FieldDefType::U16
                    | FieldDefType::U32
                    | FieldDefType::U64
                    | FieldDefType::I8
                    | FieldDefType::I16
                    | FieldDefType::I32
                    | FieldDefType::I64
                    | FieldDefType::Usize
                    | FieldDefType::Isize => {
                        if value.is_array {
                            quote! {
                                properties.insert(#field_name_str.to_string(), {
                                    serde_json::json!({
                                        "type": "object",
                                        "additionalProperties": {
                                            "type": "array",
                                            "items": { "type": "integer" }
                                        }
                                    })
                                });
                            }
                        } else {
                            quote! {
                                properties.insert(#field_name_str.to_string(), {
                                    serde_json::json!({
                                        "type": "object",
                                        "additionalProperties": {
                                            "type": "integer"
                                        }
                                    })
                                });
                            }
                        }
                    }
                    FieldDefType::F32 | FieldDefType::F64 => {
                        if value.is_array {
                            quote! {
                                properties.insert(#field_name_str.to_string(), {
                                    serde_json::json!({
                                        "type": "object",
                                        "additionalProperties": {
                                            "type": "array",
                                            "items": { "type": "number" }
                                        }
                                    })
                                });
                            }
                        } else {
                            quote! {
                                properties.insert(#field_name_str.to_string(), {
                                    serde_json::json!({
                                        "type": "object",
                                        "additionalProperties": {
                                            "type": "number"
                                        }
                                    })
                                });
                            }
                        }
                    }
                    FieldDefType::Boolean => {
                        if value.is_array {
                            quote! {
                                properties.insert(#field_name_str.to_string(), {
                                    serde_json::json!({
                                        "type": "object",
                                        "additionalProperties": {
                                            "type": "array",
                                            "items": { "type": "boolean" }
                                        }
                                    })
                                });
                            }
                        } else {
                            quote! {
                                properties.insert(#field_name_str.to_string(), {
                                    serde_json::json!({
                                        "type": "object",
                                        "additionalProperties": {
                                            "type": "boolean"
                                        }
                                    })
                                });
                            }
                        }
                    }
                    #[cfg(feature = "object_id")]
                    FieldDefType::ObjectId => {
                        if value.is_array {
                            quote! {
                                properties.insert(#field_name_str.to_string(), {
                                    serde_json::json!({
                                        "type": "object",
                                        "additionalProperties": {
                                            "type": "array",
                                            "items": {
                                                "type": "object",
                                                "properties": {
                                                    "$oid": { "type": "string" }
                                                },
                                                "required": ["$oid"],
                                                "additionalProperties": false
                                            }
                                        }
                                    })
                                });
                            }
                        } else {
                            quote! {
                                properties.insert(#field_name_str.to_string(), {
                                    serde_json::json!({
                                        "type": "object",
                                        "additionalProperties": {
                                            "type": "object",
                                            "properties": {
                                                "$oid": { "type": "string" }
                                            },
                                            "required": ["$oid"],
                                            "additionalProperties": false
                                        }
                                    })
                                });
                            }
                        }
                    }
                    FieldDefType::Map(inner_key, inner_value) => {
                        if env::var("RUST_LOG") == Ok(String::from("trace")) {
                            println!(
                                "Map Value is another Map => inner_key: {:?}, inner_value: {:?}, is_array: {}",
                                inner_key, inner_value, value.is_array
                            );
                        }

                        // Handle Vec<HashMap<String, T>> case
                        if value.is_array && matches!(inner_key.field_type, FieldDefType::String) {
                            let inner_value_schema = match &inner_value.field_type {
                                FieldDefType::U8
                                | FieldDefType::U16
                                | FieldDefType::U32
                                | FieldDefType::U64
                                | FieldDefType::I8
                                | FieldDefType::I16
                                | FieldDefType::I32
                                | FieldDefType::I64
                                | FieldDefType::Usize
                                | FieldDefType::Isize => {
                                    quote! { { "type": "integer" } }
                                }
                                FieldDefType::F32 | FieldDefType::F64 => {
                                    quote! { { "type": "number" } }
                                }
                                FieldDefType::String => {
                                    quote! { { "type": "string" } }
                                }
                                FieldDefType::Boolean => {
                                    quote! { { "type": "boolean" } }
                                }
                                #[cfg(feature = "object_id")]
                                FieldDefType::ObjectId => {
                                    quote! { {
                                        "type": "object",
                                        "properties": {
                                            "$oid": { "type": "string" }
                                        },
                                        "required": ["$oid"],
                                        "additionalProperties": false
                                    } }
                                }
                                _ => {
                                    quote! { true }
                                }
                            };

                            quote! {
                                properties.insert(#field_name_str.to_string(), {
                                    serde_json::json!({
                                        "type": "object",
                                        "additionalProperties": {
                                            "type": "array",
                                            "items": {
                                                "type": "object",
                                                "additionalProperties": #inner_value_schema
                                            }
                                        }
                                    })
                                });
                            }
                        } else {
                            // Fallback for non-array Maps or complex cases
                            quote! {
                                properties.insert(#field_name_str.to_string(), {
                                    serde_json::json!({
                                        "type": "object",
                                        "additionalProperties": true
                                    })
                                });
                            }
                        }
                    }
                    FieldDefType::SiblingType(value_type_name, value_args) => {
                        if env::var("RUST_LOG") == Ok(String::from("trace")) {
                            println!(
                                "Map Value SiblingType => value_type_name: {value_type_name}, value_args: {value_args:?}"
                            );
                        }

                        // Handle Vec<T> as map value
                        if value_type_name == "Vec" && value_args.len() == 1 {
                            let inner_type = &value_args[0];
                            match &inner_type.field_type {
                                // Vec<HashMap<String, T>>
                                FieldDefType::Map(inner_key, inner_value) => {
                                    match &inner_key.field_type {
                                        FieldDefType::String => {
                                            let inner_value_schema = match &inner_value.field_type {
                                                FieldDefType::U8
                                                | FieldDefType::U16
                                                | FieldDefType::U32
                                                | FieldDefType::U64
                                                | FieldDefType::I8
                                                | FieldDefType::I16
                                                | FieldDefType::I32
                                                | FieldDefType::I64
                                                | FieldDefType::Usize
                                                | FieldDefType::Isize => {
                                                    quote! { { "type": "integer" } }
                                                }
                                                FieldDefType::F32 | FieldDefType::F64 => {
                                                    quote! { { "type": "number" } }
                                                }
                                                FieldDefType::String => {
                                                    quote! { { "type": "string" } }
                                                }
                                                FieldDefType::Boolean => {
                                                    quote! { { "type": "boolean" } }
                                                }
                                                _ => {
                                                    quote! { true }
                                                }
                                            };

                                            quote! {
                                                properties.insert(#field_name_str.to_string(), {
                                                    serde_json::json!({
                                                        "type": "object",
                                                        "additionalProperties": {
                                                            "type": "array",
                                                            "items": {
                                                                "type": "object",
                                                                "additionalProperties": #inner_value_schema
                                                            }
                                                        }
                                                    })
                                                });
                                            }
                                        }
                                        _ => {
                                            quote! {
                                                properties.insert(#field_name_str.to_string(), {
                                                    serde_json::json!({
                                                        "type": "object",
                                                        "additionalProperties": true
                                                    })
                                                });
                                            }
                                        }
                                    }
                                }
                                // Vec<primitive>
                                FieldDefType::U8
                                | FieldDefType::U16
                                | FieldDefType::U32
                                | FieldDefType::U64
                                | FieldDefType::I8
                                | FieldDefType::I16
                                | FieldDefType::I32
                                | FieldDefType::I64
                                | FieldDefType::Usize
                                | FieldDefType::Isize => {
                                    quote! {
                                        properties.insert(#field_name_str.to_string(), {
                                            serde_json::json!({
                                                "type": "object",
                                                "additionalProperties": {
                                                    "type": "array",
                                                    "items": { "type": "integer" }
                                                }
                                            })
                                        });
                                    }
                                }
                                FieldDefType::F32 | FieldDefType::F64 => {
                                    quote! {
                                        properties.insert(#field_name_str.to_string(), {
                                            serde_json::json!({
                                                "type": "object",
                                                "additionalProperties": {
                                                    "type": "array",
                                                    "items": { "type": "number" }
                                                }
                                            })
                                        });
                                    }
                                }
                                FieldDefType::String => {
                                    quote! {
                                        properties.insert(#field_name_str.to_string(), {
                                            serde_json::json!({
                                                "type": "object",
                                                "additionalProperties": {
                                                    "type": "array",
                                                    "items": { "type": "string" }
                                                }
                                            })
                                        });
                                    }
                                }
                                FieldDefType::Boolean => {
                                    quote! {
                                        properties.insert(#field_name_str.to_string(), {
                                            serde_json::json!({
                                                "type": "object",
                                                "additionalProperties": {
                                                    "type": "array",
                                                    "items": { "type": "boolean" }
                                                }
                                            })
                                        });
                                    }
                                }
                                #[cfg(feature = "object_id")]
                                FieldDefType::ObjectId => {
                                    quote! {
                                        properties.insert(#field_name_str.to_string(), {
                                            serde_json::json!({
                                                "type": "object",
                                                "additionalProperties": {
                                                    "type": "array",
                                                    "items": {
                                                        "type": "object",
                                                        "properties": {
                                                            "$oid": { "type": "string" }
                                                        },
                                                        "required": ["$oid"],
                                                        "additionalProperties": false
                                                    }
                                                }
                                            })
                                        });
                                    }
                                }
                                _ => {
                                    quote! {
                                        properties.insert(#field_name_str.to_string(), {
                                            serde_json::json!({
                                                "type": "object",
                                                "additionalProperties": true
                                            })
                                        });
                                    }
                                }
                            }
                        } else {
                            // Other SiblingType cases - fallback to generic
                            quote! {
                                properties.insert(#field_name_str.to_string(), {
                                    serde_json::json!({
                                        "type": "object",
                                        "additionalProperties": true
                                    })
                                });
                            }
                        }
                    }
                    _ => {
                        quote! {
                            properties.insert(#field_name_str.to_string(), {
                                serde_json::json!({
                                    "type": "object",
                                    "additionalProperties": true
                                })
                            });
                        }
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

    let required_code = if fld.is_optional {
        quote! {}
    } else {
        quote! {
            required.push(serde_json::Value::String(#field_name_str.to_string()));
        }
    };

    quote! {
        #schema_code
        #required_code
    }
}

/// Writes the TypeScript type and conditionally Zod schema for a field to the provided buffers.
fn write_field_type_and_schema(type_code: &mut String, schema_code: &mut String, fld: &FieldDef) {
    // Always write TypeScript type
    if let Err(err) = writeln!(
        type_code,
        "  /**\n{}\n**/\n  {}: {};",
        fld.docs,
        fld.name,
        fld.typescript_typename()
    ) {
        panic!("Failed to write TypeScript type: {err}");
    }

    // Conditionally write Zod schema
    #[cfg(feature = "zod")]
    {
        if let Err(err) = writeln!(schema_code, "  {}: {},", fld.name, fld.zod_type()) {
            panic!("Failed to write Zod schema: {err}");
        }
    }

    #[cfg(not(feature = "zod"))]
    {
        // When zod feature is disabled, don't write to schema_code
        let _ = schema_code; // Suppress unused variable warning
    }
}

/// Processes a field and returns its definition.
fn process_field(rename_all: &Option<String>, field: &mut Field) -> FieldDef {
    let mut new_attrs = Vec::new();

    #[cfg(feature = "serde")]
    let field_rename = parse_serde_field_attributes(&field.attrs).rename;
    #[cfg(not(feature = "serde"))]
    let field_rename = None;

    // Parse model_schema_prop attributes before filtering them out
    let model_schema_prop_meta =
        crate::features::model_schema_prop::parse_model_schema_prop_attributes(&field.attrs);

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

    let final_name = get_final_name(name, &field_rename, rename_all);
    let field_docs = match get_field_docs(field) {
        Some(doc_lines) => doc_lines
            .into_iter()
            .flat_map(|v| {
                v.lines()
                    .map(std::borrow::ToOwned::to_owned)
                    .collect::<Vec<_>>()
            })
            .chain(vec![String::new()])
            .map(|l| format!(" * {l}"))
            .collect::<Vec<_>>()
            .join("\n"),
        None => [final_name.to_string(), String::new()]
            .into_iter()
            .map(|l| format!(" * {l}"))
            .collect::<Vec<_>>()
            .join("\n"),
    };

    // Create the field definition and apply any model_schema_prop overrides
    let mut field_def = get_field_def(&final_name, field_type, &field_docs);
    field_def.model_schema_prop_meta = if model_schema_prop_meta.as_type.is_some()
        || model_schema_prop_meta.literal.is_some()
        || model_schema_prop_meta.min_length.is_some()
    {
        Some(model_schema_prop_meta)
    } else {
        None
    };

    // Apply type overrides based on model_schema_prop attributes
    if let Some(ref meta) = field_def.model_schema_prop_meta
        && let Some(ref literal) = meta.literal
    {
        // If literal is specified, override the field type to StringLiteral
        field_def.field_type = crate::field_type::FieldDefType::StringLiteral(literal.clone());
    }
    // TODO: Handle `as` parameter for type overrides in future implementation

    // Update field docs to include minimum length information
    if let Some(ref meta) = field_def.model_schema_prop_meta
        && let Some(min_len) = meta.min_length
    {
        // Add minimum length information to the docs
        let min_len_doc = format!(" * Minimum length: {min_len}");
        field_def.docs = if field_def.docs.is_empty() {
            format!(" * {final_name}\n * \n{min_len_doc}")
        } else {
            format!("{}\n{}", field_def.docs, min_len_doc)
        };
    }

    field_def
}

/// Gets the final name for a field or enum variant, considering serde attributes.
fn get_final_name(
    name: String,
    field_rename: &Option<String>,
    rename_all: &Option<String>,
) -> String {
    if let Some(rename) = &field_rename {
        rename.clone()
    } else if rename_all == &Some("camelCase".to_string()) {
        snake_to_camel(&name)
    } else if rename_all == &Some("lowercase".to_string()) {
        name.to_lowercase()
    } else {
        name
    }
}

/// Converts a `snake_case` string to camelCase.
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

#[cfg(feature = "jsonschema")]
/// Generates the JSON schema method conditionally based on the jsonschema feature
fn generate_json_schema_method(
    json_schema_fields: &[proc_macro2::TokenStream],
) -> proc_macro2::TokenStream {
    crate::features::jsonschema::generate_struct_json_schema_method(json_schema_fields)
}

#[cfg(feature = "typescript")]
/// Generates the TypeScript definition method (TypeScript types only, no Zod schema)
fn generate_ts_definition_method(
    docs: &str,
    item_name: &str,
    type_code: &str,
    fields_empty: bool,
) -> proc_macro2::TokenStream {
    // TypeScript type generation (only available when typescript feature is enabled)
    let typescript_type_gen = if fields_empty {
        quote::quote! {
            format!(r#"/**\n{}\n**/\nexport type {} = Record<string, never>;"#, docs, #item_name)
        }
    } else {
        quote::quote! {
            format!("{}\n\nexport type {} = {{\n{}\n}};", docs, #item_name, #type_code)
        }
    };

    #[cfg(all(feature = "jsonschema", feature = "typescript"))]
    let json_docs_gen = generate_json_docs_part();

    #[cfg(not(feature = "jsonschema"))]
    let json_docs_gen = quote::quote! {
        let docs = format!("/**\n{docs}\n **/\n");
    };

    quote::quote! {
        pub fn ts_definition() -> String {
            let docs = #docs;
            #json_docs_gen
            #typescript_type_gen
        }
    }
}

#[cfg(feature = "zod")]
/// Generates the Zod schema method (Zod schemas only, no TypeScript types)
fn generate_zod_schema_method(
    item_name: &str,
    schema_code: &str,
    show_opts: &str,
    has_example: bool,
) -> proc_macro2::TokenStream {
    #[cfg(feature = "zod")]
    {
        // When typescript feature is enabled, generate TypeScript-style Zod schema
        #[cfg(feature = "typescript")]
        {
            if has_example {
                quote::quote! {
                    pub fn zod_schema() -> String {
                        let example_json = serde_json::to_string(&Self::schema_example()).unwrap();
                        let example_part = format!(".meta({{\n  example: {}\n}})", example_json);

                        format!(r#"const {}$RawSchema = z.strictObject({{
{}
}}){}{}

export const {}$Schema: ZodType<{}> = {}$RawSchema;"#, #item_name, #schema_code, #show_opts, example_part, #item_name, #item_name, #item_name)
                    }
                }
            } else {
                quote::quote! {
                    pub fn zod_schema() -> String {
                        format!(r#"const {}$RawSchema = z.strictObject({{
{}
}}){};

export const {}$Schema: ZodType<{}> = {}$RawSchema;"#, #item_name, #schema_code, #show_opts, #item_name, #item_name, #item_name)
                    }
                }
            }
        }

        // When typescript feature is disabled, generate JavaScript-style Zod schema
        #[cfg(not(feature = "typescript"))]
        {
            if has_example {
                quote::quote! {
                    pub fn zod_schema() -> String {
                        let example_json = serde_json::to_string(&Self::schema_example()).unwrap();
                        let example_part = format!(".meta({{\n  example: {}\n}})", example_json);

                        format!(r#"export const {}$Schema = z.strictObject({{
{}
}}){}{};"#, #item_name, #schema_code, #show_opts, example_part)
                    }
                }
            } else {
                quote::quote! {
                    pub fn zod_schema() -> String {
                        format!(r#"export const {}$Schema = z.strictObject({{
{}
}}){};"#, #item_name, #schema_code, #show_opts)
                    }
                }
            }
        }
    }

    #[cfg(not(feature = "zod"))]
    {
        let _ = (item_name, schema_code, show_opts, has_example);
        quote::quote! {
            // Zod schema method not available - zod feature disabled
            // To enable: add "zod" to your features
            // Example: tixschema = { features = ["zod"] }
        }
    }
}

#[cfg(all(feature = "jsonschema", feature = "typescript"))]
fn generate_json_docs_part() -> proc_macro2::TokenStream {
    quote::quote! {
        let prettified = serde_json::to_string_pretty(&Self::json_schema()).unwrap().lines().map(|l| format!(" * {l}")).collect::<Vec<_>>().join("\n");
        let docs = format!("/**\n{docs}\n * JSON Schema:\n{prettified}\n **/\n");
    }
}

#[cfg(feature = "jsonschema")]
/// Generates the JSON schema method for plain enums conditionally
fn generate_plain_enum_json_schema_method(
    _enumerated: &[proc_macro2::TokenStream],
) -> proc_macro2::TokenStream {
    #[cfg(feature = "jsonschema")]
    {
        crate::features::jsonschema::generate_plain_enum_json_schema_method()
    }

    #[cfg(not(feature = "jsonschema"))]
    {
        let _ = _enumerated; // Suppress unused variable warning
        quote::quote! {
            // JSON schema method not available - jsonschema feature disabled
            // To enable: add "jsonschema" to your features
            // Example: tixschema = { features = ["jsonschema"] }
        }
    }
}

#[cfg(feature = "typescript")]
/// Generates the TypeScript definition method for plain enums (TypeScript types only)
fn generate_plain_enum_ts_definition_method(
    docs: &str,
    item_name: &str,
    type_code: &str,
) -> proc_macro2::TokenStream {
    #[cfg(feature = "typescript")]
    {
        // Conditional JSON schema docs
        let json_docs_gen = quote::quote! {
            #[cfg(all(feature = "jsonschema", feature = "zod"))]
            let prettified = serde_json::to_string_pretty(&Self::json_schema()).unwrap().lines().map(|l| format!(" * {l}")).collect::<Vec<_>>().join("\n");

            #[cfg(all(feature = "jsonschema", feature = "zod"))]
            let docs = format!("/**\n{}\n * JSON Schema:\n{}\n **/\n", #docs, prettified);

            #[cfg(not(all(feature = "jsonschema", feature = "zod")))]
            let docs = format!("/**\n{}\n**/\n", #docs);
        };

        // TypeScript type generation (only available when typescript feature is enabled)
        let typescript_type_gen = quote::quote! {
            format!("{}export type {} =\n{};", docs, #item_name, #type_code)
        };

        quote::quote! {
            pub fn ts_definition() -> String {
                #json_docs_gen
                #typescript_type_gen
            }
        }
    }

    #[cfg(not(feature = "typescript"))]
    {
        quote::quote! {
            // TypeScript definition method not available - typescript feature disabled
            // To enable: add "typescript" to your features
            // Example: tixschema = { features = ["typescript"] }
        }
    }
}

#[cfg(feature = "zod")]
/// Generates the Zod schema method for plain enums (Zod schemas only)
fn generate_plain_enum_zod_schema_method(
    item_name: &str,
    schema_code: &str,
    description: &str,
    has_example: bool,
) -> proc_macro2::TokenStream {
    #[cfg(feature = "zod")]
    {
        // When typescript feature is enabled, generate TypeScript-style Zod schema
        #[cfg(feature = "typescript")]
        {
            if has_example {
                quote::quote! {
                    pub fn zod_schema() -> String {
                        let example_json = serde_json::to_string(&Self::schema_example()).unwrap();
                        format!("export const {}$Schema: ZodType<{}> = z.enum([{}]).meta({{\n  description: \"{}\",\n  example: {},\n}});", #item_name, #item_name, #schema_code, #description, example_json)
                    }
                }
            } else {
                quote::quote! {
                    pub fn zod_schema() -> String {
                        format!("export const {}$Schema: ZodType<{}> = z.enum([{}]).meta({{\n  description: \"{}\",\n}});", #item_name, #item_name, #schema_code, #description)
                    }
                }
            }
        }

        // When typescript feature is disabled, generate JavaScript-style Zod schema
        #[cfg(not(feature = "typescript"))]
        {
            if has_example {
                quote::quote! {
                    pub fn zod_schema() -> String {
                        let example_json = serde_json::to_string(&Self::schema_example()).unwrap();
                        format!("export const {}$Schema = z.enum([{}]).meta({{\n  description: \"{}\",\n  example: {},\n}});", #item_name, #schema_code, #description, example_json)
                    }
                }
            } else {
                quote::quote! {
                    pub fn zod_schema() -> String {
                        format!("export const {}$Schema = z.enum([{}]).meta({{\n  description: \"{}\",\n}});", #item_name, #schema_code, #description)
                    }
                }
            }
        }
    }

    #[cfg(not(feature = "zod"))]
    {
        let _ = (item_name, schema_code, description, has_example);
        quote::quote! {
            // Zod schema method not available - zod feature disabled
            // To enable: add "zod" to your features
            // Example: tixschema = { features = ["zod"] }
        }
    }
}

#[cfg(feature = "jsonschema")]
/// Generates the JSON schema method for discriminated enums conditionally
fn generate_discriminated_enum_json_schema_method(
    main_schema_code: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    quote::quote! {
        pub fn json_schema() -> serde_json::Value {
            #main_schema_code
        }
    }
}

#[cfg(feature = "typescript")]
/// Generates the TypeScript definition method for discriminated enums (TypeScript types only)
fn generate_discriminated_enum_ts_definition_method(
    docs: &str,
    item_name: &str,
    type_code: &str,
) -> proc_macro2::TokenStream {
    #[cfg(feature = "typescript")]
    {
        // Conditional JSON schema docs
        let json_docs_gen = quote::quote! {
            #[cfg(all(feature = "jsonschema", feature = "zod"))]
            let prettified = serde_json::to_string_pretty(&Self::json_schema()).unwrap().lines().map(|l| format!(" * {l}")).collect::<Vec<_>>().join("\n");

            #[cfg(all(feature = "jsonschema", feature = "zod"))]
            let docs = format!("/**\n{}\n * JSON Schema:\n{}\n **/\n", #docs, prettified);

            #[cfg(not(all(feature = "jsonschema", feature = "zod")))]
            let docs = format!("/**\n{}\n**/\n", #docs);
        };

        quote::quote! {
            pub fn ts_definition() -> String {
                #json_docs_gen
                let bundled_docs = docs;
                format!(r#"{bundled_docs}export type {} = {};"#, #item_name, #type_code)
            }
        }
    }

    #[cfg(not(feature = "typescript"))]
    {
        quote::quote! {
            // TypeScript definition method not available - typescript feature disabled
            // To enable: add "typescript" to your features
            // Example: tixschema = { features = ["typescript"] }
        }
    }
}

#[cfg(feature = "zod")]
/// Generates the Zod schema method for discriminated enums (Zod schemas only)
fn generate_discriminated_enum_zod_schema_method(
    item_name: &str,
    schema_code: &str,
    has_example: bool,
) -> proc_macro2::TokenStream {
    #[cfg(feature = "zod")]
    {
        // When typescript feature is enabled, generate TypeScript-style Zod schema
        #[cfg(feature = "typescript")]
        {
            if has_example {
                quote::quote! {
                    pub fn zod_schema() -> String {
                        let example_json = serde_json::to_string(&Self::schema_example()).unwrap();
                        format!(r#"export const {}$Schema: ZodType<{}> = {}.meta({{
  example: {}
}});"#, #item_name, #item_name, #schema_code, example_json)
                    }
                }
            } else {
                quote::quote! {
                    pub fn zod_schema() -> String {
                        format!(r#"export const {}$Schema: ZodType<{}> = {};"#, #item_name, #item_name, #schema_code)
                    }
                }
            }
        }

        // When typescript feature is disabled, generate JavaScript-style Zod schema
        #[cfg(not(feature = "typescript"))]
        {
            if has_example {
                quote::quote! {
                    pub fn zod_schema() -> String {
                        let example_json = serde_json::to_string(&Self::schema_example()).unwrap();
                        format!(r#"export const {}$Schema = {}.meta({{
  example: {}
}});"#, #item_name, #schema_code, example_json)
                    }
                }
            } else {
                quote::quote! {
                    pub fn zod_schema() -> String {
                        format!(r#"export const {}$Schema = {};"#, #item_name, #schema_code)
                    }
                }
            }
        }
    }

    #[cfg(not(feature = "zod"))]
    {
        let _ = (item_name, schema_code, has_example);
        quote::quote! {
            // Zod schema method not available - zod feature disabled
            // To enable: add "zod" to your features
            // Example: tixschema = { features = ["zod"] }
        }
    }
}

#[cfg(feature = "typescript")]
fn generate_ts_alias_method(
    docs: &str,
    export_name: &str,
    generics: &[String],
    field_def: &FieldDef,
) -> proc_macro2::TokenStream {
    let ts_generics = if generics.is_empty() {
        String::new()
    } else {
        format!("<{}>", generics.join(", "))
    };

    let alias_name_ts = format!("{export_name}{ts_generics}");
    let target_ts = field_def.typescript_typename();

    let docs_block = docs.to_string();

    quote! {
        pub fn ts_definition() -> String {
            format!(
                "/**\n{}\n**/\nexport type {} = {};",
                #docs_block,
                #alias_name_ts,
                #target_ts
            )
        }
    }
}

fn generate_alias_json_schema_stub() -> proc_macro2::TokenStream {
    quote! {
        #[cfg(feature = "jsonschema")]
        pub fn json_schema() -> serde_json::Value {
            serde_json::json!({
                "warning": "JSON schema generation for aliases is not yet supported"
            })
        }
    }
}

fn generate_alias_zod_stub() -> proc_macro2::TokenStream {
    quote! {
        #[cfg(feature = "zod")]
        pub fn zod_schema() -> String {
            String::from("// Zod schema generation for aliases is not yet supported")
        }
    }
}

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
    field_type::{FieldDef, FieldDefType, VariantKind, classify_variant, get_field_def, is_plain_enum},
    safe_type_name,
    utils::{extract_example_from_docs, get_field_docs, get_variant_docs, lookup_alias_info},
};

#[cfg(feature = "serde")]
use crate::field_type::{parse_serde_field_attributes, parse_serde_type_attributes};

#[cfg(any(feature = "typescript", feature = "zod"))]
use crate::utils::{
    compute_alias_export_name, format_docs_for_ts, get_enum_docs, get_item_docs, get_struct_docs,
    strip_examples_from_docs,
};

#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
use crate::utils::register_alias_info;

#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
use crate::utils::to_snake_case;

#[derive(Default, Clone)]
struct ModelSchemaArgs {
    name_override: Option<String>,
    pattern: Option<String>,
    min_length: Option<usize>,
    max_length: Option<usize>,
}

impl ModelSchemaArgs {
    fn has_string_constraints(&self) -> bool {
        self.pattern.is_some() || self.min_length.is_some() || self.max_length.is_some()
    }
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
                && let syn::Expr::Lit(expr_lit) = &meta.value
                && let syn::Lit::Str(lit_str) = &expr_lit.lit
            {
                result.name_override = Some(lit_str.value());
            } else if meta.path.is_ident("pattern")
                && let syn::Expr::Lit(expr_lit) = &meta.value
                && let syn::Lit::Str(lit_str) = &expr_lit.lit
            {
                result.pattern = Some(lit_str.value());
            } else if meta.path.is_ident("minLength")
                && let syn::Expr::Lit(expr_lit) = &meta.value
                && let syn::Lit::Int(lit_int) = &expr_lit.lit
            {
                result.min_length = Some(lit_int.base10_parse::<usize>().unwrap());
            } else if meta.path.is_ident("maxLength")
                && let syn::Expr::Lit(expr_lit) = &meta.value
                && let syn::Lit::Int(lit_int) = &expr_lit.lit
            {
                result.max_length = Some(lit_int.base10_parse::<usize>().unwrap());
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
        Item::Struct(item_struct) => process_struct(item_struct, &parsed_args),
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

fn has_serde_transparent(attrs: &[syn::Attribute]) -> bool {
    for attr in attrs {
        if attr.path().is_ident("serde") {
            let mut found = false;
            let _ = attr.parse_nested_meta(|nested| {
                if nested.path.is_ident("transparent") {
                    found = true;
                }
                Ok(())
            });
            if found {
                return true;
            }
        }
    }
    false
}

/// Processes a struct item and generates TypeScript and Zod schema definitions for it.
fn process_struct(mut item_struct: syn::ItemStruct, args: &ModelSchemaArgs) -> TokenStream {
    // Check if this is a branded newtype (transparent single-field tuple struct)
    #[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
    {
        let is_transparent = has_serde_transparent(&item_struct.attrs);
        let is_single_tuple =
            matches!(&item_struct.fields, syn::Fields::Unnamed(f) if f.unnamed.len() == 1);
        if is_transparent && is_single_tuple {
            return process_branded_newtype(item_struct, args);
        }
    }

    // String constraints (pattern, minLength, maxLength) are only valid on branded newtypes
    if args.has_string_constraints() {
        panic!("model_schema constraints (pattern, minLength, maxLength) are only supported on branded newtype structs (#[serde(transparent)] single-field tuple structs)");
    }

    let name = &item_struct.ident;

    #[cfg(feature = "serde")]
    let rename_all = parse_serde_type_attributes(&item_struct.attrs).rename_all;
    #[cfg(not(feature = "serde"))]
    let rename_all = None;

    #[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
    let item_name = safe_type_name(&name.to_string());

    // Compute module name for schema struct (same pattern as type aliases)
    #[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
    let module_name = format!("{}_schema", to_snake_case(&item_name));
    #[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
    let module_ident = Ident::new(&module_name, name.span());

    // Register struct in alias registry so other types can find it
    #[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
    register_alias_info(&name.to_string(), &item_name, module_name.clone());

    // Extract docs early for example extraction
    #[cfg(any(feature = "typescript", feature = "zod"))]
    let docs_vec = get_struct_docs(&item_struct);

    #[cfg(feature = "zod")]
    let example_code = docs_vec
        .as_ref()
        .and_then(|docs| extract_example_from_docs(docs));

    // Process all fields in the struct
    let mut field_defs = Vec::new();
    let mut validation_fns: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut validate_bodies: Vec<proc_macro2::TokenStream> = Vec::new();
    for field in &mut item_struct.fields {
        #[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
        let (f_def, validation_fn, validate_body) = process_field(&rename_all, field, Some(&module_name));
        #[cfg(not(any(feature = "typescript", feature = "zod", feature = "jsonschema")))]
        let (f_def, validation_fn, validate_body) = process_field(&rename_all, field, None);
        if let Some(vfn) = validation_fn {
            validation_fns.push(vfn);
        }
        if let Some(vb) = validate_body {
            validate_bodies.push(vb);
        }
        field_defs.push(f_def);
    }

    // Generate TypeScript type and Zod schema code
    let mut type_code = String::new();
    let mut schema_code = String::new();

    // TODO: Consider this when we add optionals to TypeScript instead of `| undefined`
    // let mut opts = Vec::new();

    #[cfg(feature = "jsonschema")]
    let mut json_schema_fields: Vec<proc_macro2::TokenStream> = Vec::new();

    // Compute fields_empty before the for loop consumes field_defs
    #[cfg(all(feature = "typescript", not(feature = "jsonschema")))]
    let fields_empty = field_defs.is_empty();

    for fld in field_defs {
        #[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
        write_field_type_and_schema(&mut type_code, &mut schema_code, &fld, Some(&item_name));

        #[cfg(not(any(feature = "typescript", feature = "zod", feature = "jsonschema")))]
        write_field_type_and_schema(&mut type_code, &mut schema_code, &fld, None);

        // if fld.is_optional {
        //     opts.push(fld.name.to_string());
        // }

        #[cfg(feature = "jsonschema")]
        json_schema_fields.push(build_field_schema(&fld));
    }

    #[cfg(all(feature = "typescript", feature = "jsonschema"))]
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

    // Generate the schema module methods
    #[cfg(feature = "jsonschema")]
    let json_schema_method = generate_json_schema_method(&json_schema_fields);

    #[cfg(feature = "typescript")]
    let ts_definition_method =
        generate_ts_definition_method(&docs, &item_name, &type_code, fields_empty);

    #[cfg(feature = "zod")]
    let has_example = example_code.is_some();

    // Schema module generates zod_schema without examples - example injection is handled
    // by the delegating method on the type itself to avoid super:: resolution issues
    #[cfg(feature = "zod")]
    let zod_schema_method = generate_zod_schema_method(&item_name, &schema_code, show_opts);

    // schema_example must be directly on the type (not in module) because
    // the example code uses type names that may not be accessible from nested module
    #[cfg(feature = "zod")]
    let schema_example_method = example_code.as_ref().map(|code| {
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

    // Build schema module impl items (without schema_example)
    #[cfg(any(feature = "zod", feature = "typescript", feature = "jsonschema"))]
    let schema_impl_items: Vec<proc_macro2::TokenStream> = vec![
        #[cfg(feature = "jsonschema")]
        json_schema_method,
        #[cfg(feature = "typescript")]
        ts_definition_method,
        #[cfg(feature = "zod")]
        zod_schema_method,
    ];

    #[cfg(not(any(feature = "zod", feature = "typescript", feature = "jsonschema")))]
    let schema_impl_items: Vec<proc_macro2::TokenStream> = vec![];

    // Generate delegating methods for backwards compatibility
    #[cfg(feature = "jsonschema")]
    let delegate_json_schema = quote! {
        pub fn json_schema() -> serde_json::Value {
            #module_ident::Schema::json_schema()
        }
    };

    #[cfg(feature = "typescript")]
    let delegate_ts_definition = quote! {
        pub fn ts_definition() -> String {
            #module_ident::Schema::ts_definition()
        }
    };

    // Generate delegating zod_schema that handles example injection
    // We need to inject examples here (not in Schema module) because Self::schema_example()
    // is accessible here but not from the nested module for function-local types
    #[cfg(feature = "zod")]
    let delegate_zod_schema = if has_example {
        quote! {
            pub fn zod_schema() -> String {
                let base_schema = #module_ident::Schema::zod_schema();
                let example_json = serde_json::to_string(&Self::schema_example()).unwrap();
                let example_part = format!(".meta({{\n  example: {}\n}})", example_json);
                // Insert .meta() before the final semicolon
                if let Some(pos) = base_schema.rfind(';') {
                    let mut result = base_schema[..pos].to_string();
                    result.push_str(&example_part);
                    result.push(';');
                    result
                } else {
                    format!("{}{}", base_schema, example_part)
                }
            }
        }
    } else {
        quote! {
            pub fn zod_schema() -> String {
                #module_ident::Schema::zod_schema()
            }
        }
    };

    // Generate the type-level validate() method if there are constrained fields.
    //
    // Architecture:
    //   - validate_{field}_value(&FieldType) -> Result<(), String>  — pure static validator per
    //     field; checks all constraints (pattern, minLength, maxLength, minimum, maximum) and
    //     returns a single combined error message.
    //   - deserialize_{field}(D) -> Result<FieldType, E>            — serde hook that calls the
    //     static validator so constraints are enforced during deserialization.
    //   - validate(&self) -> Result<(), Vec<String>>                — type-level aggregator that
    //     calls every per-field validator and collects all errors.
    //
    // This ensures the same validation rules apply during serde deserialization AND when calling
    // validate() on an already-constructed instance (e.g., built programmatically in tests).
    //
    // Only generated when serde feature is enabled AND at least one schema output feature is active.
    #[cfg(all(feature = "serde", any(feature = "typescript", feature = "zod", feature = "jsonschema")))]
    let validate_method: Option<proc_macro2::TokenStream> = if !validate_bodies.is_empty() {
        let module_name_ident = module_ident.clone();
        let validate_body_items = &validate_bodies;
        Some(quote! {
            /// Validates all constrained fields and returns all validation errors.
            ///
            /// Returns `Ok(())` if all constraints pass, or `Err(Vec<String>)` with all errors.
            pub fn validate(&self) -> Result<(), Vec<String>> {
                use #module_name_ident::*;
                let mut errors: Vec<String> = Vec::new();
                #(#validate_body_items)*
                if errors.is_empty() { Ok(()) } else { Err(errors) }
            }
        })
    } else {
        None
    };
    #[cfg(not(all(feature = "serde", any(feature = "typescript", feature = "zod", feature = "jsonschema"))))]
    let validate_method: Option<proc_macro2::TokenStream> = None;

    // Build delegating impl items (schema_example is added directly, not as a delegate)
    #[cfg(all(feature = "zod", feature = "typescript", feature = "jsonschema"))]
    let delegate_impl_items: Vec<proc_macro2::TokenStream> = vec![
        delegate_json_schema,
        delegate_ts_definition,
        delegate_zod_schema,
    ]
    .into_iter()
    .chain(schema_example_method)
    .chain(validate_method)
    .collect();

    #[cfg(all(feature = "zod", feature = "typescript", not(feature = "jsonschema")))]
    let delegate_impl_items: Vec<proc_macro2::TokenStream> = vec![
        delegate_ts_definition,
        delegate_zod_schema,
    ]
    .into_iter()
    .chain(schema_example_method)
    .chain(validate_method)
    .collect();

    #[cfg(all(feature = "zod", not(feature = "typescript"), feature = "jsonschema"))]
    let delegate_impl_items: Vec<proc_macro2::TokenStream> = vec![
        delegate_json_schema,
        delegate_zod_schema,
    ]
    .into_iter()
    .chain(schema_example_method)
    .chain(validate_method)
    .collect();

    #[cfg(all(feature = "zod", not(feature = "typescript"), not(feature = "jsonschema")))]
    let delegate_impl_items: Vec<proc_macro2::TokenStream> =
        vec![delegate_zod_schema]
            .into_iter()
            .chain(schema_example_method)
            .chain(validate_method)
            .collect();

    #[cfg(all(not(feature = "zod"), feature = "typescript", feature = "jsonschema"))]
    let delegate_impl_items: Vec<proc_macro2::TokenStream> =
        vec![delegate_json_schema, delegate_ts_definition]
            .into_iter()
            .chain(validate_method)
            .collect();

    #[cfg(all(not(feature = "zod"), feature = "typescript", not(feature = "jsonschema")))]
    let delegate_impl_items: Vec<proc_macro2::TokenStream> =
        vec![delegate_ts_definition]
            .into_iter()
            .chain(validate_method)
            .collect();

    #[cfg(all(not(feature = "zod"), not(feature = "typescript"), feature = "jsonschema"))]
    let delegate_impl_items: Vec<proc_macro2::TokenStream> =
        vec![delegate_json_schema]
            .into_iter()
            .chain(validate_method)
            .collect();

    #[cfg(any(feature = "zod", feature = "typescript", feature = "jsonschema"))]
    let output = quote! {
        #item_struct

        pub mod #module_ident {
            use super::*;

            pub struct Schema;

            impl Schema {
                #(#schema_impl_items)*
            }

            #(#validation_fns)*
        }

        impl #name {
            #(#delegate_impl_items)*
        }
    };

    #[cfg(not(any(feature = "zod", feature = "typescript", feature = "jsonschema")))]
    let output = quote! {
        #item_struct
    };

    if env::var("RUST_LOG") == Ok(String::from("trace")) {
        let output_str = output.to_string();
        println!("{output_str}");
    }

    TokenStream::from(output)
}

/// Processes a branded newtype (transparent single-field tuple struct) and generates
/// TypeScript branded type definitions and Zod brand schemas.
///
/// A branded newtype is detected when a struct has **both** `#[serde(transparent)]` and exactly
/// one unnamed field. The generated output depends on the active features:
///
/// - With `zod` + `typescript`: emits a Zod `.brand<"Name">()` schema and a
///   `type Name<T> = T & z.$brand<"Name">` alias.
/// - With `typescript` only (no `zod`): emits a `unique symbol` brand pattern and an
///   `assertName()` type-assertion helper function.
///
/// Generic parameters on the struct are preserved in the TypeScript output. For non-generic
/// newtypes, the inner field's Rust type is resolved to its TypeScript equivalent. For generic
/// newtypes, the Zod schema always uses `z.string()` as the base because the generic parameter
/// cannot be resolved at macro-expansion time.
#[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
fn process_branded_newtype(item_struct: syn::ItemStruct, args: &ModelSchemaArgs) -> TokenStream {
    let name = item_struct.ident.clone();
    let item_name = safe_type_name(&name.to_string());
    let module_name = format!("{}_schema", to_snake_case(&item_name));
    let module_ident = Ident::new(&module_name, name.span());

    register_alias_info(&name.to_string(), &item_name, module_name.clone());

    // Extract docs and example
    #[cfg(any(feature = "typescript", feature = "zod"))]
    let docs_vec = get_struct_docs(&item_struct);

    #[cfg(feature = "zod")]
    let example_code = docs_vec
        .as_ref()
        .and_then(|docs| extract_example_from_docs(docs));

    #[cfg(any(feature = "typescript", feature = "zod"))]
    let plain_description = if let Some(ref doc_lines) = docs_vec {
        let doc_lines_without_examples = strip_examples_from_docs(doc_lines);
        let plain_lines: Vec<String> = doc_lines_without_examples
            .iter()
            .flat_map(|v| {
                v.lines()
                    .map(|line| {
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
        plain_lines.join("\\n").replace('"', "\\\"")
    } else {
        item_name.clone()
    };

    // Get generic type parameters from the struct
    let generic_params: Vec<String> = item_struct
        .generics
        .params
        .iter()
        .filter_map(|p| match p {
            syn::GenericParam::Type(tp) => Some(tp.ident.to_string()),
            _ => None,
        })
        .collect();

    let is_generic = !generic_params.is_empty();

    // Get inner field type info
    let inner_field = item_struct.fields.iter().next().unwrap();
    let inner_ty = &inner_field.ty;


    #[cfg(any(feature = "typescript", feature = "zod"))]
    let ts_inner_type = if is_generic {
        generic_params[0].clone()
    } else {
        get_field_def("_inner", inner_ty, "").typescript_typename()
    };

    #[cfg(not(any(feature = "typescript", feature = "zod")))]
    let _ = inner_ty;

    #[cfg(feature = "zod")]
    let zod_inner = {
        let base = if is_generic {
            "z.string()".to_string()
        } else {
            get_field_def("_inner", inner_ty, "").zod_type()
        };
        // Apply string constraints to the zod base type
        let mut result = base;
        if let Some(min_len) = args.min_length {
            result = format!("{result}.min({min_len})");
        }
        if let Some(max_len) = args.max_length {
            result = format!("{result}.max({max_len})");
        }
        if let Some(ref pattern) = args.pattern {
            result = format!("{result}.check(z.regex(/{pattern}/))");
        }
        result
    };

    #[cfg(any(feature = "typescript", feature = "zod"))]
    let ts_generics = if is_generic {
        format!("<{}>", generic_params.join(", "))
    } else {
        String::new()
    };

    // --- Generate ts_definition method ---

    #[cfg(all(feature = "typescript", feature = "zod"))]
    let ts_definition_method = {
        let type_str = format!(
            "export type {}{} = {} & $brand<\"{}\">;",
            item_name, ts_generics, ts_inner_type, item_name
        );
        let helpers = if is_generic {
            format!(
                "\n\nfunction is{item_name}<T>(_value: T): _value is {item_name}<T> {{\n  return true;\n}}\n\nexport function assert{item_name}<T>(value: T): {item_name}<T> {{\n  if (is{item_name}(value)) {{\n    return value;\n  }}\n  throw new Error(\"assert{item_name} error\");\n}}"
            )
        } else {
            format!(
                "\n\nfunction is{item_name}(_value: {ts_inner_type}): _value is {item_name} {{\n  return true;\n}}\n\nexport function assert{item_name}(value: {ts_inner_type}): {item_name} {{\n  if (is{item_name}(value)) {{\n    return value;\n  }}\n  throw new Error(\"assert{item_name} error\");\n}}"
            )
        };
        quote! {
            pub fn ts_definition() -> String {
                format!("{}{}", #type_str, #helpers)
            }
        }
    };

    #[cfg(all(feature = "typescript", not(feature = "zod")))]
    let ts_definition_method = {
        let unique_symbol = format!("declare const __brand_{}: unique symbol;", item_name);
        let type_str = format!(
            "export type {}{} = {} & {{ readonly [__brand_{}]: true }};",
            item_name, ts_generics, ts_inner_type, item_name
        );
        let helpers = if is_generic {
            format!(
                "function is{item_name}<T>(_value: T): _value is {item_name}<T> {{\n  return true;\n}}\n\nexport function assert{item_name}<T>(value: T): {item_name}<T> {{\n  if (is{item_name}(value)) {{\n    return value;\n  }}\n  throw new Error(\"assert{item_name} error\");\n}}"
            )
        } else {
            format!(
                "function is{item_name}(_value: {ts_inner_type}): _value is {item_name} {{\n  return true;\n}}\n\nexport function assert{item_name}(value: {ts_inner_type}): {item_name} {{\n  if (is{item_name}(value)) {{\n    return value;\n  }}\n  throw new Error(\"assert{item_name} error\");\n}}"
            )
        };
        quote! {
            pub fn ts_definition() -> String {
                format!("{}\n{}\n\n{}", #unique_symbol, #type_str, #helpers)
            }
        }
    };

    // --- Generate zod_schema method ---

    #[cfg(all(feature = "zod", feature = "typescript"))]
    let zod_schema_method = {
        let zod_type_name = if is_generic {
            "ZodString".to_string()
        } else {
            match ts_inner_type.as_str() {
                "number" => "ZodNumber".to_string(),
                "boolean" => "ZodBoolean".to_string(),
                _ => "ZodString".to_string(),
            }
        };
        let zod_type_annotation = format!("$ZodBranded<{}, \"{}\">", zod_type_name, item_name);
        quote! {
            pub fn zod_schema() -> String {
                format!(
                    "const {0}$RawSchema = {1}.brand<\"{0}\">().meta({{\n  description: \"{3}\",\n}});\n\nexport const {0}$Schema: {2} = {0}$RawSchema;",
                    #item_name, #zod_inner, #zod_type_annotation, #plain_description
                )
            }
        }
    };

    #[cfg(all(feature = "zod", not(feature = "typescript")))]
    let zod_schema_method = {
        quote! {
            pub fn zod_schema() -> String {
                format!(
                    "export const {0}$Schema = {1}.brand<\"{0}\">().meta({{\n  description: \"{2}\",\n}});",
                    #item_name, #zod_inner, #plain_description
                )
            }
        }
    };

    // --- Generate validation code for constrained branded newtypes ---
    // Uses ToString so it works for String, ObjectId, and any generic ID_TYPE that implements ToString
    #[cfg(feature = "serde")]
    let branded_validation = if args.has_string_constraints() {
        let mut checks: Vec<proc_macro2::TokenStream> = Vec::new();

        if let Some(min_len) = args.min_length {
            checks.push(quote! {
                if value.len() < #min_len {
                    return Err(format!(
                        "value is too short: minimum length is {}, got {}",
                        #min_len, value.len()
                    ));
                }
            });
        }
        if let Some(max_len) = args.max_length {
            checks.push(quote! {
                if value.len() > #max_len {
                    return Err(format!(
                        "value is too long: maximum length is {}, got {}",
                        #max_len, value.len()
                    ));
                }
            });
        }
        if let Some(ref pattern) = args.pattern {
            let pattern_lit = pattern.clone();
            checks.push(quote! {
                {
                    use std::sync::LazyLock;
                    static RE: LazyLock<regex::Regex> = LazyLock::new(|| {
                        regex::Regex::new(#pattern_lit).unwrap()
                    });
                    if !RE.is_match(value) {
                        return Err(format!(
                            "value does not match pattern '{}'",
                            #pattern_lit
                        ));
                    }
                }
            });
        }

        let validate_fn = quote! {
            pub fn validate_value(value: &str) -> Result<(), String> {
                #(#checks)*
                Ok(())
            }
        };

        let deserialize_fn = if is_generic {
            quote! {
                pub fn deserialize_value<'de, D, T>(deserializer: D) -> Result<T, D::Error>
                where
                    D: serde::Deserializer<'de>,
                    T: serde::Deserialize<'de> + std::fmt::Display,
                {
                    use serde::Deserialize;
                    let v = T::deserialize(deserializer)?;
                    validate_value(&v.to_string()).map_err(serde::de::Error::custom)?;
                    Ok(v)
                }
            }
        } else {
            quote! {
                pub fn deserialize_value<'de, D>(deserializer: D) -> Result<#inner_ty, D::Error>
                where
                    D: serde::Deserializer<'de>,
                {
                    use serde::Deserialize;
                    let v = <#inner_ty>::deserialize(deserializer)?;
                    validate_value(&v.to_string()).map_err(serde::de::Error::custom)?;
                    Ok(v)
                }
            }
        };

        Some((validate_fn, deserialize_fn))
    } else {
        None
    };

    // --- Build schema module impl items ---
    let json_schema_method = generate_alias_json_schema_stub();

    let schema_impl_items: Vec<proc_macro2::TokenStream> = vec![
        #[cfg(feature = "jsonschema")]
        json_schema_method,
        #[cfg(feature = "typescript")]
        ts_definition_method,
        #[cfg(feature = "zod")]
        zod_schema_method,
    ];

    // --- Generate schema_example method ---
    #[cfg(feature = "zod")]
    let has_example = example_code.is_some();

    #[cfg(feature = "zod")]
    let schema_example_method = example_code.as_ref().map(|code| {
        let code_tokens: proc_macro2::TokenStream = code.parse().unwrap();
        if is_generic {
            // For generic newtypes, the example constructs a concrete type (e.g., DocumentId<String>)
            // We use String as the concrete type since the Zod schema always uses z.string()
            quote! {
                #[cfg(feature = "zod")]
                pub fn schema_example() -> serde_json::Value {
                    let value: #name<String> = {
                        #code_tokens
                    };
                    serde_json::to_value(&value).unwrap()
                }
            }
        } else {
            quote! {
                #[cfg(feature = "zod")]
                pub fn schema_example() -> serde_json::Value {
                    let value: #name = {
                        #code_tokens
                    };
                    serde_json::to_value(&value).unwrap()
                }
            }
        }
    });

    // --- Generate delegate methods ---

    #[cfg(feature = "typescript")]
    let delegate_ts = {
        let mi = module_ident.clone();
        quote! {
            pub fn ts_definition() -> String {
                #mi::Schema::ts_definition()
            }
        }
    };

    #[cfg(feature = "zod")]
    let delegate_zod = {
        let mi = module_ident.clone();
        if has_example {
            quote! {
                pub fn zod_schema() -> String {
                    let base_schema = #mi::Schema::zod_schema();
                    let example_json = serde_json::to_string(&Self::schema_example()).unwrap();
                    // Insert example into .meta() before the first closing \n});
                    if let Some(pos) = base_schema.find("\n});") {
                        let mut result = base_schema[..pos].to_string();
                        result.push_str(&format!("\n  example: {},", example_json));
                        result.push_str(&base_schema[pos..]);
                        result
                    } else {
                        base_schema
                    }
                }
            }
        } else {
            quote! {
                pub fn zod_schema() -> String {
                    #mi::Schema::zod_schema()
                }
            }
        }
    };

    // --- Build delegate_impl_items ---
    let delegate_impl_items: Vec<proc_macro2::TokenStream> = vec![
        #[cfg(feature = "typescript")]
        delegate_ts,
        #[cfg(feature = "zod")]
        delegate_zod,
    ];

    // --- Use generics for the impl block so it works with generic structs ---
    // When constraints exist on a generic type, add Display bound for validation via .to_string()
    let generics_for_ty = item_struct.generics.clone();
    let (_, ty_generics, _) = generics_for_ty.split_for_impl();
    let mut generics = item_struct.generics.clone();
    #[cfg(feature = "serde")]
    if is_generic && args.has_string_constraints() {
        for param in &mut generics.params {
            if let syn::GenericParam::Type(tp) = param {
                tp.bounds.push(syn::parse_quote!(std::fmt::Display));
            }
        }
    }
    let (impl_generics, _, where_clause) = generics.split_for_impl();

    // schema_example goes on the type impl, not the module (same as structs/enums)
    #[cfg(feature = "zod")]
    let schema_example_tokens = schema_example_method.unwrap_or_default();
    #[cfg(not(feature = "zod"))]
    let schema_example_tokens = quote! {};

    // --- Inject serde(deserialize_with) on inner field and generate validate() ---
    #[cfg(feature = "serde")]
    let validation_tokens;
    #[cfg(feature = "serde")]
    let validate_method;
    #[cfg(feature = "serde")]
    {
        let mut item_struct = item_struct;
        if let Some((ref validate_fn, ref deserialize_fn)) = branded_validation {
            // Add Display bound to struct generic params so serde deserialize_with works
            if is_generic {
                for param in &mut item_struct.generics.params {
                    if let syn::GenericParam::Type(tp) = param {
                        tp.bounds.push(syn::parse_quote!(std::fmt::Display));
                    }
                }
            }
            // Add serde bound attribute so derived Deserialize passes generic bounds to deserialize_with
            if is_generic {
                let bounds: Vec<String> = generic_params.iter().map(|p| {
                    format!("{p}: serde::de::DeserializeOwned + std::fmt::Display")
                }).collect();
                let bound_str = bounds.join(", ");
                let bound_lit = syn::LitStr::new(&bound_str, proc_macro2::Span::call_site());
                let bound_attr: syn::Attribute = syn::parse_quote! {
                    #[serde(bound(deserialize = #bound_lit))]
                };
                item_struct.attrs.push(bound_attr);
            }
            let deserialize_with_path = format!("{module_name}::deserialize_value");
            let path_lit = syn::LitStr::new(&deserialize_with_path, proc_macro2::Span::call_site());
            let serde_attr: syn::Attribute = syn::parse_quote! {
                #[serde(deserialize_with = #path_lit)]
            };
            if let syn::Fields::Unnamed(ref mut fields) = item_struct.fields {
                fields.unnamed.first_mut().unwrap().attrs.push(serde_attr);
            }
            validation_tokens = quote! {
                #validate_fn
                #deserialize_fn
            };
            validate_method = quote! {
                pub fn validate(&self) -> Result<(), Vec<String>> {
                    let mut errors = Vec::new();
                    if let Err(e) = #module_ident::validate_value(&self.0.to_string()) {
                        errors.push(e);
                    }
                    if errors.is_empty() { Ok(()) } else { Err(errors) }
                }
            };
        } else {
            validation_tokens = quote! {};
            validate_method = quote! {};
        }

        let output = quote! {
            #item_struct

            pub mod #module_ident {
                use super::*;

                pub struct Schema;

                impl Schema {
                    #(#schema_impl_items)*
                }

                #validation_tokens
            }

            impl #impl_generics #name #ty_generics #where_clause {
                #(#delegate_impl_items)*
                #schema_example_tokens
                #validate_method
            }
        };

        if env::var("RUST_LOG") == Ok(String::from("trace")) {
            let output_str = output.to_string();
            println!("{output_str}");
        }

        return TokenStream::from(output);
    }

    // Without serde feature, no validation
    #[cfg(not(feature = "serde"))]
    {
        let output = quote! {
            #item_struct

            pub mod #module_ident {
                use super::*;

                pub struct Schema;

                impl Schema {
                    #(#schema_impl_items)*
                }
            }

            impl #impl_generics #name #ty_generics #where_clause {
                #(#delegate_impl_items)*
                #schema_example_tokens
            }
        };

        if env::var("RUST_LOG") == Ok(String::from("trace")) {
            let output_str = output.to_string();
            println!("{output_str}");
        }

        TokenStream::from(output)
    }
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
        let (tag_name, content_name, rename_all) = (
            serde_type_meta
                .tag
                .as_ref()
                .map_or_else(|| "type".to_string(), Clone::clone),
            serde_type_meta
                .content
                .as_ref()
                .map_or_else(|| "value".to_string(), Clone::clone),
            serde_type_meta.rename_all,
        );

        #[cfg(not(feature = "serde"))]
        let (tag_name, content_name, rename_all) = ("type".to_string(), "value".to_string(), None);

        process_discriminated_enum(item_enum, &name, &tag_name, &content_name, &rename_all, &item_name)
    }
}

/// Processes a plain enum (simple string enum in TypeScript) and generates its definitions.
fn process_plain_enum(
    mut item_enum: syn::ItemEnum,
    name: &syn::Ident,
    rename_all: &Option<String>,
    item_name: &str,
) -> TokenStream {
    // Compute module name for schema struct (same pattern as type aliases)
    #[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
    let module_name = format!("{}_schema", to_snake_case(item_name));
    #[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
    let module_ident = Ident::new(&module_name, name.span());

    // Register enum in alias registry so other types can find it
    #[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
    register_alias_info(&name.to_string(), item_name, module_name);

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

    // Generate schema module methods
    #[cfg(feature = "jsonschema")]
    let json_schema_method = generate_plain_enum_json_schema_method(&enumerated);

    #[cfg(feature = "typescript")]
    let ts_definition_method =
        generate_plain_enum_ts_definition_method(&docs, item_name, &type_code);
    #[cfg(feature = "zod")]
    let has_example = example_code.is_some();

    // Schema module generates zod_schema without examples - example injection is handled
    // by the delegating method on the type itself to avoid super:: resolution issues
    #[cfg(feature = "zod")]
    let zod_schema_method = generate_plain_enum_zod_schema_method(
        item_name,
        &schema_code,
        &plain_description,
    );

    #[cfg(not(any(feature = "typescript", feature = "zod")))]
    let _ = item_name;

    // schema_example must be directly on the type (not in module) because
    // the example code uses type names that may not be accessible from nested module
    #[cfg(feature = "zod")]
    let schema_example_method = example_code.as_ref().map(|code| {
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

    // Build schema module impl items (without schema_example)
    #[cfg(any(feature = "zod", feature = "typescript", feature = "jsonschema"))]
    let schema_impl_items: Vec<proc_macro2::TokenStream> = vec![
        #[cfg(feature = "jsonschema")]
        json_schema_method,
        #[cfg(feature = "typescript")]
        ts_definition_method,
        #[cfg(feature = "zod")]
        zod_schema_method,
    ];

    #[cfg(not(any(feature = "zod", feature = "typescript", feature = "jsonschema")))]
    let schema_impl_items: Vec<proc_macro2::TokenStream> = vec![];

    // Generate delegating methods for backwards compatibility
    #[cfg(feature = "jsonschema")]
    let delegate_json_schema = quote! {
        pub fn json_schema() -> serde_json::Value {
            #module_ident::Schema::json_schema()
        }
    };

    #[cfg(feature = "typescript")]
    let delegate_ts_definition = quote! {
        pub fn ts_definition() -> String {
            #module_ident::Schema::ts_definition()
        }
    };

    // Generate delegating zod_schema that handles example injection
    // We need to inject examples here (not in Schema module) because Self::schema_example()
    // is accessible here but not from the nested module for function-local types
    #[cfg(feature = "zod")]
    let delegate_zod_schema = if has_example {
        quote! {
            pub fn zod_schema() -> String {
                let base_schema = #module_ident::Schema::zod_schema();
                let example_json = serde_json::to_string(&Self::schema_example()).unwrap();
                // For plain enums, insert example before the FIRST closing });
                // Base format: "...meta({\n  description: \"...\",\n});\n\nexport const ..."
                // We want: "...meta({\n  description: \"...\",\n  example: ...,\n});\n\nexport const ..."
                if let Some(pos) = base_schema.find("\n});") {
                    let mut result = base_schema[..pos].to_string();
                    result.push_str(&format!("\n  example: {},", example_json));
                    result.push_str(&base_schema[pos..]);
                    result
                } else {
                    base_schema
                }
            }
        }
    } else {
        quote! {
            pub fn zod_schema() -> String {
                #module_ident::Schema::zod_schema()
            }
        }
    };

    // Build delegating impl items (schema_example is added directly, not as a delegate)
    #[cfg(all(feature = "zod", feature = "typescript", feature = "jsonschema"))]
    let delegate_impl_items: Vec<proc_macro2::TokenStream> = vec![
        delegate_json_schema,
        delegate_ts_definition,
        delegate_zod_schema,
    ]
    .into_iter()
    .chain(schema_example_method)
    .collect();

    #[cfg(all(feature = "zod", feature = "typescript", not(feature = "jsonschema")))]
    let delegate_impl_items: Vec<proc_macro2::TokenStream> = vec![
        delegate_ts_definition,
        delegate_zod_schema,
    ]
    .into_iter()
    .chain(schema_example_method)
    .collect();

    #[cfg(all(feature = "zod", not(feature = "typescript"), feature = "jsonschema"))]
    let delegate_impl_items: Vec<proc_macro2::TokenStream> = vec![
        delegate_json_schema,
        delegate_zod_schema,
    ]
    .into_iter()
    .chain(schema_example_method)
    .collect();

    #[cfg(all(feature = "zod", not(feature = "typescript"), not(feature = "jsonschema")))]
    let delegate_impl_items: Vec<proc_macro2::TokenStream> =
        vec![delegate_zod_schema]
            .into_iter()
            .chain(schema_example_method)
            .collect();

    #[cfg(all(not(feature = "zod"), feature = "typescript", feature = "jsonschema"))]
    let delegate_impl_items: Vec<proc_macro2::TokenStream> =
        vec![delegate_json_schema, delegate_ts_definition];

    #[cfg(all(not(feature = "zod"), feature = "typescript", not(feature = "jsonschema")))]
    let delegate_impl_items: Vec<proc_macro2::TokenStream> = vec![delegate_ts_definition];

    #[cfg(all(not(feature = "zod"), not(feature = "typescript"), feature = "jsonschema"))]
    let delegate_impl_items: Vec<proc_macro2::TokenStream> = vec![delegate_json_schema];

    // Use the enumerated values in the quote! macro
    let enum_values = &enumerated;

    #[cfg(any(feature = "zod", feature = "typescript", feature = "jsonschema"))]
    let output = quote! {
        #item_enum

        pub mod #module_ident {
            use super::*;

            pub struct Schema;

            impl Schema {
                #(#schema_impl_items)*
            }
        }

        impl #name {
            #(#delegate_impl_items)*

            pub fn enum_members() -> Vec<String> {
                [
                    #(#enum_values),*
                ].iter().map(|v| v.to_string()).collect::<Vec<_>>()
            }
        }
    };

    #[cfg(not(any(feature = "zod", feature = "typescript", feature = "jsonschema")))]
    let output = quote! {
        #item_enum

        impl #name {
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
    content_name: &str,
    rename_all: &Option<String>,
    item_name: &str,
) -> TokenStream {
    // Compute module name for schema struct (same pattern as type aliases)
    #[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
    let module_name = format!("{}_schema", to_snake_case(item_name));
    #[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
    let module_ident = Ident::new(&module_name, name.span());

    // Register enum in alias registry so other types can find it
    #[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
    register_alias_info(&name.to_string(), item_name, module_name);

    // Extract docs early for example extraction
    #[cfg(any(feature = "typescript", feature = "zod"))]
    let docs_vec = get_enum_docs(&item_enum);

    #[cfg(feature = "zod")]
    let example_code = docs_vec
        .as_ref()
        .and_then(|docs| extract_example_from_docs(docs));

    let mut discriminator_field_defs: HashMap<String, Vec<FieldDef>> = HashMap::new();
    let mut discriminator_field_docs: HashMap<String, String> = HashMap::new();
    let mut discriminator_variant_kinds: HashMap<String, VariantKind> = HashMap::new();
    #[cfg(feature = "jsonschema")]
    let mut json_schema_variants: Vec<proc_macro2::TokenStream> = Vec::new();

    #[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
    let enum_module_name = format!("{}_schema", to_snake_case(item_name));
    let mut enum_validation_fns: Vec<proc_macro2::TokenStream> = Vec::new();

    // Process each variant in the enum
    for item in &mut item_enum.variants {
        #[cfg(feature = "serde")]
        let field_rename = parse_serde_field_attributes(&item.attrs).rename;
        #[cfg(not(feature = "serde"))]
        let field_rename = None;

        let final_name = get_final_name(item.ident.to_string(), &field_rename, rename_all);

        // Classify the variant type (Unit, Named, TupleSingle, TupleMultiple)
        let variant_kind = classify_variant(item);

        let mut field_defs: Vec<FieldDef> = Vec::new();
        // let mut json_schema_fields: Vec<proc_macro2::TokenStream> = Vec::new();

        for field in &mut item.fields {
            #[cfg(any(feature = "typescript", feature = "zod", feature = "jsonschema"))]
            let (f_def, validation_fn, _validate_body) = process_field(rename_all, field, Some(&enum_module_name));
            #[cfg(not(any(feature = "typescript", feature = "zod", feature = "jsonschema")))]
            let (f_def, validation_fn, _validate_body) = process_field(rename_all, field, None);
            if let Some(vfn) = validation_fn {
                enum_validation_fns.push(vfn);
            }
            // json_schema_fields.push(build_field_schema(&f_def));
            field_defs.push(f_def);
        }

        discriminator_field_defs.insert(final_name.clone(), field_defs);
        discriminator_variant_kinds.insert(final_name.clone(), variant_kind);
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
        let variant_kind = &discriminator_variant_kinds[&discriminator_value];

        #[cfg(feature = "jsonschema")]
        let (variant_type_code, variant_schema_code, optional_fields, json_schema_variant) =
            generate_variant_code(
                tag_name,
                content_name,
                &discriminator_value,
                field_defs,
                variant_kind,
                &discriminator_field_docs[&discriminator_value],
                item_name,
            );

        #[cfg(not(feature = "jsonschema"))]
        let (variant_type_code, variant_schema_code, optional_fields, _json_schema_variant) =
            generate_variant_code(
                tag_name,
                content_name,
                &discriminator_value,
                field_defs,
                variant_kind,
                &discriminator_field_docs[&discriminator_value],
                item_name,
            );

        type_code_items.push(variant_type_code);
        schema_code_items.push((variant_schema_code, optional_fields));
        #[cfg(feature = "jsonschema")]
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

    // Generate schema module methods
    #[cfg(feature = "jsonschema")]
    let json_schema_method = generate_discriminated_enum_json_schema_method(&main_schema_code);

    #[cfg(feature = "typescript")]
    let ts_definition_method =
        generate_discriminated_enum_ts_definition_method(&docs, item_name, &type_code);

    #[cfg(feature = "zod")]
    let has_example = example_code.is_some();

    // Schema module generates zod_schema without examples - example injection is handled
    // by the delegating method on the type itself to avoid super:: resolution issues
    #[cfg(feature = "zod")]
    let zod_schema_method =
        generate_discriminated_enum_zod_schema_method(item_name, &schema_code);

    #[cfg(not(any(feature = "typescript", feature = "zod")))]
    let _ = item_name;

    // schema_example must be directly on the type (not in module) because
    // the example code uses type names that may not be accessible from nested module
    #[cfg(feature = "zod")]
    let schema_example_method = example_code.as_ref().map(|code| {
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

    // Build schema module impl items (without schema_example)
    #[cfg(any(feature = "zod", feature = "typescript", feature = "jsonschema"))]
    let schema_impl_items: Vec<proc_macro2::TokenStream> = vec![
        #[cfg(feature = "jsonschema")]
        json_schema_method,
        #[cfg(feature = "typescript")]
        ts_definition_method,
        #[cfg(feature = "zod")]
        zod_schema_method,
    ];

    #[cfg(not(any(feature = "zod", feature = "typescript", feature = "jsonschema")))]
    let schema_impl_items: Vec<proc_macro2::TokenStream> = vec![];

    // Generate delegating methods for backwards compatibility
    #[cfg(feature = "jsonschema")]
    let delegate_json_schema = quote! {
        pub fn json_schema() -> serde_json::Value {
            #module_ident::Schema::json_schema()
        }
    };

    #[cfg(feature = "typescript")]
    let delegate_ts_definition = quote! {
        pub fn ts_definition() -> String {
            #module_ident::Schema::ts_definition()
        }
    };

    // Generate delegating zod_schema that handles example injection
    // We need to inject examples here (not in Schema module) because Self::schema_example()
    // is accessible here but not from the nested module for function-local types
    #[cfg(feature = "zod")]
    let delegate_zod_schema = if has_example {
        quote! {
            pub fn zod_schema() -> String {
                let base_schema = #module_ident::Schema::zod_schema();
                let example_json = serde_json::to_string(&Self::schema_example()).unwrap();
                let example_part = format!(".meta({{\n  example: {}\n}})", example_json);
                // Insert .meta() before the final semicolon
                if let Some(pos) = base_schema.rfind(';') {
                    let mut result = base_schema[..pos].to_string();
                    result.push_str(&example_part);
                    result.push(';');
                    result
                } else {
                    format!("{}{}", base_schema, example_part)
                }
            }
        }
    } else {
        quote! {
            pub fn zod_schema() -> String {
                #module_ident::Schema::zod_schema()
            }
        }
    };

    // Build delegating impl items (schema_example is added directly, not as a delegate)
    #[cfg(all(feature = "zod", feature = "typescript", feature = "jsonschema"))]
    let delegate_impl_items: Vec<proc_macro2::TokenStream> = vec![
        delegate_json_schema,
        delegate_ts_definition,
        delegate_zod_schema,
    ]
    .into_iter()
    .chain(schema_example_method)
    .collect();

    #[cfg(all(feature = "zod", feature = "typescript", not(feature = "jsonschema")))]
    let delegate_impl_items: Vec<proc_macro2::TokenStream> = vec![
        delegate_ts_definition,
        delegate_zod_schema,
    ]
    .into_iter()
    .chain(schema_example_method)
    .collect();

    #[cfg(all(feature = "zod", not(feature = "typescript"), feature = "jsonschema"))]
    let delegate_impl_items: Vec<proc_macro2::TokenStream> = vec![
        delegate_json_schema,
        delegate_zod_schema,
    ]
    .into_iter()
    .chain(schema_example_method)
    .collect();

    #[cfg(all(feature = "zod", not(feature = "typescript"), not(feature = "jsonschema")))]
    let delegate_impl_items: Vec<proc_macro2::TokenStream> =
        vec![delegate_zod_schema]
            .into_iter()
            .chain(schema_example_method)
            .collect();

    #[cfg(all(not(feature = "zod"), feature = "typescript", feature = "jsonschema"))]
    let delegate_impl_items: Vec<proc_macro2::TokenStream> =
        vec![delegate_json_schema, delegate_ts_definition];

    #[cfg(all(not(feature = "zod"), feature = "typescript", not(feature = "jsonschema")))]
    let delegate_impl_items: Vec<proc_macro2::TokenStream> = vec![delegate_ts_definition];

    #[cfg(all(not(feature = "zod"), not(feature = "typescript"), feature = "jsonschema"))]
    let delegate_impl_items: Vec<proc_macro2::TokenStream> = vec![delegate_json_schema];

    #[cfg(any(feature = "zod", feature = "typescript", feature = "jsonschema"))]
    let output = quote! {
        #item_enum

        pub mod #module_ident {
            use super::*;

            pub struct Schema;

            impl Schema {
                #(#schema_impl_items)*
            }

            #(#enum_validation_fns)*
        }

        impl #name {
            #(#delegate_impl_items)*
        }
    };

    #[cfg(not(any(feature = "zod", feature = "typescript", feature = "jsonschema")))]
    let output = quote! {
        #item_enum
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
///
/// Handles different variant kinds:
/// - Unit: `{ type: "Variant" }` (no content field)
/// - Named: `{ type: "Variant", field1: T1, field2: T2 }` (individual named fields)
/// - TupleSingle: `{ type: "Variant", value: T }` (single value flattened)
/// - TupleMultiple: `{ type: "Variant", value: [T1, T2, ...] }` (tuple array)
///
/// The `self_type_name` is used to detect recursive type references and use getter syntax.
fn generate_variant_code(
    tag_name: &str,
    content_name: &str,
    discriminator_value: &str,
    field_defs: Vec<FieldDef>,
    variant_kind: &VariantKind,
    discriminator_docs: &str,
    self_type_name: &str,
) -> (String, String, Vec<String>, proc_macro2::TokenStream) {
    // Generate TypeScript type code - start with discriminator
    let mut variant_type_code =
        format!("{{  /**\n{discriminator_docs}\n**/\n  {tag_name}: \"{discriminator_value}\";\n");

    // Generate Zod schema code - start with discriminator
    let mut variant_schema_code =
        format!("{{\n  {tag_name}: z.literal(\"{discriminator_value}\"),\n");

    let mut optional_fields = Vec::new();
    #[cfg(feature = "jsonschema")]
    let mut json_schema_variant_fields: Vec<proc_macro2::TokenStream> = Vec::new();

    match variant_kind {
        VariantKind::Unit => {
            // Unit variant: no additional fields beyond the discriminator
            // TypeScript: { type: "Variant" }
            // Zod: { type: z.literal("Variant") }
        }
        VariantKind::Named => {
            // Named struct variant: keep current behavior with individual named fields
            // TypeScript: { type: "Variant", field1: T1, field2: T2 }
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
                #[cfg(feature = "zod")]
                {
                    let zod_field_type = fld.zod_type();
                    let is_recursive = fld.contains_type_reference(self_type_name);

                    if is_recursive {
                        // Use getter syntax to defer the reference
                        if let Err(err) = writeln!(
                            variant_schema_code,
                            "  get {}() {{ return {}; }},",
                            fld.name, zod_field_type
                        ) {
                            panic!("Failed to write Zod schema: {err}");
                        }
                    } else {
                        if let Err(err) =
                            writeln!(variant_schema_code, "  {}: {},", fld.name, zod_field_type)
                        {
                            panic!("Failed to write Zod schema: {err}");
                        }
                    }
                }

                #[cfg(not(feature = "zod"))]
                {
                    let _ = &variant_schema_code;
                }

                #[cfg(feature = "jsonschema")]
                if fld.name != tag_name {
                    json_schema_variant_fields.push(build_field_schema(fld));
                }

                if fld.is_optional {
                    optional_fields.push(fld.name.to_string());
                }
            }
        }
        VariantKind::TupleSingle => {
            // Single-element tuple: flatten to `value: T`
            // TypeScript: { type: "Variant", value: T }
            if let Some(fld) = field_defs.first() {
                // Add TypeScript type definition with JSDoc comment
                if let Err(err) = writeln!(
                    variant_type_code,
                    "  /** Tuple value */\n  {}: {};",
                    content_name,
                    fld.typescript_typename()
                ) {
                    panic!("Failed to write TypeScript type: {err}");
                }

                // Add Zod schema definition
                #[cfg(feature = "zod")]
                {
                    let zod_field_type = fld.zod_type();
                    let is_recursive = fld.contains_type_reference(self_type_name);

                    if is_recursive {
                        // Use getter syntax to defer the reference
                        if let Err(err) = writeln!(
                            variant_schema_code,
                            "  get {}() {{ return {}; }},",
                            content_name, zod_field_type
                        ) {
                            panic!("Failed to write Zod schema: {err}");
                        }
                    } else {
                        if let Err(err) =
                            writeln!(variant_schema_code, "  {}: {},", content_name, zod_field_type)
                        {
                            panic!("Failed to write Zod schema: {err}");
                        }
                    }
                }

                #[cfg(not(feature = "zod"))]
                {
                    let _ = &variant_schema_code;
                }

                // JSON Schema for single tuple value
                #[cfg(feature = "jsonschema")]
                {
                    let content_name_str = content_name.to_string();
                    let field_schema = build_tuple_element_json_schema(fld);
                    json_schema_variant_fields.push(quote! {
                        properties.insert(#content_name_str.to_string(), #field_schema);
                        required.push(serde_json::Value::String(#content_name_str.to_string()));
                    });
                }

                if fld.is_optional {
                    optional_fields.push(content_name.to_string());
                }
            }
        }
        VariantKind::TupleMultiple => {
            // Multi-element tuple: use TypeScript tuple type `value: [T1, T2, ...]`
            // TypeScript: { type: "Variant", value: [T1, T2, ...] }
            let ts_tuple_types: Vec<String> = field_defs
                .iter()
                .map(|fld| fld.typescript_typename())
                .collect();
            let ts_tuple = format!("[{}]", ts_tuple_types.join(", "));

            // Add TypeScript type definition with JSDoc comment explaining tuple structure
            let tuple_desc: Vec<String> = field_defs
                .iter()
                .enumerate()
                .map(|(i, _)| format!("element {}", i))
                .collect();
            if let Err(err) = writeln!(
                variant_type_code,
                "  /** Tuple: [{}] */\n  {}: {};",
                tuple_desc.join(", "),
                content_name,
                ts_tuple
            ) {
                panic!("Failed to write TypeScript type: {err}");
            }

            // Add Zod schema definition using z.tuple()
            #[cfg(feature = "zod")]
            {
                let zod_tuple_types: Vec<String> =
                    field_defs.iter().map(|fld| fld.zod_type()).collect();
                let zod_tuple = format!("z.tuple([{}])", zod_tuple_types.join(", "));

                // Check if any field in the tuple contains a recursive reference
                let is_recursive = field_defs
                    .iter()
                    .any(|fld| fld.contains_type_reference(self_type_name));

                if is_recursive {
                    // Use getter syntax to defer the reference
                    if let Err(err) = writeln!(
                        variant_schema_code,
                        "  get {}() {{ return {}; }},",
                        content_name, zod_tuple
                    ) {
                        panic!("Failed to write Zod schema: {err}");
                    }
                } else {
                    if let Err(err) =
                        writeln!(variant_schema_code, "  {}: {},", content_name, zod_tuple)
                    {
                        panic!("Failed to write Zod schema: {err}");
                    }
                }
            }

            #[cfg(not(feature = "zod"))]
            {
                let _ = &variant_schema_code;
            }

            // JSON Schema for tuple (using prefixItems)
            #[cfg(feature = "jsonschema")]
            {
                let content_name_str = content_name.to_string();
                let tuple_schemas: Vec<proc_macro2::TokenStream> = field_defs
                    .iter()
                    .map(build_tuple_element_json_schema)
                    .collect();
                json_schema_variant_fields.push(quote! {
                    properties.insert(#content_name_str.to_string(), {
                        serde_json::json!({
                            "type": "array",
                            "prefixItems": [#(#tuple_schemas),*],
                            "items": false
                        })
                    });
                    required.push(serde_json::Value::String(#content_name_str.to_string()));
                });
            }
        }
    }

    // Complete the type and schema code
    variant_type_code.push('}');
    variant_schema_code.push('}');

    // Create JSON schema for this variant
    #[cfg(feature = "jsonschema")]
    let json_schema_variant = {
        let discriminator_value_str = discriminator_value.to_string();
        let tag_name_str = tag_name.to_string();

        quote! {
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
        }
    };

    #[cfg(not(feature = "jsonschema"))]
    let json_schema_variant = quote! {};

    (
        variant_type_code,
        variant_schema_code,
        optional_fields,
        json_schema_variant,
    )
}

/// Builds JSON schema for a tuple element (used for single tuple and multi-tuple variants).
#[cfg(feature = "jsonschema")]
fn build_tuple_element_json_schema(fld: &FieldDef) -> proc_macro2::TokenStream {
    let field_type = &fld.field_type;

    match field_type {
        FieldDefType::String => {
            if fld.is_array {
                quote! { serde_json::json!({ "type": "array", "items": { "type": "string" } }) }
            } else {
                quote! { serde_json::json!({ "type": "string" }) }
            }
        }
        FieldDefType::StringLiteral(literal) => {
            if fld.is_array {
                quote! { serde_json::json!({ "type": "array", "items": { "type": "string", "const": #literal } }) }
            } else {
                quote! { serde_json::json!({ "type": "string", "const": #literal }) }
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
            if fld.is_array {
                quote! { serde_json::json!({ "type": "array", "items": { "type": "integer" } }) }
            } else {
                quote! { serde_json::json!({ "type": "integer" }) }
            }
        }
        FieldDefType::F32 | FieldDefType::F64 => {
            if fld.is_array {
                quote! { serde_json::json!({ "type": "array", "items": { "type": "number" } }) }
            } else {
                quote! { serde_json::json!({ "type": "number" }) }
            }
        }
        FieldDefType::Boolean => {
            if fld.is_array {
                quote! { serde_json::json!({ "type": "array", "items": { "type": "boolean" } }) }
            } else {
                quote! { serde_json::json!({ "type": "boolean" }) }
            }
        }
        #[cfg(feature = "chrono")]
        FieldDefType::NaiveDate => {
            if fld.is_array {
                quote! { serde_json::json!({ "type": "array", "items": { "type": "string", "format": "date" } }) }
            } else {
                quote! { serde_json::json!({ "type": "string", "format": "date" }) }
            }
        }
        #[cfg(feature = "chrono")]
        FieldDefType::NaiveTime => {
            if fld.is_array {
                quote! { serde_json::json!({ "type": "array", "items": { "type": "string", "format": "time" } }) }
            } else {
                quote! { serde_json::json!({ "type": "string", "format": "time" }) }
            }
        }
        #[cfg(feature = "chrono")]
        FieldDefType::NaiveDateTime | FieldDefType::DateTime => {
            if fld.is_array {
                quote! { serde_json::json!({ "type": "array", "items": { "type": "string", "format": "date-time" } }) }
            } else {
                quote! { serde_json::json!({ "type": "string", "format": "date-time" }) }
            }
        }
        #[cfg(feature = "object_id")]
        FieldDefType::ObjectId => {
            if fld.is_array {
                quote! { serde_json::json!({
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "$oid": { "type": "string", "pattern": "^[a-f\\d]{24}$" }
                        },
                        "required": ["$oid"]
                    }
                }) }
            } else {
                quote! { serde_json::json!({
                    "type": "object",
                    "properties": {
                        "$oid": { "type": "string", "pattern": "^[a-f\\d]{24}$" }
                    },
                    "required": ["$oid"]
                }) }
            }
        }
        _ => {
            // For unknown/sibling types, use a generic object schema
            quote! { serde_json::json!({ "type": "object" }) }
        }
    }
}

/// Builds JSON schema for a field.
#[cfg(feature = "jsonschema")]
fn build_field_schema(fld: &FieldDef) -> proc_macro2::TokenStream {
    let field_name = &fld.name;
    let field_name_str = field_name.to_string();
    let field_type = &fld.field_type;

    let schema_code = match field_type {
        FieldDefType::String => {
            // Extract string constraints from model_schema_prop_meta
            let min_len_opt = fld.model_schema_prop_meta.as_ref().and_then(|m| m.min_length);
            let max_len_opt = fld.model_schema_prop_meta.as_ref().and_then(|m| m.max_length);
            let pattern_opt = fld
                .model_schema_prop_meta
                .as_ref()
                .and_then(|m| m.pattern.as_deref().map(ToString::to_string));

            // Generate constraint insertion statements
            let min_len_insert = min_len_opt.map(|min_len| {
                quote! { schema_obj.insert("minLength".to_string(), serde_json::json!(#min_len)); }
            });
            let max_len_insert = max_len_opt.map(|max_len| {
                quote! { schema_obj.insert("maxLength".to_string(), serde_json::json!(#max_len)); }
            });
            let pattern_insert = pattern_opt.as_ref().map(|pattern| {
                quote! { schema_obj.insert("pattern".to_string(), serde_json::json!(#pattern)); }
            });

            if fld.is_array {
                quote! {
                    properties.insert(#field_name_str.to_string(), {
                        let mut schema_obj = serde_json::Map::new();
                        schema_obj.insert("type".to_string(), serde_json::json!("string"));
                        #min_len_insert
                        #max_len_insert
                        #pattern_insert
                        let items = serde_json::Value::Object(schema_obj);
                        serde_json::json!({
                            "type": "array",
                            "items": items
                        })
                    });
                }
            } else {
                quote! {
                    properties.insert(#field_name_str.to_string(), {
                        let mut schema_obj = serde_json::Map::new();
                        schema_obj.insert("type".to_string(), serde_json::json!("string"));
                        #min_len_insert
                        #max_len_insert
                        #pattern_insert
                        serde_json::Value::Object(schema_obj)
                    });
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
            let minimum_opt = fld.model_schema_prop_meta.as_ref().and_then(|m| m.minimum);
            let maximum_opt = fld.model_schema_prop_meta.as_ref().and_then(|m| m.maximum);
            let minimum_insert = minimum_opt.map(|min| {
                quote! { schema_obj.insert("minimum".to_string(), serde_json::json!(#min)); }
            });
            let maximum_insert = maximum_opt.map(|max| {
                quote! { schema_obj.insert("maximum".to_string(), serde_json::json!(#max)); }
            });

            if fld.is_array {
                quote! {
                    properties.insert(#field_name_str.to_string(), {
                        let mut schema_obj = serde_json::Map::new();
                        schema_obj.insert("type".to_string(), serde_json::json!("integer"));
                        #minimum_insert
                        #maximum_insert
                        let items = serde_json::Value::Object(schema_obj);
                        serde_json::json!({
                            "type": "array",
                            "items": items
                        })
                    });
                }
            } else {
                quote! {
                    properties.insert(#field_name_str.to_string(), {
                        let mut schema_obj = serde_json::Map::new();
                        schema_obj.insert("type".to_string(), serde_json::json!("integer"));
                        #minimum_insert
                        #maximum_insert
                        serde_json::Value::Object(schema_obj)
                    });
                }
            }
        }
        FieldDefType::F32 | FieldDefType::F64 => {
            let minimum_opt = fld.model_schema_prop_meta.as_ref().and_then(|m| m.minimum);
            let maximum_opt = fld.model_schema_prop_meta.as_ref().and_then(|m| m.maximum);
            let minimum_insert = minimum_opt.map(|min| {
                quote! { schema_obj.insert("minimum".to_string(), serde_json::json!(#min)); }
            });
            let maximum_insert = maximum_opt.map(|max| {
                quote! { schema_obj.insert("maximum".to_string(), serde_json::json!(#max)); }
            });

            if fld.is_array {
                quote! {
                    properties.insert(#field_name_str.to_string(), {
                        let mut schema_obj = serde_json::Map::new();
                        schema_obj.insert("type".to_string(), serde_json::json!("number"));
                        #minimum_insert
                        #maximum_insert
                        let items = serde_json::Value::Object(schema_obj);
                        serde_json::json!({
                            "type": "array",
                            "items": items
                        })
                    });
                }
            } else {
                quote! {
                    properties.insert(#field_name_str.to_string(), {
                        let mut schema_obj = serde_json::Map::new();
                        schema_obj.insert("type".to_string(), serde_json::json!("number"));
                        #minimum_insert
                        #maximum_insert
                        serde_json::Value::Object(schema_obj)
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
        #[cfg(feature = "chrono")]
        FieldDefType::NaiveDate => {
            if fld.is_array {
                quote! {
                    properties.insert(#field_name_str.to_string(), {
                        serde_json::json!({
                            "type": "array",
                            "items": { "type": "string", "format": "date" }
                        })
                    });
                }
            } else {
                quote! {
                    properties.insert(#field_name_str.to_string(), {
                        serde_json::json!({
                            "type": "string",
                            "format": "date"
                        })
                    });
                }
            }
        }
        #[cfg(feature = "chrono")]
        FieldDefType::NaiveTime => {
            if fld.is_array {
                quote! {
                    properties.insert(#field_name_str.to_string(), {
                        serde_json::json!({
                            "type": "array",
                            "items": { "type": "string", "format": "time" }
                        })
                    });
                }
            } else {
                quote! {
                    properties.insert(#field_name_str.to_string(), {
                        serde_json::json!({
                            "type": "string",
                            "format": "time"
                        })
                    });
                }
            }
        }
        #[cfg(feature = "chrono")]
        FieldDefType::NaiveDateTime => {
            if fld.is_array {
                quote! {
                    properties.insert(#field_name_str.to_string(), {
                        serde_json::json!({
                            "type": "array",
                            "items": { "type": "string", "format": "date-time" }
                        })
                    });
                }
            } else {
                quote! {
                    properties.insert(#field_name_str.to_string(), {
                        serde_json::json!({
                            "type": "string",
                            "format": "date-time"
                        })
                    });
                }
            }
        }
        #[cfg(feature = "chrono")]
        FieldDefType::DateTime => {
            if fld.is_array {
                quote! {
                    properties.insert(#field_name_str.to_string(), {
                        serde_json::json!({
                            "type": "array",
                            "items": { "type": "string", "format": "date-time" }
                        })
                    });
                }
            } else {
                quote! {
                    properties.insert(#field_name_str.to_string(), {
                        serde_json::json!({
                            "type": "string",
                            "format": "date-time"
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
                    // Fallback: Use module pattern for types that may be defined elsewhere.
                    // The type should have been registered - generate a module reference.
                    let safe_name = safe_type_name(name);
                    let module_name = format!("{}_schema", to_snake_case(&safe_name));
                    let module_ident = proc_macro2::Ident::new(
                        module_name.as_str(),
                        proc_macro2::Span::call_site(),
                    );
                    let type_json_schema = quote! { #module_ident::Schema::json_schema() };

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
                    // For enum_members(), we call the type directly (delegation works)
                    let key_type_name_ident = proc_macro2::Ident::new(
                        key_type_name.as_str(),
                        proc_macro2::Span::call_site(),
                    );

                    let value_schema_code = match &value.field_type {
                        FieldDefType::SiblingType(value_type_name, lst) if lst.is_empty() => {
                            // For json_schema(), use the module pattern
                            let safe_value_name = safe_type_name(value_type_name);
                            let value_module_name =
                                format!("{}_schema", to_snake_case(&safe_value_name));
                            let value_module_ident = proc_macro2::Ident::new(
                                value_module_name.as_str(),
                                proc_macro2::Span::call_site(),
                            );
                            quote! { let value_schema = #value_module_ident::Schema::json_schema(); }
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
            // Fallback: Use module pattern. This should not typically be reached
            // for properly annotated types, but provides a reasonable fallback.
            let name = &fld.name;
            let safe_name = safe_type_name(name);
            let module_name = format!("{}_schema", to_snake_case(&safe_name));
            let module_ident = proc_macro2::Ident::new(
                module_name.as_str(),
                proc_macro2::Span::call_site(),
            );
            let type_json_schema = quote! { #module_ident::Schema::json_schema() };
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
///
/// The `self_type_name` parameter is used to detect recursive type references.
/// When a field references the type being defined, we use JavaScript getter syntax
/// to defer the reference and avoid "use before declaration" errors.
fn write_field_type_and_schema(
    type_code: &mut String,
    schema_code: &mut String,
    fld: &FieldDef,
    self_type_name: Option<&str>,
) {
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
        let zod_type = fld.zod_type();

        // Check if this field contains a recursive reference to self
        let is_recursive = self_type_name.is_some_and(|name| fld.contains_type_reference(name));

        if is_recursive {
            // Use getter syntax to defer the reference
            if let Err(err) = writeln!(
                schema_code,
                "  get {}() {{ return {}; }},",
                fld.name, zod_type
            ) {
                panic!("Failed to write Zod schema: {err}");
            }
        } else {
            // Normal property syntax
            if let Err(err) = writeln!(schema_code, "  {}: {},", fld.name, zod_type) {
                panic!("Failed to write Zod schema: {err}");
            }
        }
    }

    #[cfg(not(feature = "zod"))]
    {
        // When zod feature is disabled, don't write to schema_code
        let _ = schema_code; // Suppress unused variable warning
        let _ = self_type_name; // Suppress unused variable warning
    }
}

/// Holds the generated validation code for a single field.
#[cfg(feature = "serde")]
struct FieldValidationCode {
    /// Functions to emit into the schema module (static validator + serde deserializer)
    pub module_items: proc_macro2::TokenStream,
    /// Code to contribute to the type-level `validate()` method body
    pub validate_body: proc_macro2::TokenStream,
}

/// Generates the static validator and serde deserializer for a String field with constraints.
///
/// Returns (`module_items`, `validate_body`) — both go into the schema module and validate() respectively.
#[cfg(feature = "serde")]
fn generate_string_validation_code(
    field_ident: &str,
    meta: &crate::features::model_schema_prop::ModelSchemaPropMeta,
) -> FieldValidationCode {
    let validate_value_fn_name = format!("validate_{field_ident}_value");
    let validate_value_fn_ident =
        proc_macro2::Ident::new(&validate_value_fn_name, proc_macro2::Span::call_site());
    let deserialize_fn_name = format!("deserialize_{field_ident}");
    let deserialize_fn_ident =
        proc_macro2::Ident::new(&deserialize_fn_name, proc_macro2::Span::call_site());

    let field_name_lit = field_ident.to_string();

    // Build validation checks
    let mut checks: Vec<proc_macro2::TokenStream> = Vec::new();

    if let Some(min_len) = meta.min_length {
        checks.push(quote! {
            if value.len() < #min_len {
                return Err(format!(
                    "'{}' is too short: minimum length is {}, got {}",
                    #field_name_lit, #min_len, value.len()
                ));
            }
        });
    }

    if let Some(max_len) = meta.max_length {
        checks.push(quote! {
            if value.len() > #max_len {
                return Err(format!(
                    "'{}' is too long: maximum length is {}, got {}",
                    #field_name_lit, #max_len, value.len()
                ));
            }
        });
    }

    if let Some(ref pattern) = meta.pattern {
        let pattern_lit = pattern.to_string();
        checks.push(quote! {
            {
                use std::sync::LazyLock;
                static RE: LazyLock<regex::Regex> = LazyLock::new(|| {
                    regex::Regex::new(#pattern_lit).unwrap()
                });
                if !RE.is_match(value) {
                    return Err(format!(
                        "'{}' does not match pattern '{}'",
                        #field_name_lit, #pattern_lit
                    ));
                }
            }
        });
    }

    let module_items = quote! {
        pub fn #validate_value_fn_ident(value: &str) -> Result<(), String> {
            #(#checks)*
            Ok(())
        }

        pub fn #deserialize_fn_ident<'de, D>(deserializer: D) -> Result<String, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            use serde::Deserialize;
            let s = String::deserialize(deserializer)?;
            #validate_value_fn_ident(&s).map_err(serde::de::Error::custom)?;
            Ok(s)
        }
    };

    let field_ident_tok =
        proc_macro2::Ident::new(field_ident, proc_macro2::Span::call_site());

    let validate_body = quote! {
        if let Err(e) = #validate_value_fn_ident(&self.#field_ident_tok) {
            errors.push(e);
        }
    };

    FieldValidationCode {
        module_items,
        validate_body,
    }
}

/// Generates the static validator and serde deserializer for a numeric field with constraints.
#[cfg(feature = "serde")]
fn generate_numeric_validation_code(
    field_ident: &str,
    rust_type_str: &str,
    meta: &crate::features::model_schema_prop::ModelSchemaPropMeta,
) -> FieldValidationCode {
    let validate_value_fn_name = format!("validate_{field_ident}_value");
    let validate_value_fn_ident =
        proc_macro2::Ident::new(&validate_value_fn_name, proc_macro2::Span::call_site());
    let deserialize_fn_name = format!("deserialize_{field_ident}");
    let deserialize_fn_ident =
        proc_macro2::Ident::new(&deserialize_fn_name, proc_macro2::Span::call_site());

    let rust_type_ident: proc_macro2::TokenStream = rust_type_str.parse().unwrap();
    let field_name_lit = field_ident.to_string();

    let mut checks: Vec<proc_macro2::TokenStream> = Vec::new();

    if let Some(minimum) = meta.minimum {
        // Cast to the correct type for comparison
        let min_cast: proc_macro2::TokenStream = format!("{minimum} as {rust_type_str}").parse().unwrap();
        checks.push(quote! {
            if *value < #min_cast {
                return Err(format!(
                    "'{}' is too small: minimum is {}, got {}",
                    #field_name_lit, #minimum, value
                ));
            }
        });
    }

    if let Some(maximum) = meta.maximum {
        let max_cast: proc_macro2::TokenStream = format!("{maximum} as {rust_type_str}").parse().unwrap();
        checks.push(quote! {
            if *value > #max_cast {
                return Err(format!(
                    "'{}' is too large: maximum is {}, got {}",
                    #field_name_lit, #maximum, value
                ));
            }
        });
    }

    let module_items = quote! {
        pub fn #validate_value_fn_ident(value: &#rust_type_ident) -> Result<(), String> {
            #(#checks)*
            Ok(())
        }

        pub fn #deserialize_fn_ident<'de, D>(deserializer: D) -> Result<#rust_type_ident, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            use serde::Deserialize;
            let v = #rust_type_ident::deserialize(deserializer)?;
            #validate_value_fn_ident(&v).map_err(serde::de::Error::custom)?;
            Ok(v)
        }
    };

    let field_ident_tok =
        proc_macro2::Ident::new(field_ident, proc_macro2::Span::call_site());

    let validate_body = quote! {
        if let Err(e) = #validate_value_fn_ident(&self.#field_ident_tok) {
            errors.push(e);
        }
    };

    FieldValidationCode {
        module_items,
        validate_body,
    }
}

/// Processes a field and returns its definition, optional module items (validators/deserializers),
/// and optional validate_body (contribution to the type-level `validate()` method).
fn process_field(
    rename_all: &Option<String>,
    field: &mut Field,
    schema_module_name: Option<&str>,
) -> (FieldDef, Option<proc_macro2::TokenStream>, Option<proc_macro2::TokenStream>) {
    let mut new_attrs = Vec::new();

    #[cfg(feature = "serde")]
    let field_rename = parse_serde_field_attributes(&field.attrs).rename;
    #[cfg(not(feature = "serde"))]
    let field_rename = None;

    // Get raw field ident (before renaming) for validation function name
    let raw_field_ident = field
        .ident
        .as_ref()
        .map(ToString::to_string)
        .unwrap_or_default();

    // Parse model_schema_prop attributes before filtering them out
    let model_schema_prop_meta =
        crate::features::model_schema_prop::parse_model_schema_prop_attributes(&field.attrs);

    // Validate: cannot use both `as` and `preprocess` on the same field
    if model_schema_prop_meta.as_type.is_some() && !model_schema_prop_meta.preprocess.is_empty() {
        panic!("Cannot use both `as` and `preprocess` on the same field in model_schema_prop");
    }

    // Filter out model_schema_prop attributes, and optionally inject serde deserialize_with
    for attr in &field.attrs {
        if !attr.path().is_ident("model_schema_prop") {
            new_attrs.push(attr.clone());
        }
    }

    // Determine the Rust type for numeric validation (bare numeric types only)
    #[cfg(feature = "serde")]
    fn field_rust_type_str(field: &Field) -> Option<&'static str> {
        // Look at the field type to determine the concrete Rust type for numeric validators.
        // Only matches bare types (not wrapped in Vec, Option, etc.)
        if let syn::Type::Path(tp) = &field.ty
            && let Some(seg) = tp.path.segments.last()
            && matches!(seg.arguments, syn::PathArguments::None)
        {
            return match seg.ident.to_string().as_str() {
                "u8" => Some("u8"),
                "u16" => Some("u16"),
                "u32" => Some("u32"),
                "u64" => Some("u64"),
                "i8" => Some("i8"),
                "i16" => Some("i16"),
                "i32" => Some("i32"),
                "i64" => Some("i64"),
                "usize" => Some("usize"),
                "isize" => Some("isize"),
                "f32" => Some("f32"),
                "f64" => Some("f64"),
                _ => None,
            };
        }
        None
    }

    // Check if the field is a bare String type (not Vec<String>, Option<String>, etc.)
    #[cfg(feature = "serde")]
    fn field_is_bare_string(field: &Field) -> bool {
        if let syn::Type::Path(tp) = &field.ty
            && let Some(seg) = tp.path.segments.last()
        {
            return seg.ident == "String"
                && matches!(seg.arguments, syn::PathArguments::None);
        }
        false
    }

    // Determine if this field has string constraints (minLength, maxLength, pattern)
    // Only applicable to bare String fields (not Vec<String>, Option<String>, etc.)
    #[cfg(feature = "serde")]
    let has_string_constraints = (model_schema_prop_meta.min_length.is_some()
        || model_schema_prop_meta.max_length.is_some()
        || model_schema_prop_meta.pattern.is_some())
        && field_is_bare_string(field);

    // Determine if this field has numeric constraints (minimum, maximum)
    #[cfg(feature = "serde")]
    let has_numeric_constraints =
        model_schema_prop_meta.minimum.is_some() || model_schema_prop_meta.maximum.is_some();

    // Generate validation code and inject serde attribute if serde feature is enabled
    #[cfg(feature = "serde")]
    let (validation_fn, validate_body): (
        Option<proc_macro2::TokenStream>,
        Option<proc_macro2::TokenStream>,
    ) = if let Some(module_name) = schema_module_name {
        if has_string_constraints {
            // String field with constraints: generate static validator + deserializer
            let validation_code =
                generate_string_validation_code(&raw_field_ident, &model_schema_prop_meta);
            let deserialize_with_path =
                format!("{module_name}::deserialize_{raw_field_ident}");
            let path_lit =
                syn::LitStr::new(&deserialize_with_path, proc_macro2::Span::call_site());
            let serde_attr: syn::Attribute = syn::parse_quote! {
                #[serde(deserialize_with = #path_lit)]
            };
            new_attrs.push(serde_attr);
            (
                Some(validation_code.module_items),
                Some(validation_code.validate_body),
            )
        } else if has_numeric_constraints {
            // Numeric field with constraints: generate static validator + deserializer
            if let Some(rust_type) = field_rust_type_str(field) {
                let validation_code = generate_numeric_validation_code(
                    &raw_field_ident,
                    rust_type,
                    &model_schema_prop_meta,
                );
                let deserialize_with_path =
                    format!("{module_name}::deserialize_{raw_field_ident}");
                let path_lit =
                    syn::LitStr::new(&deserialize_with_path, proc_macro2::Span::call_site());
                let serde_attr: syn::Attribute = syn::parse_quote! {
                    #[serde(deserialize_with = #path_lit)]
                };
                new_attrs.push(serde_attr);
                (
                    Some(validation_code.module_items),
                    Some(validation_code.validate_body),
                )
            } else {
                (None, None)
            }
        } else {
            (None, None)
        }
    } else {
        (None, None)
    };

    #[cfg(not(feature = "serde"))]
    let (validation_fn, validate_body): (
        Option<proc_macro2::TokenStream>,
        Option<proc_macro2::TokenStream>,
    ) = (None, None);

    field.attrs = new_attrs;

    let field_type: &syn::Type = &field.ty;
    let name = raw_field_ident;

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
        || model_schema_prop_meta.max_length.is_some()
        || model_schema_prop_meta.pattern.is_some()
        || model_schema_prop_meta.minimum.is_some()
        || model_schema_prop_meta.maximum.is_some()
        || !model_schema_prop_meta.preprocess.is_empty()
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

    // Update field docs to include length/range constraint information
    if let Some(ref meta) = field_def.model_schema_prop_meta {
        let mut constraint_docs: Vec<String> = Vec::new();
        if let Some(min_len) = meta.min_length {
            constraint_docs.push(format!(" * Minimum length: {min_len}"));
        }
        if let Some(max_len) = meta.max_length {
            constraint_docs.push(format!(" * Maximum length: {max_len}"));
        }
        if let Some(minimum) = meta.minimum {
            constraint_docs.push(format!(" * Minimum value: {minimum}"));
        }
        if let Some(maximum) = meta.maximum {
            constraint_docs.push(format!(" * Maximum value: {maximum}"));
        }
        if !constraint_docs.is_empty() {
            let extra_docs = constraint_docs.join("\n");
            field_def.docs = if field_def.docs.is_empty() {
                format!(" * {final_name}\n * \n{extra_docs}")
            } else {
                format!("{}\n{}", field_def.docs, extra_docs)
            };
        }
    }

    (field_def, validation_fn, validate_body)
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
) -> proc_macro2::TokenStream {
    #[cfg(feature = "zod")]
    {
        // When typescript feature is enabled, generate TypeScript-style Zod schema
        // Note: Example injection is handled by the delegating method on the type itself
        #[cfg(feature = "typescript")]
        {
            quote::quote! {
                pub fn zod_schema() -> String {
                    format!(r#"const {}$RawSchema = z.strictObject({{
{}
}}){};

export const {}$Schema: ZodType<{}> = {}$RawSchema;"#, #item_name, #schema_code, #show_opts, #item_name, #item_name, #item_name)
                }
            }
        }

        // When typescript feature is disabled, generate JavaScript-style Zod schema
        #[cfg(not(feature = "typescript"))]
        {
            quote::quote! {
                pub fn zod_schema() -> String {
                    format!(r#"export const {}$Schema = z.strictObject({{
{}
}}){};"#, #item_name, #schema_code, #show_opts)
                }
            }
        }
    }

    #[cfg(not(feature = "zod"))]
    {
        let _ = (item_name, schema_code, show_opts);
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
    enumerated: &[proc_macro2::TokenStream],
) -> proc_macro2::TokenStream {
    #[cfg(feature = "jsonschema")]
    {
        crate::features::jsonschema::generate_plain_enum_json_schema_method(enumerated)
    }

    #[cfg(not(feature = "jsonschema"))]
    {
        let _ = enumerated; // Suppress unused variable warning
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
/// Note: Example injection is handled by the delegating method on the type itself
fn generate_plain_enum_zod_schema_method(
    item_name: &str,
    schema_code: &str,
    description: &str,
) -> proc_macro2::TokenStream {
    #[cfg(feature = "zod")]
    {
        // When typescript feature is enabled, generate TypeScript-style Zod schema
        #[cfg(feature = "typescript")]
        {
            quote::quote! {
                pub fn zod_schema() -> String {
                    format!("const {}$RawSchema = z.enum([{}]).meta({{\n  description: \"{}\",\n}});\n\nexport const {}$Schema: ZodType<{}> = {}$RawSchema;", #item_name, #schema_code, #description, #item_name, #item_name, #item_name)
                }
            }
        }

        // When typescript feature is disabled, generate JavaScript-style Zod schema
        #[cfg(not(feature = "typescript"))]
        {
            quote::quote! {
                pub fn zod_schema() -> String {
                    format!("export const {}$Schema = z.enum([{}]).meta({{\n  description: \"{}\",\n}});", #item_name, #schema_code, #description)
                }
            }
        }
    }

    #[cfg(not(feature = "zod"))]
    {
        let _ = (item_name, schema_code, description);
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
/// Note: Example injection is handled by the delegating method on the type itself
fn generate_discriminated_enum_zod_schema_method(
    item_name: &str,
    schema_code: &str,
) -> proc_macro2::TokenStream {
    #[cfg(feature = "zod")]
    {
        // When typescript feature is enabled, generate TypeScript-style Zod schema
        #[cfg(feature = "typescript")]
        {
            quote::quote! {
                pub fn zod_schema() -> String {
                    format!("const {}$RawSchema = {};\n\nexport const {}$Schema: ZodType<{}> = {}$RawSchema;", #item_name, #schema_code, #item_name, #item_name, #item_name)
                }
            }
        }

        // When typescript feature is disabled, generate JavaScript-style Zod schema
        #[cfg(not(feature = "typescript"))]
        {
            quote::quote! {
                pub fn zod_schema() -> String {
                    format!(r#"export const {}$Schema = {};"#, #item_name, #schema_code)
                }
            }
        }
    }

    #[cfg(not(feature = "zod"))]
    {
        let _ = (item_name, schema_code);
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

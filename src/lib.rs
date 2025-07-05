mod field_type;
mod model_schema;
mod utils;
use model_schema::exec_model_schema;
use proc_macro::TokenStream;
use utils::safe_type_name;

/// # model_schema
///
/// A macro that generates TypeScript type definitions and Zod validation schemas for Rust structs and enums.
///
/// This macro adds a `ts_definition()` method to the annotated type that returns TypeScript type definitions
/// and Zod schemas as strings. It's particularly useful for maintaining consistent data structures
/// between your Rust backend and TypeScript/JavaScript frontend.
///
/// ## Features
///
/// - Generates TypeScript interfaces/types that mirror your Rust structs and enums
/// - Creates Zod validation schemas for runtime validation in JavaScript
/// - Respects Serde attributes like `rename` and `rename_all`
/// - Provides proper type mappings between Rust and TypeScript
/// - Handles nested types, generics, optional fields, and collections
///
/// ## Usage
///
/// ```rust
/// use core_model_macros::model_schema;
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Serialize, Deserialize)]
/// #[serde(rename_all = "camelCase")]
/// #[model_schema()]
/// pub struct User {
///     pub id: String,
///     pub first_name: String,
///     pub last_name: String,
///     pub age: Option<u32>,
///     pub roles: Vec<String>,
/// }
///
/// // This will generate a ts_definition() method that returns:
/// //
/// // export type User = {
/// //   id: string,
/// //   firstName: string,
/// //   lastName: string,
/// //   age: number | undefined,
/// //   roles: Array<string>,
/// // };
/// //
/// // export const User$Schema: z.Schema<User, z.ZodTypeDef, unknown> = z.strictObject({
/// //   id: z.string(),
/// //   firstName: z.string(),
/// //   lastName: z.string(),
/// //   age: z.number().optional(),
/// //   roles: z.array(z.string()),
/// // });
/// ```
///
/// ## Enum Support
///
/// ```rust
/// use core_model_macros::model_schema;
/// use serde;
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Serialize, Deserialize)]
/// #[serde(rename_all = "lowercase")]
/// #[model_schema()]
/// pub enum Status {
///     Active,
///     Pending,
///     Inactive,
/// }
///
/// // Generates:
/// // export type Status = "active" | "pending" | "inactive";
/// // export const Status$Schema: z.Schema<Status> = z.enum(["active", "pending", "inactive"]);
/// ```
///
/// ## Tagged Unions (Discriminated Unions)
///
/// ```rust
/// use core_model_macros::model_schema;
/// use serde;
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Serialize, Deserialize)]
/// #[serde(tag = "type", rename_all = "camelCase")]
/// #[model_schema()]
/// pub enum Event {
///     UserCreated {
///         user_id: String,
///         timestamp: String,
///     },
///     UserDeleted {
///         user_id: String,
///         reason: Option<String>,
///     }
/// }
///
/// // Generates a discriminated union in TypeScript:
/// // export type Event = {
/// //   type: "userCreated";
/// //   userId: string;
/// //   timestamp: string;
/// // } | {
/// //   type: "userDeleted";
/// //   userId: string;
/// //   reason: string | undefined;
/// // };
/// ```
///
#[proc_macro_attribute]
pub fn model_schema(args: TokenStream, input: TokenStream) -> TokenStream {
    exec_model_schema(args, input)
}

/// # model_schema_prop
///
/// A field-level attribute for customizing the TypeScript type generation for specific fields
/// within a struct or enum marked with `#[model_schema()]`.
///
/// ## Usage
///
/// ```rust
/// use core_model_macros::model_schema;
/// use core_model_macros::model_schema_prop;
/// use serde::{Deserialize, Serialize};
///
/// #[model_schema()]
/// #[derive(Serialize, Deserialize)]
/// pub struct ApiConfig {
///     // Override the TypeScript type for this field
///     #[model_schema_prop(as = String)]
///     pub metric: String,
///
///     // Regular fields without customization
///     pub enabled: bool,
/// }
/// ```
///
/// ## Parameters
///
/// - `as`: Specifies an explicit type to use for the field in TypeScript
///
/// ## Example
///
/// ```rust
/// use core_model_macros::model_schema_prop;
/// use serde::{Deserialize, Serialize};
/// use core_model_macros::model_schema;
///
/// #[model_schema()]
/// #[derive(Serialize, Deserialize)]
/// pub struct UsagePricingJson {
///     // This will be rendered as "string" in TypeScript, potentially
///     // overriding a different default mapping
///     #[model_schema_prop(as = String)]
///     pub metric: String,
///
///     pub free_units: Vec<FreeUnitsJson>,
/// }
///
/// #[model_schema()]
/// #[derive(Serialize, Deserialize)]
/// pub enum FreeUnitsJson {
///     Fixed {
///         value: u32,
///     },
///     PerEntity {
///         entity_type: String,
///         amount_per_entity: u32,
///     },
/// }
/// ```
#[proc_macro_attribute]
pub fn model_schema_prop(_args: TokenStream, input: TokenStream) -> TokenStream {
    // For now, simply pass through the input
    input
}

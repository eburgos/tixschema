# Core Model Macros

A Rust procedural macro library for generating TypeScript type definitions and Zod validation schemas from Rust structs and enums in Tixena applications.

## Overview

`core_model_macros` provides procedural macros that automatically generate TypeScript types and Zod schemas from your Rust data models. This ensures type safety and consistency between your Rust backend and TypeScript frontend without manual synchronization.

## Features

- **Automatic TypeScript Generation**: Creates TypeScript type definitions from Rust structs and enums
- **Zod v4 Schema Generation**: Generates modern runtime validation schemas using Zod v4 syntax
- **JSON Schema Support**: Generates JSON schemas for API documentation and validation (enabled by Zod v4 compatibility)
- **MongoDB ObjectId Support**: First-class support for MongoDB ObjectId types with proper serialization and validation
- **Serde Integration**: Respects Serde attributes for consistent naming and serialization
- **Type Mapping**: Handles complex types including:
  - Nested objects and references
  - Arrays and collections (`Vec<T>` → `Array<T>`)
  - Optional fields (`Option<T>` → `T | undefined`)
  - Maps (`HashMap<String, T>` → `Partial<Record<string, T>>`)
  - MongoDB ObjectId fields (`ObjectId` → `ObjectId` with JSON schema validation)
  - Primitive types (bool, String, numeric types)
  - Discriminated unions (tagged enums)
  - Complex nested structures (including deeply nested HashMaps)

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
core_model_macros = <path or crate_id or repo>  # eg: { git = "https://github.com/tixena/core_model_macros.git" }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

### Frontend Dependencies

**⚠️ Important: This crate requires Zod v4 for full functionality, especially JSON schema generation.**

Install Zod v4 in your TypeScript/JavaScript project:

```bash
npm install zod@^4.0.0
# or
yarn add zod@^4.0.0
```

**Note**: Zod v3 is not supported. The generated schemas use Zod v4 syntax (`.or(z.undefined())`) which is incompatible with earlier versions.

## Usage

### Basic Struct

```rust
use core_model_macros::model_schema;
use serde::{Deserialize, Serialize};

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UserJson {
    pub id: String,
    pub name: String,
    pub email: String,
    pub age: u32,
    pub is_active: bool,
}
```

This generates:
- `UserJson::json_schema()` - Returns a JSON schema
- `UserJson::ts_definition()` - Returns TypeScript type and Zod schema as a string

### Serde Attributes

The macros respect Serde attributes for field renaming:

```rust
#[model_schema()]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserProfileJson {
    pub user_id: String,
    pub first_name: String,
    pub last_name: String,
    #[serde(rename = "emailAddress")]
    pub email: String,
    pub created_at: String,
}
```

### Optional Fields

```rust
#[model_schema()]
#[derive(Serialize, Deserialize)]
pub struct UserWithOptionalsJson {
    pub id: String,
    pub name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,
}
```

### Collections and Maps

```rust
use std::collections::HashMap;

#[model_schema()]
#[derive(Serialize, Deserialize)]
pub struct UserWithCollectionsJson {
    pub id: String,
    pub tags: Vec<String>,
    pub scores: Vec<u32>,
    pub metadata: HashMap<String, String>,
    pub settings: Option<HashMap<String, String>>,
}
```

### Plain Enums

```rust
#[model_schema()]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UserStatusJson {
    Active,
    Inactive,
    Pending,
    Suspended,
}
```

### Discriminated Unions (Tagged Enums)

```rust
#[model_schema()]
#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum PaymentMethodJson {
    CreditCard {
        card_number: String,
        expiry_date: String,
        cvv: String,
    },
    BankTransfer {
        account_number: String,
        routing_number: String,
    },
    PayPal {
        email: String,
    },
}
```

### Nested Types

```rust
#[model_schema()]
#[derive(Serialize, Deserialize)]
pub struct AddressJson {
    pub street: String,
    pub city: String,
    pub zip_code: String,
}

#[model_schema()]
#[derive(Serialize, Deserialize)]
pub struct UserWithAddressJson {
    pub id: String,
    pub name: String,
    pub address: AddressJson,
    pub backup_addresses: Vec<AddressJson>,
}
```

### Field-Level Customization

Use `model_schema_prop` for field-specific overrides:

```rust
use core_model_macros::{model_schema, model_schema_prop};

#[model_schema()]
#[derive(Serialize, Deserialize)]
pub struct ApiConfigJson {
    pub id: String,
    #[model_schema_prop(as = String)]
    pub metric_type: String,
    pub enabled: bool,
}
```

### MongoDB ObjectId Support

The crate provides first-class support for MongoDB ObjectId types with proper serialization and validation:

```rust
use core_model_macros::model_schema;
use serde::{Deserialize, Serialize};
use mongodb::bson::oid::ObjectId;

#[model_schema()]
#[derive(Serialize, Deserialize)]
pub struct DocumentJson {
    pub id: ObjectId,
    pub title: String,
    pub author_id: ObjectId,
    pub tags: Vec<ObjectId>,
    pub metadata: HashMap<String, ObjectId>,
    pub parent_id: Option<ObjectId>,
    pub related_docs: HashMap<String, Vec<ObjectId>>,
}
```

**Generated TypeScript:**
```typescript
export type Document = {
  id: ObjectId;
  title: string;
  author_id: ObjectId;
  tags: Array<ObjectId>;
  metadata: Partial<Record<string, ObjectId>>;
  parent_id: ObjectId | undefined;
  related_docs: Partial<Record<string, Array<ObjectId>>>;
};

export const Document$Schema = z.strictObject({
  id: z.object({ $oid: z.string().regex(/^[a-f\d]{24}$/i, { message: "Invalid ObjectId" }) }),
  title: z.string(),
  author_id: z.object({ $oid: z.string().regex(/^[a-f\d]{24}$/i, { message: "Invalid ObjectId" }) }),
  tags: z.array(z.object({ $oid: z.string().regex(/^[a-f\d]{24}$/i, { message: "Invalid ObjectId" }) })),
  metadata: z.record(z.string(), z.object({ $oid: z.string().regex(/^[a-f\d]{24}$/i, { message: "Invalid ObjectId" }) })),
  parent_id: z.object({ $oid: z.string().regex(/^[a-f\d]{24}$/i, { message: "Invalid ObjectId" }) }).or(z.undefined()),
  related_docs: z.record(z.string(), z.array(z.object({ $oid: z.string().regex(/^[a-f\d]{24}$/i, { message: "Invalid ObjectId" }) }))),
});
```

**MongoDB JSON Serialization:**
```json
{
  "id": { "$oid": "507f1f77bcf86cd799439011" },
  "title": "My Document",
  "author_id": { "$oid": "507f1f77bcf86cd799439012" },
  "tags": [
    { "$oid": "507f1f77bcf86cd799439013" },
    { "$oid": "507f1f77bcf86cd799439014" }
  ],
  "metadata": {
    "template": { "$oid": "507f1f77bcf86cd799439015" }
  },
  "parent_id": { "$oid": "507f1f77bcf86cd799439016" },
  "related_docs": {
    "references": [
      { "$oid": "507f1f77bcf86cd799439017" },
      { "$oid": "507f1f77bcf86cd799439018" }
    ]
  }
}
```

**ObjectId Features:**
- **Proper MongoDB Serialization**: Uses `{ "$oid": "hex_string" }` format
- **Regex Validation**: Validates 24-character hexadecimal ObjectId format
- **JSON Schema Generation**: Generates correct MongoDB-compatible JSON schemas
- **Complex Nesting**: Supports ObjectIds in arrays, HashMaps, and optional fields
- **Production Safe**: MongoDB dependency is dev-only for testing, no production overhead

## Generating TypeScript Files

Create a utility function to generate TypeScript files with all your types:

```rust
// In your tests/mod.rs or similar
use std::fs;

pub enum MyEntities {}

impl MyEntities {
    pub fn get_entities() -> (String, Vec<String>) {
        (
            "Generated Types".to_string(),
            vec![
                UserJson::ts_definition(),
                UserStatusJson::ts_definition(),
                PaymentMethodJson::ts_definition(),
                AddressJson::ts_definition(),
                UserWithAddressJson::ts_definition(),
            ],
        )
    }
}

#[test]
fn test_generate_typescript() {
    generate_ts_schemas("../frontend/src/types/generated.ts").unwrap();
}

pub fn generate_ts_schemas(target_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut file_contents = String::from("import { z } from \"zod\";\n\n");

    let (header, type_definitions) = MyEntities::get_entities();
    
    file_contents.push_str(&format!(
        "/*\n * {}\n */\n\n",
        header
    ));
    
    file_contents.push_str(&type_definitions.join("\n\n"));
    file_contents.push('\n');

    fs::write(target_path, file_contents)?;
    println!("Generated TypeScript types at: {}", target_path);
    Ok(())
}
```

## Generated Output Example

For the `UserJson` struct above, the generated TypeScript would be:

```typescript
import { z } from "zod";

/**
 * UserJson
 * 
 * JSON Schema:
 * {
 *   "type": "object",
 *   "properties": {
 *     "id": { "type": "string" },
 *     "name": { "type": "string" },
 *     "email": { "type": "string" },
 *     "age": { "type": "integer" },
 *     "is_active": { "type": "boolean" }
 *   },
 *   "required": ["id", "name", "email", "age", "is_active"],
 *   "additionalProperties": false
 * }
 **/
export type User = {
  id: string;
  name: string;
  email: string;
  age: number;
  is_active: boolean;
};

export const User$Schema: z.Schema<User, z.ZodTypeDef, unknown> = z.strictObject({
  id: z.string(),
  name: z.string(),
  email: z.string(),
  age: z.number().int(),
  is_active: z.boolean(),
});
```

## Important Notes

1. **Naming Convention**: Use `Json` suffix for Rust types (e.g., `UserJson`). The generated TypeScript will strip this suffix (becomes `User`).

2. **Type References**: Nested types reference each other without the `Json` suffix in TypeScript.

3. **HashMap Handling**: `HashMap<String, T>` becomes `Partial<Record<string, T>>` in TypeScript.

4. **Array Types**: `Vec<T>` becomes `Array<T>` in TypeScript.

5. **Optional Fields**: `Option<T>` becomes `T | undefined` in TypeScript and `.or(z.undefined())` in Zod (v4 syntax).

6. **Supported Map Keys**: Currently only `HashMap<String, T>` is fully supported.

7. **MongoDB ObjectId**: `ObjectId` fields are supported with proper JSON schema validation and MongoDB-compatible serialization format `{ "$oid": "hex_string" }`.

8. **Complex Nesting**: The crate supports extremely complex nested structures including `HashMap<String, Vec<HashMap<String, ObjectId>>>` and similar deep nesting patterns.

## Zod v4 Migration & JSON Schema Generation

This library now generates **Zod v4 compatible schemas** using the modern `.or(z.undefined())` syntax for optional fields instead of the older `.optional()` + `.transform()` approach.

### Benefits of Zod v4 Support:

- **🚀 JSON Schema Generation**: Zod v4 can generate JSON schemas directly from the validation schemas
- **🧹 Cleaner Code**: No complex transform functions needed
- **⚡ Better Performance**: Eliminates runtime transform overhead
- **🎯 Type Safety**: Maintains the same `T | undefined` TypeScript semantics

### Optional Field Examples:

**Generated Zod v4 Schema (Current):**
```typescript
export const User$Schema = z.strictObject({
  id: z.string(),
  name: z.string(),
  email: z.string().or(z.undefined()),    // ✅ Modern v4 syntax
  age: z.number().int().or(z.undefined()), // ✅ Works with JSON schema generation
});
```

**Old Zod v3 Style (No longer generated):**
```typescript
// ❌ This format is no longer generated
export const User$Schema = z.strictObject({
  id: z.string(),
  name: z.string(),
  email: z.string().optional(),
  age: z.number().int().optional(),
}).transform(args => Object.assign(args, {
  email: args.email,
  age: args.age
}));
```

### Using with Zod v4 JSON Schema Generation:

```typescript
import { generateSchema } from '@zod-schema/json-schema';
import { User$Schema } from './types/generated';

// Generate JSON schema for API docs, OpenAPI, etc.
const jsonSchema = generateSchema(User$Schema);
console.log(jsonSchema);
```

## Testing

The crate includes comprehensive tests covering all features. Run them with:

```bash
cargo test
```

**Test Coverage (59+ tests across 9 specialized modules):**

- **Basic Types**: Struct generation, optional fields, primitive types
- **Collections**: Arrays, HashMaps, complex nested structures
- **Enums**: Plain enums, discriminated unions, tagged enums
- **Serde Integration**: All attribute combinations and naming conventions
- **Advanced Features**: Complex nested maps, edge cases, serialization consistency
- **MongoDB ObjectId**: 
  - Mock ObjectId implementation (10 tests)
  - Real MongoDB ObjectId integration (8 tests with actual `mongodb` crate)
  - Complex ObjectId nesting, arrays, HashMaps, optional fields
  - JSON schema validation and regex pattern matching
- **Zod v4 Compatibility**: Modern syntax generation, JSON schema output
- **Edge Cases**: Deeply nested structures, compilation safety, performance

**Production Safety**: MongoDB ObjectId tests use the real `mongodb` crate as a dev-dependency only, ensuring zero production overhead while providing complete compatibility validation.

## Integration with Frontend

1. Run your TypeScript generation test: `cargo test test_generate_typescript`
2. The generated file will include all your types and schemas
3. Import and use in your TypeScript/JavaScript code:

```typescript
import { User, User$Schema } from './types/generated';

// Runtime validation
const userData = User$Schema.parse(apiResponse);

// Type-safe usage
const user: User = {
  id: "123",
  name: "John Doe", 
  email: "john@example.com",
  age: 30,
  is_active: true
};
```

## Best Practices

1. **Consistent Naming**: Always use `Json` suffix for Rust types that will be serialized
2. **Validation**: Use generated Zod schemas for runtime validation
3. **Documentation**: Add doc comments to your Rust types - they'll appear in generated TypeScript
4. **Testing**: Include the TypeScript generation in your CI/CD pipeline
5. **Version Control**: Consider committing generated TypeScript files or generating them in build steps
6. **MongoDB ObjectId**: For MongoDB applications, use `mongodb::bson::oid::ObjectId` directly in your structs for proper serialization and validation
7. **Complex Nesting**: The crate handles deep nesting well, but consider flattening overly complex structures for better maintainability
8. **Production Dependencies**: The crate has zero production dependencies - MongoDB support is validated through dev-only testing

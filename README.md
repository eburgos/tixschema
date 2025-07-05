# Core Model Macros

A Rust procedural macro library for generating TypeScript type definitions and Zod validation schemas from Rust structs and enums in Tixena applications.

## Overview

`core_model_macros` provides procedural macros that automatically generate TypeScript types and Zod schemas from your Rust data models. This ensures type safety and consistency between your Rust backend and TypeScript frontend without manual synchronization.

## Features

- **Automatic TypeScript Generation**: Creates TypeScript type definitions from Rust structs and enums
- **Zod Schema Generation**: Generates runtime validation schemas for TypeScript
- **Serde Integration**: Respects Serde attributes for consistent naming and serialization
- **JSON Schema Support**: Generates JSON schemas for API documentation and validation
- **Type Mapping**: Handles complex types including:
  - Nested objects and references
  - Arrays and collections (`Vec<T>` → `Array<T>`)
  - Optional fields (`Option<T>` → `T | undefined`)
  - Maps (`HashMap<String, T>` → `Partial<Record<string, T>>`)
  - Primitive types (bool, String, numeric types)
  - Discriminated unions (tagged enums)

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
core_model_macros = <path or crate_id or repo>  # eg: { git = "https://github.com/tixena/core_model_macros.git" }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

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

5. **Optional Fields**: `Option<T>` becomes `T | undefined` in TypeScript and `.optional()` in Zod.

6. **Supported Map Keys**: Currently only `HashMap<String, T>` is fully supported.

## Testing

The crate includes comprehensive tests. Run them with:

```bash
cargo test
```

This will test:
- JSON schema generation
- TypeScript type generation
- Zod schema generation
- Serialization consistency
- Nested type handling
- All Serde attribute combinations

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

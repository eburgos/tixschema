# Core Model Macros Usage Rules

## Overview
This crate provides procedural macros for generating TypeScript types and Zod schemas from Rust structs and enums.

## Usage Rules

### 1. ALWAYS use `Json` suffix for Rust types
```rust
// ✅ CORRECT
#[model_schema()]
#[derive(Serialize, Deserialize)]
pub struct UserJson {
    pub id: String,
    pub name: String,
}

// ❌ WRONG - missing Json suffix
#[model_schema()]
#[derive(Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub name: String,
}
```

### 2. Required derives and imports
```rust
// ✅ REQUIRED imports and derives
use core_model_macros::model_schema;
use serde::{Deserialize, Serialize};

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MyTypeJson {
    // fields...
}
```

### 3. Serde attribute support
```rust
// ✅ Use serde attributes for field naming
#[model_schema()]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserProfileJson {
    pub user_id: String,          // becomes userId in TypeScript
    pub first_name: String,       // becomes firstName in TypeScript
    #[serde(rename = "email")]
    pub email_address: String,    // becomes email in TypeScript
}
```

### 4. HashMap limitations
```rust
// ✅ SUPPORTED - String keys only
pub struct ConfigJson {
    pub settings: HashMap<String, String>,
    pub metadata: HashMap<String, i32>,
}

// ❌ NOT SUPPORTED - Non-string keys
pub struct BadConfigJson {
    pub settings: HashMap<i32, String>,  // Will cause compilation error
}
```

### 5. Optional fields
```rust
// ✅ CORRECT optional field handling
#[model_schema()]
#[derive(Serialize, Deserialize)]
pub struct UserJson {
    pub id: String,
    pub name: String,
    pub email: Option<String>,                    // becomes email?: string
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone: Option<String>,                    // becomes phone?: string
}
```

### 6. Collections and arrays
```rust
// ✅ SUPPORTED collection types
#[model_schema()]
#[derive(Serialize, Deserialize)]
pub struct DataJson {
    pub tags: Vec<String>,                        // becomes Array<string>
    pub scores: Vec<u32>,                         // becomes Array<number>
    pub nested: Vec<OtherJson>,                   // becomes Array<Other>
}
```

### 7. Enums - Plain vs Tagged
```rust
// ✅ Plain enum (string union)
#[model_schema()]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StatusJson {
    Active,
    Inactive,
    Pending,
}

// ✅ Tagged enum (discriminated union)
#[model_schema()]
#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum PaymentJson {
    CreditCard { number: String, expiry: String },
    BankTransfer { account: String, routing: String },
    PayPal { email: String },
}
```

### 8. Nested types
```rust
// ✅ CORRECT nested type usage
#[model_schema()]
#[derive(Serialize, Deserialize)]
pub struct AddressJson {
    pub street: String,
    pub city: String,
}

#[model_schema()]
#[derive(Serialize, Deserialize)]
pub struct UserJson {
    pub id: String,
    pub address: AddressJson,                     // References Address in TypeScript
    pub addresses: Vec<AddressJson>,              // Array<Address>
}
```

### 9. Field-level customization
```rust
// ✅ Use model_schema_prop for field overrides
use core_model_macros::{model_schema, model_schema_prop};

#[model_schema()]
#[derive(Serialize, Deserialize)]
pub struct ApiConfigJson {
    pub id: String,
    #[model_schema_prop(as = String)]
    pub custom_field: String,
    pub enabled: bool,
}
```

## TypeScript Generation Pattern

### 1. Create entities enum
```rust
// ✅ Standard pattern for TypeScript generation
pub enum MyEntities {}

impl MyEntities {
    pub fn get_entities() -> (String, Vec<String>) {
        (
            "Generated Types".to_string(),
            vec![
                UserJson::ts_definition(),
                AddressJson::ts_definition(),
                StatusJson::ts_definition(),
                // Add all your types here
            ],
        )
    }
}
```

### 2. Create generation test
```rust
// ✅ Standard test for TypeScript generation
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
    Ok(())
}
```

## Common Mistakes to Avoid

### 1. ❌ Forgetting Json suffix
```rust
// Wrong - will not follow naming convention
#[model_schema()]
pub struct User { ... }
```

### 2. ❌ Missing required derives
```rust
// Wrong - missing Serialize, Deserialize
#[model_schema()]
#[derive(Debug)]
pub struct UserJson { ... }
```

### 3. ❌ Non-string HashMap keys
```rust
// Wrong - will cause compilation error
pub struct ConfigJson {
    pub settings: HashMap<i32, String>,
}
```

### 4. ❌ Forgetting to add types to entities
```rust
// Wrong - new types not added to get_entities()
impl MyEntities {
    pub fn get_entities() -> (String, Vec<String>) {
        (
            "Generated Types".to_string(),
            vec![
                UserJson::ts_definition(),
                // Missing: NewTypeJson::ts_definition(),
            ],
        )
    }
}
```

## Generated Output Understanding

1. **Type Name Transformation**: `UserJson` in Rust becomes `User` in TypeScript
2. **Field Names**: Respect serde rename attributes
3. **Optional Fields**: `Option<T>` becomes `T | undefined`
4. **Arrays**: `Vec<T>` becomes `Array<T>`
5. **Maps**: `HashMap<String, T>` becomes `Partial<Record<string, T>>`
6. **Nested Types**: Reference other types without Json suffix

## Testing Best Practices

1. **Always test TypeScript generation**: Include generation tests in your test suite
2. **Validate JSON schemas**: Test that generated schemas are valid
3. **Test serialization roundtrips**: Ensure serde compatibility
4. **Version control**: Consider committing generated TypeScript files
5. **CI/CD integration**: Run generation tests in your pipeline

## File Organization

```
my_project/
├── src/
│   ├── types/
│   │   ├── user.rs        # Contains UserJson, UserStatusJson, etc.
│   │   └── mod.rs
│   └── lib.rs
├── tests/
│   ├── generation.rs      # TypeScript generation tests
│   └── mod.rs
└── Cargo.toml
```

Remember: The goal is type safety and consistency between Rust and TypeScript codebases! 
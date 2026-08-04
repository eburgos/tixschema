# CLAUDE.md

**Note**: This project uses [bd (beads)](https://github.com/steveyegge/beads) for issue tracking. Use `bd` commands instead of markdown TODOs. See [AGENTS.md](AGENTS.md) for workflow details.

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**tixschema** is a Rust procedural macro library that generates TypeScript type definitions and Zod v4 validation schemas from Rust structs and enums. This ensures type safety and consistency between Rust backends and TypeScript frontends in Tixena applications.

## Task Management System

**MANDATORY: Use `bd` (beads) for ALL task tracking.**

- ✅ **DO**: Use `bd` commands for issue/task tracking (`bd create`, `bd ready`, `bd update`, `bd close`)
- ❌ **DO NOT**: Use TodoWrite tool, markdown TODO lists, or any other task tracking system
- 📖 **Reference**: See [AGENTS.md](AGENTS.md) for complete bd workflow and commands

**Quick bd commands:**
```bash
bd ready --json           # Check for available work
bd create "Task" -t task -p 2 --json
bd update <id> --status in_progress --json
bd close <id> --reason "Completed" --json
```

## Essential Commands

### Testing
```bash
# Quick test with default features (recommended for rapid iteration)
just quick
# or
cargo test

# Comprehensive test - all 32 feature combinations (run before commits)
just test

# Test specific feature combinations
just test-minimal          # No features
just test-default          # Default features only
just test-named-features   # Key combinations

# Run a specific test by name
just test-name <TEST_NAME>
# or
cargo test <TEST_NAME>
```

### Code Quality
```bash
# Check code (runs cargo check + clippy with warnings as errors)
just check

# Format code
just fmt

# Check all feature combinations without running tests
just check-all
```

### Build & Documentation
```bash
# Build the project
cargo build

# Build and open documentation
just docs-open

# Build docs only
just docs
```

### Full CI Pipeline (what CI runs)
```bash
just ci
# Runs: clean, check-all, test, fmt
```

## Code Architecture

### Macro Processing Flow

1. **Entry Point** ([lib.rs](src/lib.rs))
   - Exports two proc macros: `#[model_schema]` and `#[model_schema_prop]`
   - `model_schema` is the main macro for structs/enums/type aliases
   - `model_schema_prop` is for field-level customization

2. **Macro Execution** ([model_schema.rs](src/model_schema.rs))
   - `exec_model_schema()` routes to `process_struct()`, `process_enum()`, `process_type_alias()`, or `process_branded_newtype()`
   - Parses Serde attributes when `serde` feature enabled
   - Extracts example code from doc comments (` ```rust example` fences)
   - Generates methods: `ts_definition()`, optionally `json_schema()`, optionally `schema_example()`, and optionally `validate()`
   - `process_branded_newtype()`: handles `#[serde(transparent)]` single-field tuple structs, generating Zod brand schemas or `unique symbol` branded TypeScript types

3. **Type Analysis** ([field_type.rs](src/field_type.rs))
   - `FieldDef`: Core data structure representing a field's type, optionality, docs, etc.
   - `FieldDefType`: Enum categorizing Rust types (primitives, collections, custom types, ObjectId, etc.)
   - `get_field_def()`: Recursively analyzes Rust types and builds FieldDef trees
   - Handles: Option<T>, Vec<T>, HashMap<String, T>, nested types, generics

4. **Feature Modules** ([features/](src/features/))
   - `serde.rs`: Parses Serde attributes (`rename`, `rename_all`, `tag`, etc.)
   - `zod.rs`: Generates Zod v4 schema strings (`z.string()`, `z.union()`, etc.), embeds examples in `.meta()`
   - `jsonschema.rs`: Generates JSON schema objects
   - `object_id.rs`: MongoDB ObjectId type detection and schema generation
   - `model_schema_prop.rs`: Parses field-level customization attributes (`pattern`, `minLength`, `maxLength`, `minimum`, `maximum`, `literal`, `as`, `preprocess`)

5. **Code Generation** ([generation/](src/generation/))
   - `typescript.rs`: Generates TypeScript type definitions and Zod schemas
   - Combines FieldDef trees into complete type/schema strings
   - Handles discriminated unions, generics, nested types
   - Calls `schema_example()` method to embed examples in Zod `.meta()` when available

6. **Example Extraction** ([utils.rs](src/utils.rs))
   - `extract_example_from_docs()`: Parses ` ```rust example` code fences from doc comments
   - Returns first example found (if multiple exist)
   - Example code is inserted into generated `schema_example()` method
   - Compiler validates example code at compile time

### Key Data Structures

**FieldDef** (central type representation):
```rust
pub struct FieldDef {
    pub name: String,                    // Field name (respects Serde rename)
    pub docs: String,                    // Rust doc comments → JSDoc
    pub field_type: FieldDefType,        // The actual type category
    pub array_depth: u8,                 // Vec<T> → Array<T>, one level per Vec/slice/set written
    pub nullable_levels: Vec<u8>,        // Which array levels were written as Option
    pub array_lengths: Vec<(u8, usize)>, // Length of each level written as [T; N]
    pub model_schema_prop_meta: ...,     // Field-level overrides
}
```

Levels count from the innermost value outward, so `Vec<Option<T>>` records level 0 — a `null`
among the array's items, rendered `Array<T | null>` — while `Option<Vec<T>>` records level 1, a
`null` in place of the whole array, rendered `Array<T> | undefined` in a field and
`Array<T> | null` in a slot. `is_optional()` asks the question of the outermost level, the only
one whose rendering depends on where the field sits.

`array_lengths` numbers levels the same way: `[[T; 2]; 3]` records `(0, 2)` and `(1, 3)`. Only the
validating surfaces spend it — `minItems`/`maxItems` in the JSON schema, `.length(N)` in Zod —
while TypeScript stays `Array<T>`. A length the expansion cannot read (const generic, `const`
item, computed expression) records nothing and describes as an unbounded array.

**FieldDefType** (type categories):
- Primitives: `Boolean`, `String`, `U8-U64`, `I8-I64`, `F32`, `F64`, `Usize`, `Isize`
- Complex: `SiblingType` (custom types), `Map` (HashMap), `Tuple`
- Special: `ObjectId` (MongoDB, feature-gated), `StringLiteral`, `TypeParam`, `Unknown`

`TypeParam` is one of the enclosing item's own type parameters, which `erase_type_parameters`
rewrites a written name into. The three surfaces answer for it differently: TypeScript renders the
name it was written with, JSON Schema describes it as `{}`, and Zod composes the argument the
enclosing factory binds for it (`idType` for `IdType`). A surface with no factory to bind an
argument — an alias, a branded newtype — calls `with_opaque_type_parameters` first and writes
`z.unknown()`.

### Feature Flag System

The crate uses 5 optional features for minimal dependencies:

- `serde`: Enables Serde attribute parsing and field renaming
- `zod`: Enables Zod schema generation (v4 syntax)
- `jsonschema`: Enables `json_schema()` method generation
- `object_id`: Enables MongoDB ObjectId type support
- `typescript`: Enables TypeScript type generation

**Total combinations tested**: 2^5 = 32 (via `cargo-hack` in CI)

**Default configuration**: All features enabled

## Critical Development Rules

### 1. Type Naming Convention

The `Json` suffix is **optional** on Rust type names. If present, it is stripped from the generated TypeScript name. If absent, the Rust name is used as-is.

```rust
// Both are valid:
#[model_schema()]
pub struct User { ... }     // → TypeScript: User

#[model_schema()]
pub struct UserJson { ... } // → TypeScript: User (suffix stripped)
```

The codebase convention is to use type names WITHOUT the `Json` suffix.

### 2. Required Derives and Imports

```rust
use tixschema::model_schema;
use serde::{Deserialize, Serialize};

#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MyType { ... }
```

### 3. HashMap Key Restriction

**ONLY `HashMap<String, T>` is supported**. Non-string keys cause compilation errors.

```rust
// ✅ Supported
pub struct Config {
    pub settings: HashMap<String, String>,
}

// ❌ NOT supported - will fail
pub struct BadConfig {
    pub settings: HashMap<i32, String>,
}
```

### 4. Type Mappings (Rust → TypeScript)

- `String` → `string`
- `bool` → `boolean`
- Numeric types → `number`
- `Option<T>` → `T | undefined` (Zod: `z.union([T, z.undefined()]).prefault(undefined)`); with `#[model_schema_prop(ts_optional)]` the TypeScript key becomes optional instead: `field?: T`
- `Vec<T>` → `Array<T>`
- `HashMap<String, T>` → `Partial<Record<string, T>>`
- Custom types → Reference by name (Json suffix stripped if present)
- `ObjectId` → `ObjectId` (with $oid validation)
- `DateTime<Tz>` → `Date` (Zod `z.coerce.date()`); with `#[model_schema_prop(as_number)]` → `number` (inline epoch-ms coercer)
- `NaiveTime` → `string` (Zod `z.iso.time()` wrapped in an inline preprocessor that also accepts millis-since-start-of-day)
- `#[serde(flatten)]` field → TypeScript intersection (`A & B`), Zod `.and(...)`
- `#[serde(untagged)]` enum → TypeScript union (`A | B`), Zod `z.union([...])`, JSON Schema `anyOf`
- Type parameter → TypeScript parameter (`X<IdType>`), Zod `X$SchemaFactory(idType)` (memoized, one cache level per parameter), JSON Schema the document of whatever fills it — `default_types(...)` where the type stands alone, the reference site's arguments where a field embeds it. A lifetime is dropped on every surface, a borrowed value writing what its owned form writes; a const renders as an array length only, and is refused where it is handed to a written type as an argument

### 5. Zod v4 Requirements

**⚠️ IMPORTANT: This crate requires Zod v4 for full functionality.**

Frontend dependencies:
```bash
npm install zod@^4.0.0
```

Generated schemas use Zod v4's modern syntax:

```typescript
// ✅ Generated (Zod v4 compatible)
export const User$Schema = z.strictObject({
  id: z.string(),
  name: z.string(),
  email: z.union([z.string(), z.undefined()]).prefault(undefined),      // Modern v4 syntax
  age: z.union([z.number().int(), z.undefined()]).prefault(undefined),  // Works with JSON schema generation
});

// ❌ OLD FORMAT (no longer generated)
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

**Benefits of Zod v4**:
- **JSON Schema Generation**: Can generate JSON schemas from Zod schemas
- **Cleaner Code**: No transform functions needed
- **Better Performance**: No runtime transform overhead

### 6. Serde Attribute Support

The macro respects Serde attributes when the `serde` feature is enabled:

```rust
#[model_schema()]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserProfile {
    pub user_id: String,  // → userId in TypeScript
    #[serde(rename = "emailAddress")]
    pub email: String,    // → emailAddress in TypeScript
}
```

### 7. Field Validation Attributes (`#[model_schema_prop(...)]`)

All validation constraints generate checks in **Zod (frontend), JSON Schema, and Rust (Serde deserialization)**:

| Attribute | Field Type | Zod | JSON Schema | Rust serde |
|-----------|------------|-----|-------------|------------|
| `pattern = "regex"` | `String` | `.check(z.regex(/regex/))` | `"pattern"` | validator + deserializer |
| `minLength = N` | `String` | `.min(N)` | `"minLength"` | validator + deserializer |
| `maxLength = N` | `String` | `.max(N)` | `"maxLength"` | validator + deserializer |
| `minimum = N` | numeric | `.min(N)` | `"minimum"` | validator + deserializer |
| `maximum = N` | numeric | `.max(N)` | `"maximum"` | validator + deserializer |
| `literal = "val"` | `String` | `z.literal("val")` | `"const"` | — |
| `preprocess = ["fn"]` | any | `z.preprocess(fn, ...)` | — | — (Zod-only) |
| `ts_optional` | `Option<T>` | — | — | — (TypeScript-only) |
| `as_number` | `DateTime<Tz>` | inline `z.preprocess(..., z.number())` | — | — (TS+Zod) |

Multiple constraints on one field are combined. Multiple `preprocess` functions nest:
`z.preprocess(fn1, z.preprocess(fn2, innerSchema))`.

`ts_optional` is a bare flag (no value): it renders an `Option<T>` field as the optional TypeScript key `field?: T` instead of the default `field: T | undefined`. Zod and JSON Schema output are unchanged. It is only valid on `Option<T>` fields (non-`Option` is a compile error) and composes with `as = Type`.

`as_number` is a bare flag (no value): it renders a `DateTime<Tz>` field as a `number` (epoch milliseconds) with an inline self-contained Zod coercer, instead of the default native `Date` (`z.coerce.date()`). It is only valid on `DateTime<Tz>` fields (anything else is a compile error) and is honored on a tuple-variant `DateTime<Tz>` enum payload.

```rust
#[model_schema()]
#[derive(Serialize, Deserialize)]
pub struct User {
    #[model_schema_prop(minLength = 3, maxLength = 50, pattern = "^[a-z0-9_]+$")]
    pub username: String,

    #[model_schema_prop(minimum = 0, maximum = 120)]
    pub age: u32,

    #[model_schema_prop(preprocess = ["epoch_to_date"])]
    pub date_value: NaiveDate,
}
```

#### Generated `validate()` method

When any field has constraints, the macro generates a `validate(&self) -> Result<(), Vec<String>>` method that aggregates all per-field errors. This is useful when constructing instances in code rather than through serde (serde validates automatically):

```rust
let result = my_instance.validate();
match result {
    Ok(()) => println!("Valid"),
    Err(errors) => println!("Errors: {:?}", errors),
}
```

The macro also generates into the type's schema module:
- `validate_{field}_value(&FieldType) -> Result<(), String>` — pure static validator per field
- `deserialize_{field}(D) -> Result<FieldType, E>` — serde hook that calls the static validator

### 8. Branded Newtypes

Single-field tuple structs with `#[serde(transparent)]` are treated as branded/opaque types. If the Rust name has a `Json` suffix, it is stripped from the TypeScript name.

```rust
#[model_schema(default_types(ID_TYPE = String))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct UserId<ID_TYPE>(pub ID_TYPE);
```

With `zod` + `typescript` features:
```typescript
export type UserId<ID_TYPE> = ID_TYPE & z.$brand<"UserId">;
const UserId$RawSchema = z.string().brand<"UserId">();
export const UserId$Schema: ZodType<UserId<string>> = UserId$RawSchema;
```

With `typescript` only (no `zod`):
```typescript
declare const __brand_UserId: unique symbol;
export type UserId<ID_TYPE> = ID_TYPE & { readonly [__brand_UserId]: true };
```

Rules:
- Generic parameter names are preserved exactly (`ID_TYPE` stays `ID_TYPE` in TypeScript)
- Non-generic: `struct CorrelationId(pub String)` generates `string & z.$brand<"CorrelationId">`
- A brand publishes a `const`, so a parameter in its inner has no argument to name and renders as
  the opaque `z.unknown()`
- Serde transparent serialization works normally — the newtype is invisible in JSON

### Generic Types and Zod Factories

A Zod schema is a runtime value and TypeScript generics do not exist at runtime, so a generic
struct, tuple struct or enum has no one schema to publish. It exports `X$SchemaFactory` instead —
a function taking one required schema argument per parameter — while a type that binds no type
parameter keeps exporting `X$Schema` exactly as before. `zod_binding_suffix` is the one seam that
decides which, and `zod_published_binding` the one seam that writes either.

Beside the factory, a generic type also exports `X$SchemaDefault` — the factory called at the
type's own declared `default_types`, so a consumer who wants the ordinary filling shares the memo
rather than building a second schema for it. `zod_default_block` builds the const, through
`default_zod_argument`, the same `get_field_def` path every field renders through; where a
declared default names another generic item at exactly the arguments that item calls its own,
the rendered argument folds onto that item's `$SchemaDefault` instead of reconstructing a factory
call the memo would not share with it — see `record_zod_default_arguments`. `zod_binding_reexport`
covers both bindings wherever a renamed item re-exports its factory.

`X$SchemaDefault` is a module-scope `const`, and any argument `default_zod_argument` renders as a
reference to another item — the fold above, or an ordinary (non-folding) call to that item's own
`X$SchemaFactory` — names one more module-scope `const`. One macro invocation sees one type, so
nothing here can know whether that `const` is written above this one in the generated module or
below it; `default_zod_argument` defers every such reference through `deferred_zod_operand`, the
same `z.lazy(() => …)` wrap `.and(...)` already relies on for a flattened base. A self-contained
expression like `z.string()` names no sibling `const` and is left eager. The deferral is also what
ends a cycle between two declared defaults — `CycleLeader`'s naming `CycleFollower` and
`CycleFollower`'s naming `CycleLeader` back: neither can have registered the other's arguments yet
when it expands, so neither side folds, and both calls read the other's factory behind `z.lazy`
rather than at the top of a `const` initializer — the module loads regardless of which of the two
names the consuming project's entity list writes first.

Each factory memoizes on the identity of its arguments, one cache level per parameter, so two
calls with the same argument objects return the identical schema and no two argument lists
collide. The levels are written out to the exact depth the type declares rather than looped over:
a loop needs a key type it cannot name and a value it cannot type, which means `unknown` and a
cast at every level, while the written-out form comes back already typed.

Every parameter is a real TypeScript type parameter (`<IdType extends ZodType>`), never a bare
`ZodType` annotation — `ZodType` defaults its own parameters, so an argument annotated with it
infers every field as `unknown`.

`typescript_preamble!()` returns the one helper the factories share, `createSchemaCache`, which
every generated module carries once above its per-type definitions. It holds the only assertion
anywhere in the output: a cache's value type depends on its key type, which TypeScript can declare
but cannot construct.

A generic type may reach itself, directly or through a second type reaching back. JSON Schema
hoists it into `$defs` once and points a `$ref` at that definition; TypeScript writes the name
inside itself with its arguments; Zod calls the factory again with the argument the outer call was
handed, the memo cache being what ends the recursion — the schema is cached before the recursive
call is made. Which of the item's two bindings a self-reference names is read off the store
`record_zod_factory` writes, and that is written at `exec_model_schema` ahead of every shape rather
than where the binding is finally spelled: the fields are rendered before then, so an answer stored
on the item's own registry entry would be read before the item had put one there.

`write_field_type_and_schema` defers a member whose type reaches the item being defined, and one
that reaches a type declared *below* it — `reaches_a_type_declared_later`. The second is what ends
a cycle spanning two types: every cycle contains at least one reference pointing forward, since
declaration positions cannot strictly decrease all the way round one, and what is left once those
are deferred cannot cycle. The deferral is a getter rather than `z.lazy`, which at an operand
position collapses the factory's inferred return type to `any`.

### Declaring a Default Type per Parameter

JSON Schema has no type parameters, so a generic item's document is built from one concrete
filling, and `default_types(IdType = String, DateType = f64)` is where an author names it — one
`Parameter = Type` pair per type parameter, in any order, beside every other item-level argument.

`default_types_guard_errors` reads the declaration against the item's own parameters at
`exec_model_schema`, the one seam every expanded shape is dispatched from, so a struct, a tuple
struct, a branded newtype, an enum and an alias are all answered the same way and before any of
them is split off. It refuses in both directions:

- an entry naming nothing the item declares — refused in **every** feature configuration, spanned
  on the entry, since a misspelled parameter fills nothing while the one it was meant for keeps no
  default;
- a type parameter with no entry — refused only under `#[cfg(feature = "jsonschema")]`, spanned on
  the parameter, because nothing else reads the default;
- `default_types` on an item with no type parameter, and an empty `default_types()`, both being a
  declaration with nothing to declare;
- an entry filled at a type the field-definition dispatch has no arm for — refused only under
  `#[cfg(feature = "jsonschema")]`, spanned on the filling. That dispatch takes a name it does not
  recognise for another `#[model_schema]` item, which is right for a sibling declared below and
  gibberish for `char`: the emission names a `char_schema` module nothing publishes. Tokens cannot
  separate the two, so `is_undescribable_primitive` refuses only what provably cannot become a
  sibling — the primitive names the language reserves that the dispatch answers for nowhere
  (`char`, `i128`, `u128`, `f16`, `f128`).

A const parameter earns its own refusal, from `const_parameter_argument_errors` at the same seam
and gated on `typescript`/`jsonschema`: a const handed to a written type as an *argument* stands
where a type is read, so the JSON side would call into a module named after it and the TypeScript
side would write a name its declaration binds nothing for. A const written as an array length is
untouched — that is the one position it renders in, as an unbounded array.

There is no fallback filling. A guessed one produces a document that silently rejects valid
payloads, which is what the declaration exists to prevent.

### String Constraints on a Generic Brand's Declared Default

A branded newtype's own `pattern`/`minLength`/`maxLength` normally have no inner to measure when
the inner is one of the brand's own bare type parameters — a parameter is the opaque value on
every surface, and `z.unknown()` carries no `.min`/`.max`. `branded_constraint_inner_error` reads
the declared default for that parameter instead of refusing outright: `declared_default_field`
resolves the same `default_types` entry `zod_default_block` reads, and `non_string_inner_shape`
asks of it the same question asked of a concrete argument — string-shaped, the checks compose onto
the default; not string-shaped, the refusal names the default rather than the parameter, through
`declared_default_constraint_message`. An entry the declaration left out falls back to `String`,
the same fallback `schema_example_value_type` uses for the identical gap, so the guard alone
requires nothing; only `jsonschema`'s own separate requirement (above) forces every parameter to
declare one.

The checks never reach the factory's own parameter — `branded_zod_inner` renders a bare-parameter
inner with no check appended regardless of genericness, since a caller filling the parameter with
something other than the default (an `ObjectId` schema, say) must not inherit bounds meant for it.
They land once, on `$SchemaDefault`'s argument for that one parameter: `zod_default_block` and
`zod_factory_block` both take a `ZodDefaultInputs` bundling `default_types` with the optional
`(parameter, checks)` pair a branded newtype's own expansion supplies — `None` for every ordinary
generic struct, tuple struct, enum and alias, which carry no type-level string constraint of their
own. The JSON surface needs no separate change: a bare parameter's document is already the runtime
argument bound to the parameter's declared filling (`json_argument_value`, defaulting through
`declared_filling_json_schema_value`), which `branded_layered_over` narrows with the brand's own
bounds through the same `allOf` a named inner is narrowed through — never the inert `{}` a
parameter with no default at all would describe as.

### Adding Examples to Types

To add examples to your types for inclusion in Zod schemas:

```rust
/// User profile description
/// ```rust example
/// User {
///     name: "John Doe".to_string(),
///     email: "john@example.com".to_string(),
///     age: 25,
/// }
/// ```
#[model_schema()]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct User {
    pub name: String,
    pub email: String,
    pub age: u32,
}
```

**Key Points:**
- Use the exact syntax: ` ```rust example` (note the space and `example` keyword)
- The example code must type-check at compile time
- The last expression in the block should evaluate to the type
- Examples are **optional** - if not provided, no `schema_example()` method is generated
- If multiple examples exist, only the first one is used
- Examples respect Serde attributes (field names are serialized correctly)

**Generated Code:**
- A `schema_example()` method is generated that returns `serde_json::Value`
- The Zod schema includes `.meta({ example: <serialized_json> })`
- The example is serialized using Serde, so it matches your API's JSON format

## Common Development Tasks

### Adding a New Type Mapping

1. Add variant to `FieldDefType` in [src/field_type.rs](src/field_type.rs)
2. Update `get_field_def()` to detect the new type
3. Add TypeScript generation logic in [src/generation/typescript.rs](src/generation/typescript.rs)
4. Add Zod generation logic in [src/features/zod.rs](src/features/zod.rs) (if `zod` feature)
5. Add tests in appropriate test file (e.g., `tests/primitive_types_tests.rs`)

### Adding a New Feature

1. Add feature to `Cargo.toml` `[features]` section
2. Create feature module in [src/features/](src/features/)
3. Add `#[cfg(feature = "...")]` guards in affected code
4. Update `Features` struct in [src/features/mod.rs](src/features/mod.rs)
5. Add tests that verify behavior with/without the feature

### Adding Tests

Tests are organized by category in [tests/](tests/):

- `basic_tests.rs`: Simple structs, basic types
- `collection_tests.rs`: Vec, HashMap, nested collections
- `enum_tests.rs`: Plain enums, discriminated unions
- `serde_tests.rs`: Serde attribute handling
- `mongodb_tests.rs` / `mongodb_real_tests.rs`: ObjectId support
- `edge_cases_tests.rs`: Complex scenarios, deep nesting
- `semantic_types_tests.rs`: Type aliases

**Test pattern**:
```rust
#[test]
fn test_my_feature() {
    #[model_schema()]
    #[derive(Serialize, Deserialize)]
    pub struct Test { ... }

    let ts = Test::ts_definition();
    assert!(ts.contains("expected output"));
}
```

### Running Single Tests During Development

```bash
# Run a specific test
cargo test test_basic_struct

# Run all tests in a module with output
cargo test basic_tests -- --nocapture

# Run tests matching a pattern
cargo test objectid
```

## TypeScript Generation Pattern

Standard pattern for generating TypeScript files:

```rust
// Define entities enum
pub enum MyEntities {}

impl MyEntities {
    pub fn get_entities() -> (String, Vec<String>) {
        (
            "Generated Types".to_string(),
            vec![
                User::ts_definition(),
                Address::ts_definition(),
                // Add all types here
            ],
        )
    }
}

// Generation function
pub fn generate_ts_schemas(target_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut file_contents = String::from("import { z } from \"zod\";\n\n");
    let (header, type_definitions) = MyEntities::get_entities();

    file_contents.push_str(&format!("/*\n * {}\n */\n\n", header));
    file_contents.push_str(&type_definitions.join("\n\n"));
    file_contents.push('\n');

    fs::write(target_path, file_contents)?;
    Ok(())
}

// Test that runs generation
#[test]
fn test_generate_typescript() {
    generate_ts_schemas("../frontend/src/types/generated.ts").unwrap();
}
```

## Generated Output Understanding

The macro transforms Rust types to TypeScript following these rules:

1. **Type Name Transformation**: If the Rust type name ends with `Json`, the suffix is stripped (e.g., `UserJson` → `User`). Otherwise, the name is used as-is (e.g., `User` → `User`).
2. **Field Names**: Respect serde rename attributes (`#[serde(rename = "...")]`, `#[serde(rename_all = "...")]`)
3. **Optional Fields**: `Option<T>` becomes `T | undefined` in TypeScript and `z.union([type, z.undefined()]).prefault(undefined)` in Zod
4. **Arrays**: `Vec<T>` becomes `Array<T>` in TypeScript
5. **Maps**: `HashMap<String, T>` becomes `Partial<Record<string, T>>` in TypeScript
6. **Nested Types**: Reference other types by name (Json suffix stripped if present)
7. **MongoDB ObjectId**: `ObjectId` becomes `ObjectId` in TypeScript with proper JSON schema validation
8. **ObjectId Serialization**: Uses MongoDB format `{ "$oid": "hex_string" }`
9. **ObjectId Validation**: Includes regex validation for 24-character hexadecimal strings

## Testing Best Practices

### 1. Always Test TypeScript Generation

Include generation tests in your test suite to catch issues early:

```rust
#[test]
fn test_generate_typescript() {
    generate_ts_schemas("../frontend/src/types/generated.ts").unwrap();
}
```

### 2. Validate JSON Schemas

When using the `jsonschema` feature, test that generated schemas are valid:

```rust
#[cfg(feature = "jsonschema")]
#[test]
fn test_json_schema_validity() {
    let schema = MyType::json_schema();
    assert!(schema.get("type").is_some());
}
```

### 3. Test Serialization Roundtrips

Ensure serde serialization/deserialization works correctly:

```rust
#[test]
fn test_serde_roundtrip() {
    let original = MyType { /* ... */ };
    let json = serde_json::to_string(&original).unwrap();
    let deserialized: MyType = serde_json::from_str(&json).unwrap();
    assert_eq!(original, deserialized);
}
```

### 4. Version Control Generated Files

Consider committing generated TypeScript files to version control for easier code review and frontend development without requiring Rust builds.

### 5. CI/CD Integration

Run generation tests in your CI pipeline to ensure frontend types stay in sync with backend changes (see [CI/CD Integration](#cicd-integration) section).

### 6. MongoDB ObjectId Testing

The crate includes comprehensive ObjectId tests with real MongoDB library integration (dev-only dependency). Test various ObjectId scenarios:

```rust
#[cfg(all(feature = "object_id", test))]
#[test]
fn test_objectid_types() {
    #[model_schema()]
    #[derive(Serialize, Deserialize)]
    pub struct Test {
        pub id: ObjectId,
        pub tags: Vec<ObjectId>,
        pub metadata: HashMap<String, ObjectId>,
    }

    let ts = Test::ts_definition();
    assert!(ts.contains("ObjectId"));
}
```

### 7. Complex Structure Testing

Test deeply nested structures and edge cases to ensure macro robustness:

```rust
#[test]
fn test_complex_nested() {
    #[model_schema()]
    #[derive(Serialize, Deserialize)]
    pub struct Complex {
        pub nested: Vec<HashMap<String, Vec<Other>>>,
    }

    let ts = Complex::ts_definition();
    // Verify correct nesting
}
```

### 8. Production Safety

- MongoDB dependency is dev-only, ensuring zero production overhead
- Feature flags allow minimal dependency footprint
- All type analysis happens at compile time (no runtime cost)

## CI/CD Integration

The CI pipeline ([.github/workflows/ci.yml](.github/workflows/ci.yml)) runs:

1. `cargo build --verbose`
2. `just check` (cargo check + clippy)
3. `cargo test --verbose` (basic tests)
4. `just test` (all 32 feature combinations via cargo-hack)
5. Discord notification with build status

**Before pushing**, run `just ci` locally to replicate the CI pipeline.

## MongoDB ObjectId Support

The `object_id` feature provides comprehensive MongoDB ObjectId type support with proper validation and serialization.

### Basic Usage

```rust
use mongodb::bson::oid::ObjectId;

#[model_schema()]
#[derive(Serialize, Deserialize)]
pub struct Document {
    pub id: ObjectId,                           // Scalar ObjectId
    pub author_id: ObjectId,
    pub tags: Vec<ObjectId>,                    // Array of ObjectIds
    pub metadata: HashMap<String, ObjectId>,    // HashMap with ObjectId values
    pub parent_id: Option<ObjectId>,            // Optional ObjectId
    pub related: HashMap<String, Vec<ObjectId>>, // Nested structures
}
```

### Generated TypeScript

```typescript
export type Document = {
  id: ObjectId;
  author_id: ObjectId;
  tags: Array<ObjectId>;
  metadata: Partial<Record<string, ObjectId>>;
  parent_id: ObjectId | undefined;
  related: Partial<Record<string, Array<ObjectId>>>;
};
```

### Generated Zod Schema

```typescript
export const Document$Schema = z.strictObject({
  id: z.object({ $oid: z.string().regex(/^[a-f0-9]{24}$/, { message: "Invalid ObjectId" }) }),
  author_id: z.object({ $oid: z.string().regex(/^[a-f0-9]{24}$/, { message: "Invalid ObjectId" }) }),
  tags: z.array(z.object({ $oid: z.string().regex(/^[a-f0-9]{24}$/, { message: "Invalid ObjectId" }) })),
  metadata: z.record(z.string(), z.object({ $oid: z.string().regex(/^[a-f0-9]{24}$/, { message: "Invalid ObjectId" }) })),
  parent_id: z.union([z.object({ $oid: z.string().regex(/^[a-f0-9]{24}$/, { message: "Invalid ObjectId" }) }), z.undefined()]).prefault(undefined),
  related: z.record(z.string(), z.array(z.object({ $oid: z.string().regex(/^[a-f0-9]{24}$/, { message: "Invalid ObjectId" }) }))),
});
```

### JSON Serialization Format

ObjectIds serialize to MongoDB's standard JSON format:

```json
{
  "id": { "$oid": "507f1f77bcf86cd799439011" },
  "author_id": { "$oid": "507f1f77bcf86cd799439012" },
  "tags": [
    { "$oid": "507f1f77bcf86cd799439013" },
    { "$oid": "507f1f77bcf86cd799439014" }
  ],
  "metadata": {
    "template": { "$oid": "507f1f77bcf86cd799439015" }
  },
  "parent_id": { "$oid": "507f1f77bcf86cd799439016" },
  "related": {
    "references": [
      { "$oid": "507f1f77bcf86cd799439017" },
      { "$oid": "507f1f77bcf86cd799439018" }
    ]
  }
}
```

### Key Features

- **Validation**: Regex validation ensures 24-character hexadecimal strings
- **Type Safety**: Full TypeScript type checking for ObjectId fields
- **MongoDB Compatibility**: Matches MongoDB's native JSON format (`{ "$oid": "..." }`)
- **Zero Production Overhead**: MongoDB crate is a dev-dependency only
- **Comprehensive Testing**: Tested with real MongoDB library integration

## File Organization

```
tixschema/
├── src/
│   ├── lib.rs                    # Entry point, macro exports
│   ├── model_schema.rs           # Main macro logic
│   ├── field_type.rs             # Type system (FieldDef, FieldDefType)
│   ├── utils.rs                  # Helpers (naming, docs, serde parsing)
│   ├── features/
│   │   ├── mod.rs                # Feature detection
│   │   ├── serde.rs              # Serde attribute parsing
│   │   ├── zod.rs                # Zod schema generation
│   │   ├── jsonschema.rs         # JSON schema generation
│   │   ├── object_id.rs          # ObjectId support
│   │   └── model_schema_prop.rs  # Field attribute parsing
│   └── generation/
│       ├── mod.rs
│       └── typescript.rs         # TypeScript/Zod code generation
├── tests/
│   ├── basic_tests.rs
│   ├── collection_tests.rs
│   ├── enum_tests.rs
│   ├── serde_tests.rs
│   ├── mongodb_tests.rs
│   ├── mongodb_real_tests.rs
│   ├── edge_cases_tests.rs
│   ├── semantic_types_tests.rs
│   └── ...
├── justfile                      # Task runner (commands)
├── Cargo.toml                    # Dependencies, features
├── README.md                     # User documentation
├── CURSORRULES.md                # Detailed usage rules
└── .cursorrules                  # Original cursor rules
```

## Common Pitfalls

1. **Type name ambiguity** → Ensure Rust type names and TypeScript output names don't collide with built-in types
2. **Missing required derives** → Compilation errors
3. **Non-string HashMap keys** → Compilation errors
4. **Forgetting to add types to entities enum** → Types not included in generated output
5. **Using Zod v3** → Generated schemas use v4 syntax and won't work
6. **Testing without feature combinations** → May break in different feature configurations

## Debugging Tips

```rust
// Print generated TypeScript during tests
let ts = MyType::ts_definition();
println!("Generated:\n{}", ts);

// Print JSON schema
#[cfg(feature = "jsonschema")]
println!("{}", serde_json::to_string_pretty(&MyType::json_schema())?);

// Check which features are enabled
#[cfg(test)]
use crate::features::Features;
println!("Enabled: {:?}", Features::enabled_features());
```

## Performance Considerations

- Macro expansion happens at compile time (zero runtime overhead)
- Generated methods return `String` (consider caching if called frequently)
- Feature flags reduce dependencies and compilation time when not needed
- MongoDB ObjectId validation regex is compiled once by Zod

## Related Documentation

- [README.md](README.md) - User guide, examples, troubleshooting
- [CURSORRULES.md](CURSORRULES.md) - Detailed usage rules and patterns
- [justfile](justfile) - All available commands with descriptions

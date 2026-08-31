# Contributing to TixSchema

## Getting Started

Clone the repository and ensure you have Rust installed.

## Task Management

This project uses [bd (beads)](https://github.com/steveyegge/beads) for issue tracking. See [AGENTS.md](AGENTS.md) for the complete workflow.

## Development Commands

### Testing

```bash
# Quick test with default features
just quick
# or
cargo test

# Comprehensive test - all feature combinations (run before commits)
just test

# Test specific feature combinations
just test-minimal
just test-default
just test-named-features

# Run a specific test
cargo test <TEST_NAME>
```

### Code Quality

```bash
# Check code (cargo check + clippy with warnings as errors)
just check

# Format code
just fmt

# Check all feature combinations
just check-all
```

### Type-Checking the Emitted TypeScript

The `service_schema` tests that compile an emitted bundle need a TypeScript compiler, which a
fresh clone does not have. They look for `tsc` on `PATH` (or wherever `TIXSCHEMA_TSC` names) and
stand down when they find none, saying so on stderr — so `just quick`, `just test` and `just all`
never require one, and a run that did not type-check anything says so rather than passing quietly.

```bash
# Refuses to stand down: fails if no compiler is reachable.
just typecheck-ts
```

### Full CI Pipeline

```bash
just ci
# Runs: clean, check-all, test, fmt
```

## Test Organization

Tests are organized by category in `tests/`:

- `basic_tests.rs` — simple structs, basic types
- `primitive_types_tests.rs` — all numeric types, bool, String
- `collection_tests.rs` — Vec, HashMap, nested collections
- `enum_tests.rs` — plain enums, discriminated unions
- `tuple_variant_tests.rs` — single/multi-element tuples, mixed variants
- `recursive_type_tests.rs` — self-referential structs and enums
- `semantic_types_tests.rs` — type aliases
- `serde_tests.rs` — Serde attribute handling
- `model_schema_prop_tests.rs` — field-level customization
- `pattern_preprocess_tests.rs` — pattern/preprocess validation
- `validation_tests.rs` — validate() method generation
- `branded_newtype_tests.rs` — transparent tuple structs
- `example_tests.rs` — compiler-validated examples
- `chrono_tests.rs` — date/time types
- `mongodb_tests.rs`, `mongodb_real_tests.rs` — ObjectId support
- `jsonschema_tests.rs` — JSON Schema generation
- `zod_tests.rs` — Zod v4 schema generation
- `typescript_feature_tests.rs` — feature-gated TypeScript
- `advanced_tests.rs`, `edge_cases_tests.rs` — complex scenarios

### Test Pattern

```rust
#[test]
fn test_my_feature() {
    #[model_schema()]
    #[derive(Serialize, Deserialize)]
    pub struct MyType { ... }

    let ts = MyType::ts_definition();
    assert!(ts.contains("expected output"));
}
```

## Best Practices

- Run `just ci` before pushing to replicate the CI pipeline
- Test TypeScript generation in your test suite
- Consider committing generated TypeScript files for easier code review
- Pin Zod v4 in frontend dependencies
- MongoDB dependency is dev-only — zero production overhead

## Architecture

See [CLAUDE.md](CLAUDE.md) for detailed architecture documentation.

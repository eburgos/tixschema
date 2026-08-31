# Justfile for tixschema
# Run `just --list` to see all available commands

# Default recipe - runs comprehensive tests
default: test

# Install required tools
install-tools:
    @echo "Installing required tools..."
    cargo install cargo-hack || echo "cargo-hack already installed"
    cargo install just || echo "just already installed"

# Test all possible feature combinations (2^5 = 32 combinations)
test:
    @echo "Testing all feature combinations..."
    @echo "This will test 32 different feature combinations (2^5 with 5 features)"
    cargo hack test --feature-powerset
    @echo "✅ All feature combinations passed!"

# Test all combinations with verbose output
test-verbose:
    @echo "Testing all feature combinations (verbose)..."
    cargo hack test --feature-powerset --verbose
    @echo "✅ All feature combinations passed!"

# Test specific feature combinations manually
test-named-features:
    @echo "Testing key feature combinations..."
    cargo test --no-default-features
    cargo test --no-default-features --features "zod"
    cargo test --no-default-features --features "typescript"
    cargo test --no-default-features --features "typescript,zod"
    cargo test --no-default-features --features "serde,zod"
    cargo test --no-default-features --features "serde,zod,object_id"
    cargo test --features "serde,zod,jsonschema,object_id,typescript"
    @echo "✅ Key feature combinations passed!"

# Test with default features
test-default:
    @echo "Testing with default features..."
    cargo test
    @echo "✅ Default features test passed!"

# Test with no features (minimal build)
test-minimal:
    @echo "Testing with no features..."
    cargo test --no-default-features
    @echo "✅ Minimal test passed!"

# Test specific feature combinations individually
test-combinations:
    @echo "Testing individual feature combinations..."
    cargo test --no-default-features --features "serde"
    cargo test --no-default-features --features "zod"
    cargo test --no-default-features --features "jsonschema"
    cargo test --no-default-features --features "object_id"
    cargo test --no-default-features --features "typescript"
    cargo test --no-default-features --features "serde,zod"
    cargo test --no-default-features --features "serde,typescript"
    cargo test --no-default-features --features "zod,typescript"
    cargo test --no-default-features --features "serde,zod,typescript"
    @echo "✅ Individual combinations passed!"

# Quick test - just run default tests
quick:
    @echo "Quick test with default features..."
    cargo test

# Lint Rust (clippy + fmt check). Lint levels are configured in Cargo.toml [lints.clippy].
lint:
    cargo clippy --all-targets -- -D warnings
    cargo fmt --check

# Lint every feature combination (clippy over the full feature powerset). Fails on any
# warning in any toggle, including feature-gated test code that `lint` (default features) misses.
lint-all:
    @echo "Linting all feature combinations..."
    cargo hack clippy --feature-powerset --all-targets -- -D warnings
    @echo "✅ All feature combinations lint passed!"

# Type-check the emitted TypeScript bundle with a real compiler, in the build that publishes the
# client and the dispatcher and in the one that publishes neither.
#
# Deliberately outside `all` and `ci`: a fresh clone has no TypeScript compiler, and the type-check
# tests inside `cargo test` stand down when they find none, saying so on stderr. This recipe is the
# one that refuses to stand down — it resolves the compiler up front and names it for the tests,
# where a named compiler that cannot be started is a failure rather than a stand-down. Set
# TIXSCHEMA_TSC to use a compiler that is not on PATH.
typecheck-ts:
    @command -v "${TIXSCHEMA_TSC:-tsc}" >/dev/null 2>&1 || { echo "No TypeScript compiler: put \`tsc\` on PATH, or set TIXSCHEMA_TSC to one." >&2; exit 1; }
    @echo "Type-checking the emitted bundle with $(command -v "${TIXSCHEMA_TSC:-tsc}")..."
    TIXSCHEMA_TSC="$(command -v "${TIXSCHEMA_TSC:-tsc}")" cargo test --test service_schema_typescript_tests type_check
    TIXSCHEMA_TSC="$(command -v "${TIXSCHEMA_TSC:-tsc}")" cargo test --no-default-features --features "serde,typescript" --test service_schema_typescript_tests type_check
    @echo "✅ The emitted bundle type-checks!"

# Check code without running tests
check:
    @echo "Checking code..."
    cargo check
    just lint
    @echo "✅ Code check passed!"

# Check all feature combinations without running tests
check-all:
    @echo "Checking all feature combinations..."
    cargo hack check --feature-powerset
    @echo "✅ All feature combinations check passed!"

# Format code
fmt:
    @echo "Formatting code..."
    cargo fmt
    @echo "✅ Code formatted!"

# Clean build artifacts
clean:
    @echo "Cleaning build artifacts..."
    cargo clean
    @echo "✅ Build artifacts cleaned!"

# Full pipeline (standardized `all` entry point across tixena repos).
all: lint lint-all test
    @echo "All checks completed successfully!"

# Full CI pipeline - what CI would run
ci: clean check-all lint-all test fmt
    @echo "Full CI pipeline completed successfully!"

# Build documentation
docs:
    @echo "Building documentation..."
    cargo doc --no-deps --all-features
    @echo "✅ Documentation built!"

# Open documentation in browser
docs-open: docs
    @echo "Opening documentation..."
    cargo doc --no-deps --all-features --open

# Run specific tests by name
test-name TEST_NAME:
    @echo "Running specific test: {{TEST_NAME}}"
    cargo test {{TEST_NAME}}

# Generate code coverage report (requires cargo-llvm-cov)
test-coverage:
    @echo "Generating code coverage report..."
    cargo llvm-cov --workspace --all-features

# Generate code coverage HTML report (requires cargo-llvm-cov)
test-coverage-html:
    @echo "Generating code coverage HTML report..."
    cargo llvm-cov --workspace --all-features --html
    @echo "Coverage report generated in target/llvm-cov/html/"

# Benchmark tests
bench:
    @echo "Running benchmarks..."
    cargo bench
    @echo "✅ Benchmarks completed!"

# List all available commands
help:
    @just --list

# Run all tests in different modes for comprehensive validation
test-comprehensive: test-minimal test-default test-named-features test
    @echo "Comprehensive testing completed!"
    @echo "✅ All tests passed in all modes!" 
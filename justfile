# Justfile for core_model_macros
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
    @echo "🧪 Testing all feature combinations..."
    @echo "This will test 32 different feature combinations (2^5 with 5 features)"
    cargo hack test --feature-powerset
    @echo "✅ All feature combinations passed!"

# Test all combinations with verbose output
test-verbose:
    @echo "🧪 Testing all feature combinations (verbose)..."
    cargo hack test --feature-powerset --verbose
    @echo "✅ All feature combinations passed!"

# Test specific feature combinations manually
test-named-features:
    @echo "🧪 Testing key feature combinations..."
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
    @echo "🧪 Testing with default features..."
    cargo test
    @echo "✅ Default features test passed!"

# Test with no features (minimal build)
test-minimal:
    @echo "🧪 Testing with no features..."
    cargo test --no-default-features
    @echo "✅ Minimal test passed!"

# Test specific feature combinations individually
test-combinations:
    @echo "🧪 Testing individual feature combinations..."
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
    @echo "🏃 Quick test with default features..."
    cargo test

# Check code without running tests
check:
    @echo "🔍 Checking code..."
    cargo check
    cargo clippy -- -D warnings
    @echo "✅ Code check passed!"

# Check all feature combinations without running tests
check-all:
    @echo "🔍 Checking all feature combinations..."
    cargo hack check --feature-powerset
    @echo "✅ All feature combinations check passed!"

# Format code
fmt:
    @echo "🎨 Formatting code..."
    cargo fmt
    @echo "✅ Code formatted!"

# Clean build artifacts
clean:
    @echo "🧹 Cleaning build artifacts..."
    cargo clean
    @echo "✅ Build artifacts cleaned!"

# Full CI pipeline - what CI would run
ci: clean check-all test fmt
    @echo "🚀 Full CI pipeline completed successfully!"

# Build documentation
docs:
    @echo "📚 Building documentation..."
    cargo doc --no-deps --all-features
    @echo "✅ Documentation built!"

# Open documentation in browser
docs-open: docs
    @echo "🌐 Opening documentation..."
    cargo doc --no-deps --all-features --open

# Show feature combinations that will be tested
show-features:
    @echo "📋 Feature combinations that will be tested:"
    @echo "Core features: serde, zod, jsonschema, object_id, typescript"
    @echo "Total combinations: 32 (2^5)"
    @echo ""
    @echo "Key combinations tested by 'just test-named-features':"
    @echo "  • no features"
    @echo "  • zod only"
    @echo "  • typescript only"
    @echo "  • typescript + zod"
    @echo "  • serde + zod"
    @echo "  • serde + zod + object_id"
    @echo "  • all features (default)"
    @echo ""
    @echo "All 32 combinations will be tested by 'just test'"

# Run specific tests by name
test-name TEST_NAME:
    @echo "🧪 Running specific test: {{TEST_NAME}}"
    cargo test {{TEST_NAME}}

# Show test coverage information
coverage:
    @echo "📊 Generating test coverage..."
    cargo install cargo-tarpaulin || echo "cargo-tarpaulin already installed"
    cargo tarpaulin --all-features --out html
    @echo "✅ Coverage report generated in tarpaulin-report.html"

# Benchmark tests
bench:
    @echo "⚡ Running benchmarks..."
    cargo bench
    @echo "✅ Benchmarks completed!"

# List all available commands
help:
    @just --list

# Run all tests in different modes for comprehensive validation
test-comprehensive: test-minimal test-default test-named-features test
    @echo "🎉 Comprehensive testing completed!"
    @echo "✅ All tests passed in all modes!" 
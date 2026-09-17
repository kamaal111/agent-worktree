set export

alias fmt := format
alias fmt-c := format-check

# List available commands
default:
    just --list --unsorted

# Run all sanity checks
ready: quality test

# Run quality checks
quality: format-check lint

# Test package
test:
    cargo test --all-targets

# Build package
build:
    cargo build --release

# Format code
format:
    cargo fmt --all

# Check code formatting
format-check:
    cargo fmt --all -- --check

# Lint package
lint:
    cargo clippy --all-targets -- -D warnings

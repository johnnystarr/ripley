.PHONY: build test clean install help

# Default target
.DEFAULT_GOAL := help

# Build the project in release mode
build:
	@echo "🔨 Building Ripley (release mode)..."
	@cargo build --release
	@echo "✅ Build complete: target/release/ripley"

# Build debug version
debug:
	@echo "🔨 Building Ripley (debug mode)..."
	@cargo build
	@echo "✅ Debug build complete: target/debug/ripley"

# Run tests
test:
	@echo "🧪 Running tests..."
	@cargo test
	@echo "✅ Tests complete"

# Clean build artifacts
clean:
	@echo "🧹 Cleaning build artifacts..."
	@cargo clean
	@rm -rf target/
	@echo "✅ Clean complete"

# Install the binary to ~/.cargo/bin
install:
	@echo "📦 Installing Ripley..."
	@cargo install --path .
	@echo "✅ Ripley installed to ~/.cargo/bin/ripley"
	@echo "   Run with: ripley --output-folder ~/Music/Ripped"

# Uninstall the binary
uninstall:
	@echo "🗑️  Uninstalling Ripley..."
	@cargo uninstall ripley
	@echo "✅ Ripley uninstalled"

# Run the application with default settings
run:
	@echo "🎵 Running Ripley..."
	@cargo run --release

# Check code without building
check:
	@echo "🔍 Checking code..."
	@cargo check
	@echo "✅ Check complete"

# Format code
fmt:
	@echo "✨ Formatting code..."
	@cargo fmt
	@echo "✅ Format complete"

# Run clippy linter
lint:
	@echo "🔎 Running clippy..."
	@cargo clippy -- -D warnings
	@echo "✅ Lint complete"

# Run all checks (format, lint, test)
ci: fmt lint test
	@echo "✅ All CI checks passed"

# Setup dependencies
setup:
	@echo "🔧 Running setup..."
	@./setup.sh

# Show help
help:
	@echo "Ripley - Automated CD Ripper"
	@echo ""
	@echo "Available targets:"
	@echo "  make build      - Build release binary"
	@echo "  make debug      - Build debug binary"
	@echo "  make test       - Run tests"
	@echo "  make clean      - Remove build artifacts"
	@echo "  make install    - Install to ~/.cargo/bin"
	@echo "  make uninstall  - Remove installed binary"
	@echo "  make run        - Run the application"
	@echo "  make check      - Check code without building"
	@echo "  make fmt        - Format code"
	@echo "  make lint       - Run clippy linter"
	@echo "  make ci         - Run all checks (fmt, lint, test)"
	@echo "  make setup      - Install dependencies via setup.sh"
	@echo "  make help       - Show this help message"

.PHONY: build test clean install help dev

# Default target
.DEFAULT_GOAL := help

# Build the project in release mode (includes web UI and agent)
build:
	@echo "🔨 Cleaning Web UI build artifacts..."
	@rm -rf web-ui/dist
	@echo "🔨 Copying sound files to Web UI..."
	@mkdir -p web-ui/public/sounds
	@if [ -f "$(HOME)/.config/ripley/sounds/complete.mp3" ]; then \
		cp "$(HOME)/.config/ripley/sounds/complete.mp3" web-ui/public/sounds/complete.mp3 && \
		echo "   ✓ Copied complete.mp3"; \
	else \
		echo "   ⚠️  complete.mp3 not found in ~/.config/ripley/sounds/ (will use fallback)"; \
	fi
	@if [ -f "$(HOME)/.config/ripley/sounds/error.mp3" ]; then \
		cp "$(HOME)/.config/ripley/sounds/error.mp3" web-ui/public/sounds/error.mp3 && \
		echo "   ✓ Copied error.mp3"; \
	else \
		echo "   ⚠️  error.mp3 not found in ~/.config/ripley/sounds/ (will use fallback)"; \
	fi
	@echo "🔨 Building Web UI..."
	@cd web-ui && npm run build
	@echo "🔨 Building Ripley (release mode)..."
	@cargo build --release
	@echo "🔨 Building Ripley Agent (release mode)..."
	@cd agent && cargo build --release
	@echo "✅ Build complete:"
	@echo "   - target/release/ripley"
	@echo "   - agent/target/release/ripley-agent"

# Build debug version (includes web UI and agent)
debug:
	@echo "🔨 Cleaning Web UI build artifacts..."
	@rm -rf web-ui/dist
	@echo "🔨 Copying sound files to Web UI..."
	@mkdir -p web-ui/public/sounds
	@if [ -f "$(HOME)/.config/ripley/sounds/complete.mp3" ]; then \
		cp "$(HOME)/.config/ripley/sounds/complete.mp3" web-ui/public/sounds/complete.mp3 && \
		echo "   ✓ Copied complete.mp3"; \
	else \
		echo "   ⚠️  complete.mp3 not found in ~/.config/ripley/sounds/ (will use fallback)"; \
	fi
	@if [ -f "$(HOME)/.config/ripley/sounds/error.mp3" ]; then \
		cp "$(HOME)/.config/ripley/sounds/error.mp3" web-ui/public/sounds/error.mp3 && \
		echo "   ✓ Copied error.mp3"; \
	else \
		echo "   ⚠️  error.mp3 not found in ~/.config/ripley/sounds/ (will use fallback)"; \
	fi
	@echo "🔨 Building Web UI..."
	@cd web-ui && npm run build
	@echo "🔨 Building Ripley (debug mode)..."
	@cargo build
	@echo "🔨 Building Ripley Agent (debug mode)..."
	@cd agent && cargo build
	@echo "✅ Debug build complete:"
	@echo "   - target/debug/ripley"
	@echo "   - agent/target/debug/ripley-agent"

# Run development server with hot reload
# Set NO_BROWSER=1 to disable automatic browser opening
dev:
	@echo "🚀 Starting Ripley development server..."
	@echo "   API server: http://localhost:3000/api"
	@echo "   Web UI: http://localhost:5173"
	@echo ""
	@NO_BROWSER=$(NO_BROWSER) ./scripts/dev.sh

# Run tests
test:
	@echo "🧪 Running macOS tests..."
	@cargo test
	@echo "✅ macOS tests complete"

# Run Linux tests via Podman
test-linux:
	@echo "🐳 Running Linux tests via Podman..."
	@./scripts/test-linux.sh

# Run all tests (macOS and Linux)
test-all: test test-linux
	@echo "✅ All tests complete"

# Clean build artifacts
clean:
	@echo "🧹 Cleaning build artifacts..."
	@cargo clean
	@cd agent && cargo clean || true
	@rm -rf target/
	@rm -rf agent/target/
	@rm -rf web-ui/dist
	@echo "✅ Clean complete (removed all binaries and build artifacts)"

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
run: clean build
	@echo "🎵 Running Ripley..."
	@open -a "Google Chrome" http://localhost:8080 2>/dev/null || echo "⚠️  Failed to open Chrome (may not be installed)"
	@target/release/ripley serve --port 8080


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
	@./scripts/setup.sh

reinstall: uninstall install
	@echo "✅ Ripley reinstalled"
# Show help
help:
	@echo "Ripley - Automated Optical Disc Ripper"
	@echo ""
	@echo "Available targets:"
	@echo "  make build      - Build release binary (includes web UI)"
	@echo "  make debug      - Build debug binary (includes web UI)"
	@echo "  make dev        - Run development server with hot reload"
	@echo "  make test       - Run macOS tests"
	@echo "  make test-linux - Run Linux tests via Podman"
	@echo "  make test-all   - Run all tests (macOS and Linux)"
	@echo "  make clean      - Remove build artifacts"
	@echo "  make install    - Install to ~/.cargo/bin"
	@echo "  make uninstall  - Remove installed binary"
	@echo "  make reinstall  - Reinstall the binary"
	@echo "  make run        - Run the application"
	@echo "  make check      - Check code without building"
	@echo "  make fmt        - Format code"
	@echo "  make lint       - Run clippy linter"
	@echo "  make ci         - Run all checks (fmt, lint, test)"
	@echo "  make setup      - Install dependencies via setup.sh"
	@echo "  make help       - Show this help message"

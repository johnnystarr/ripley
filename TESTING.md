# Testing Guide

## Overview

Ripley has comprehensive testing infrastructure for both macOS and Linux platforms.

## Test Environment Setup

### macOS Testing

Run tests natively on macOS:

```bash
make test
# or
cargo test
```

### Linux Testing via Podman

Test Linux compatibility using Podman:

```bash
make test-linux
# or
./test-linux.sh
```

This will:
1. Build a Docker container with Linux dependencies
2. Run all tests in a Linux environment
3. Verify cross-platform compatibility

## Test Structure

### Unit Tests

Located in `src/*.rs` files under `#[cfg(test)]` blocks:

- **database.rs**: Database operations, migrations, queries
- **config.rs**: Configuration loading/saving, validation
- **checksum.rs**: SHA-256 checksum calculations
- **metadata.rs**: MusicBrainz metadata fetching
- **speech_match.rs**: Episode matching and transcription
- **drive.rs**: Drive detection and media type identification

### Integration Tests

Located in `tests/` directory:

- **integration_test.rs**: End-to-end ripping workflows
- **api_test.rs**: API endpoints and WebSocket functionality

## Running Specific Tests

```bash
# Run all tests
cargo test

# Run specific module tests
cargo test --lib database::tests
cargo test --lib config::tests
cargo test --lib checksum::tests

# Run with output
cargo test -- --nocapture

# Run in release mode
cargo test --release
```

## Test Coverage

### Backend Modules Tested

✅ Database operations (CRUD, migrations, queries)  
✅ Configuration management  
✅ Checksum verification  
✅ Metadata fetching  
✅ Episode matching logic  
✅ Drive detection (macOS)  
✅ Speech transcription parsing  

### Test Statistics

- **Total Tests**: 60+ unit tests
- **Coverage**: All core modules
- **Platforms**: macOS (native), Linux (via Podman)

## Writing New Tests

When adding new features, include tests in the module's `#[cfg(test)]` block:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_feature() {
        // Test implementation
    }
}
```

## Continuous Integration

The test suite is designed to run in CI/CD pipelines:

```bash
# Full CI pipeline
make ci  # Runs format, lint, and test
```


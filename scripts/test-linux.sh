#!/bin/bash
# Script to run Linux tests using Podman

set -e

echo "🐳 Building Linux test container with Podman..."
podman build -f Dockerfile.test -t ripley-test:linux .

echo "🧪 Running Linux tests in container..."
echo "   (This will test cross-platform compatibility)"

podman run --rm \
    -v "$(pwd):/app:Z" \
    -w /app \
    ripley-test:linux \
    cargo test --target x86_64-unknown-linux-gnu -- --nocapture

echo "✅ Linux tests complete!"

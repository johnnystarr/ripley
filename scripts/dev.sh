#!/bin/bash
# Development server startup script

set -e

# Clean up on exit
cleanup() {
    echo ""
    echo "🛑 Shutting down servers..."
    kill $(jobs -p) 2>/dev/null || true
    wait $(jobs -p) 2>/dev/null || true
    echo "✅ Shutdown complete"
    exit 0
}

trap cleanup SIGINT SIGTERM EXIT

# Check if browser opening should be disabled
NO_BROWSER=${NO_BROWSER:-0}

# Start Vite dev server in background
echo "🌐 Starting Vite dev server..."
cd web-ui
npm run dev &
VITE_PID=$!
cd ..

# Wait for Vite to start (check port 5173)
echo "⏳ Waiting for Vite server..."
VITE_READY=0
for i in {1..30}; do
    if curl -s http://localhost:5173 > /dev/null 2>&1; then
        VITE_READY=1
        break
    fi
    sleep 1
done

if [ $VITE_READY -eq 0 ]; then
    echo "⚠️  Vite server didn't start in time"
else
    echo "✅ Vite server ready"
fi

# Wait for Rust API server to be ready (check port 3000)
echo "⏳ Waiting for Rust API server..."
RUST_READY=0

# Start Rust API server in background
echo "🚀 Starting Rust API server..."
cargo run -- serve --dev --port 3000 &
RUST_PID=$!

for i in {1..30}; do
    if curl -s http://localhost:3000/api/health > /dev/null 2>&1; then
        RUST_READY=1
        break
    fi
    sleep 1
done

if [ $RUST_READY -eq 0 ]; then
    echo "⚠️  Rust API server didn't start in time"
else
    echo "✅ Rust API server ready"
fi

# Open browser on macOS if not disabled
if [ "$NO_BROWSER" = "0" ] && [ "$(uname)" = "Darwin" ]; then
    if [ $VITE_READY -eq 1 ]; then
        echo "🌐 Opening browser..."
        sleep 1  # Give a moment for everything to settle
        if command -v "Google Chrome" >/dev/null 2>&1 || [ -d "/Applications/Google Chrome.app" ]; then
            open -a "Google Chrome" http://localhost:5173 2>/dev/null || echo "⚠️  Failed to open Chrome (may not be installed)"
        else
            echo "⚠️  Google Chrome not found, skipping browser open"
        fi
    fi
fi

# Wait for both processes
wait $VITE_PID 2>/dev/null || true
wait $RUST_PID 2>/dev/null || true

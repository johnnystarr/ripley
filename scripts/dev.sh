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

# Start Vite dev server in background
echo "🌐 Starting Vite dev server..."
cd web-ui
npm run dev &
VITE_PID=$!
cd ..

# Wait a moment for Vite to start
sleep 2

# Start Rust API server in foreground (this will block)
echo "🚀 Starting Rust API server..."
cargo run -- serve --dev --port 3000


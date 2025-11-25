#!/bin/bash

# Ripley Setup Script
# This script installs dependencies and sets up the Ripley CD ripper

set -e

echo "🎵 Ripley Setup"
echo "==============="
echo ""

# Check if Homebrew is installed
if ! command -v brew &> /dev/null; then
    echo "❌ Homebrew is not installed."
    echo "Please install Homebrew first:"
    echo "  /bin/bash -c \"\$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)\""
    exit 1
fi

echo "✅ Homebrew found"

# Install dependencies
echo ""
echo "📦 Installing dependencies..."
echo ""

brew install abcde
brew install flac
brew install cd-discid

echo ""
echo "✅ Dependencies installed"

# Create sounds directory
echo ""
echo "📁 Creating sounds directory..."
SOUNDS_DIR="$HOME/.config/ripley/sounds"
mkdir -p "$SOUNDS_DIR"

cat > "$SOUNDS_DIR/README.txt" << 'EOF'
Ripley Audio Notifications
===========================

Place your audio notification files here:

- complete.mp3: Played when a CD finishes ripping successfully
- error.mp3: Played when metadata lookup fails after all retries

These files are optional. If not present, notifications will be skipped.

You can use any MP3 files you like. Here are some suggestions:
- Use short sound effects (1-3 seconds)
- Keep volume moderate
- Test them before putting CDs in!

Example sources for free sounds:
- https://freesound.org/
- https://mixkit.co/free-sound-effects/
EOF

echo "✅ Created: $SOUNDS_DIR"
echo ""

# Check if Rust is installed
if ! command -v cargo &> /dev/null; then
    echo "⚠️  Rust is not installed."
    echo "Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
else
    echo "✅ Rust found"
fi

# Build Ripley
echo ""
echo "🔨 Building Ripley..."
cargo build --release

echo ""
echo "✅ Build complete!"
echo ""
echo "📝 Next steps:"
echo ""
echo "1. Add notification sounds (optional):"
echo "   cp /path/to/complete.mp3 $SOUNDS_DIR/"
echo "   cp /path/to/error.mp3 $SOUNDS_DIR/"
echo ""
echo "2. Run Ripley:"
echo "   cargo run --release -- --output-folder ~/Music/Ripped"
echo ""
echo "   Or install it globally:"
echo "   cargo install --path ."
echo "   ripley --output-folder ~/Music/Ripped"
echo ""
echo "🎵 Happy ripping!"

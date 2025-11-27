# Ripley 🎬

Automated Optical Disc Ripper with AI-powered episode matching and distributed video upscaling.

[![CI/CD Pipeline](https://github.com/johnny/ripley/actions/workflows/ci.yml/badge.svg)](https://github.com/johnny/ripley/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.70+-orange.svg)](https://www.rust-lang.org/)

## Features

- 🎥 **Real-time TUI** with multi-drive support
- 🤖 **AI-powered episode matching** via speech recognition + GPT-4o
- 📺 **Automatic metadata** from TMDB + TheTVDB
- 📝 **Smart renaming** with Filebot integration
- 📤 **Background rsync** to network storage
- 🔔 **macOS notifications** on completion
- 🌐 **Web UI** for monitoring and configuration
- 🖥️ **Windows Agent** for distributed Topaz Video AI upscaling
- 📊 **Real-time monitoring** of all operations

## Quick Start

### Prerequisites

- Rust 1.70+
- Node.js 18+ (for web UI)
- macOS or Linux (main server)
- Windows (for agent, optional)

### Installation

```bash
# Clone the repository
git clone https://github.com/johnny/ripley.git
cd ripley

# Build the project
make build

# Or install directly
make install
```

### Development

```bash
# Start development servers (auto-opens browser on macOS)
make dev

# Or disable auto-open
NO_BROWSER=1 make dev
```

## Architecture

- **Main Server** (`ripley`): Rust backend with REST API and WebSocket support
- **Web UI** (`web-ui/`): React frontend for monitoring and configuration
- **Agent** (`agent/`): Windows TUI client for Topaz Video AI processing

## Documentation

See [Ripley3.0.md](Ripley3.0.md) for the complete feature roadmap and implementation status.

## License

MIT License - see [LICENSE](LICENSE) file for details.

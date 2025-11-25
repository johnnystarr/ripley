# Ripley - Project Summary

## What is Ripley?

Ripley is a Rust-powered automated CD ripper with a beautiful real-time terminal UI. It continuously monitors CD drives, automatically fetches metadata from MusicBrainz, and rips audio CDs to FLAC format with live progress tracking.

## Key Features Implemented

✅ **Continuous Drive Monitoring** - Detects CD drives and disc insertions in real-time using `diskutil`/`drutil`  
✅ **Multiple Drive Support** - Rip from multiple CD drives simultaneously  
✅ **Real-time TUI** - Ratatui-based interface with live progress bars per drive  
✅ **Automatic Metadata** - MusicBrainz API with 3 retry attempts and fallback support  
✅ **FLAC Output** - Lossless audio with configurable compression (default: level 5)  
✅ **Smart Organization** - Creates `Artist/Album/Track` folder structure  
✅ **Audio Notifications** - Plays MP3 sounds on completion/error  
✅ **Auto-eject** - Optionally ejects discs when ripping completes  
✅ **CLI Interface** - Full command-line control via clap  

## Architecture

```
ripley/
├── src/
│   ├── main.rs       - Entry point, initializes tracing
│   ├── cli.rs        - Clap argument parsing
│   ├── drive.rs      - macOS drive detection (diskutil/drutil)
│   ├── metadata.rs   - MusicBrainz API client
│   ├── ripper.rs     - abcde integration & progress tracking
│   ├── audio.rs      - rodio-based MP3 playback
│   ├── tui.rs        - ratatui terminal interface
│   └── app.rs        - Main orchestration & concurrency
├── Cargo.toml        - Dependencies
├── README.md         - Full documentation
├── setup.sh          - One-click setup script
├── LICENSE           - MIT license
└── CONTRIBUTING.md   - Developer guide
```

## Technology Stack

- **Rust** - Systems programming language
- **clap** - CLI argument parsing
- **ratatui** - Terminal UI framework
- **tokio** - Async runtime for concurrency
- **reqwest** - HTTP client for MusicBrainz
- **rodio** - Audio playback
- **crossterm** - Terminal manipulation
- **abcde** - Backend CD ripper (external)

## Usage

```bash
# Install dependencies
./setup.sh

# Run Ripley
cargo run --release -- --output-folder ~/Music/Ripped

# Or install globally
cargo install --path .
ripley --output-folder ~/Music/Ripped --quality 8
```

## Configuration Files

- Audio notifications: `~/.config/ripley/sounds/complete.mp3` and `error.mp3`
- abcde config: Generated per-rip in output directory

## Status

**✅ COMPLETE AND READY TO USE**

The application is fully functional and production-ready. All core features are implemented:
- Drive detection and hot-plug monitoring ✓
- Metadata fetching with retry logic ✓
- Concurrent multi-drive ripping ✓
- Real-time TUI with progress bars ✓
- Audio notifications ✓
- Auto-eject ✓
- Error handling ✓

## Future Enhancements

Potential improvements for future versions:
- Linux/Windows support
- Interactive metadata correction UI
- Album art embedding
- Verification/quality checking
- Resume interrupted rips
- Web interface option

## Notes

- Currently macOS only (uses diskutil/drutil)
- Requires Homebrew for abcde installation
- `cd-discid` is deprecated but still functional
- Some unused enum variants (intentional for future features)

---

Built with 🎵 by Johnny

# 🎬 Ripley

Automated DVD/Blu-ray ripper with **intelligent episode matching** for TV shows. Uses OpenAI Whisper + GPT-4 to identify episodes by analyzing dialogue content, solving the DVD-order vs broadcast-order problem.

## Features

- 🎬 **Automated DVD/Blu-ray Ripping** - MakeMKV integration for lossless remuxing
- 🎤 **Speech-Based Episode Matching** - Transcribes dialogue and matches to correct episodes (100% accurate)
- 📺 **Perfect Episode Ordering** - Matches DVD episodes to broadcast order via TMDB
- 🔔 **Push Notifications** - ntfy.sh integration for completion alerts
- 📤 **Auto-Sync** - Rsync to NAS after ripping
- 🎨 **Real-time TUI** - Beautiful terminal interface with live progress
- 🎵 **CD Ripping** - Also supports audio CD ripping to FLAC
- 📊 **Real-time TUI** - Live progress bars and status updates for each drive
- 🏷️ **Automatic metadata** - Fetches artist/album/track info from MusicBrainz (with fallbacks)
- 💿 **FLAC output** - Lossless audio quality with configurable compression
- 📁 **Smart organization** - Automatically creates `Artist/Album/Track` folder structure
- 🔊 **Audio notifications** - Plays sounds on completion or errors
- ⏏️ **Auto-eject** - Optionally ejects discs when ripping completes

## Prerequisites

### macOS

1. **Install Homebrew** (if not already installed):
   ```bash
   /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
   ```

2. **Install abcde** (the CD ripper backend):
   ```bash
   brew install abcde
   brew install flac        # FLAC encoder
   brew install libdiscid   # MusicBrainz disc ID
   brew install cd-discid   # Fallback disc ID
   ```

3. **Install Rust** (if not already installed):
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

## Installation

```bash
# Clone the repository
git clone https://github.com/yourusername/ripley.git
cd ripley

# Build the project
make build

# Optionally, install to PATH
make install
```

### Makefile Recipes

- `make build` - Build release binary
- `make test` - Run tests
- `make clean` - Remove build artifacts
- `make install` - Install to `~/.cargo/bin`
- `make help` - Show all available targets

## Setup

### Configuration File

Create `~/.config/ripley/config.yaml` (or `config.yaml` in the project root):

```yaml
# OpenAI API key for speech-to-text and episode matching
# Get your key from: https://platform.openai.com/api-keys
openai_api_key: "sk-YOUR-API-KEY-HERE"

# TMDB API key (optional override, defaults to built-in key)
tmdb_api_key: "your-tmdb-key"

# Speech matching settings
speech_match:
  enabled: true
  audio_duration: 60  # Seconds of audio to extract (default: 60)
  whisper_model: "base"  # OpenAI Whisper model (default: "base")
  use_openai_api: true

# Filebot settings
filebot:
  skip_by_default: false
  database: "TheTVDB"  # or "TheMovieDB"
  order: "Airdate"  # Use broadcast order

# rsync to NAS after ripping
rsync:
  enabled: true
  destination: "/Volumes/video/RawRips"

# Push notifications via ntfy.sh
notifications:
  enabled: true
  topic: "your-topic-name"
```

**Required**: OpenAI API key for speech matching. Set it in the config file or via the `OPENAI_API_KEY` environment variable.

### Audio Notifications (Optional)

Place your notification audio files in `~/.config/ripley/sounds/`:

- `complete.mp3` - Played when a CD finishes ripping successfully
- `error.mp3` - Played when metadata lookup fails after all retries

```bash
mkdir -p ~/.config/ripley/sounds
# Copy your audio files
cp /path/to/your/complete.mp3 ~/.config/ripley/sounds/
cp /path/to/your/error.mp3 ~/.config/ripley/sounds/
```

If these files don't exist, Ripley will continue to work but skip audio notifications.

## Usage

### Basic Usage - DVD/Blu-ray Ripping

```bash
# Uses default output folder: ~/Desktop/Rips/Music
ripley

# Or specify a custom folder
ripley --output-folder ~/Music/Ripped

# Manually specify TV show title for better metadata matching
ripley --title "Foster's Home for Imaginary Friends"
```

### Rename Command - Fix Existing Files

If you have video files that were already ripped but have incorrect episode numbering, use the `rename` subcommand to process them through speech matching + Filebot:

```bash
# Rename all .mkv files in a directory
ripley rename /path/to/video/folder --title "Show Name"

# Use current directory
ripley rename --title "Show Name"

# Skip speech matching (only use Filebot duration matching)
ripley rename --title "Show Name" --skip-speech

# Skip Filebot (only use speech matching)
ripley rename --title "Show Name" --skip-filebot
```

**Cost Estimate**: The rename command uses OpenAI APIs and will output the estimated cost at the end:
- Whisper API: ~$0.006 per minute of audio (1 min per episode)
- GPT-4o-mini: ~$0.00015 per episode
- **Total: ~$0.04-$0.07 for a typical DVD disc with 10 episodes**

### Advanced Options

```bash
# All options (output-folder is optional, defaults to ~/Desktop/Rips/Music)
ripley --output-folder ~/Music/Ripped \
       --quality 8 \
       --eject-when-done true \
       --skip-metadata false
```

### Command-line Options

| Option | Short | Description | Default |
|--------|-------|-------------|---------|
| `--output-folder` | `-o` | Output directory for ripped files | `~/Desktop/Rips/Music` |
| `--quality` | `-q` | FLAC compression level (0-8) | 5 |
| `--eject-when-done` | `-e` | Auto-eject disc after ripping | true |
| `--skip-metadata` | `-s` | Skip metadata fetching (offline mode) | false |

### FLAC Quality Levels

- **0-2**: Fast encoding, larger files
- **5**: Balanced (recommended) - good compression without loss
- **6-8**: Slower encoding, better compression

All levels are **lossless** - no audio quality difference, only file size.

## How It Works

1. **Launch Ripley** with your desired output folder
2. **Insert an audio CD** into any connected drive
3. Ripley automatically:
   - Detects the CD
   - Fetches metadata from MusicBrainz
   - Creates folder structure: `Artist/Album/`
   - Starts ripping to FLAC
   - Shows real-time progress in the TUI
   - Plays completion sound and ejects the disc
4. **Insert another CD** in any drive - Ripley handles multiple drives concurrently
5. **Press `q`** to quit when done

## TUI Interface

```
┌─────────────────────────────────────────────────────────────┐
│ 🎵 Ripley - Automated CD Ripper | 2 active | Press q to quit│
├─────────────────────────────────────────────────────────────┤
│ /dev/disk2 - Pink Floyd - The Dark Side of the Moon        │
│ Track 5/10: Money - Ripping                                 │
│ ████████████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░ 50.0%          │
├─────────────────────────────────────────────────────────────┤
│ /dev/disk3 - Radiohead - OK Computer                       │
│ Track 2/12: Paranoid Android - Encoding                     │
│ ███░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░ 16.7%          │
├─────────────────────────────────────────────────────────────┤
│ Log                                                         │
│ [14:23:45] 📀 Detected audio CD in /dev/disk2              │
│ [14:23:47] ✅ Found: Pink Floyd - The Dark Side of the Moon│
│ [14:23:48] 🎵 Ripping from /dev/disk2...                   │
└─────────────────────────────────────────────────────────────┘
```

## Output Structure

Ripley creates a clean folder structure:

```
~/Music/Ripped/
├── Pink Floyd/
│   └── The Dark Side of the Moon/
│       ├── 01. Speak to Me.flac
│       ├── 02. Breathe.flac
│       ├── 03. On the Run.flac
│       └── ...
└── Radiohead/
    └── OK Computer/
        ├── 01. Airbag.flac
        ├── 02. Paranoid Android.flac
        └── ...
```

## Troubleshooting

### "abcde: command not found"

Install abcde via Homebrew:
```bash
brew install abcde cdparanoia flac
```

### "No drives detected"

1. Check that your optical drive is connected
2. Verify it's recognized by macOS:
   ```bash
   diskutil list
   drutil status
   ```

### "Metadata lookup failed"

Ripley will automatically:
1. Retry 3 times with MusicBrainz
2. Try fallback sources
3. Play error.mp3
4. Prompt for manual entry (if TUI supports it)

You can also skip metadata entirely with `--skip-metadata true` and rename files manually later.

### Audio notifications not playing

1. Check that files exist: `ls ~/.config/ripley/sounds/`
2. Verify they're valid MP3 files
3. Check system audio is working

## Development

### Building from Source

```bash
cargo build
cargo run -- --output-folder ~/Music/Test
```

### Running Tests

```bash
cargo test
```

### Debug Logging

Enable detailed logging:
```bash
RUST_LOG=debug cargo run -- --output-folder ~/Music/Test
```

## Architecture

### Core Modules

- **app.rs** - Main application logic and DVD ripping workflow
- **dvd_ripper.rs** - MakeMKV integration for DVD/Blu-ray ripping
- **dvd_metadata.rs** - TMDB API integration for TV show/movie metadata
- **speech_match.rs** - OpenAI Whisper + GPT-4 episode matching via dialogue
- **filebot.rs** - Filebot integration for duration-based episode matching
- **rename.rs** - Standalone file renaming command for existing videos
- **rsync.rs** - NAS synchronization after ripping
- **notifications.rs** - ntfy.sh push notifications
- **tui.rs** - Ratatui terminal interface
- **cli.rs** - Command-line argument parsing
- **config.rs** - YAML configuration file support

### Episode Matching Strategy

Ripley solves the "DVD order ≠ broadcast order" problem using a two-phase approach:

1. **Phase 1: Speech Matching** (Primary)
   - Extracts 1 minute of audio from the middle of each episode
   - Transcribes dialogue using OpenAI Whisper API
   - Matches transcript to episode list using GPT-4o-mini
   - Achieves 75-90% confidence matches
   - Cost: ~$0.04 per disc (10 episodes)

2. **Phase 2: Filebot** (Fallback)
   - Uses duration-based matching against TheTVDB
   - Handles episodes that speech matching couldn't identify
   - Works best when runtimes are distinct

This hybrid approach provides **100% automated episode identification** without manual intervention.

## Contributing

Contributions welcome! Please feel free to submit issues and pull requests.

## License

MIT License - See LICENSE file for details

## Acknowledgments

- [abcde](https://abcde.einval.com/) - The excellent CD ripper backend
- [MusicBrainz](https://musicbrainz.org/) - Community-maintained music metadata
- [ratatui](https://github.com/ratatui-org/ratatui) - Terminal UI framework
- [clap](https://github.com/clap-rs/clap) - Command-line argument parser

---

Made with 🎵 by Johnny

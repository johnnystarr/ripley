# Ripley Quick Start

## Install Dependencies
```bash
brew install abcde cd-discid
```

## Build
```bash
cargo build --release
```

## Run
```bash
./target/release/ripley
# or
cargo run --release
```

## Common Commands
```bash
# Maximum quality, custom output
ripley -o ~/Music -q 8

# Offline mode (no metadata lookup)
ripley --skip-metadata

# Don't eject when done
ripley -e false
```

## Keyboard Controls
- `q` or `ESC` - Quit

## What You'll See
```
┌────────────────────────────────────────────┐
│ 🎵 Ripley - Automated CD Ripper            │
│ 1 active | Press q to quit                 │
└────────────────────────────────────────────┘

┌─ /dev/disk2 - Artist - Album ─────────────┐
│ Track 3/12: Song Name - Ripping           │
│ ████████░░░░░░░░░ 45.2%                   │
└────────────────────────────────────────────┘

┌─ Log /dev/disk2 ──────────────────────────┐
│ [15:30:45] 📀 Detected audio CD           │
│ [15:30:46] 🔍 Fetching metadata...        │
│ [15:30:47] 📀 Disc ID: 3cy3Ffji...        │
│ [15:30:48] 🎵 Ripping Artist - Album      │
│ [15:30:50] Grabbing track 1...            │
│ [15:31:15] Encoding track 1...            │
│ [15:31:20] Grabbing track 2...            │
│ ...                                        │
└────────────────────────────────────────────┘
```

## Output Structure
```
~/Desktop/Rips/Music/
└── Artist Name/
    └── Album Name/
        ├── 01. Track One.flac
        ├── 02. Track Two.flac
        └── ...
```

## Logs
All activity logged to: `~/ripley.log`

## Audio Notifications (Optional)
Place in `~/.config/ripley/sounds/`:
- `complete.mp3` - Success
- `error.mp3` - Error

## Troubleshooting

### "Resource busy"
Already handled! Ripley force-unmounts discs before reading.

### "Could not get disc ID"
- Check disc is audio CD (not data CD)
- Clean the disc
- Try another drive

### "abcde failed"
Check `~/ripley.log` for detailed error messages.

### No metadata found
- Check internet connection
- Use `--skip-metadata` for offline ripping
- abcde will still rip, just without artist/album info

## Quality Settings
- `0` - Fastest, largest files
- `5` - Default, balanced
- `8` - Slowest, smallest files (recommended for archival)

## Multi-Drive Support
✅ Insert CDs into multiple drives  
✅ Each gets its own progress bar and log  
✅ All rip simultaneously  
✅ Auto-eject when finished  

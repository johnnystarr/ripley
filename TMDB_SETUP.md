# Setting up TMDB API for DVD Metadata

To enable automatic DVD metadata lookup and proper episode/movie identification:

## 1. Get TMDB API Key

1. Go to https://www.themoviedb.org/
2. Create a free account
3. Go to Settings → API
4. Request an API key (it's free for personal use)
5. Copy your API key (v3 auth)

## 2. Configure Ripley

Edit `src/dvd_metadata.rs` and replace:

```rust
const TMDB_API_KEY: &str = "YOUR_TMDB_API_KEY";
```

With your actual API key:

```rust
const TMDB_API_KEY: &str = "abc123yourkeyhere";
```

## 3. Rebuild

```bash
cargo build --release
```

## Features with TMDB

### TV Shows
- Automatically identifies episodes
- Renames files as: `Show Name - S01E01 - Episode Title.mkv`
- Detects season and episode numbers
- Uses disc volume name to search TMDB

### Movies  
- Renames as: `Movie Title (Year).mkv`
- Extracts release year
- Clean filenames

### Without TMDB API
- Files are saved with generic names
- Uses disc volume name as title
- No episode identification

## Example Output

**With TMDB (TV Show):**
```
The Office/
  The Office - S01E01 - Pilot.mkv
  The Office - S01E02 - Diversity Day.mkv
  The Office - S01E03 - Health Care.mkv
```

**With TMDB (Movie):**
```
The Matrix (1999).mkv
```

**Without TMDB:**
```
DVD_20251125_163045/
  title_t00.mkv
  title_t01.mkv
```

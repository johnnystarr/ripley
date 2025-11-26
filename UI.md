# Ripley Web UI Implementation Checklist

## Architecture Overview

The Ripley web UI is a fully automated disc ripping monitoring system with:
- **TUI-inspired Dashboard**: Real-time drive detection and automatic operation tracking
- **SQLite Database**: Persistent storage for logs and issues
- **WebSocket Events**: Live updates for drive detection, progress, and logs
- **No Manual Controls**: Fully automated ripping - UI is for monitoring only
- **Tailwind Alerts**: All notifications use Tailwind components (no JS alerts)

## Project Setup
- [x] Initialize Vite + React project in `web-ui/` directory
- [x] Install dependencies: tailwindcss, @fortawesome/react-fontawesome, @fortawesome/free-solid-svg-icons
- [x] Configure Tailwind CSS with dark mode
- [x] Set up Vite build to output to `web-ui/dist/`
- [x] Add `include_dir` dependency to Rust project
- [x] Create `src/web_ui.rs` module for embedded static file serving
- [x] Update `src/api.rs` to prefix all API routes with `/api`
- [x] Add static file fallback route for SPA routing
- [x] Create `build.rs` for automatic UI build during cargo compilation
- [x] Add `--dev` flag to proxy to Vite dev server (hot reload)

## UI Design & Styling
- [x] **Dark Mode**: Use Tailwind's dark mode as default
- [x] **Color Scheme**: Tailwind's slate/gray for backgrounds, cyan/blue for accents, green for success, red for errors, yellow for warnings
- [x] **Typography**: Modern sans-serif font (Inter or similar via Tailwind)
- [x] **Icons**: Font Awesome for all icons (disc, play, stop, folder, cog, etc.)
- [x] **Layout**: Single-page responsive dashboard with sidebar navigation

## Core Components

### 1. Layout & Navigation
- [x] `App.jsx` - Main app container with routing
- [x] Sidebar navigation with icons (integrated in App.jsx)
  - [x] Dashboard link
  - [x] Configuration link
  - [x] Logs link
  - [x] ~~Drives link~~ (removed - drives shown in Dashboard)
- [x] Header component - Top bar with app title and WebSocket connection status
- [x] Responsive layout with collapsible sidebar

### 2. Dashboard Page (TUI-Inspired Real-Time Monitoring)
- [x] `Dashboard.jsx` - Real-time monitoring of automatic ripping operations
- [x] **Active Issues Alert Section**
  - [x] Tailwind alert cards for unresolved issues
  - [x] Issue type badges (RipFailure, MetadataFailure, FilebotError, etc.)
  - [x] Timestamp and description
  - [x] "Resolve" button for each issue
  - [x] Color-coded by severity (red for errors)
- [x] **Detected Drives Grid**
  - [x] Auto-polling every 3 seconds for drive changes
  - [x] Drive cards showing device name and model
  - [x] Disc present indicator (green check / gray x)
  - [x] Disc type display (DVD, BluRay, etc.)
  - [x] Live progress bar during ripping
  - [x] Status message for current operation
  - [x] Empty state when no drives detected
- [x] **Live Log Stream**
  - [x] Real-time log display with WebSocket updates
  - [x] Color-coded log levels (error=red, warning=yellow, success=green, info=blue)
  - [x] Timestamp for each log entry
  - [x] Drive label for each log (when applicable)
  - [x] Auto-scroll with scrollable container
  - [x] Last 100 logs displayed

### 3. Configuration Page
- [ ] `Configuration.jsx` - Editable configuration
- [ ] **API Keys Section**
  - [ ] OpenAI API Key input (password field with show/hide toggle)
  - [ ] TMDB API Key input
- [ ] **Notifications Section**
  - [ ] Enable/disable toggle
  - [ ] Topic input field
- [ ] **Rsync Section**
  - [ ] Enable/disable toggle
  - [ ] Destination path input
- [ ] **Speech Match Section**
  - [ ] Enable/disable toggle
  - [ ] Audio duration slider (30-600 seconds)
  - [ ] Whisper model dropdown
  - [ ] Use OpenAI API toggle
- [ ] **Filebot Section**
  - [ ] Skip by default toggle
  - [ ] Database dropdown (TheTVDB, TheMovieDB)
  - [ ] Order dropdown (Airdate, DVD)
  - [ ] Use for music toggle
- [ ] "Save Configuration" button
- [ ] Success/error toast notifications

### 4. Logs Page (SQLite History)
- [x] `Logs.jsx` - Full log history viewer with database integration
- [x] **Search & Filter Section**
  - [x] Search input with real-time query
  - [x] Filter by log level (info/warning/error/success)
  - [x] Filter by drive (dropdown of detected drives)
  - [x] "Apply Filters" button
  - [x] "Clear" button to reset filters
- [x] **Log Entry Display**
  - [x] Tailwind alert cards color-coded by level
  - [x] Level badges with icons
  - [x] Drive labels when applicable
  - [x] Timestamp (formatted locale string)
  - [x] Disc, title, and context information when available
  - [x] Scrollable container for large logs
- [x] "Refresh" button to reload logs from database
- [x] Empty state when no logs found

## Backend Integration

### 5. SQLite Database
- [x] `src/database.rs` - Complete database module
- [x] **Schema**
  - [x] `logs` table with timestamp, level, message, drive, disc, title, context
  - [x] `issues` table with timestamp, type, title, description, resolved status
  - [x] Indexes on timestamp and resolved columns
- [x] **LogLevel enum**: Info, Warning, Error, Success
- [x] **IssueType enum**: RipFailure, MetadataFailure, FilebotError, SpeechMatchFailure, RsyncFailure, DriveError, Other
- [x] **Database operations**
  - [x] `add_log()` - Insert log entry
  - [x] `get_recent_logs()` - Retrieve last N logs
  - [x] `search_logs()` - Filter by query, level, drive
  - [x] `add_issue()` - Create new issue
  - [x] `get_all_issues()` - Retrieve all issues
  - [x] `get_active_issues()` - Get unresolved issues
  - [x] `resolve_issue()` - Mark issue as resolved
- [x] Database location: `~/.config/ripley/ripley.db`
- [x] Unit tests for database operations

## API Integration

### 6. API Client (`web-ui/src/api.js`)
- [x] Base API client with fetch wrapper
- [x] Error handling with console logging
- [x] **Configuration Endpoints**
  - [x] `getHealth()` - GET /api/health
  - [x] `getStatus()` - GET /api/status
  - [x] `getConfig()` - GET /api/config
  - [x] `updateConfig(config)` - POST /api/config
- [x] **Drive Endpoints**
  - [x] `getDrives()` - GET /api/drives
  - [x] `detectDrives()` - Alias for getDrives()
- [x] **Rip Operations**
  - [x] `startRip(params)` - POST /api/rip/start
  - [x] `stopRip()` - POST /api/rip/stop
  - [x] `renameFiles(params)` - POST /api/rename
- [x] **Log Endpoints**
  - [x] `getLogs()` - GET /api/logs (last 100 entries)
  - [x] `searchLogs(params)` - GET /api/logs/search?query=&level=&drive=
- [x] **Issue Endpoints**
  - [x] `getIssues()` - GET /api/issues
  - [x] `getActiveIssues()` - GET /api/issues/active
  - [x] `resolveIssue(id)` - POST /api/issues/:id/resolve
- [x] WebSocket URL helper for dev/prod environments

### 7. WebSocket Integration (`web-ui/src/websocket.js`)
- [x] WebSocket connection manager with singleton pattern
- [x] Auto-reconnect with exponential backoff
- [x] Event subscription system with unsubscribe capability
- [x] **Event Types** (enhanced with drive tracking):
  - [x] `RipStarted` - Includes disc and drive
  - [x] `RipProgress` - Includes progress, message, and drive
  - [x] `RipCompleted` - Includes disc and drive
  - [x] `RipError` - Includes error and optional drive
  - [x] `Log` - Enhanced with level (info/warning/error/success), message, and optional drive
  - [x] `StatusUpdate` - Full status object
  - [x] `DriveDetected` - NEW: Emitted when drive is detected (includes drive info)
  - [x] `DriveRemoved` - NEW: Emitted when drive is removed (includes device)
  - [x] `DriveEjected` - NEW: Emitted after successful eject (includes device)
  - [x] `IssueCreated` - NEW: Emitted when issue is created (includes full issue object)
- [x] Connection status indicator in header
- [x] Dev/prod URL handling

## UI Polish & Features

### 8. Notifications & Feedback
- [x] Tailwind alert components (no JavaScript alerts per requirement)
- [x] Loading spinners (FontAwesome spinner with animate-spin)
- [x] Issue alerts displayed inline in Dashboard
- [x] Error state displays with Tailwind alert styling
- [ ] Toast notification system (react-hot-toast) - planned for non-critical notifications
- [ ] Error boundaries for component errors

### 9. Responsive Design
- [x] Mobile-friendly layout with collapsible sidebar
- [x] Responsive grid layouts (md: 2 columns, lg: 3 columns for drive cards)
- [x] Desktop full-width layout
- [x] Touch-friendly buttons and controls
- [x] Breakpoint-aware navigation

### 10. Performance
- [x] Route-based code splitting via Vite
- [x] Scrollable containers for large log output
- [ ] Virtualized lists for very large log datasets (future optimization)
- [ ] Debounced search inputs
- [ ] Memoized expensive computations

## Development & Build

### 11. Development Experience
- [x] Hot module replacement (HMR) working via Vite
- [x] API proxy via `--dev` flag to Vite dev server
- [x] Environment variables for API endpoint (import.meta.env.DEV)
- [x] Automatic build integration via build.rs
- [ ] ESLint configuration
- [ ] Prettier configuration

## Build & Deployment

### 12. Production Build
- [x] Optimized Vite production build
- [x] Asset minification and compression
- [x] Cache busting for static assets (Vite hashed filenames)
- [ ] Source maps for debugging
- [ ] Bundle size analysis

### 13. Integration with Rust Binary
- [x] Static files embedded via `include_dir!` macro
- [x] Proper MIME types for all assets (via mime_guess)
- [x] SPA fallback routing (all routes → index.html)
- [x] Single binary deployment with embedded UI
- [x] Automatic build via build.rs during cargo compilation
- [ ] Gzip compression for text assets
- [ ] Cache headers for static assets

## Progress Tracking

**Total Tasks**: ~75 completed / ~95 total (79%)

**Current Phase**: Core Monitoring UI Complete

**Completed**:
- ✅ **Project Setup**: Vite + React + Tailwind + Font Awesome + build automation
- ✅ **Rust Backend**:
  - ✅ SQLite database with logs and issues tables
  - ✅ Enhanced API events with drive tracking
  - ✅ New endpoints for logs/issues (8 new endpoints)
  - ✅ Static file embedding via include_dir
  - ✅ SPA fallback routing
  - ✅ --dev flag for hot reload
- ✅ **Dashboard Page** (TUI-inspired):
  - ✅ Active issues display with resolution
  - ✅ Real-time drive detection cards
  - ✅ Live log stream with color coding
  - ✅ WebSocket integration for real-time updates
- ✅ **Logs Page**:
  - ✅ SQLite database integration
  - ✅ Search and filter functionality
  - ✅ Color-coded log levels
  - ✅ Drive filtering
- ✅ **Navigation**: Simplified to Dashboard/Configuration/Logs (drives removed)
- ✅ **API Client**: Complete with all 13 endpoints
- ✅ **WebSocket**: Auto-reconnect with 9 event types

**Next Steps**: 
1. **Integrate database logging in app.rs** - Connect ripping operations to emit events that log to SQLite
2. **Background drive polling** - Add task to periodically detect drives and emit DriveDetected/DriveRemoved events
3. **Configuration page** - Build UI for managing ripley settings
4. **Auto-eject on complete** - Emit DriveEjected event after successful rip
5. **Issue creation on failures** - Automatically create issues when operations fail
6. **Toast notifications** - Add react-hot-toast for success/error messages
7. **Test with real disc ripping** - Full integration testing

## Notes & Architecture Decisions

### API & WebSocket
- All API endpoints are prefixed with `/api`
- WebSocket connects to `/api/ws`
- WebSocket events include drive field for operation tracking
- Enhanced event types for drive detection, ejection, and issue creation

### Database
- SQLite database at `~/.config/ripley/ripley.db`
- Logs table stores all operations with timestamps, levels, drives, discs
- Issues table tracks failures with resolution status
- Database initialized on API server start

### UI Philosophy
- **Automated Monitoring Only**: No manual rip start buttons - fully automated workflow
- **TUI-Inspired Dashboard**: Real-time drive detection similar to terminal UI
- **Tailwind Alerts**: All notifications use Tailwind components (no JS alerts)
- **Dark Mode Default**: Not toggleable (slate/cyan color scheme)
- **Real-time Updates**: WebSocket + polling for responsive UI

### Data Flow
- Dashboard polls drives every 3 seconds
- WebSocket provides instant updates for rip progress, logs, issues
- Logs page fetches from SQLite database (persistent history)
- Issues created automatically on failures, resolved manually via UI

## Design Reference

### Color Palette (Tailwind)
- **Background**: `bg-slate-900` (main), `bg-slate-800` (cards)
- **Text**: `text-slate-100` (primary), `text-slate-400` (secondary)
- **Accent**: `text-cyan-400`, `border-cyan-500`
- **Success**: `text-green-400`, `bg-green-500`
- **Error**: `text-red-400`, `bg-red-500`
- **Warning**: `text-yellow-400`, `bg-yellow-500`
- **Borders**: `border-slate-700`

### Typography
- **Headings**: `font-bold text-xl/2xl/3xl`
- **Body**: `font-normal text-base`
- **Monospace**: `font-mono text-sm` (for logs)

### Spacing
- **Cards**: `p-6 rounded-lg`
- **Sections**: `space-y-4`
- **Grid**: `grid gap-4 md:grid-cols-2 lg:grid-cols-3`

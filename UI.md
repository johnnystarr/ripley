# Ripley Web UI Implementation Checklist

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
- [x] `Sidebar.jsx` - Navigation menu with icons
  - [x] Dashboard link
  - [x] Drives link
  - [x] Configuration link
  - [x] Logs link
- [x] `Header.jsx` - Top bar with app title and status indicator
- [x] Responsive mobile menu (hamburger icon)

### 2. Dashboard Page (Main View)
- [x] `Dashboard.jsx` - Overview of current status
- [x] **Current Status Card**
  - [x] Display `is_ripping` status with colored badge
  - [x] Show `current_disc` name
  - [x] Show `current_title` being processed
  - [x] Progress bar with percentage (0-100%)
  - [x] Icon: FontAwesome disc/spinner when ripping
- [x] **Quick Actions Card**
  - [x] "Start Rip" button (redirects to drives)
  - [x] "Stop Rip" button (enabled only when ripping)
  - [x] "Refresh Status" button
- [x] **Recent Logs Card**
  - [x] Display last 10 log messages
  - [x] Auto-scroll to bottom
  - [x] Timestamp for each log
  - [ ] Different colors for different log types

### 3. Drives Page
- [ ] `Drives.jsx` - List of optical drives
- [ ] **Drive Cards** (one per drive)
  - [ ] Device name (e.g., `/dev/disk2`)
  - [ ] Media type badge (DVD, BluRay, Audio CD, None)
  - [ ] Icon based on media type
  - [ ] "Rip This Drive" button
- [ ] **Empty State** when no drives detected
- [ ] Auto-refresh every 5 seconds

### 4. Configuration Page
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

### 5. Logs Page
- [ ] `Logs.jsx` - Full log viewer
- [ ] Scrollable log container (virtualized for performance)
- [ ] Search/filter functionality
- [ ] "Clear Logs" button
- [ ] Export logs button (download as .txt)
- [ ] Auto-scroll toggle

### 6. Modals & Dialogs
- [ ] `StartRipModal.jsx` - Start rip configuration
  - [ ] Output path input (with folder icon)
  - [ ] TV show title input
  - [ ] Skip metadata checkbox
  - [ ] Skip filebot checkbox
  - [ ] "Start" and "Cancel" buttons
- [ ] `RenameModal.jsx` - Batch rename configuration
  - [ ] Directory input
  - [ ] Title input
  - [ ] Skip speech checkbox
  - [ ] Skip filebot checkbox
  - [ ] "Start" and "Cancel" buttons

## API Integration

### 7. API Client (`src/api.js`)
- [x] Base API client with fetch wrapper
- [x] `getHealth()` - GET /api/health
- [x] `getStatus()` - GET /api/status
- [x] `getConfig()` - GET /api/config
- [x] `updateConfig(config)` - POST /api/config
- [x] `getDrives()` - GET /api/drives
- [x] `startRip(params)` - POST /api/rip/start
- [x] `stopRip()` - POST /api/rip/stop
- [x] `renameFiles(params)` - POST /api/rename
- [ ] Error handling with toast notifications

### 8. WebSocket Integration (`src/websocket.js`)
- [x] WebSocket connection manager
- [x] Auto-reconnect on disconnect
- [x] Event handlers for all event types:
  - [x] `RipStarted` - Show notification
  - [x] `RipProgress` - Update progress bar
  - [x] `RipCompleted` - Show success notification
  - [x] `RipError` - Show notification
  - [x] `Log` - Append to log viewer
  - [x] `StatusUpdate` - Update status display
- [x] Connection status indicator in header

## State Management

### 9. React Context/Hooks
- [ ] `StatusContext.jsx` - Global rip status state
- [ ] `ConfigContext.jsx` - Global configuration state
- [ ] `LogsContext.jsx` - Global logs state
- [ ] `WebSocketContext.jsx` - WebSocket connection state
- [ ] Custom hooks:
  - [ ] `useStatus()` - Hook for status polling
  - [ ] `useDrives()` - Hook for drives polling
  - [ ] `useWebSocket()` - Hook for WebSocket events

## UI Polish & Features

### 10. Notifications & Feedback
- [ ] Toast notification system (react-hot-toast or similar)
- [ ] Loading spinners for async operations
- [ ] Skeleton loaders for initial data fetch
- [ ] Error boundaries for component errors
- [ ] Confirmation dialogs for destructive actions

### 11. Responsive Design
- [ ] Mobile-friendly layout (collapsible sidebar)
- [ ] Tablet breakpoint optimizations
- [ ] Desktop full-width layout
- [ ] Touch-friendly buttons and controls

### 12. Accessibility
- [ ] Proper ARIA labels
- [ ] Keyboard navigation support
- [ ] Focus management for modals
- [ ] Screen reader friendly
- [ ] High contrast mode support

### 13. Performance
- [ ] Code splitting for route-based chunks
- [ ] Lazy loading for heavy components
- [ ] Virtualized lists for large log output
- [ ] Debounced search inputs
- [ ] Memoized expensive computations

## Development Tools

### 14. Development Experience
- [ ] ESLint configuration
- [ ] Prettier configuration
- [ ] Hot module replacement (HMR) working
- [ ] API proxy configuration for dev mode
- [ ] Environment variables for API endpoint

## Testing & Documentation

### 15. Testing
- [ ] Unit tests for API client functions
- [ ] Component tests for key UI components
- [ ] E2E tests for critical user flows
- [ ] WebSocket event handling tests

### 16. Documentation
- [ ] README.md for web-ui development setup
- [ ] Component documentation with examples
- [ ] API client usage examples
- [ ] Deployment instructions

## Build & Deployment

### 17. Production Build
- [ ] Optimized Vite production build
- [ ] Asset minification and compression
- [ ] Source maps for debugging
- [ ] Bundle size analysis
- [ ] Cache busting for static assets

### 18. Integration with Rust Binary
- [ ] Static files embedded via `include_dir!`
- [ ] Proper MIME types for all assets
- [ ] SPA fallback routing (all routes → index.html)
- [ ] Gzip compression for text assets
- [ ] Cache headers for static assets

## Progress Tracking

**Total Tasks**: ~45 completed / 150+ total (30%)

**Current Phase**: Completing UI Pages & Features

**Completed**:
- ✅ Vite + React project initialized
- ✅ Tailwind CSS configured with dark mode
- ✅ Font Awesome installed and integrated
- ✅ App.jsx with routing and sidebar navigation
- ✅ API client layer complete
- ✅ WebSocket manager with auto-reconnect
- ✅ Dashboard page with real-time status
- ✅ Basic layout responsive and functional
- ✅ **Rust backend integrated:**
  - ✅ API routes prefixed with `/api`
  - ✅ Static files embedded via `include_dir!`
  - ✅ SPA fallback routing working
  - ✅ Build automation via `build.rs`
  - ✅ `--dev` flag for hot reload development
  - ✅ Single binary with embedded UI

**Next Steps**: 
1. Build Drives page with drive cards and rip controls
2. Implement Configuration page with all settings
3. Create Logs page with filtering and export
4. Add modals for Start Rip and Rename operations
5. Test full integration with real disc ripping

## Notes

- All API endpoints are prefixed with `/api`
- WebSocket connects to `/api/ws`
- Dark mode is default (not toggleable initially)
- Use Tailwind's built-in colors (slate, cyan, green, red, yellow)
- Font Awesome icons for all actions (play, stop, folder, cog, etc.)
- Status updates every 2 seconds via polling + WebSocket for real-time events
- Configuration saves persist to disk immediately
- Logs are kept in memory (last 1000 entries)

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

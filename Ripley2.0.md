# Ripley 2.0 - Enhancement Roadmap

## TUI Feature Parity
- [ ] Validate DVD, BluRay, CDs work still
- [ ] Multiple Disk Drives

## Frontend Enhancements

### Dashboard Improvements
- [x] Show disc title/name when available (not just disc type)
- [x] Add statistics cards (total rips, success rate, storage used)
- [x] Auto-scroll logs to bottom when new entries arrive
- [x] Add log level filtering on Dashboard (like Logs page)
- [x] Add clear logs button
- [x] Show rip duration/elapsed time for active operations

### Shows Page Improvements
- [x] Add search/filter for shows list
- [x] Add bulk operations (delete multiple shows)
- [x] Add import/export shows (JSON/CSV)
- [x] Show last used date for each show
- [x] Add sorting options (alphabetical, last used, date added)
- [x] Add pagination for large show lists

### Configuration Page Improvements
- [x] Add validation feedback for API keys (visual indicators)
- [x] Add "Test Connection" buttons for TMDB/OpenAI APIs
- [x] Group settings into collapsible sections (API Keys, Ripping, Metadata, etc.)
- [x] Add tooltips/help text explaining each setting
- [x] Add "Reset to Defaults" button
- [x] Show config file location
- [x] Add config export/import feature

### Navigation & Layout
- [x] Add breadcrumbs navigation
- [x] Improve mobile responsiveness (sidebar toggle, responsive layouts)
- [x] Add global search (search across logs, shows, issues) with Cmd/Ctrl+K shortcut
- [x] Add user preferences (logs per page, polling interval, etc.)

### Real-time Features
- [x] Add browser/desktop notifications for completed rips
- [ ] Add configurable sound notifications
- [x] Show active rip progress in browser tab title
- [ ] Add pause/resume for rip operations
- [x] Add cancel/abort operation button
- [ ] Real-time bandwidth/speed monitoring
- [ ] Show ETA for active rips

### Data Visualization
- [x] Add charts for rip history (successful vs failed over time)
- [x] Show status distribution pie chart
- [x] Show storage usage statistics with charts (cumulative growth)
- [ ] Display ripping speed/performance metrics
- [ ] Add drive usage heatmap (which drives used most)
- [ ] Show episode matching accuracy statistics
- [ ] Add timeline view of all rip operations

### Error Handling & UX
- [x] Better error messages with suggested fixes
- [x] Add retry button for failed operations
- [x] Export error logs for debugging
- [ ] Add issue resolution workflow (assign, track, notes)
- [ ] Show error frequency/patterns
- [ ] Add error categorization (drive errors, network errors, etc.)
- [ ] Contextual help based on error type

### Issues Page (New)
- [x] Create dedicated Issues page with filtering
- [x] Add issue categories and tags
- [x] Add notes/comments to issues
- [x] Show related logs for each issue
- [x] Add issue export (for bug reports)
- [ ] Track issue resolution time

## Backend Enhancements

### Database Enhancements
- [x] Add rip history table (completed rips with metadata)
- [x] Add drive statistics table (usage, errors, performance)
- [x] Add user preferences table
- [ ] Add database migrations system
- [ ] Add database backup/restore functionality
- [ ] Add full-text search for logs

### Ripping Features
- [x] Add rip history logging (saves to database on completion)
- [x] Add support for multiple simultaneous rips (multi-drive backend with per-drive tracking)
- [ ] Add rip queue management
- [ ] Add priority system for rip queue
- [ ] Add automatic retry logic with backoff
- [ ] Add checksum verification for ripped files
- [ ] Add automatic disc identification improvements
- [ ] Add custom rip profiles (quality presets)
- [ ] Add pause/resume for active rips
- [ ] Show ETA calculations for active operations

---

## Priority Levels

**High Priority (v2.0):**
- Statistics cards on Dashboard
- Shows page search/filter
- Configuration validation & test buttons
- Real-time notifications (browser/desktop)
- Better error messages with retry
- Database: rip history tracking
- Database: drive statistics
- Multi-drive simultaneous rips

**Medium Priority (v2.1):**
- Data visualization/charts
- Global search
- Dark/light mode toggle
- Rip queue management
- Database migrations system
- Automatic retry logic
- Custom rip profiles

**Low Priority (v2.2+):**
- PWA support
- Checksum verification
- Full-text log search

---

**Last Updated:** November 26, 2025  
**Version:** 2.0 Roadmap Draft

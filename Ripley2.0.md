# Ripley 2.0 - Enhancement Roadmap

## Frontend Enhancements

### Dashboard Improvements
- [ ] Show disc title/name when available (not just disc type)
- [ ] Add statistics cards (total rips, success rate, storage used)
- [ ] Auto-scroll logs to bottom when new entries arrive
- [ ] Add log level filtering on Dashboard (like Logs page)
- [ ] Add clear logs button
- [ ] Show rip duration/elapsed time for active operations

### Shows Page Improvements
- [ ] Add search/filter for shows list
- [ ] Add bulk operations (delete multiple shows)
- [ ] Add import/export shows (JSON/CSV)
- [ ] Show last used date for each show
- [ ] Add sorting options (alphabetical, last used, date added)
- [ ] Add pagination for large show lists

### Configuration Page Improvements
- [ ] Add validation feedback for API keys (visual indicators)
- [ ] Add "Test Connection" buttons for TMDB/OpenAI APIs
- [ ] Group settings into collapsible sections (API Keys, Ripping, Metadata, etc.)
- [ ] Add tooltips/help text explaining each setting
- [ ] Add "Reset to Defaults" button
- [ ] Show config file location
- [ ] Add config export/import feature

### Navigation & Layout
- [ ] Add breadcrumbs navigation
- [ ] Improve mobile responsiveness (sidebar toggle)
- [ ] Add keyboard shortcuts (Ctrl+K for search, etc.)
- [ ] Add global search (search across logs, shows, issues)
- [ ] Add dark/light mode toggle (currently dark only)
- [ ] Add user preferences (logs per page, polling interval, etc.)

### Real-time Features
- [ ] Add browser/desktop notifications for completed rips
- [ ] Add configurable sound notifications
- [ ] Show active rip progress in browser tab title
- [ ] Add pause/resume for rip operations
- [ ] Add cancel/abort operation button
- [ ] Real-time bandwidth/speed monitoring
- [ ] Show ETA for active rips

### Data Visualization
- [ ] Add charts for rip history (successful vs failed over time)
- [ ] Show storage usage statistics with charts
- [ ] Display ripping speed/performance metrics
- [ ] Add drive usage heatmap (which drives used most)
- [ ] Show episode matching accuracy statistics
- [ ] Add timeline view of all rip operations

### Error Handling & UX
- [ ] Better error messages with suggested fixes
- [ ] Add retry button for failed operations
- [ ] Export error logs for debugging
- [ ] Add issue resolution workflow (assign, track, notes)
- [ ] Show error frequency/patterns
- [ ] Add error categorization (drive errors, network errors, etc.)
- [ ] Contextual help based on error type

### Issues Page (New)
- [ ] Create dedicated Issues page with filtering
- [ ] Add issue categories and tags
- [ ] Add notes/comments to issues
- [ ] Show related logs for each issue
- [ ] Add issue export (for bug reports)
- [ ] Track issue resolution time

## Backend Enhancements

### Database Enhancements
- [ ] Add rip history table (completed rips with metadata)
- [ ] Add drive statistics table (usage, errors, performance)
- [ ] Add user preferences table
- [ ] Add database migrations system
- [ ] Add database backup/restore functionality
- [ ] Add database vacuum/optimization scheduler
- [ ] Add full-text search for logs
- [ ] Add automatic log rotation (by size/date)
- [ ] Add configurable log retention policies

### Ripping Features
- [ ] Add support for multiple simultaneous rips (multi-drive)
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
- Database backup/restore
- Full-text log search
- Auto log rotation

---

**Last Updated:** November 26, 2025  
**Version:** 2.0 Roadmap Draft

# Ripley 3.0 Enhancement Roadmap

This document tracks the major features planned for Ripley 3.0. Check off items as they are completed.

---

## 1. Monitor Tab - Real-Time Operation Monitoring

### Backend Changes

- [x] Create new API endpoint `/api/monitor/operations` to get active operations
- [x] Add operation tracking to API state (track multiple concurrent rips/operations)
- [x] Enhance WebSocket events to include operation IDs and process types
- [x] Create operation status model (operation_id, type, drive, status, progress, logs)
- [x] Add endpoint to get drive information `/api/monitor/drives`
- [x] Implement operation lifecycle management (start, update, complete, error)
- [ ] Add real-time log streaming per operation via WebSocket
- [ ] Store operation history in database for past operations view

### Frontend - Monitor Page

- [x] Create new `Monitor.jsx` page component
- [x] Add "Monitor" route to App.jsx navigation
- [x] Design layout with left panel (operations) and right panel (drive info)
- [x] Create `OperationLogWindow` component for individual operation logs (integrated into Monitor.jsx)
- [x] Create `DriveInfoPanel` component for drive status (integrated into Monitor.jsx)
- [x] Implement real-time WebSocket connection for operation updates
- [x] Add operation status indicators (running, completed, failed, paused)
- [x] Create progress bars for each active operation
- [x] Implement collapsible/expandable log windows per operation
- [x] Add filtering/sorting for operations (by type, status, drive) - status filter implemented
- [x] Create operation detail view (expand to see full logs)
- [x] Add auto-scroll to latest logs in each window
- [x] Implement log level filtering (error, warning, info) - shown in log entries
- [x] Add timestamps and drive indicators to log entries
- [x] Create empty state when no operations are active
- [ ] Add operation history view (show recently completed operations)

### Dashboard Cleanup

- [x] Remove log display section from Dashboard
- [x] Remove log-related state from Dashboard component
- [x] Remove log fetching logic from Dashboard
- [x] Update Dashboard to focus on statistics and quick actions only
- [x] Add link/navigation hint to Monitor tab for viewing logs

### Testing

- [ ] Test multiple concurrent rip operations
- [ ] Test log window spawning for new operations
- [ ] Test real-time updates via WebSocket
- [ ] Test drive information panel updates
- [ ] Test operation completion and cleanup
- [ ] Test error handling and failed operation display

---

## 2. Auto-Open Browser on `make dev`

### macOS Detection & Browser Opening

- [x] Detect macOS platform in dev script
- [x] Wait for Vite dev server to be ready (check port 5173)
- [x] Wait for Rust API server to be ready (check port 3000)
- [x] Execute `open -a "Google Chrome" http://localhost:5173` on macOS
- [x] Add optional flag to disable auto-open (e.g., `make dev NO_BROWSER=1`)
- [x] Add cross-platform support (skip browser open on non-macOS)
- [x] Log browser open attempt to console
- [x] Handle errors gracefully if Chrome is not installed

### Script Updates

- [x] Update `scripts/dev.sh` to include browser opening logic
- [x] Add delay/wait logic to ensure servers are fully started
- [x] Add health check for both servers before opening browser
- [x] Update Makefile `dev` target if needed
- [x] Add documentation comment about browser opening behavior

### Testing

- [ ] Test on macOS (Chrome installed)
- [ ] Test on macOS (Chrome not installed - should fail gracefully)
- [ ] Test on Linux (should skip browser open)
- [ ] Test with NO_BROWSER flag
- [ ] Verify browser opens to correct URL

---

## 3. GUI Agents - Windows TUI Client for Topaz Video Processing

### Backend - Agent Infrastructure

- [x] Create `Agent` database table (id, name, platform, ip, status, last_seen, capabilities)
- [x] Create agent registration API endpoint `/api/agents/register`
- [x] Create agent heartbeat API endpoint `/api/agents/heartbeat`
- [x] Create agent status API endpoint `/api/agents`
- [x] Create agent instruction queue system in database
- [x] Create instruction API endpoint `/api/agents/:id/instructions` (with auto-assignment)
- [x] Create instruction assignment endpoint (assign to next available agent)
- [x] Create instruction creation endpoint `/api/agents/instructions`
- [x] Create instruction lifecycle endpoints (start, complete, fail)
- [x] Add file upload endpoint for agent file transfers `/api/agents/upload`
- [x] Add file download endpoint for agents `/api/agents/download/:file_id`
- [x] Create Topaz profile management endpoints (CRUD)
- [x] Create profile-to-show association system
- [x] Create upscaling job queue in database
- [x] Create job status update endpoint `/api/upscaling-jobs/:job_id/status`
- [x] Implement job assignment logic (next available agent)
- [x] Create job creation and listing endpoints
- [x] Add agent capability detection (Topaz Video installed, version, etc.)
- [x] Create agent disconnection/cleanup logic
- [ ] Add agent authentication/security (optional API key)

### Backend - Upscaling Workflow Integration

- [x] Hook into rip completion workflow to queue upscaling job
- [x] Create upscaling job when DVD/BluRay rip completes
- [x] Associate upscaling job with show and profile
- [ ] Add file transfer preparation (prepare file for agent download)
- [ ] Implement job status tracking (queued, assigned, processing, completed, failed)
- [ ] Add job result reporting (output file path, processing time, etc.)
- [ ] Create job cleanup/garbage collection for old jobs
- [ ] Add job retry logic for failed upscaling jobs
- [ ] Integrate with existing rename workflow (wait for upscaling before renaming)

### Frontend - Agent Management UI

- [x] Create new `Agents.jsx` page component
- [x] Add "Agents" route to App.jsx navigation
- [x] Create agent list view showing all registered agents
- [x] Display agent status (online, offline, busy, idle)
- [x] Show agent capabilities (Topaz version, platform, etc.)
- [x] Display agent last seen timestamp
- [x] Create agent detail view (current job, queue, history)
- [x] Add agent action buttons (force disconnect, restart, etc.)
- [x] Create agent connection status indicators
- [x] Add real-time WebSocket updates for agent status
- [ ] Create Topaz profile management UI (list, create, edit, delete)
- [ ] Add profile-to-show association UI
- [ ] Create upscaling job queue view
- [ ] Display job status and progress
- [ ] Add job history view
- [ ] Create job detail view (logs, settings, output)

### Frontend - Monitor Tab Integration

- [x] Add agent operation log window type to Monitor tab
- [x] Display agent operations (upscaling, etc.) alongside rip operations
- [x] Show agent operation progress and status
- [ ] Stream agent operation logs in real-time
- [ ] Add agent operation filtering

### Windows Agent - Rust TUI Application

- [x] Create new Rust project `ripley-agent` in `agent/` directory
- [x] Set up Cargo.toml with Windows-specific dependencies
- [x] Add ratatui for TUI interface
- [x] Add tokio for async networking
- [x] Add reqwest for HTTP client
- [x] Create agent configuration system (server URL, agent name, API key)
- [x] Implement server connection logic
- [x] Create agent registration on startup
- [x] Implement heartbeat mechanism (send every 30 seconds)
- [x] Create instruction polling loop (check for new instructions)
- [x] Implement file download from server
- [x] Implement file upload to server (for completed jobs)
- [x] Create Topaz Video command wrapper/execution
- [x] Add Topaz profile loading and application
- [x] Implement upscaling job execution
- [x] Create progress reporting (send updates to server)
- [x] Add job result reporting (success/failure, output path)
- [ ] Implement error handling and retry logic
        - [x] Create TUI dashboard showing:
          - [x] Agent status (connected/disconnected)
          - [x] Current job status
          - [x] Queue position
          - [x] Job progress
          - [ ] Recent job history
          - [x] Connection status to server
        - [x] Add server URL input field in TUI (no localhost assumption)
        - [x] Implement real-time connection status display with color coding
        - [x] Add connection log showing step-by-step connection progress
        - [x] Create folder structure for agent output (processing/, upscaled/, encoded/)
        - [x] Implement configurable output location managed from web UI
        - [ ] Add TUI controls (pause, resume, disconnect)
        - [ ] Implement graceful shutdown
        - [ ] Add logging to file
        - [ ] Create Windows installer/build script

### Agent - Topaz Integration

- [x] Detect Topaz Video installation path
- [x] Verify Topaz Video executable exists
- [x] Get Topaz Video version information
- [x] Implement Topaz profile parsing (JSON/YAML)
- [x] Create Topaz command builder
- [x] Execute Topaz upscaling commands
- [ ] Parse Topaz output for progress
- [ ] Handle Topaz errors and failures
- [ ] Support multiple Topaz Video versions
- [ ] Add Topaz Video configuration validation

### Agent - File Transfer

- [ ] Implement chunked file download for large video files
- [ ] Add download progress tracking
- [ ] Implement resume capability for interrupted downloads
- [ ] Add file verification (checksum)
- [ ] Implement upload progress tracking
- [ ] Add upload retry logic
- [ ] Create temporary file cleanup
- [ ] Add disk space checking before download

### Testing

- [ ] Test agent registration and heartbeat
- [ ] Test instruction queue and assignment
- [ ] Test file download/upload
- [ ] Test Topaz command execution
- [ ] Test upscaling workflow end-to-end
- [ ] Test agent reconnection after disconnect
- [ ] Test multiple agents handling queue
- [ ] Test agent failure handling
- [ ] Test Web UI agent monitoring
- [ ] Test Monitor tab agent log display

---

## 4. GitHub Actions CI/CD Pipeline

### macOS Build Job

- [ ] Create `.github/workflows/ci.yml` file
- [ ] Set up macOS runner (macos-latest)
- [ ] Install Rust toolchain
- [ ] Cache Cargo dependencies
- [ ] Run `cargo build --release`
- [ ] Run `cargo test` for all tests
- [ ] Create macOS package/bundle (optional .dmg or .app)
- [ ] Upload build artifacts
- [ ] Create release tag workflow

### Linux Build Job

- [ ] Set up Ubuntu runner (ubuntu-latest)
- [ ] Install Rust toolchain
- [ ] Install required system dependencies (lsblk, udisks2, eject, etc.)
- [ ] Cache Cargo dependencies
- [ ] Run `cargo build --release`
- [ ] Run `cargo test` for all tests
- [ ] Run Linux-specific tests (via Docker/Podman)
- [ ] Create Linux package (Debian .deb or AppImage)
- [ ] Upload build artifacts
- [ ] Add package signing (optional)

### Windows Build Job (Agent Only)

- [ ] Set up Windows runner (windows-latest)
- [ ] Install Rust toolchain (x86_64-pc-windows-msvc)
- [ ] Cache Cargo dependencies
- [ ] Navigate to `agent/` directory
- [ ] Run `cargo build --release` for ripley-agent only
- [ ] Run `cargo test` for agent tests
- [ ] Create Windows installer (optional .msi or .exe)
- [ ] Upload build artifacts
- [ ] Add code signing (optional)

### Workflow Configuration

- [ ] Set up workflow triggers (push, pull_request, release)
- [ ] Add matrix strategy for multiple Rust versions (optional)
- [ ] Add job dependencies and ordering
- [ ] Configure artifact retention
- [ ] Add workflow status badges to README
- [ ] Set up secrets for API keys (if needed)
- [ ] Add notification on failure (optional)

### Testing

- [ ] Test workflow on push to main
- [ ] Test workflow on pull request
- [ ] Test workflow on release tag
- [ ] Verify all artifacts are created
- [ ] Verify tests run successfully
- [ ] Verify packages are valid
- [ ] Test workflow failure scenarios

---

## Summary

- **Total Features**: 4 major features
- **Total Tasks**: ~150+ detailed tasks
- **Estimated Complexity**: High (substantial new functionality)

---

## Completed This Session

- [x] Created Ripley3.0.md roadmap with comprehensive checkboxes
- [x] Organized features into logical sub-tasks
- [x] Added testing checkboxes for each feature
- [x] Implemented agent folder structure with configurable output location
- [x] Added server URL input field to agent TUI (no localhost assumption)
- [x] Implemented real-time connection status display with connection logs
- [x] Fixed all compilation errors and critical warnings
- [x] Built all binaries successfully (ripley and ripley-agent)


# Ripley 3.0 Enhancement Roadmap

This document tracks the major features planned for Ripley 3.0. Check off items as they are completed.

---

## 1. Monitor Tab - Real-Time Operation Monitoring

### Backend Changes

- [ ] Create new API endpoint `/api/monitor/operations` to get active operations
- [ ] Add operation tracking to API state (track multiple concurrent rips/operations)
- [ ] Enhance WebSocket events to include operation IDs and process types
- [ ] Create operation status model (operation_id, type, drive, status, progress, logs)
- [ ] Add endpoint to get drive information `/api/monitor/drives`
- [ ] Implement operation lifecycle management (start, update, complete, error)
- [ ] Add real-time log streaming per operation via WebSocket
- [ ] Store operation history in database for past operations view

### Frontend - Monitor Page

- [ ] Create new `Monitor.jsx` page component
- [ ] Add "Monitor" route to App.jsx navigation
- [ ] Design layout with left panel (operations) and right panel (drive info)
- [ ] Create `OperationLogWindow` component for individual operation logs
- [ ] Create `DriveInfoPanel` component for drive status
- [ ] Implement real-time WebSocket connection for operation updates
- [ ] Add operation status indicators (running, completed, failed, paused)
- [ ] Create progress bars for each active operation
- [ ] Implement collapsible/expandable log windows per operation
- [ ] Add filtering/sorting for operations (by type, status, drive)
- [ ] Create operation detail view (expand to see full logs)
- [ ] Add auto-scroll to latest logs in each window
- [ ] Implement log level filtering (error, warning, info)
- [ ] Add timestamps and drive indicators to log entries
- [ ] Create empty state when no operations are active
- [ ] Add operation history view (show recently completed operations)

### Dashboard Cleanup

- [ ] Remove log display section from Dashboard
- [ ] Remove log-related state from Dashboard component
- [ ] Remove log fetching logic from Dashboard
- [ ] Update Dashboard to focus on statistics and quick actions only
- [ ] Add link/navigation hint to Monitor tab for viewing logs

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

- [ ] Detect macOS platform in dev script
- [ ] Wait for Vite dev server to be ready (check port 5173)
- [ ] Wait for Rust API server to be ready (check port 3000)
- [ ] Execute `open -a "Google Chrome" http://localhost:5173` on macOS
- [ ] Add optional flag to disable auto-open (e.g., `make dev NO_BROWSER=1`)
- [ ] Add cross-platform support (skip browser open on non-macOS)
- [ ] Log browser open attempt to console
- [ ] Handle errors gracefully if Chrome is not installed

### Script Updates

- [ ] Update `scripts/dev.sh` to include browser opening logic
- [ ] Add delay/wait logic to ensure servers are fully started
- [ ] Add health check for both servers before opening browser
- [ ] Update Makefile `dev` target if needed
- [ ] Add documentation comment about browser opening behavior

### Testing

- [ ] Test on macOS (Chrome installed)
- [ ] Test on macOS (Chrome not installed - should fail gracefully)
- [ ] Test on Linux (should skip browser open)
- [ ] Test with NO_BROWSER flag
- [ ] Verify browser opens to correct URL

---

## 3. GUI Agents - Windows TUI Client for Topaz Video Processing

### Backend - Agent Infrastructure

- [ ] Create `Agent` database table (id, name, platform, ip, status, last_seen, capabilities)
- [ ] Create agent registration API endpoint `/api/agents/register`
- [ ] Create agent heartbeat API endpoint `/api/agents/heartbeat`
- [ ] Create agent status API endpoint `/api/agents`
- [ ] Create agent instruction queue system in database
- [ ] Create instruction API endpoint `/api/agents/:id/instructions`
- [ ] Create instruction assignment endpoint (assign to next available agent)
- [ ] Add file upload endpoint for agent file transfers `/api/agents/upload`
- [ ] Add file download endpoint for agents `/api/agents/download/:file_id`
- [ ] Create Topaz profile management endpoints (CRUD)
- [ ] Create profile-to-show association system
- [ ] Create upscaling job queue in database
- [ ] Create job status update endpoint `/api/agents/jobs/:id/status`
- [ ] Implement job assignment logic (next available agent)
- [ ] Add agent capability detection (Topaz Video installed, version, etc.)
- [ ] Create agent disconnection/cleanup logic
- [ ] Add agent authentication/security (optional API key)

### Backend - Upscaling Workflow Integration

- [ ] Hook into rip completion workflow to queue upscaling job
- [ ] Create upscaling job when DVD/BluRay rip completes
- [ ] Associate upscaling job with show and profile
- [ ] Add file transfer preparation (prepare file for agent download)
- [ ] Implement job status tracking (queued, assigned, processing, completed, failed)
- [ ] Add job result reporting (output file path, processing time, etc.)
- [ ] Create job cleanup/garbage collection for old jobs
- [ ] Add job retry logic for failed upscaling jobs
- [ ] Integrate with existing rename workflow (wait for upscaling before renaming)

### Frontend - Agent Management UI

- [ ] Create new `Agents.jsx` page component
- [ ] Add "Agents" route to App.jsx navigation
- [ ] Create agent list view showing all registered agents
- [ ] Display agent status (online, offline, busy, idle)
- [ ] Show agent capabilities (Topaz version, platform, etc.)
- [ ] Display agent last seen timestamp
- [ ] Create agent detail view (current job, queue, history)
- [ ] Add agent action buttons (force disconnect, restart, etc.)
- [ ] Create agent connection status indicators
- [ ] Add real-time WebSocket updates for agent status
- [ ] Create Topaz profile management UI (list, create, edit, delete)
- [ ] Add profile-to-show association UI
- [ ] Create upscaling job queue view
- [ ] Display job status and progress
- [ ] Add job history view
- [ ] Create job detail view (logs, settings, output)

### Frontend - Monitor Tab Integration

- [ ] Add agent operation log window type to Monitor tab
- [ ] Display agent operations (upscaling, etc.) alongside rip operations
- [ ] Show agent operation progress and status
- [ ] Stream agent operation logs in real-time
- [ ] Add agent operation filtering

### Windows Agent - Rust TUI Application

- [ ] Create new Rust project `ripley-agent` in `agent/` directory
- [ ] Set up Cargo.toml with Windows-specific dependencies
- [ ] Add ratatui for TUI interface
- [ ] Add tokio for async networking
- [ ] Add reqwest for HTTP client
- [ ] Create agent configuration system (server URL, agent name, API key)
- [ ] Implement server connection logic
- [ ] Create agent registration on startup
- [ ] Implement heartbeat mechanism (send every 30 seconds)
- [ ] Create instruction polling loop (check for new instructions)
- [ ] Implement file download from server
- [ ] Implement file upload to server (for completed jobs)
- [ ] Create Topaz Video command wrapper/execution
- [ ] Add Topaz profile loading and application
- [ ] Implement upscaling job execution
- [ ] Create progress reporting (send updates to server)
- [ ] Add job result reporting (success/failure, output path)
- [ ] Implement error handling and retry logic
- [ ] Create TUI dashboard showing:
  - [ ] Agent status (connected/disconnected)
  - [ ] Current job status
  - [ ] Queue position
  - [ ] Job progress
  - [ ] Recent job history
  - [ ] Connection status to server
- [ ] Add TUI controls (pause, resume, disconnect)
- [ ] Implement graceful shutdown
- [ ] Add logging to file
- [ ] Create Windows installer/build script

### Agent - Topaz Integration

- [ ] Detect Topaz Video installation path
- [ ] Verify Topaz Video executable exists
- [ ] Get Topaz Video version information
- [ ] Implement Topaz profile parsing (JSON/YAML)
- [ ] Create Topaz command builder
- [ ] Execute Topaz upscaling commands
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

- [ ] Created Ripley3.0.md roadmap with comprehensive checkboxes
- [ ] Organized features into logical sub-tasks
- [ ] Added testing checkboxes for each feature


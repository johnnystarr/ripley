use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use tracing::{debug, info, warn};

/// Log entry stored in database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub id: Option<i64>,
    pub timestamp: DateTime<Utc>,
    pub level: LogLevel,
    pub message: String,
    pub drive: Option<String>,
    pub disc: Option<String>,
    pub title: Option<String>,
    pub context: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Info,
    Warning,
    Error,
    Success,
}

impl LogLevel {
    fn to_string(&self) -> &str {
        match self {
            LogLevel::Info => "info",
            LogLevel::Warning => "warning",
            LogLevel::Error => "error",
            LogLevel::Success => "success",
        }
    }

    fn from_string(s: &str) -> Self {
        match s {
            "warning" => LogLevel::Warning,
            "error" => LogLevel::Error,
            "success" => LogLevel::Success,
            _ => LogLevel::Info,
        }
    }
}

/// Issue entry for tracking failures
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Issue {
    pub id: Option<i64>,
    pub timestamp: DateTime<Utc>,
    pub issue_type: IssueType,
    pub title: String,
    pub description: String,
    pub drive: Option<String>,
    pub disc: Option<String>,
    pub resolved: bool,
    pub resolved_at: Option<DateTime<Utc>>,
    pub assigned_to: Option<String>,
    pub resolution_notes: Option<String>,
}

/// Issue note/comment for tracking resolution progress
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueNote {
    pub id: Option<i64>,
    pub issue_id: i64,
    pub timestamp: DateTime<Utc>,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueType {
    RipFailure,
    MetadataFailure,
    FilebotError,
    SpeechMatchFailure,
    RsyncFailure,
    DriveError,
    Other,
}

impl IssueType {
    fn to_string(&self) -> &str {
        match self {
            IssueType::RipFailure => "rip_failure",
            IssueType::MetadataFailure => "metadata_failure",
            IssueType::FilebotError => "filebot_error",
            IssueType::SpeechMatchFailure => "speech_match_failure",
            IssueType::RsyncFailure => "rsync_failure",
            IssueType::DriveError => "drive_error",
            IssueType::Other => "other",
        }
    }

    fn from_string(s: &str) -> Self {
        match s {
            "rip_failure" => IssueType::RipFailure,
            "metadata_failure" => IssueType::MetadataFailure,
            "filebot_error" => IssueType::FilebotError,
            "speech_match_failure" => IssueType::SpeechMatchFailure,
            "rsync_failure" => IssueType::RsyncFailure,
            "drive_error" => IssueType::DriveError,
            _ => IssueType::Other,
        }
    }
}

/// Show entry for TV shows/series
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Show {
    pub id: Option<i64>,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
}

/// Rip history entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RipHistory {
    pub id: Option<i64>,
    pub timestamp: DateTime<Utc>,
    pub drive: String,
    pub disc: Option<String>,
    pub title: Option<String>,
    pub disc_type: Option<String>,
    pub status: RipStatus,
    pub duration_seconds: Option<i64>,
    pub file_size_bytes: Option<i64>,
    pub output_path: Option<String>,
    pub error_message: Option<String>,
    pub avg_speed_mbps: Option<f32>, // Average ripping speed in MB/s
    pub checksum: Option<String>, // SHA-256 checksum of ripped files
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum RipStatus {
    Success,
    Failed,
    Cancelled,
}

impl RipStatus {
    fn to_string(&self) -> &str {
        match self {
            RipStatus::Success => "success",
            RipStatus::Failed => "failed",
            RipStatus::Cancelled => "cancelled",
        }
    }

    fn from_string(s: &str) -> Self {
        match s {
            "success" => RipStatus::Success,
            "failed" => RipStatus::Failed,
            "cancelled" => RipStatus::Cancelled,
            _ => RipStatus::Failed,
        }
    }
}

/// User preferences
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPreferences {
    pub logs_per_page: i64,
    pub polling_interval_ms: i64,
    pub theme: String,
    pub sound_notifications: bool,
}

impl Default for UserPreferences {
    fn default() -> Self {
        UserPreferences {
            logs_per_page: 100,
            polling_interval_ms: 3000,
            theme: "dark".to_string(),
            sound_notifications: true,
        }
    }
}

/// Drive statistics entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriveStats {
    pub drive: String,
    pub rips_completed: i64,
    pub rips_failed: i64,
    pub total_bytes_ripped: i64,
    pub avg_speed_mbps: f64,
    pub last_used: Option<DateTime<Utc>>,
}

/// Rip queue entry for managing pending rip operations
/// Episode match result for tracking accuracy statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct EpisodeMatchResult {
    pub id: Option<i64>,
    pub timestamp: DateTime<Utc>,
    pub show_name: String,
    pub season: u32,
    pub episode: u32,
    pub episode_title: Option<String>,
    pub match_method: String, // "duration", "transcript", "manual"
    pub confidence: Option<f32>,
    pub title_index: Option<u32>,
    pub rip_history_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RipQueueEntry {
    pub id: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub drive: Option<String>, // None means any available drive
    pub output_path: Option<String>,
    pub title: Option<String>,
    pub skip_metadata: bool,
    pub skip_filebot: bool,
    pub profile: Option<String>,
    pub priority: i32, // Higher number = higher priority (default 0)
    pub status: QueueStatus,
    pub started_at: Option<DateTime<Utc>>,
}

/// Agent information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    pub id: i64,
    pub agent_id: String,
    pub name: String,
    pub platform: String,
    pub ip_address: Option<String>,
    pub status: String,
    pub last_seen: String,
    pub capabilities: Option<String>,
    pub topaz_version: Option<String>,
    pub output_location: Option<String>,
    pub created_at: String,
    pub os_version: Option<String>,
    pub os_arch: Option<String>,
}

/// Topaz Video AI profile
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopazProfile {
    pub id: Option<i64>,
    pub name: String,
    pub command: String, // Command to run for this profile
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Upscaling job entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpscalingJob {
    pub id: Option<i64>,
    pub job_id: String,
    pub input_file_path: String,
    pub output_file_path: Option<String>,
    pub show_id: Option<i64>,
    pub topaz_profile_id: Option<i64>,
    pub status: JobStatus,
    pub priority: i32,
    pub agent_id: Option<String>,
    pub instruction_id: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub assigned_at: Option<DateTime<Utc>>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub progress: f32,
    pub error_message: Option<String>,
    pub processing_time_seconds: Option<i64>,
    pub retry_count: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    Queued,
    Assigned,
    Processing,
    Completed,
    Failed,
    Cancelled,
}

impl JobStatus {
    pub fn to_string(&self) -> &str {
        match self {
            JobStatus::Queued => "queued",
            JobStatus::Assigned => "assigned",
            JobStatus::Processing => "processing",
            JobStatus::Completed => "completed",
            JobStatus::Failed => "failed",
            JobStatus::Cancelled => "cancelled",
        }
    }

    pub fn from_string(s: &str) -> Self {
        match s {
            "assigned" => JobStatus::Assigned,
            "processing" => JobStatus::Processing,
            "completed" => JobStatus::Completed,
            "failed" => JobStatus::Failed,
            "cancelled" => JobStatus::Cancelled,
            _ => JobStatus::Queued,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QueueStatus {
    Pending,
    Processing,
    Completed,
    Failed,
    Cancelled,
}

impl QueueStatus {
    fn to_string(&self) -> &str {
        match self {
            QueueStatus::Pending => "pending",
            QueueStatus::Processing => "processing",
            QueueStatus::Completed => "completed",
            QueueStatus::Failed => "failed",
            QueueStatus::Cancelled => "cancelled",
        }
    }

    fn from_string(s: &str) -> Self {
        match s {
            "processing" => QueueStatus::Processing,
            "completed" => QueueStatus::Completed,
            "failed" => QueueStatus::Failed,
            "cancelled" => QueueStatus::Cancelled,
            _ => QueueStatus::Pending,
        }
    }
}

/// Database manager for logs and issues
pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    /// Create or open the database
    pub fn new() -> Result<Self> {
        let db_path = Self::get_db_path();
        
        // Ensure directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(&db_path)?;
        info!("Opened database at {:?}", db_path);

        let db = Database {
            conn: Mutex::new(conn),
        };

        db.initialize_schema()?;
        db.run_migrations()?;
        
        // Seed initial data if tables are empty
        // IMPORTANT: Profiles must be seeded before shows so associations can be created
        let conn = db.conn.lock().unwrap();
        Self::seed_initial_topaz_profiles(&conn)?;
        Self::seed_initial_shows(&conn)?;
        drop(conn);
        
        Ok(db)
    }


    /// Initialize database schema
    fn initialize_schema(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        Self::initialize_schema_static(&conn)
    }

    /// Static version of initialize_schema for use in reset
    fn initialize_schema_static(conn: &Connection) -> Result<()> {
        // Create logs table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT NOT NULL,
                level TEXT NOT NULL,
                message TEXT NOT NULL,
                drive TEXT,
                disc TEXT,
                title TEXT,
                context TEXT
            )",
            [],
        )?;

        // Create issues table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS issues (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT NOT NULL,
                issue_type TEXT NOT NULL,
                title TEXT NOT NULL,
                description TEXT NOT NULL,
                drive TEXT,
                disc TEXT,
                resolved INTEGER NOT NULL DEFAULT 0,
                resolved_at TEXT,
                assigned_to TEXT,
                resolution_notes TEXT
            )",
            [],
        )?;

        // Create issue notes table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS issue_notes (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                issue_id INTEGER NOT NULL,
                timestamp TEXT NOT NULL,
                note TEXT NOT NULL,
                FOREIGN KEY (issue_id) REFERENCES issues(id) ON DELETE CASCADE
            )",
            [],
        )?;

        // Create settings table for persistent user preferences
        conn.execute(
            "CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
            [],
        )?;

        // Create shows table for managing show list
        conn.execute(
            "CREATE TABLE IF NOT EXISTS shows (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE,
                created_at TEXT NOT NULL,
                last_used_at TEXT
            )",
            [],
        )?;

        // Create rip history table for tracking completed rips
        conn.execute(
            "CREATE TABLE IF NOT EXISTS rip_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT NOT NULL,
                drive TEXT NOT NULL,
                disc TEXT,
                title TEXT,
                disc_type TEXT,
                status TEXT NOT NULL,
                duration_seconds INTEGER,
                file_size_bytes INTEGER,
                output_path TEXT,
                error_message TEXT,
                avg_speed_mbps REAL,
                checksum TEXT
            )",
            [],
        )?;

        // Create drive statistics table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS drive_stats (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                drive TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                rips_completed INTEGER DEFAULT 0,
                rips_failed INTEGER DEFAULT 0,
                total_bytes_ripped INTEGER DEFAULT 0,
                avg_speed_mbps REAL DEFAULT 0.0,
                last_used TEXT
            )",
            [],
        )?;

        // Create user preferences table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS user_preferences (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                logs_per_page INTEGER DEFAULT 100,
                polling_interval_ms INTEGER DEFAULT 3000,
                theme TEXT DEFAULT 'dark',
                sound_notifications INTEGER DEFAULT 1
            )",
            [],
        )?;

        // Insert default preferences if table is empty
        conn.execute(
            "INSERT OR IGNORE INTO user_preferences (id, logs_per_page, polling_interval_ms, theme, sound_notifications)
             VALUES (1, 100, 3000, 'dark', 1)",
            [],
        )?;

        // Create indexes
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_logs_timestamp ON logs(timestamp DESC)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_issues_resolved ON issues(resolved)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_issue_notes_issue_id ON issue_notes(issue_id)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_rip_history_timestamp ON rip_history(timestamp DESC)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_rip_history_status ON rip_history(status)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_drive_stats_drive ON drive_stats(drive)",
            [],
        )?;

        // Create rip queue table (created via migration)
        // Create FTS5 virtual table for full-text search (created via migration)

        debug!("Database schema initialized");
        
        Ok(())
    }

    /// Run database migrations
    pub fn run_migrations(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        Self::run_migrations_static(&conn)
    }

    /// Static version of run_migrations for use in reset
    fn run_migrations_static(conn: &Connection) -> Result<()> {
        
        // Create migrations tracking table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS migrations (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                version INTEGER NOT NULL UNIQUE,
                name TEXT NOT NULL,
                applied_at TEXT NOT NULL
            )",
            [],
        )?;

        // Get current database version (highest migration version applied)
        let current_version: i64 = conn.query_row(
            "SELECT COALESCE(MAX(version), 0) FROM migrations",
            [],
            |row| row.get(0),
        ).unwrap_or(0);

        // Migration 1: Add last_used_at to shows (if not exists)
        if current_version < 1 {
            info!("Applying migration 1: add_last_used_at_to_shows");
        let column_exists: Result<i64, _> = conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('shows') WHERE name='last_used_at'",
            [],
            |row| row.get(0),
        );
        
        if column_exists.unwrap_or(0) == 0 {
            conn.execute(
                "ALTER TABLE shows ADD COLUMN last_used_at TEXT",
                [],
            )?;
        }

            conn.execute(
                "INSERT INTO migrations (version, name, applied_at) VALUES (?1, ?2, ?3)",
                params![1, "add_last_used_at_to_shows", chrono::Utc::now().to_rfc3339()],
            )?;
        }

        // Migration 2: Add assignment fields to issues (if not exists)
        if current_version < 2 {
            info!("Applying migration 2: add_issue_assignment_fields");
            let assigned_to_exists: Result<i64, _> = conn.query_row(
                "SELECT COUNT(*) FROM pragma_table_info('issues') WHERE name='assigned_to'",
                [],
                |row| row.get(0),
            );
            
            if assigned_to_exists.unwrap_or(0) == 0 {
                conn.execute(
                    "ALTER TABLE issues ADD COLUMN assigned_to TEXT",
                    [],
                )?;
            }
            
            let resolution_notes_exists: Result<i64, _> = conn.query_row(
                "SELECT COUNT(*) FROM pragma_table_info('issues') WHERE name='resolution_notes'",
                [],
                |row| row.get(0),
            );
            
            if resolution_notes_exists.unwrap_or(0) == 0 {
                conn.execute(
                    "ALTER TABLE issues ADD COLUMN resolution_notes TEXT",
                    [],
                )?;
            }
            
            conn.execute(
                "INSERT INTO migrations (version, name, applied_at) VALUES (?1, ?2, ?3)",
                params![2, "add_issue_assignment_fields", chrono::Utc::now().to_rfc3339()],
            )?;
        }

        // Migration 3: Add checksum and avg_speed_mbps columns to rip_history (if not exists)
        if current_version < 3 {
            info!("Applying migration 3: add_checksum_and_speed_to_rip_history");
            
            // Check and add checksum column
            let checksum_exists: Result<i64, _> = conn.query_row(
                "SELECT COUNT(*) FROM pragma_table_info('rip_history') WHERE name='checksum'",
                [],
                |row| row.get(0),
            );
            
            if checksum_exists.unwrap_or(0) == 0 {
                conn.execute(
                    "ALTER TABLE rip_history ADD COLUMN checksum TEXT",
                    [],
                )?;
            }
            
            // Check and add avg_speed_mbps column
            let speed_exists: Result<i64, _> = conn.query_row(
                "SELECT COUNT(*) FROM pragma_table_info('rip_history') WHERE name='avg_speed_mbps'",
                [],
                |row| row.get(0),
            );
            
            if speed_exists.unwrap_or(0) == 0 {
                conn.execute(
                    "ALTER TABLE rip_history ADD COLUMN avg_speed_mbps REAL",
                    [],
                )?;
            }
            
            conn.execute(
                "INSERT INTO migrations (version, name, applied_at) VALUES (?1, ?2, ?3)",
                params![3, "add_checksum_and_speed_to_rip_history", chrono::Utc::now().to_rfc3339()],
            )?;
        }

        // Migration 4: Add FTS5 full-text search for logs
        if current_version < 4 {
            info!("Applying migration 4: add_fts5_for_logs");
            
            // Check if FTS5 table exists
            let fts_exists: Result<i64, _> = conn.query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='logs_fts'",
                [],
                |row| row.get(0),
            );
            
            if fts_exists.unwrap_or(0) == 0 {
                // Create FTS5 virtual table
                conn.execute(
                    "CREATE VIRTUAL TABLE logs_fts USING fts5(
                        id UNINDEXED,
                        message,
                        drive,
                        disc,
                        title,
                        context,
                        content='logs',
                        content_rowid='id'
                    )",
                    [],
                )?;
                
                // Create triggers to keep FTS5 table in sync with logs table
                conn.execute(
                    "CREATE TRIGGER logs_fts_insert AFTER INSERT ON logs BEGIN
                        INSERT INTO logs_fts(rowid, message, drive, disc, title, context)
                        VALUES (new.id, new.message, new.drive, new.disc, new.title, new.context);
                    END",
                    [],
                )?;
                
                conn.execute(
                    "CREATE TRIGGER logs_fts_delete AFTER DELETE ON logs BEGIN
                        INSERT INTO logs_fts(logs_fts, rowid, message, drive, disc, title, context)
                        VALUES ('delete', old.id, old.message, old.drive, old.disc, old.title, old.context);
                    END",
                    [],
                )?;
                
                conn.execute(
                    "CREATE TRIGGER logs_fts_update AFTER UPDATE ON logs BEGIN
                        INSERT INTO logs_fts(logs_fts, rowid, message, drive, disc, title, context)
                        VALUES ('delete', old.id, old.message, old.drive, old.disc, old.title, old.context);
                        INSERT INTO logs_fts(rowid, message, drive, disc, title, context)
                        VALUES (new.id, new.message, new.drive, new.disc, new.title, new.context);
                    END",
                    [],
                )?;
                
                // Populate FTS5 table with existing logs
                info!("Populating FTS5 index with existing logs...");
                conn.execute(
                    "INSERT INTO logs_fts(rowid, message, drive, disc, title, context)
                     SELECT id, message, drive, disc, title, context FROM logs",
                    [],
                )?;
                
                info!("FTS5 index populated with {} log entries", conn.changes());
            }
            
            conn.execute(
                "INSERT INTO migrations (version, name, applied_at) VALUES (?1, ?2, ?3)",
                params![4, "add_fts5_for_logs", chrono::Utc::now().to_rfc3339()],
            )?;
        }

        // Migration 5: Add rip queue table
        if current_version < 5 {
            info!("Applying migration 5: add_rip_queue_table");
            
            let queue_exists: Result<i64, _> = conn.query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='rip_queue'",
                [],
                |row| row.get(0),
            );
            
            if queue_exists.unwrap_or(0) == 0 {
                conn.execute(
                    "CREATE TABLE rip_queue (
                        id INTEGER PRIMARY KEY AUTOINCREMENT,
                        created_at TEXT NOT NULL,
                        drive TEXT,
                        output_path TEXT,
                        title TEXT,
                        skip_metadata INTEGER NOT NULL DEFAULT 0,
                        skip_filebot INTEGER NOT NULL DEFAULT 0,
                        profile TEXT,
                        priority INTEGER NOT NULL DEFAULT 0,
                        status TEXT NOT NULL DEFAULT 'pending',
                        started_at TEXT
                    )",
                    [],
                )?;
                
                conn.execute(
                    "CREATE INDEX IF NOT EXISTS idx_rip_queue_status ON rip_queue(status, priority DESC, created_at ASC)",
                    [],
                )?;
            }
            
            conn.execute(
                "INSERT INTO migrations (version, name, applied_at) VALUES (?1, ?2, ?3)",
                params![5, "add_rip_queue_table", chrono::Utc::now().to_rfc3339()],
            )?;
        }

        // Migration 6: Add episode match results table
        if current_version < 6 {
            info!("Applying migration 6: add_episode_match_results_table");
            
            let table_exists: Result<i64, _> = conn.query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='episode_match_results'",
                [],
                |row| row.get(0),
            );
            
            if table_exists.unwrap_or(0) == 0 {
                conn.execute(
                    "CREATE TABLE episode_match_results (
                        id INTEGER PRIMARY KEY AUTOINCREMENT,
                        timestamp TEXT NOT NULL,
                        show_name TEXT NOT NULL,
                        season INTEGER NOT NULL,
                        episode INTEGER NOT NULL,
                        episode_title TEXT,
                        match_method TEXT NOT NULL,
                        confidence REAL,
                        title_index INTEGER,
                        rip_history_id INTEGER,
                        FOREIGN KEY (rip_history_id) REFERENCES rip_history(id) ON DELETE SET NULL
                    )",
                    [],
                )?;
                
                conn.execute(
                    "CREATE INDEX IF NOT EXISTS idx_episode_match_show ON episode_match_results(show_name)",
                    [],
                )?;
                
                conn.execute(
                    "CREATE INDEX IF NOT EXISTS idx_episode_match_method ON episode_match_results(match_method)",
                    [],
                )?;
                
                conn.execute(
                    "CREATE INDEX IF NOT EXISTS idx_episode_match_timestamp ON episode_match_results(timestamp DESC)",
                    [],
                )?;
            }
            
            conn.execute(
                "INSERT INTO migrations (version, name, applied_at) VALUES (?1, ?2, ?3)",
                params![6, "add_episode_match_results_table", chrono::Utc::now().to_rfc3339()],
            )?;
        }

        // Migration 7: Add agent infrastructure tables
        if current_version < 7 {
            info!("Applying migration 7: add_agent_infrastructure");
            
            // Create agents table
            let agents_exists: Result<i64, _> = conn.query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='agents'",
                [],
                |row| row.get(0),
            );
            
            if agents_exists.unwrap_or(0) == 0 {
                conn.execute(
                    "CREATE TABLE agents (
                        id INTEGER PRIMARY KEY AUTOINCREMENT,
                        agent_id TEXT NOT NULL UNIQUE,
                        name TEXT NOT NULL,
                        platform TEXT NOT NULL,
                        ip_address TEXT,
                        status TEXT NOT NULL DEFAULT 'offline',
                        last_seen TEXT NOT NULL,
                        capabilities TEXT,
                        topaz_version TEXT,
                        api_key TEXT,
                        created_at TEXT NOT NULL
                    )",
                    [],
                )?;
                
                conn.execute(
                    "CREATE INDEX IF NOT EXISTS idx_agents_status ON agents(status)",
                    [],
                )?;
                
                conn.execute(
                    "CREATE INDEX IF NOT EXISTS idx_agents_last_seen ON agents(last_seen DESC)",
                    [],
                )?;
            }
            
            // Create agent_instructions table (instruction queue)
            let instructions_exists: Result<i64, _> = conn.query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='agent_instructions'",
                [],
                |row| row.get(0),
            );
            
            if instructions_exists.unwrap_or(0) == 0 {
                conn.execute(
                    "CREATE TABLE agent_instructions (
                        id INTEGER PRIMARY KEY AUTOINCREMENT,
                        instruction_type TEXT NOT NULL,
                        payload TEXT NOT NULL,
                        status TEXT NOT NULL DEFAULT 'pending',
                        assigned_to_agent_id TEXT,
                        created_at TEXT NOT NULL,
                        assigned_at TEXT,
                        started_at TEXT,
                        completed_at TEXT,
                        error_message TEXT,
                        FOREIGN KEY (assigned_to_agent_id) REFERENCES agents(agent_id) ON DELETE SET NULL
                    )",
                    [],
                )?;
                
                conn.execute(
                    "CREATE INDEX IF NOT EXISTS idx_agent_instructions_status ON agent_instructions(status, created_at ASC)",
                    [],
                )?;
                
                conn.execute(
                    "CREATE INDEX IF NOT EXISTS idx_agent_instructions_agent ON agent_instructions(assigned_to_agent_id)",
                    [],
                )?;
            }
            
            // Create upscaling_jobs table
            let jobs_exists: Result<i64, _> = conn.query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='upscaling_jobs'",
                [],
                |row| row.get(0),
            );
            
            if jobs_exists.unwrap_or(0) == 0 {
                conn.execute(
                    "CREATE TABLE upscaling_jobs (
                        id INTEGER PRIMARY KEY AUTOINCREMENT,
                        job_id TEXT NOT NULL UNIQUE,
                        input_file_path TEXT NOT NULL,
                        output_file_path TEXT,
                        show_id INTEGER,
                        topaz_profile_id INTEGER,
                        status TEXT NOT NULL DEFAULT 'queued',
                        priority INTEGER NOT NULL DEFAULT 0,
                        agent_id TEXT,
                        instruction_id INTEGER,
                        created_at TEXT NOT NULL,
                        assigned_at TEXT,
                        started_at TEXT,
                        completed_at TEXT,
                        progress REAL DEFAULT 0.0,
                        error_message TEXT,
                        processing_time_seconds INTEGER,
                        FOREIGN KEY (show_id) REFERENCES shows(id) ON DELETE SET NULL,
                        FOREIGN KEY (topaz_profile_id) REFERENCES topaz_profiles(id) ON DELETE SET NULL,
                        FOREIGN KEY (agent_id) REFERENCES agents(agent_id) ON DELETE SET NULL,
                        FOREIGN KEY (instruction_id) REFERENCES agent_instructions(id) ON DELETE SET NULL
                    )",
                    [],
                )?;
                
                conn.execute(
                    "CREATE INDEX IF NOT EXISTS idx_upscaling_jobs_status ON upscaling_jobs(status, priority DESC, created_at ASC)",
                    [],
                )?;
                
                conn.execute(
                    "CREATE INDEX IF NOT EXISTS idx_upscaling_jobs_show ON upscaling_jobs(show_id)",
                    [],
                )?;
            }
            
            // Create topaz_profiles table
            let profiles_exists: Result<i64, _> = conn.query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='topaz_profiles'",
                [],
                |row| row.get(0),
            );
            
            if profiles_exists.unwrap_or(0) == 0 {
                conn.execute(
                    "CREATE TABLE topaz_profiles (
                        id INTEGER PRIMARY KEY AUTOINCREMENT,
                        name TEXT NOT NULL UNIQUE,
                        command TEXT NOT NULL,
                        created_at TEXT NOT NULL,
                        updated_at TEXT NOT NULL
                    )",
                    [],
                )?;
            }
            
            // Create show_topaz_profiles table (many-to-many association)
            let show_profiles_exists: Result<i64, _> = conn.query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='show_topaz_profiles'",
                [],
                |row| row.get(0),
            );
            
            if show_profiles_exists.unwrap_or(0) == 0 {
                conn.execute(
                    "CREATE TABLE show_topaz_profiles (
                        show_id INTEGER NOT NULL,
                        topaz_profile_id INTEGER NOT NULL,
                        PRIMARY KEY (show_id, topaz_profile_id),
                        FOREIGN KEY (show_id) REFERENCES shows(id) ON DELETE CASCADE,
                        FOREIGN KEY (topaz_profile_id) REFERENCES topaz_profiles(id) ON DELETE CASCADE
                    )",
                    [],
                )?;
            }
            
            // Create agent_file_transfers table (for tracking file uploads/downloads)
            let transfers_exists: Result<i64, _> = conn.query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='agent_file_transfers'",
                [],
                |row| row.get(0),
            );
            
            if transfers_exists.unwrap_or(0) == 0 {
                conn.execute(
                    "CREATE TABLE agent_file_transfers (
                        id INTEGER PRIMARY KEY AUTOINCREMENT,
                        transfer_id TEXT NOT NULL UNIQUE,
                        file_path TEXT NOT NULL,
                        transfer_type TEXT NOT NULL,
                        direction TEXT NOT NULL,
                        agent_id TEXT,
                        job_id TEXT,
                        status TEXT NOT NULL DEFAULT 'pending',
                        size_bytes INTEGER,
                        created_at TEXT NOT NULL,
                        started_at TEXT,
                        completed_at TEXT,
                        error_message TEXT,
                        FOREIGN KEY (agent_id) REFERENCES agents(agent_id) ON DELETE SET NULL
                    )",
                    [],
                )?;
                
                conn.execute(
                    "CREATE INDEX IF NOT EXISTS idx_file_transfers_status ON agent_file_transfers(status)",
                    [],
                )?;
            }
            
            conn.execute(
                "INSERT INTO migrations (version, name, applied_at) VALUES (?1, ?2, ?3)",
                params![7, "add_agent_infrastructure", chrono::Utc::now().to_rfc3339()],
            )?;
        }

        // Migration 8: Add output_location to agents table
        if current_version < 8 {
            info!("Applying migration 8: add_agent_output_location");
            
            let column_exists: Result<i64, _> = conn.query_row(
                "SELECT COUNT(*) FROM pragma_table_info('agents') WHERE name='output_location'",
                [],
                |row| row.get(0),
            );
            
            if column_exists.unwrap_or(0) == 0 {
                conn.execute(
                    "ALTER TABLE agents ADD COLUMN output_location TEXT",
                    [],
                )?;
            }
            
            conn.execute(
                "INSERT INTO migrations (version, name, applied_at) VALUES (?1, ?2, ?3)",
                params![8, "add_agent_output_location", chrono::Utc::now().to_rfc3339()],
            )?;
        }

        // Migration 9: Add operation_history table
        if current_version < 9 {
            info!("Applying migration 9: add_operation_history_table");
            
            let table_exists: Result<i64, _> = conn.query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='operation_history'",
                [],
                |row| row.get(0),
            );
            
            if table_exists.unwrap_or(0) == 0 {
                conn.execute(
                    "CREATE TABLE operation_history (
                        id INTEGER PRIMARY KEY AUTOINCREMENT,
                        operation_id TEXT NOT NULL UNIQUE,
                        operation_type TEXT NOT NULL,
                        status TEXT NOT NULL,
                        drive TEXT,
                        title TEXT,
                        progress REAL NOT NULL DEFAULT 0.0,
                        message TEXT NOT NULL,
                        started_at TEXT NOT NULL,
                        completed_at TEXT,
                        error TEXT,
                        created_at TEXT NOT NULL
                    )",
                    [],
                )?;
                
                conn.execute(
                    "CREATE INDEX IF NOT EXISTS idx_operation_history_status ON operation_history(status, completed_at DESC)",
                    [],
                )?;
                
                conn.execute(
                    "CREATE INDEX IF NOT EXISTS idx_operation_history_type ON operation_history(operation_type, completed_at DESC)",
                    [],
                )?;
            }
            
            conn.execute(
                "INSERT INTO migrations (version, name, applied_at) VALUES (?1, ?2, ?3)",
                params![9, "add_operation_history_table", chrono::Utc::now().to_rfc3339()],
            )?;
        }

        // Migration 10: Add retry_count to upscaling_jobs
        if current_version < 10 {
            info!("Applying migration 10: add_retry_count_to_upscaling_jobs");
            
            let column_exists: Result<i64, _> = conn.query_row(
                "SELECT COUNT(*) FROM pragma_table_info('upscaling_jobs') WHERE name='retry_count'",
                [],
                |row| row.get(0),
            );
            
            if column_exists.unwrap_or(0) == 0 {
                conn.execute(
                    "ALTER TABLE upscaling_jobs ADD COLUMN retry_count INTEGER NOT NULL DEFAULT 0",
                    [],
                )?;
            }
            
            conn.execute(
                "INSERT INTO migrations (version, name, applied_at) VALUES (?1, ?2, ?3)",
                params![10, "add_retry_count_to_upscaling_jobs", chrono::Utc::now().to_rfc3339()],
            )?;
        }

        // Migration 11: Add OS details to agents table
        if current_version < 11 {
            info!("Applying migration 11: add_os_details_to_agents");
            
            let os_version_exists: Result<i64, _> = conn.query_row(
                "SELECT COUNT(*) FROM pragma_table_info('agents') WHERE name='os_version'",
                [],
                |row| row.get(0),
            );
            
            if os_version_exists.unwrap_or(0) == 0 {
                conn.execute(
                    "ALTER TABLE agents ADD COLUMN os_version TEXT",
                    [],
                )?;
            }
            
            let os_arch_exists: Result<i64, _> = conn.query_row(
                "SELECT COUNT(*) FROM pragma_table_info('agents') WHERE name='os_arch'",
                [],
                |row| row.get(0),
            );
            
            if os_arch_exists.unwrap_or(0) == 0 {
                conn.execute(
                    "ALTER TABLE agents ADD COLUMN os_arch TEXT",
                    [],
                )?;
            }
            
            conn.execute(
                "INSERT INTO migrations (version, name, applied_at) VALUES (?1, ?2, ?3)",
                params![11, "add_os_details_to_agents", chrono::Utc::now().to_rfc3339()],
            )?;
        }

        // Migration 12: Add output column to agent_instructions table
        if current_version < 12 {
            info!("Applying migration 12: add_output_to_agent_instructions");
            
            let output_exists: Result<i64, _> = conn.query_row(
                "SELECT COUNT(*) FROM pragma_table_info('agent_instructions') WHERE name='output'",
                [],
                |row| row.get(0),
            );
            
            if output_exists.unwrap_or(0) == 0 {
                conn.execute(
                    "ALTER TABLE agent_instructions ADD COLUMN output TEXT",
                    [],
                )?;
            }
            
            conn.execute(
                "INSERT INTO migrations (version, name, applied_at) VALUES (?1, ?2, ?3)",
                params![12, "add_output_to_agent_instructions", chrono::Utc::now().to_rfc3339()],
            )?;
        }

        Ok(())
    }

    /// Record an episode match result for statistics tracking
    #[allow(dead_code)]
    pub fn record_episode_match(&self, match_result: &EpisodeMatchResult) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        
        conn.execute(
            "INSERT INTO episode_match_results (timestamp, show_name, season, episode, episode_title, match_method, confidence, title_index, rip_history_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                match_result.timestamp.to_rfc3339(),
                match_result.show_name,
                match_result.season as i64,
                match_result.episode as i64,
                match_result.episode_title,
                match_result.match_method,
                match_result.confidence,
                match_result.title_index.map(|t| t as i64),
                match_result.rip_history_id,
            ],
        )?;

        Ok(conn.last_insert_rowid())
    }

    /// Get episode matching statistics
    pub fn get_episode_match_statistics(&self) -> Result<serde_json::Value> {
        let conn = self.conn.lock().unwrap();
        
        // Check if episode_match_results table exists
        let table_exists: bool = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='episode_match_results'",
            [],
            |row| Ok(row.get::<_, i64>(0)? > 0),
        ).unwrap_or(false);
        
        if !table_exists {
            return Ok(serde_json::json!({
                "total_matches": 0,
                "average_confidence": None::<f64>,
                "by_method": {},
                "confidence_distribution": {},
                "over_time": [],
                "top_shows": [],
            }));
        }
        
        // Total matches
        let total_matches: i64 = conn.query_row(
            "SELECT COUNT(*) FROM episode_match_results",
            [],
            |row| row.get(0),
        )?;
        
        // Average confidence
        let avg_confidence: Option<f64> = conn.query_row(
            "SELECT AVG(confidence) FROM episode_match_results WHERE confidence IS NOT NULL",
            [],
            |row| row.get(0),
        ).ok();
        
        // Matches by method
        let mut method_stats = std::collections::HashMap::new();
        let mut stmt = conn.prepare(
            "SELECT match_method, COUNT(*) as count, AVG(confidence) as avg_conf
             FROM episode_match_results
             GROUP BY match_method"
        )?;
        
        let method_rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, Option<f64>>(2)?,
            ))
        })?;
        
        for row in method_rows {
            let (method, count, avg_conf) = row?;
            method_stats.insert(method, serde_json::json!({
                "count": count,
                "avg_confidence": avg_conf,
            }));
        }
        
        // Confidence distribution (grouped into buckets)
        let mut confidence_dist = std::collections::HashMap::new();
        let buckets = vec![(0.0, 50.0), (50.0, 70.0), (70.0, 85.0), (85.0, 95.0), (95.0, 100.0)];
        
        for (min, max) in buckets {
            let bucket_label = format!("{:.0}-{:.0}", min, max);
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM episode_match_results WHERE confidence >= ?1 AND confidence < ?2",
                params![min, max],
                |row| row.get(0),
            ).unwrap_or(0);
            
            confidence_dist.insert(bucket_label, count);
        }
        
        // Matches over time (last 30 days)
        let thirty_days_ago = chrono::Utc::now() - chrono::Duration::days(30);
        let mut time_series = Vec::new();
        let mut stmt = conn.prepare(
            "SELECT DATE(timestamp) as date, COUNT(*) as count, AVG(confidence) as avg_conf
             FROM episode_match_results
             WHERE timestamp >= ?1
             GROUP BY DATE(timestamp)
             ORDER BY date ASC"
        )?;
        
        let time_rows = stmt.query_map([thirty_days_ago.to_rfc3339()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, Option<f64>>(2)?,
            ))
        })?;
        
        for row in time_rows {
            let (date, count, avg_conf) = row?;
            time_series.push(serde_json::json!({
                "date": date,
                "count": count,
                "avg_confidence": avg_conf,
            }));
        }
        
        // Top shows by match count
        let mut top_shows = Vec::new();
        let mut stmt = conn.prepare(
            "SELECT show_name, COUNT(*) as count, AVG(confidence) as avg_conf
             FROM episode_match_results
             GROUP BY show_name
             ORDER BY count DESC
             LIMIT 10"
        )?;
        
        let show_rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, Option<f64>>(2)?,
            ))
        })?;
        
        for row in show_rows {
            let (show, count, avg_conf) = row?;
            top_shows.push(serde_json::json!({
                "show_name": show,
                "count": count,
                "avg_confidence": avg_conf,
            }));
        }
        
        Ok(serde_json::json!({
            "total_matches": total_matches,
            "average_confidence": avg_confidence,
            "by_method": method_stats,
            "confidence_distribution": confidence_dist,
            "over_time": time_series,
            "top_shows": top_shows,
        }))
    }

    /// Backup database to a file
    pub fn backup_database(&self, backup_path: &std::path::Path) -> Result<()> {
        let db_path = Self::get_db_path();
        
        if !db_path.exists() {
            return Err(anyhow::anyhow!("Database file does not exist"));
        }
        
        // Ensure backup directory exists
        if let Some(parent) = backup_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        // Copy database file
        std::fs::copy(&db_path, backup_path)?;
        info!("Database backed up to {}", backup_path.display());
        
        Ok(())
    }

    /// Restore database from a backup file
    pub fn restore_database(&self, backup_path: &std::path::Path) -> Result<()> {
        if !backup_path.exists() {
            return Err(anyhow::anyhow!("Backup file does not exist"));
        }
        
        let db_path = Self::get_db_path();
        
        // Close current connection
        drop(self.conn.lock().unwrap());
        
        // Backup current database if it exists
        if db_path.exists() {
            let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
            let auto_backup = db_path.with_file_name(format!("ripley.db.backup.{}", timestamp));
            std::fs::copy(&db_path, &auto_backup)?;
            info!("Current database backed up to {}", auto_backup.display());
        }
        
        // Ensure directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        // Copy backup over current database
        std::fs::copy(backup_path, &db_path)?;
        info!("Database restored from {}", backup_path.display());
        
        // Reopen connection (will happen on next access)
        Ok(())
    }

    // Agent methods

    /// Register or update an agent
    pub fn register_agent(
        &self,
        agent_id: &str,
        name: &str,
        platform: &str,
        ip_address: Option<&str>,
        capabilities: Option<&str>,
        topaz_version: Option<&str>,
        api_key: Option<&str>,
        os_version: Option<&str>,
        os_arch: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();

        // Check if agent exists
        let exists: bool = conn.query_row(
            "SELECT COUNT(*) FROM agents WHERE agent_id = ?1",
            params![agent_id],
            |row| Ok(row.get::<_, i64>(0)? > 0),
        )?;

        if exists {
            // Update existing agent
            conn.execute(
                "UPDATE agents SET name = ?1, platform = ?2, ip_address = ?3, status = 'online', 
                 last_seen = ?4, capabilities = ?5, topaz_version = ?6, api_key = ?7, os_version = ?8, os_arch = ?9
                 WHERE agent_id = ?10",
                params![
                    name,
                    platform,
                    ip_address,
                    now,
                    capabilities,
                    topaz_version,
                    api_key,
                    os_version,
                    os_arch,
                    agent_id
                ],
            )?;
        } else {
            // Insert new agent
            conn.execute(
                "INSERT INTO agents (agent_id, name, platform, ip_address, status, last_seen, capabilities, topaz_version, api_key, os_version, os_arch, created_at)
                 VALUES (?1, ?2, ?3, ?4, 'online', ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![agent_id, name, platform, ip_address, now, capabilities, topaz_version, api_key, os_version, os_arch, now],
            )?;
        }

        Ok(())
    }

    /// Update agent heartbeat (last_seen timestamp)
    pub fn update_agent_heartbeat(&self, agent_id: &str, status: Option<&str>) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();

        if let Some(status_str) = status {
            conn.execute(
                "UPDATE agents SET last_seen = ?1, status = ?2 WHERE agent_id = ?3",
                params![now, status_str, agent_id],
            )?;
        } else {
            conn.execute(
                "UPDATE agents SET last_seen = ?1, status = 'online' WHERE agent_id = ?2",
                params![now, agent_id],
            )?;
        }

        Ok(())
    }

    /// Get all agents
    pub fn get_agents(&self) -> Result<Vec<AgentInfo>> {
        let conn = self.conn.lock().unwrap();
        
        // Check if agents table exists
        let table_exists: bool = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='agents'",
            [],
            |row| Ok(row.get::<_, i64>(0)? > 0),
        ).unwrap_or(false);
        
        if !table_exists {
            return Ok(vec![]); // Table doesn't exist yet, return empty list
        }
        
        let mut stmt = conn.prepare(
            "SELECT id, agent_id, name, platform, ip_address, status, last_seen, capabilities, topaz_version, output_location, created_at, os_version, os_arch
             FROM agents
             ORDER BY last_seen DESC"
        )?;

        let agents = stmt.query_map([], |row| {
            Ok(AgentInfo {
                id: row.get(0)?,
                agent_id: row.get(1)?,
                name: row.get(2)?,
                platform: row.get(3)?,
                ip_address: row.get(4)?,
                status: row.get(5)?,
                last_seen: row.get(6)?,
                capabilities: row.get(7)?,
                topaz_version: row.get(8)?,
                output_location: row.get(9)?,
                created_at: row.get(10)?,
                os_version: row.get(11)?,
                os_arch: row.get(12)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

        Ok(agents)
    }

    /// Get agent by agent_id
    pub fn get_agent_by_id(&self, agent_id: &str) -> Result<Option<AgentInfo>> {
        let conn = self.conn.lock().unwrap();
        
        let mut stmt = conn.prepare(
            "SELECT id, agent_id, name, platform, ip_address, status, last_seen, capabilities, topaz_version, output_location, created_at, os_version, os_arch
             FROM agents
             WHERE agent_id = ?1"
        )?;

        let agent = match stmt.query_row(params![agent_id], |row| {
            Ok(AgentInfo {
                id: row.get(0)?,
                agent_id: row.get(1)?,
                name: row.get(2)?,
                platform: row.get(3)?,
                ip_address: row.get(4)?,
                status: row.get(5)?,
                last_seen: row.get(6)?,
                capabilities: row.get(7)?,
                topaz_version: row.get(8)?,
                output_location: row.get(9)?,
                created_at: row.get(10)?,
                os_version: row.get(11)?,
                os_arch: row.get(12)?,
            })
        }) {
            Ok(agent) => Some(agent),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(e) => return Err(anyhow::anyhow!("Database error: {}", e)),
        };

        Ok(agent)
    }

    /// Delete an agent
    pub fn delete_agent(&self, agent_id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        
        conn.execute(
            "DELETE FROM agents WHERE agent_id = ?1",
            params![agent_id],
        )?;
        
        Ok(())
    }

    /// Update agent output location
    pub fn update_agent_output_location(&self, agent_id: &str, output_location: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        
        conn.execute(
            "UPDATE agents SET output_location = ?1 WHERE agent_id = ?2",
            params![output_location, agent_id],
        )?;
        
        Ok(())
    }

    /// Get recent completed instructions for an agent
    pub fn get_recent_completed_instructions(&self, agent_id: &str, limit: i64) -> Result<Vec<serde_json::Value>> {
        let conn = self.conn.lock().unwrap();
        
        // Check if agent_instructions table exists
        let table_exists: bool = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='agent_instructions'",
            [],
            |row| Ok(row.get::<_, i64>(0)? > 0),
        ).unwrap_or(false);
        
        if !table_exists {
            return Ok(vec![]); // Table doesn't exist yet, return empty list
        }
        
        // Check if output column exists (migration 12)
        let output_column_exists: bool = conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('agent_instructions') WHERE name='output'",
            [],
            |row| Ok(row.get::<_, i64>(0)? > 0),
        ).unwrap_or(false);
        
        let query = if output_column_exists {
            "SELECT id, instruction_type, payload, status, assigned_to_agent_id, created_at, started_at, completed_at, error_message, output
             FROM agent_instructions
             WHERE assigned_to_agent_id = ?1 AND (status = 'completed' OR status = 'failed')
             ORDER BY COALESCE(completed_at, created_at) DESC
             LIMIT ?2"
        } else {
            "SELECT id, instruction_type, payload, status, assigned_to_agent_id, created_at, started_at, completed_at, error_message, NULL as output
             FROM agent_instructions
             WHERE assigned_to_agent_id = ?1 AND (status = 'completed' OR status = 'failed')
             ORDER BY COALESCE(completed_at, created_at) DESC
             LIMIT ?2"
        };
        
        let mut stmt = conn.prepare(query)?;
        
        let instructions = stmt.query_map(params![agent_id, limit], |row| {
            let payload_str: String = row.get(2)?;
            let payload: serde_json::Value = serde_json::from_str(&payload_str)
                .unwrap_or_else(|_| serde_json::json!({}));
            
            // Handle output column - may not exist in older databases
            let output: Option<String> = if output_column_exists {
                row.get(9).ok()
            } else {
                None
            };
            
            Ok(serde_json::json!({
                "id": row.get::<_, i64>(0)?,
                "instruction_type": row.get::<_, String>(1)?,
                "payload": payload,
                "status": row.get::<_, String>(3)?,
                "assigned_to_agent_id": row.get::<_, Option<String>>(4)?,
                "created_at": row.get::<_, String>(5)?,
                "started_at": row.get::<_, Option<String>>(6)?,
                "completed_at": row.get::<_, Option<String>>(7)?,
                "error_message": row.get::<_, Option<String>>(8)?,
                "output": output,
            }))
        })?
        .collect::<Result<Vec<_>, _>>()?;
        
        Ok(instructions)
    }

    /// Get instruction by ID
    pub fn get_instruction(&self, instruction_id: i64) -> Result<Option<serde_json::Value>> {
        let conn = self.conn.lock().unwrap();
        
        // Check if output column exists (migration 12)
        let output_column_exists: bool = conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('agent_instructions') WHERE name='output'",
            [],
            |row| Ok(row.get::<_, i64>(0)? > 0),
        ).unwrap_or(false);
        
        let query = if output_column_exists {
            "SELECT id, instruction_type, payload, status, assigned_to_agent_id, created_at, started_at, completed_at, error_message, output
             FROM agent_instructions
             WHERE id = ?1"
        } else {
            "SELECT id, instruction_type, payload, status, assigned_to_agent_id, created_at, started_at, completed_at, error_message, NULL as output
             FROM agent_instructions
             WHERE id = ?1"
        };
        
        let mut stmt = conn.prepare(query)?;
        
        let instruction = match stmt.query_row(params![instruction_id], |row| {
            let payload_str: String = row.get(2)?;
            let payload: serde_json::Value = serde_json::from_str(&payload_str)
                .unwrap_or_else(|_| serde_json::json!({}));
            
            // Handle output column - may not exist in older databases
            let output: Option<String> = if output_column_exists {
                row.get(9).ok()
            } else {
                None
            };
            
            Ok(serde_json::json!({
                "id": row.get::<_, i64>(0)?,
                "instruction_type": row.get::<_, String>(1)?,
                "payload": payload,
                "status": row.get::<_, String>(3)?,
                "assigned_to_agent_id": row.get::<_, Option<String>>(4)?,
                "created_at": row.get::<_, String>(5)?,
                "started_at": row.get::<_, Option<String>>(6)?,
                "completed_at": row.get::<_, Option<String>>(7)?,
                "error_message": row.get::<_, Option<String>>(8)?,
                "output": output,
            }))
        }) {
            Ok(inst) => Some(inst),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(e) => return Err(anyhow::anyhow!("Database error: {}", e)),
        };
        
        Ok(instruction)
    }

    /// Get pending instructions for an agent (or any available agent)
    pub fn get_pending_instructions(&self, agent_id: Option<&str>) -> Result<Vec<serde_json::Value>> {
        let conn = self.conn.lock().unwrap();
        
        // Check if agent_instructions table exists
        let table_exists: bool = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='agent_instructions'",
            [],
            |row| Ok(row.get::<_, i64>(0)? > 0),
        ).unwrap_or(false);
        
        if !table_exists {
            return Ok(vec![]); // Table doesn't exist yet, return empty list
        }
        
        // Get pending or assigned instructions that are either:
        // 1. Not assigned to any agent, OR
        // 2. Assigned to this specific agent
        let sql = if agent_id.is_some() {
            "SELECT id, instruction_type, payload, status, assigned_to_agent_id, created_at
             FROM agent_instructions
             WHERE (status = 'pending' OR status = 'assigned') AND (assigned_to_agent_id IS NULL OR assigned_to_agent_id = ?1)
             ORDER BY created_at ASC
             LIMIT 1"
        } else {
            "SELECT id, instruction_type, payload, status, assigned_to_agent_id, created_at
             FROM agent_instructions
             WHERE status = 'pending' AND assigned_to_agent_id IS NULL
             ORDER BY created_at ASC
             LIMIT 1"
        };
        
        let mut stmt = conn.prepare(sql)?;
        
        let instructions = if let Some(id) = agent_id {
            stmt.query_map(params![id], |row| {
                let payload_str: String = row.get(2)?;
                let payload: serde_json::Value = serde_json::from_str(&payload_str)
                    .unwrap_or_else(|_| serde_json::json!({}));
                
                Ok(serde_json::json!({
                    "id": row.get::<_, i64>(0)?,
                    "instruction_type": row.get::<_, String>(1)?,
                    "payload": payload,
                    "status": row.get::<_, String>(3)?,
                    "assigned_to_agent_id": row.get::<_, Option<String>>(4)?,
                    "created_at": row.get::<_, String>(5)?,
                    "output": None::<String>, // Output not included in pending instructions query
                }))
            })?
            .collect::<Result<Vec<_>, _>>()?
        } else {
            stmt.query_map([], |row| {
                let payload_str: String = row.get(2)?;
                let payload: serde_json::Value = serde_json::from_str(&payload_str)
                    .unwrap_or_else(|_| serde_json::json!({}));
                
                Ok(serde_json::json!({
                    "id": row.get::<_, i64>(0)?,
                    "instruction_type": row.get::<_, String>(1)?,
                    "payload": payload,
                    "status": row.get::<_, String>(3)?,
                    "assigned_to_agent_id": row.get::<_, Option<String>>(4)?,
                    "created_at": row.get::<_, String>(5)?,
                    "output": None::<String>, // Output not included in pending instructions query
                }))
            })?
            .collect::<Result<Vec<_>, _>>()?
        };
        
        Ok(instructions)
    }

    /// Create a new instruction
    pub fn create_instruction(
        &self,
        instruction_type: &str,
        payload: &serde_json::Value,
    ) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        let payload_str = serde_json::to_string(payload)?;
        
        conn.execute(
            "INSERT INTO agent_instructions (instruction_type, payload, status, created_at)
             VALUES (?1, ?2, 'pending', ?3)",
            params![instruction_type, payload_str, now],
        )?;
        
        Ok(conn.last_insert_rowid())
    }

    /// Assign an instruction to an agent
    pub fn assign_instruction_to_agent(&self, instruction_id: i64, agent_id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        
        // Check if agent_instructions table exists
        let table_exists: bool = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='agent_instructions'",
            [],
            |row| Ok(row.get::<_, i64>(0)? > 0),
        ).unwrap_or(false);
        
        if !table_exists {
            return Err(anyhow::anyhow!("agent_instructions table does not exist"));
        }
        
        let now = chrono::Utc::now().to_rfc3339();
        
        conn.execute(
            "UPDATE agent_instructions 
             SET assigned_to_agent_id = ?1, assigned_at = ?2, status = 'assigned'
             WHERE id = ?3 AND status = 'pending'",
            params![agent_id, now, instruction_id],
        )?;
        
        Ok(())
    }

    /// Mark instruction as started
    pub fn start_instruction(&self, instruction_id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        
        conn.execute(
            "UPDATE agent_instructions 
             SET status = 'processing', started_at = ?1
             WHERE id = ?2",
            params![now, instruction_id],
        )?;
        
        Ok(())
    }

    /// Mark instruction as completed
    pub fn complete_instruction(&self, instruction_id: i64, output: Option<&str>) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        
        conn.execute(
            "UPDATE agent_instructions 
             SET status = 'completed', completed_at = ?1, output = ?2
             WHERE id = ?3",
            params![now, output, instruction_id],
        )?;
        
        Ok(())
    }

    /// Mark instruction as failed
    pub fn fail_instruction(&self, instruction_id: i64, error_message: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        
        conn.execute(
            "UPDATE agent_instructions 
             SET status = 'failed', completed_at = ?1, error_message = ?2
             WHERE id = ?3",
            params![now, error_message, instruction_id],
        )?;
        
        Ok(())
    }

    /// Mark agents as offline if they haven't sent heartbeat in X minutes
    pub fn cleanup_stale_agents(&self, minutes_threshold: i64) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        
        // Check if agents table exists
        let table_exists: bool = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='agents'",
            [],
            |row| Ok(row.get::<_, i64>(0)? > 0),
        ).unwrap_or(false);
        
        if !table_exists {
            return Ok(0); // Table doesn't exist yet, nothing to clean up
        }
        
        let threshold = chrono::Utc::now() - chrono::Duration::minutes(minutes_threshold);
        
        // More aggressive cleanup: mark as offline if no heartbeat in threshold
        // Also check for agents that haven't been seen recently
        let count = conn.execute(
            "UPDATE agents SET status = 'offline' 
             WHERE (status = 'online' OR status = 'busy') 
             AND last_seen < ?1",
            params![threshold.to_rfc3339()],
        )?;

        Ok(count)
    }

    /// Mark an agent as offline (force disconnect)
    #[allow(dead_code)]
    pub fn disconnect_agent(&self, agent_id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        
        conn.execute(
            "UPDATE agents SET status = 'offline', last_seen = ?1 WHERE agent_id = ?2",
            params![now, agent_id],
        )?;
        
        Ok(())
    }

    // Topaz Profile methods

    /// Create a new Topaz profile
    pub fn create_topaz_profile(
        &self,
        name: &str,
        command: &str,
    ) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        
        // Check if table needs migration (has old schema)
        let table_info: Result<Vec<String>, _> = conn.prepare("PRAGMA table_info(topaz_profiles)")?
            .query_map([], |row| Ok(row.get::<_, String>(1)?))?
            .collect();
        
        let columns = table_info.unwrap_or_default();
        if columns.contains(&"settings_json".to_string()) && !columns.contains(&"command".to_string()) {
            // Migrate old schema to new schema
            conn.execute(
                "ALTER TABLE topaz_profiles ADD COLUMN command TEXT",
                [],
            )?;
            // Copy description to command if command is empty (for existing profiles)
            conn.execute(
                "UPDATE topaz_profiles SET command = COALESCE(description, '') WHERE command IS NULL OR command = ''",
                [],
            )?;
            // Drop old columns
            conn.execute(
                "CREATE TABLE topaz_profiles_new (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT NOT NULL UNIQUE,
                    command TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                )",
                [],
            )?;
            // Try to get command from existing columns, fallback to empty string
            let has_description = columns.contains(&"description".to_string());
            let select_sql = if has_description {
                "INSERT INTO topaz_profiles_new (id, name, command, created_at, updated_at)
                 SELECT id, name, COALESCE(command, description, ''), created_at, updated_at
                 FROM topaz_profiles"
            } else {
                "INSERT INTO topaz_profiles_new (id, name, command, created_at, updated_at)
                 SELECT id, name, COALESCE(command, ''), created_at, updated_at
                 FROM topaz_profiles"
            };
            conn.execute(select_sql, [])?;
            conn.execute("DROP TABLE topaz_profiles", [])?;
            conn.execute("ALTER TABLE topaz_profiles_new RENAME TO topaz_profiles", [])?;
        }
        
        conn.execute(
            "INSERT INTO topaz_profiles (name, command, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![name, command, now, now],
        )?;
        
        Ok(conn.last_insert_rowid())
    }

    /// Get all Topaz profiles
    pub fn get_topaz_profiles(&self) -> Result<Vec<TopazProfile>> {
        let conn = self.conn.lock().unwrap();
        
        // Check if command column exists, fallback to old schema if needed
        let has_command = conn.prepare("PRAGMA table_info(topaz_profiles)")?
            .query_map([], |row| Ok(row.get::<_, String>(1)?))?
            .collect::<Result<Vec<_>, _>>()?
            .contains(&"command".to_string());
        
        let sql = if has_command {
            "SELECT id, name, command, created_at, updated_at FROM topaz_profiles ORDER BY name ASC"
        } else {
            "SELECT id, name, COALESCE(description, ''), created_at, updated_at FROM topaz_profiles ORDER BY name ASC"
        };
        
        let mut stmt = conn.prepare(sql)?;

        let profiles = stmt.query_map([], |row| {
            Ok(TopazProfile {
                id: Some(row.get(0)?),
                name: row.get(1)?,
                command: row.get(2)?,
                created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(3)?)
                    .unwrap()
                    .with_timezone(&Utc),
                updated_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(4)?)
                    .unwrap()
                    .with_timezone(&Utc),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

        Ok(profiles)
    }

    /// Get a Topaz profile by ID
    pub fn get_topaz_profile(&self, id: i64) -> Result<Option<TopazProfile>> {
        let conn = self.conn.lock().unwrap();
        
        // Check if command column exists
        let has_command = conn.prepare("PRAGMA table_info(topaz_profiles)")?
            .query_map([], |row| Ok(row.get::<_, String>(1)?))?
            .collect::<Result<Vec<_>, _>>()?
            .contains(&"command".to_string());
        
        let sql = if has_command {
            "SELECT id, name, command, created_at, updated_at FROM topaz_profiles WHERE id = ?1"
        } else {
            "SELECT id, name, COALESCE(description, ''), created_at, updated_at FROM topaz_profiles WHERE id = ?1"
        };
        
        let mut stmt = conn.prepare(sql)?;

        let profile = match stmt.query_row(params![id], |row| {
            Ok(TopazProfile {
                id: Some(row.get(0)?),
                name: row.get(1)?,
                command: row.get(2)?,
                created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(3)?)
                    .unwrap()
                    .with_timezone(&Utc),
                updated_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(4)?)
                    .unwrap()
                    .with_timezone(&Utc),
            })
        }) {
            Ok(profile) => Some(profile),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(e) => return Err(anyhow::anyhow!("Database error: {}", e)),
        };

        Ok(profile)
    }

    /// Update a Topaz profile
    pub fn update_topaz_profile(
        &self,
        id: i64,
        name: Option<&str>,
        command: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();

        // Build update query dynamically based on what's provided
        let mut updates = Vec::new();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(n) = name {
            updates.push("name = ?");
            params.push(Box::new(n.to_string()));
        }
        
        if let Some(cmd) = command {
            updates.push("command = ?");
            params.push(Box::new(cmd.to_string()));
        }
        updates.push("updated_at = ?");
        params.push(Box::new(now.clone()));

        params.push(Box::new(id));

        let sql = format!("UPDATE topaz_profiles SET {} WHERE id = ?", updates.join(", "));
        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        conn.execute(&sql, &param_refs[..])?;

        Ok(())
    }

    /// Delete a Topaz profile
    pub fn delete_topaz_profile(&self, id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM topaz_profiles WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// Associate a Topaz profile with a show
    pub fn associate_profile_with_show(&self, show_id: i64, profile_id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO show_topaz_profiles (show_id, topaz_profile_id)
             VALUES (?1, ?2)",
            params![show_id, profile_id],
        )?;
        Ok(())
    }

    /// Remove association between a Topaz profile and a show
    pub fn remove_profile_from_show(&self, show_id: i64, profile_id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM show_topaz_profiles WHERE show_id = ?1 AND topaz_profile_id = ?2",
            params![show_id, profile_id],
        )?;
        Ok(())
    }

    /// Get Topaz profiles associated with a show
    pub fn get_profiles_for_show(&self, show_id: i64) -> Result<Vec<TopazProfile>> {
        let conn = self.conn.lock().unwrap();
        
        // Check if command column exists
        let has_command = conn.prepare("PRAGMA table_info(topaz_profiles)")?
            .query_map([], |row| Ok(row.get::<_, String>(1)?))?
            .collect::<Result<Vec<_>, _>>()?
            .contains(&"command".to_string());
        
        let sql = if has_command {
            "SELECT p.id, p.name, p.command, p.created_at, p.updated_at
             FROM topaz_profiles p
             INNER JOIN show_topaz_profiles stp ON p.id = stp.topaz_profile_id
             WHERE stp.show_id = ?1
             ORDER BY p.name ASC"
        } else {
            "SELECT p.id, p.name, COALESCE(p.description, ''), p.created_at, p.updated_at
             FROM topaz_profiles p
             INNER JOIN show_topaz_profiles stp ON p.id = stp.topaz_profile_id
             WHERE stp.show_id = ?1
             ORDER BY p.name ASC"
        };
        
        let mut stmt = conn.prepare(sql)?;

        let profiles = stmt.query_map(params![show_id], |row| {
            Ok(TopazProfile {
                id: Some(row.get(0)?),
                name: row.get(1)?,
                command: row.get(2)?,
                created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(3)?)
                    .unwrap()
                    .with_timezone(&Utc),
                updated_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(4)?)
                    .unwrap()
                    .with_timezone(&Utc),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

        Ok(profiles)
    }

    // Upscaling Job methods

    /// Create a new upscaling job
    pub fn create_upscaling_job(
        &self,
        job_id: &str,
        input_file_path: &str,
        show_id: Option<i64>,
        topaz_profile_id: Option<i64>,
        priority: i32,
    ) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        
        conn.execute(
            "INSERT INTO upscaling_jobs (job_id, input_file_path, show_id, topaz_profile_id, status, priority, created_at, progress, retry_count)
             VALUES (?1, ?2, ?3, ?4, 'queued', ?5, ?6, 0.0, 0)",
            params![job_id, input_file_path, show_id, topaz_profile_id, priority, now],
        )?;
        
        Ok(conn.last_insert_rowid())
    }

    /// Get next available upscaling job for assignment
    pub fn get_next_upscaling_job(&self) -> Result<Option<UpscalingJob>> {
        let conn = self.conn.lock().unwrap();
        
        let mut stmt = conn.prepare(
            "SELECT id, job_id, input_file_path, output_file_path, show_id, topaz_profile_id, status, priority, 
                    agent_id, instruction_id, created_at, assigned_at, started_at, completed_at, progress, 
                    error_message, processing_time_seconds, retry_count
             FROM upscaling_jobs
             WHERE status = 'queued'
             ORDER BY priority DESC, created_at ASC
             LIMIT 1"
        )?;

        let job = match stmt.query_row([], |row| {
            Ok(UpscalingJob {
                id: Some(row.get(0)?),
                job_id: row.get(1)?,
                input_file_path: row.get(2)?,
                output_file_path: row.get(3)?,
                show_id: row.get(4)?,
                topaz_profile_id: row.get(5)?,
                status: JobStatus::from_string(&row.get::<_, String>(6)?),
                priority: row.get(7)?,
                agent_id: row.get(8)?,
                instruction_id: row.get(9)?,
                created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(10)?)
                    .unwrap()
                    .with_timezone(&Utc),
                assigned_at: row.get::<_, Option<String>>(11)?
                    .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                    .map(|dt| dt.with_timezone(&Utc)),
                started_at: row.get::<_, Option<String>>(12)?
                    .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                    .map(|dt| dt.with_timezone(&Utc)),
                completed_at: row.get::<_, Option<String>>(13)?
                    .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                    .map(|dt| dt.with_timezone(&Utc)),
                progress: row.get(14)?,
                error_message: row.get(15)?,
                processing_time_seconds: row.get(16)?,
                retry_count: row.get(17).unwrap_or(0),
            })
        }) {
            Ok(job) => Some(job),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(e) => return Err(anyhow::anyhow!("Database error: {}", e)),
        };

        Ok(job)
    }

    /// Assign an upscaling job to an agent
    pub fn assign_upscaling_job(&self, job_id: &str, agent_id: &str, instruction_id: Option<i64>) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        
        conn.execute(
            "UPDATE upscaling_jobs 
             SET status = 'assigned', agent_id = ?1, instruction_id = ?2, assigned_at = ?3
             WHERE job_id = ?4 AND status = 'queued'",
            params![agent_id, instruction_id, now, job_id],
        )?;
        
        Ok(())
    }

    /// Update upscaling job status
    pub fn update_upscaling_job_status(
        &self,
        job_id: &str,
        status: JobStatus,
        progress: Option<f32>,
        error_message: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();

        if status == JobStatus::Processing {
            // Update started_at if transitioning to processing
            conn.execute(
                "UPDATE upscaling_jobs 
                 SET status = ?1, started_at = COALESCE(started_at, ?2), progress = COALESCE(?3, progress)
                 WHERE job_id = ?4",
                params![status.to_string(), now, progress, job_id],
            )?;
        } else if status == JobStatus::Completed || status == JobStatus::Failed {
            // Calculate processing time if completing
            let processing_time: Option<i64> = conn.query_row(
                "SELECT CASE 
                    WHEN started_at IS NOT NULL 
                    THEN CAST((julianday(?) - julianday(started_at)) * 86400 AS INTEGER)
                    ELSE NULL
                 END
                 FROM upscaling_jobs
                 WHERE job_id = ?2",
                params![now, job_id],
                |row| row.get(0),
            ).ok().flatten();

            conn.execute(
                "UPDATE upscaling_jobs 
                 SET status = ?1, completed_at = ?2, progress = COALESCE(?3, progress), 
                     error_message = ?4, processing_time_seconds = ?5
                 WHERE job_id = ?6",
                params![status.to_string(), now, progress, error_message, processing_time, job_id],
            )?;
        } else {
            conn.execute(
                "UPDATE upscaling_jobs 
                 SET status = ?1, progress = COALESCE(?2, progress)
                 WHERE job_id = ?3",
                params![status.to_string(), progress, job_id],
            )?;
        }

        Ok(())
    }

    /// Update upscaling job output path
    pub fn update_upscaling_job_output(&self, job_id: &str, output_file_path: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        
        conn.execute(
            "UPDATE upscaling_jobs 
             SET output_file_path = ?1
             WHERE job_id = ?2",
            params![output_file_path, job_id],
        )?;
        
        Ok(())
    }

    /// Retry a failed upscaling job (reset status to queued and increment retry_count)
    pub fn retry_upscaling_job(&self, job_id: &str, max_retries: i32) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        
        // Check current retry count
        let current_retry_count: i32 = conn.query_row(
            "SELECT retry_count FROM upscaling_jobs WHERE job_id = ?1",
            params![job_id],
            |row| row.get(0),
        )?;
        
        if current_retry_count >= max_retries {
            return Ok(false); // Max retries reached
        }
        
        // Reset job to queued status and increment retry count
        conn.execute(
            "UPDATE upscaling_jobs 
             SET status = 'queued', agent_id = NULL, instruction_id = NULL, 
                 assigned_at = NULL, started_at = NULL, completed_at = NULL,
                 progress = 0.0, error_message = NULL, processing_time_seconds = NULL,
                 retry_count = retry_count + 1
             WHERE job_id = ?1 AND status = 'failed'",
            params![job_id],
        )?;
        
        Ok(true)
    }

    /// Clean up old upscaling jobs (delete completed/failed jobs older than X days, keeping recent N jobs)
    /// Returns total number of deleted jobs
    pub fn cleanup_old_upscaling_jobs(&self, days_threshold: i64, keep_recent: Option<i64>) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let threshold = chrono::Utc::now() - chrono::Duration::days(days_threshold);
        let threshold_str = threshold.to_rfc3339();
        
        // First, count jobs that will be deleted
        let _completed_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM upscaling_jobs 
             WHERE status IN ('completed', 'failed', 'cancelled') 
             AND completed_at IS NOT NULL 
             AND completed_at < ?1",
            params![&threshold_str],
            |row| row.get(0),
        ).unwrap_or(0);
        
        let _queued_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM upscaling_jobs 
             WHERE status = 'queued' 
             AND created_at < ?1",
            params![&threshold_str],
            |row| row.get(0),
        ).unwrap_or(0);
        
        // If keep_recent is specified, adjust threshold to keep that many recent jobs
        if let Some(keep) = keep_recent {
            // Get the oldest job that should be kept
            let oldest_to_keep: Option<String> = conn.query_row(
                "SELECT completed_at FROM upscaling_jobs 
                 WHERE status IN ('completed', 'failed', 'cancelled') 
                 AND completed_at IS NOT NULL
                 ORDER BY completed_at DESC 
                 LIMIT 1 OFFSET ?",
                params![keep],
                |row| row.get(0),
            ).ok();
            
            if let Some(oldest_date) = oldest_to_keep {
                // Only delete jobs older than the oldest job we want to keep
                let deleted_completed = conn.execute(
                    "DELETE FROM upscaling_jobs 
                     WHERE status IN ('completed', 'failed', 'cancelled') 
                     AND completed_at IS NOT NULL 
                     AND completed_at < ?1 
                     AND completed_at < ?2",
                    params![&threshold_str, &oldest_date],
                )?;
                
                let deleted_queued = conn.execute(
                    "DELETE FROM upscaling_jobs 
                     WHERE status = 'queued' 
                     AND created_at < ?1",
                    params![&threshold_str],
                )?;
                
                return Ok(deleted_completed + deleted_queued);
            }
        }
        
        // Delete completed/failed/cancelled jobs older than threshold
        let deleted_completed = conn.execute(
            "DELETE FROM upscaling_jobs 
             WHERE status IN ('completed', 'failed', 'cancelled') 
             AND completed_at IS NOT NULL 
             AND completed_at < ?1",
            params![&threshold_str],
        )?;
        
        // Delete stale queued jobs (never assigned)
        let deleted_queued = conn.execute(
            "DELETE FROM upscaling_jobs 
             WHERE status = 'queued' 
             AND created_at < ?1",
            params![&threshold_str],
        )?;
        
        Ok(deleted_completed + deleted_queued)
    }

    // Operation History methods

    /// Save an operation to history (when it completes or fails)
    pub fn save_operation_to_history(
        &self,
        operation_id: &str,
        operation_type: &str,
        status: &str,
        drive: Option<&str>,
        title: Option<&str>,
        progress: f32,
        message: &str,
        started_at: &str,
        completed_at: Option<&str>,
        error: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();
        
        // Insert or replace (in case operation was already saved)
        conn.execute(
            "INSERT OR REPLACE INTO operation_history 
             (operation_id, operation_type, status, drive, title, progress, message, started_at, completed_at, error, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                operation_id,
                operation_type,
                status,
                drive,
                title,
                progress,
                message,
                started_at,
                completed_at,
                error,
                now,
            ],
        )?;
        
        Ok(())
    }

    /// Get operation history (completed/failed operations)
    pub fn get_operation_history(&self, limit: Option<i64>, status_filter: Option<&str>) -> Result<Vec<crate::api::Operation>> {
        use chrono::DateTime;
        use chrono::Utc;
        
        let conn = self.conn.lock().unwrap();
        let limit = limit.unwrap_or(100);
        
        let (sql, params_vec): (String, Vec<Box<dyn rusqlite::ToSql>>) = if let Some(ref status) = status_filter {
            (
                "SELECT operation_id, operation_type, status, drive, title, progress, message, started_at, completed_at, error
                 FROM operation_history
                 WHERE status = ?
                 ORDER BY completed_at DESC, created_at DESC
                 LIMIT ?".to_string(),
                vec![Box::new(status.to_string()), Box::new(limit)],
            )
        } else {
            (
                "SELECT operation_id, operation_type, status, drive, title, progress, message, started_at, completed_at, error
                 FROM operation_history
                 ORDER BY completed_at DESC, created_at DESC
                 LIMIT ?".to_string(),
                vec![Box::new(limit)],
            )
        };
        
        let mut stmt = conn.prepare(&sql)?;
        let param_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();
        
        let operations = stmt.query_map(&param_refs[..], |row| {
            Ok(crate::api::Operation {
                operation_id: row.get(0)?,
                operation_type: match row.get::<_, String>(1)?.as_str() {
                    "rip" => crate::api::OperationType::Rip,
                    "upscale" => crate::api::OperationType::Upscale,
                    "rename" => crate::api::OperationType::Rename,
                    "transfer" => crate::api::OperationType::Transfer,
                    _ => crate::api::OperationType::Other,
                },
                status: match row.get::<_, String>(2)?.as_str() {
                    "queued" => crate::api::OperationStatus::Queued,
                    "running" => crate::api::OperationStatus::Running,
                    "paused" => crate::api::OperationStatus::Paused,
                    "completed" => crate::api::OperationStatus::Completed,
                    "failed" => crate::api::OperationStatus::Failed,
                    "cancelled" => crate::api::OperationStatus::Cancelled,
                    _ => crate::api::OperationStatus::Failed,
                },
                drive: row.get(3)?,
                title: row.get(4)?,
                progress: row.get(5)?,
                message: row.get(6)?,
                started_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(7)?)
                    .unwrap()
                    .with_timezone(&Utc),
                completed_at: row.get::<_, Option<String>>(8)?
                    .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                    .map(|dt| dt.with_timezone(&Utc)),
                error: row.get(9)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
        
        Ok(operations)
    }

    /// Get all upscaling jobs
    pub fn get_upscaling_jobs(&self, status_filter: Option<JobStatus>) -> Result<Vec<UpscalingJob>> {
        let conn = self.conn.lock().unwrap();
        
        let (sql, params_vec): (String, Vec<Box<dyn rusqlite::ToSql>>) = if let Some(ref status) = status_filter {
            (
                "SELECT id, job_id, input_file_path, output_file_path, show_id, topaz_profile_id, status, priority, 
                        agent_id, instruction_id, created_at, assigned_at, started_at, completed_at, progress, 
                        error_message, processing_time_seconds, retry_count
                 FROM upscaling_jobs
                 WHERE status = ?
                 ORDER BY priority DESC, created_at DESC".to_string(),
                vec![Box::new(status.to_string())],
            )
        } else {
            (
                "SELECT id, job_id, input_file_path, output_file_path, show_id, topaz_profile_id, status, priority, 
                        agent_id, instruction_id, created_at, assigned_at, started_at, completed_at, progress, 
                        error_message, processing_time_seconds, retry_count
                 FROM upscaling_jobs
                 ORDER BY priority DESC, created_at DESC".to_string(),
                vec![],
            )
        };

        let mut stmt = conn.prepare(&sql)?;
        let param_refs: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();
        
        let jobs: Vec<UpscalingJob> = stmt.query_map(&param_refs[..], |row| {
            self.row_to_upscaling_job(row)
        })?
        .collect::<Result<Vec<_>, _>>()?;
        
        Ok(jobs)
    }

    /// Helper function to convert database row to UpscalingJob
    fn row_to_upscaling_job(&self, row: &rusqlite::Row<'_>) -> Result<UpscalingJob, rusqlite::Error> {
        Ok(UpscalingJob {
            id: Some(row.get(0)?),
            job_id: row.get(1)?,
            input_file_path: row.get(2)?,
            output_file_path: row.get(3)?,
            show_id: row.get(4)?,
            topaz_profile_id: row.get(5)?,
            status: JobStatus::from_string(&row.get::<_, String>(6)?),
            priority: row.get(7)?,
            agent_id: row.get(8)?,
            instruction_id: row.get(9)?,
            created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(10)?)
                .unwrap()
                .with_timezone(&Utc),
            assigned_at: row.get::<_, Option<String>>(11)?
                .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&Utc)),
            started_at: row.get::<_, Option<String>>(12)?
                .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&Utc)),
            completed_at: row.get::<_, Option<String>>(13)?
                .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&Utc)),
            progress: row.get(14)?,
            error_message: row.get(15)?,
            processing_time_seconds: row.get(16)?,
            retry_count: row.get(17).unwrap_or(0),
        })
    }

    /// Get database file path
    pub fn get_db_path() -> PathBuf {
        // Check for test database environment variable
        if let Ok(test_db) = std::env::var("RIPLEY_TEST_DB") {
            return PathBuf::from(test_db);
        }
        
        if let Some(home) = dirs::home_dir() {
            home.join(".config").join("ripley").join("ripley.db")
        } else {
            PathBuf::from("ripley.db")
        }
    }

    /// Reset database - delete the file and let normal initialization recreate it
    pub fn reset_database(&self) -> Result<()> {
        let db_path = Self::get_db_path();
        
        // Close the connection by dropping the lock guard
        // This ensures any pending transactions are committed and the connection is closed
        {
            let _conn_guard = self.conn.lock().unwrap();
            // Connection will be closed when guard is dropped
        }
        
        // Small delay to ensure file handles are released on all platforms
        std::thread::sleep(std::time::Duration::from_millis(200));
        
        // Delete the database file if it exists
        // On some platforms (Windows), we may need to retry if the file is still locked
        let mut attempts = 0;
        while db_path.exists() && attempts < 5 {
            match std::fs::remove_file(&db_path) {
                Ok(_) => {
                    info!("Deleted database file: {:?}", db_path);
                    break;
                }
                Err(e) => {
                    attempts += 1;
                    if attempts >= 5 {
                        return Err(anyhow::anyhow!("Failed to delete database file after {} attempts: {}", attempts, e));
                    }
                    std::thread::sleep(std::time::Duration::from_millis(200));
                }
            }
        }
        
        // Reopen the connection and reinitialize
        let new_conn = Connection::open(&db_path)?;
        
        // Reinitialize schema, run migrations, and seed data
        // IMPORTANT: Profiles must be seeded before shows so associations can be created
        Self::initialize_schema_static(&new_conn)?;
        Self::run_migrations_static(&new_conn)?;
        Self::seed_initial_topaz_profiles(&new_conn)?;
        Self::seed_initial_shows(&new_conn)?;
        
        // Replace the connection in the Mutex
        {
            let mut conn_guard = self.conn.lock().unwrap();
            *conn_guard = new_conn;
        }
        
        info!("Database reset complete - file deleted and recreated with fresh schema");
        Ok(())
    }

    /// Seed initial shows if the table is empty
    /// Reads seed data from config.yaml and creates profile associations
    fn seed_initial_shows(conn: &Connection) -> Result<()> {
        // Check if shows table exists and is empty
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM shows",
            [],
            |row| row.get(0),
        ).unwrap_or(0);
        
        if count == 0 {
            // Load config to get seed shows
            let config = crate::config::Config::load()
                .unwrap_or_else(|e| {
                    warn!("Failed to load config for seeding: {}, using defaults", e);
                    crate::config::Config::default()
                });
            
            // All seed data must come from config.yaml - no hardcoded fallbacks
            let show_seeds = config.seed.shows;
            
            let now = Utc::now().to_rfc3339();
            let show_count = show_seeds.len();
            
            // First, get all profiles by name for lookup
            let all_profiles = Self::get_all_topaz_profiles_static(conn)?;
            let profile_map: std::collections::HashMap<String, i64> = all_profiles
                .iter()
                .filter_map(|p| p.id.map(|id| (p.name.clone(), id)))
                .collect();
            
            // Create shows and associations
            for show_seed in show_seeds {
                let (show_name, profile_names) = match show_seed {
                    crate::config::ShowSeed::Simple(name) => (name, vec![]),
                    crate::config::ShowSeed::WithProfiles { name, topaz_profiles } => (name, topaz_profiles),
                };
                
                // Insert the show
                conn.execute(
                    "INSERT INTO shows (name, created_at) VALUES (?1, ?2)",
                    params![show_name, now],
                )?;
                
                let show_id = conn.last_insert_rowid();
                
                // Associate profiles if specified
                for profile_name in profile_names {
                    if let Some(&profile_id) = profile_map.get(&profile_name) {
                        match conn.execute(
                            "INSERT OR IGNORE INTO show_topaz_profiles (show_id, topaz_profile_id) VALUES (?1, ?2)",
                            params![show_id, profile_id],
                        ) {
                            Ok(_) => {
                                info!("Associated Topaz profile '{}' with show '{}'", profile_name, show_name);
                            }
                            Err(e) => {
                                warn!("Failed to associate profile '{}' with show '{}': {}", profile_name, show_name, e);
                            }
                        }
                    } else {
                        warn!("Topaz profile '{}' not found when seeding show '{}' - make sure profiles are defined before shows", profile_name, show_name);
                    }
                }
            }
            
            info!("Seeded {} initial shows from config.yaml", show_count);
        }
        
        Ok(())
    }

    /// Static version of get_topaz_profiles for use in seeding
    fn get_all_topaz_profiles_static(conn: &Connection) -> Result<Vec<TopazProfile>> {
        // Check if command column exists
        let has_command = conn.prepare("PRAGMA table_info(topaz_profiles)")?
            .query_map([], |row| Ok(row.get::<_, String>(1)?))?
            .collect::<Result<Vec<_>, _>>()?
            .contains(&"command".to_string());
        
        let sql = if has_command {
            "SELECT id, name, command, created_at, updated_at FROM topaz_profiles ORDER BY name ASC"
        } else {
            "SELECT id, name, COALESCE(description, ''), created_at, updated_at FROM topaz_profiles ORDER BY name ASC"
        };
        
        let mut stmt = conn.prepare(sql)?;
        
        let profiles = stmt.query_map([], |row| {
            Ok(TopazProfile {
                id: Some(row.get(0)?),
                name: row.get(1)?,
                command: row.get(2)?,
                created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(3)?)
                    .unwrap()
                    .with_timezone(&Utc),
                updated_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(4)?)
                    .unwrap()
                    .with_timezone(&Utc),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
        
        Ok(profiles)
    }

    /// Seed initial Topaz profiles if the table is empty
    /// Reads seed data from config.yaml
    fn seed_initial_topaz_profiles(conn: &Connection) -> Result<()> {
        // Check if topaz_profiles table exists and is empty
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM topaz_profiles",
            [],
            |row| row.get(0),
        ).unwrap_or(0);
        
        if count == 0 {
            // Load config to get seed Topaz profiles
            let config = crate::config::Config::load()
                .unwrap_or_else(|e| {
                    warn!("Failed to load config for seeding Topaz profiles: {}, using defaults", e);
                    crate::config::Config::default()
                });
            
            let initial_profiles = if config.seed.topaz_profiles.is_empty() {
                // Empty by default - users should configure their own
                vec![]
            } else {
                config.seed.topaz_profiles
            };
            
            let profile_count = initial_profiles.len();
            
            for profile in initial_profiles {
                match Self::create_topaz_profile_static(conn, &profile.name, &profile.command) {
                    Ok(_) => {
                        info!("Seeded Topaz profile: {}", profile.name);
                    }
                    Err(e) => {
                        warn!("Failed to seed Topaz profile {}: {}", profile.name, e);
                    }
                }
            }
            
            if profile_count > 0 {
                info!("Seeded {} initial Topaz profiles from config.yaml", profile_count);
            }
        }
        
        Ok(())
    }

    /// Static version of create_topaz_profile for use in seeding
    fn create_topaz_profile_static(conn: &Connection, name: &str, command: &str) -> Result<i64> {
        let now = Utc::now().to_rfc3339();
        
        conn.execute(
            "INSERT INTO topaz_profiles (name, command, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![name, command, now, now],
        )?;
        
        Ok(conn.last_insert_rowid())
    }

    /// Add a log entry
    pub fn add_log(&self, entry: &LogEntry) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        
        conn.execute(
            "INSERT INTO logs (timestamp, level, message, drive, disc, title, context)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                entry.timestamp.to_rfc3339(),
                entry.level.to_string(),
                entry.message,
                entry.drive,
                entry.disc,
                entry.title,
                entry.context,
            ],
        )?;

        Ok(conn.last_insert_rowid())
    }

    /// Get recent logs (limit)
    pub fn get_recent_logs(&self, limit: usize) -> Result<Vec<LogEntry>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, timestamp, level, message, drive, disc, title, context
             FROM logs
             ORDER BY timestamp DESC
             LIMIT ?1"
        )?;

        let logs = stmt.query_map([limit], |row| {
            Ok(LogEntry {
                id: Some(row.get(0)?),
                timestamp: DateTime::parse_from_rfc3339(&row.get::<_, String>(1)?)
                    .unwrap()
                    .with_timezone(&Utc),
                level: LogLevel::from_string(&row.get::<_, String>(2)?),
                message: row.get(3)?,
                drive: row.get(4)?,
                disc: row.get(5)?,
                title: row.get(6)?,
                context: row.get(7)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

        Ok(logs)
    }

    /// Search logs with filters (using FTS5 for full-text search)
    pub fn search_logs(
        &self,
        query: Option<&str>,
        level: Option<&str>,
        drive: Option<&str>,
        limit: usize,
    ) -> Result<Vec<LogEntry>> {
        let conn = self.conn.lock().unwrap();
        
        // Build the search query
        let mut sql = if query.is_some() {
            // Use FTS5 for full-text search when query is provided
            "SELECT l.id, l.timestamp, l.level, l.message, l.drive, l.disc, l.title, l.context
             FROM logs l
             INNER JOIN logs_fts fts ON l.id = fts.rowid
             WHERE 1=1".to_string()
        } else {
            // Regular search without FTS5
            "SELECT id, timestamp, level, message, drive, disc, title, context FROM logs WHERE 1=1".to_string()
        };
        
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(q) = query {
            // Use FTS5 syntax for full-text search
            // Escape special FTS5 characters and build query
            let fts_query = q.replace('"', "\"\"") // Escape quotes
                .replace("'", "''") // Escape single quotes
                .split_whitespace()
                .map(|term| format!("\"{}\"", term)) // Wrap each term in quotes for phrase matching
                .collect::<Vec<_>>()
                .join(" OR "); // Use OR to match any term
            
            sql.push_str(" AND fts MATCH ?");
            params.push(Box::new(fts_query));
        }

        if let Some(l) = level {
            sql.push_str(" AND level = ?");
            params.push(Box::new(l.to_string()));
        }

        if let Some(d) = drive {
            sql.push_str(" AND drive = ?");
            params.push(Box::new(d.to_string()));
        }

        sql.push_str(" ORDER BY timestamp DESC LIMIT ?");
        params.push(Box::new(limit));

        let mut stmt = conn.prepare(&sql)?;
        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        
        let logs = stmt.query_map(&param_refs[..], |row| {
            Ok(LogEntry {
                id: Some(row.get(0)?),
                timestamp: DateTime::parse_from_rfc3339(&row.get::<_, String>(1)?)
                    .unwrap()
                    .with_timezone(&Utc),
                level: LogLevel::from_string(&row.get::<_, String>(2)?),
                message: row.get(3)?,
                drive: row.get(4)?,
                disc: row.get(5)?,
                title: row.get(6)?,
                context: row.get(7)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

        Ok(logs)
    }

    /// Clear all logs
    pub fn clear_logs(&self) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let count = conn.execute("DELETE FROM logs", [])?;
        Ok(count)
    }

    /// Add an issue
    pub fn add_issue(&self, issue: &Issue) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        
        conn.execute(
            "INSERT INTO issues (timestamp, issue_type, title, description, drive, disc, resolved, resolved_at, assigned_to, resolution_notes)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                issue.timestamp.to_rfc3339(),
                issue.issue_type.to_string(),
                issue.title,
                issue.description,
                issue.drive,
                issue.disc,
                issue.resolved as i32,
                issue.resolved_at.as_ref().map(|dt| dt.to_rfc3339()),
                issue.assigned_to,
                issue.resolution_notes,
            ],
        )?;

        Ok(conn.last_insert_rowid())
    }

    /// Get active issues (unresolved)
    pub fn get_active_issues(&self) -> Result<Vec<Issue>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, timestamp, issue_type, title, description, drive, disc, resolved, resolved_at, assigned_to, resolution_notes
             FROM issues
             WHERE resolved = 0
             ORDER BY timestamp DESC"
        )?;

        let issues = stmt.query_map([], |row| {
            Ok(Issue {
                id: Some(row.get(0)?),
                timestamp: DateTime::parse_from_rfc3339(&row.get::<_, String>(1)?)
                    .unwrap()
                    .with_timezone(&Utc),
                issue_type: IssueType::from_string(&row.get::<_, String>(2)?),
                title: row.get(3)?,
                description: row.get(4)?,
                drive: row.get(5)?,
                disc: row.get(6)?,
                resolved: row.get::<_, i32>(7)? != 0,
                resolved_at: row.get::<_, Option<String>>(8)?
                    .map(|s| DateTime::parse_from_rfc3339(&s).unwrap().with_timezone(&Utc)),
                assigned_to: row.get(9)?,
                resolution_notes: row.get(10)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

        Ok(issues)
    }

    /// Get all issues (including resolved)
    pub fn get_all_issues(&self, limit: usize) -> Result<Vec<Issue>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, timestamp, issue_type, title, description, drive, disc, resolved, resolved_at, assigned_to, resolution_notes
             FROM issues
             ORDER BY timestamp DESC
             LIMIT ?1"
        )?;

        let issues = stmt.query_map([limit], |row| {
            Ok(Issue {
                id: Some(row.get(0)?),
                timestamp: DateTime::parse_from_rfc3339(&row.get::<_, String>(1)?)
                    .unwrap()
                    .with_timezone(&Utc),
                issue_type: IssueType::from_string(&row.get::<_, String>(2)?),
                title: row.get(3)?,
                description: row.get(4)?,
                drive: row.get(5)?,
                disc: row.get(6)?,
                resolved: row.get::<_, i32>(7)? != 0,
                resolved_at: row.get::<_, Option<String>>(8)?
                    .map(|s| DateTime::parse_from_rfc3339(&s).unwrap().with_timezone(&Utc)),
                assigned_to: row.get(9)?,
                resolution_notes: row.get(10)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

        Ok(issues)
    }

    /// Resolve an issue
    pub fn resolve_issue(&self, id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        
        conn.execute(
            "UPDATE issues SET resolved = 1, resolved_at = ?1 WHERE id = ?2",
            params![Utc::now().to_rfc3339(), id],
        )?;

        Ok(())
    }

    /// Update issue assignment
    pub fn assign_issue(&self, id: i64, assigned_to: Option<&str>) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        
        conn.execute(
            "UPDATE issues SET assigned_to = ?1 WHERE id = ?2",
            params![assigned_to, id],
        )?;

        Ok(())
    }

    /// Update issue resolution notes
    pub fn update_resolution_notes(&self, id: i64, notes: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        
        conn.execute(
            "UPDATE issues SET resolution_notes = ?1 WHERE id = ?2",
            params![notes, id],
        )?;

        Ok(())
    }

    /// Add a note to an issue
    pub fn add_issue_note(&self, issue_id: i64, note: &str) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        
        conn.execute(
            "INSERT INTO issue_notes (issue_id, timestamp, note) VALUES (?1, ?2, ?3)",
            params![issue_id, Utc::now().to_rfc3339(), note],
        )?;

        Ok(conn.last_insert_rowid())
    }

    /// Get notes for an issue
    pub fn get_issue_notes(&self, issue_id: i64) -> Result<Vec<IssueNote>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, issue_id, timestamp, note FROM issue_notes WHERE issue_id = ?1 ORDER BY timestamp ASC"
        )?;

        let notes = stmt.query_map([issue_id], |row| {
            Ok(IssueNote {
                id: Some(row.get(0)?),
                issue_id: row.get(1)?,
                timestamp: DateTime::parse_from_rfc3339(&row.get::<_, String>(2)?)
                    .unwrap()
                    .with_timezone(&Utc),
                note: row.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

        Ok(notes)
    }

    /// Delete an issue note
    pub fn delete_issue_note(&self, note_id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM issue_notes WHERE id = ?1", [note_id])?;
        Ok(())
    }

    /// Get a setting value
    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT value FROM settings WHERE key = ?1")?;
        
        let result = stmt.query_row([key], |row| row.get::<_, String>(0));
        
        match result {
            Ok(value) => Ok(Some(value)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Set a setting value
    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)",
            params![key, value, Utc::now().to_rfc3339()],
        )?;

        Ok(())
    }

    /// Get the last used title for ripping
    pub fn get_last_title(&self) -> Result<Option<String>> {
        self.get_setting("last_rip_title")
    }

    /// Set the last used title for ripping
    pub fn set_last_title(&self, title: &str) -> Result<()> {
        self.set_setting("last_rip_title", title)
    }

    /// Get the last selected show ID
    pub fn get_last_show_id(&self) -> Result<Option<i64>> {
        match self.get_setting("last_show_id")? {
            Some(id_str) => Ok(id_str.parse().ok()),
            None => Ok(None),
        }
    }

    /// Set the last selected show ID
    pub fn set_last_show_id(&self, show_id: i64) -> Result<()> {
        self.set_setting("last_show_id", &show_id.to_string())
    }

    /// Add a new show
    pub fn add_show(&self, name: &str) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        
        conn.execute(
            "INSERT INTO shows (name, created_at) VALUES (?1, ?2)",
            params![name, Utc::now().to_rfc3339()],
        )?;

        Ok(conn.last_insert_rowid())
    }

    /// Get all shows
    pub fn get_shows(&self) -> Result<Vec<Show>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, created_at, last_used_at FROM shows ORDER BY name ASC"
        )?;

        let shows = stmt.query_map([], |row| {
            Ok(Show {
                id: Some(row.get(0)?),
                name: row.get(1)?,
                created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(2)?)
                    .unwrap()
                    .with_timezone(&Utc),
                last_used_at: row.get::<_, Option<String>>(3)?
                    .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                    .map(|dt| dt.with_timezone(&Utc)),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

        Ok(shows)
    }

    /// Get a show by ID
    pub fn get_show(&self, id: i64) -> Result<Option<Show>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, created_at, last_used_at FROM shows WHERE id = ?1"
        )?;

        let result = stmt.query_row([id], |row| {
            Ok(Show {
                id: Some(row.get(0)?),
                name: row.get(1)?,
                created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(2)?)
                    .unwrap()
                    .with_timezone(&Utc),
                last_used_at: row.get::<_, Option<String>>(3)?
                    .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                    .map(|dt| dt.with_timezone(&Utc)),
            })
        });

        match result {
            Ok(show) => Ok(Some(show)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Update a show
    pub fn update_show(&self, id: i64, name: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        
        conn.execute(
            "UPDATE shows SET name = ?1 WHERE id = ?2",
            params![name, id],
        )?;

        Ok(())
    }

    /// Delete a show
    pub fn delete_show(&self, id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        
        conn.execute("DELETE FROM shows WHERE id = ?1", [id])?;

        Ok(())
    }

    /// Update a show's last used timestamp
    pub fn update_show_last_used(&self, id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        
        conn.execute(
            "UPDATE shows SET last_used_at = ?1 WHERE id = ?2",
            params![Utc::now().to_rfc3339(), id],
        )?;

        Ok(())
    }

    /// Get user preferences
    pub fn get_preferences(&self) -> Result<UserPreferences> {
        let conn = self.conn.lock().unwrap();
        let result = conn.query_row(
            "SELECT logs_per_page, polling_interval_ms, theme, sound_notifications FROM user_preferences WHERE id = 1",
            [],
            |row| {
                Ok(UserPreferences {
                    logs_per_page: row.get(0)?,
                    polling_interval_ms: row.get(1)?,
                    theme: row.get(2)?,
                    sound_notifications: row.get::<_, i64>(3)? != 0,
                })
            },
        );

        match result {
            Ok(prefs) => Ok(prefs),
            Err(_) => Ok(UserPreferences::default()),
        }
    }

    /// Update user preferences
    pub fn update_preferences(&self, prefs: &UserPreferences) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        
        conn.execute(
            "UPDATE user_preferences SET logs_per_page = ?1, polling_interval_ms = ?2, theme = ?3, sound_notifications = ?4 WHERE id = 1",
            params![prefs.logs_per_page, prefs.polling_interval_ms, prefs.theme, prefs.sound_notifications as i64],
        )?;

        Ok(())
    }

    /// Add a rip history entry
    pub fn add_rip_history(&self, entry: &RipHistory) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        
        // Check if avg_speed_mbps column exists, add it if not
        let speed_exists: Result<i64, _> = conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('rip_history') WHERE name='avg_speed_mbps'",
            [],
            |row| row.get(0),
        );
        
        if speed_exists.unwrap_or(0) == 0 {
            let _ = conn.execute(
                "ALTER TABLE rip_history ADD COLUMN avg_speed_mbps REAL",
                [],
            );
        }
        
        // Check if checksum column exists, add it if not
        let checksum_exists: Result<i64, _> = conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('rip_history') WHERE name='checksum'",
            [],
            |row| row.get(0),
        );
        
        if checksum_exists.unwrap_or(0) == 0 {
            let _ = conn.execute(
                "ALTER TABLE rip_history ADD COLUMN checksum TEXT",
                [],
            );
        }
        
        conn.execute(
            "INSERT INTO rip_history (timestamp, drive, disc, title, disc_type, status, duration_seconds, file_size_bytes, output_path, error_message, avg_speed_mbps, checksum)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                entry.timestamp.to_rfc3339(),
                entry.drive,
                entry.disc,
                entry.title,
                entry.disc_type,
                entry.status.to_string(),
                entry.duration_seconds,
                entry.file_size_bytes,
                entry.output_path,
                entry.error_message,
                entry.avg_speed_mbps,
                entry.checksum,
            ],
        )?;

        Ok(conn.last_insert_rowid())
    }

    /// Get statistics summary
    pub fn get_statistics(&self) -> Result<serde_json::Value> {
        let conn = self.conn.lock().unwrap();
        
        // Check if rip_history table exists
        let table_exists: bool = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='rip_history'",
            [],
            |row| Ok(row.get::<_, i64>(0)? > 0),
        ).unwrap_or(false);
        
        if !table_exists {
            return Ok(serde_json::json!({
                "total_rips": 0,
                "successful_rips": 0,
                "failed_rips": 0,
                "success_rate": 0.0,
                "total_storage_bytes": 0,
            }));
        }
        
        // Total rips
        let total_rips: i64 = conn.query_row(
            "SELECT COUNT(*) FROM rip_history",
            [],
            |row| row.get(0),
        ).unwrap_or(0);
        
        // Successful rips
        let successful_rips: i64 = conn.query_row(
            "SELECT COUNT(*) FROM rip_history WHERE status = 'success'",
            [],
            |row| row.get(0),
        ).unwrap_or(0);
        
        // Failed rips
        let failed_rips: i64 = conn.query_row(
            "SELECT COUNT(*) FROM rip_history WHERE status = 'failed'",
            [],
            |row| row.get(0),
        ).unwrap_or(0);
        
        // Total storage used
        let total_storage: i64 = conn.query_row(
            "SELECT COALESCE(SUM(file_size_bytes), 0) FROM rip_history WHERE status = 'success'",
            [],
            |row| row.get(0),
        ).unwrap_or(0);
        
        // Success rate
        let success_rate = if total_rips > 0 {
            (successful_rips as f64 / total_rips as f64) * 100.0
        } else {
            0.0
        };
        
        Ok(serde_json::json!({
            "total_rips": total_rips,
            "successful_rips": successful_rips,
            "failed_rips": failed_rips,
            "success_rate": success_rate,
            "total_storage_bytes": total_storage,
        }))
    }

    /// Get drive statistics
    pub fn get_drive_statistics(&self) -> Result<Vec<DriveStats>> {
        let conn = self.conn.lock().unwrap();
        
        // Check if rip_history table exists
        let table_exists: bool = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='rip_history'",
            [],
            |row| Ok(row.get::<_, i64>(0)? > 0),
        ).unwrap_or(false);
        
        if !table_exists {
            return Ok(vec![]); // Table doesn't exist yet, return empty list
        }
        
        let mut stmt = conn.prepare(
            "SELECT 
                drive,
                SUM(CASE WHEN status = 'success' THEN 1 ELSE 0 END) as completed,
                SUM(CASE WHEN status = 'failed' THEN 1 ELSE 0 END) as failed,
                COALESCE(SUM(CASE WHEN status = 'success' THEN file_size_bytes ELSE 0 END), 0) as total_bytes,
                MAX(timestamp) as last_used
             FROM rip_history
             GROUP BY drive"
        )?;

        let stats = stmt.query_map([], |row| {
            Ok(DriveStats {
                drive: row.get(0)?,
                rips_completed: row.get(1)?,
                rips_failed: row.get(2)?,
                total_bytes_ripped: row.get(3)?,
                avg_speed_mbps: 0.0, // Calculate this later if we track speed
                last_used: row.get::<_, Option<String>>(4)?
                    .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                    .map(|dt| dt.with_timezone(&Utc)),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

        Ok(stats)
    }

    /// Get error frequency statistics by issue type
    pub fn get_error_frequency(&self) -> Result<serde_json::Value> {
        let conn = self.conn.lock().unwrap();
        
        // Check if issues table exists
        let table_exists: bool = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='issues'",
            [],
            |row| Ok(row.get::<_, i64>(0)? > 0),
        ).unwrap_or(false);
        
        if !table_exists {
            return Ok(serde_json::json!([])); // Table doesn't exist yet, return empty array
        }
        
        let mut stmt = conn.prepare(
            "SELECT 
                issue_type,
                COUNT(*) as count,
                SUM(CASE WHEN resolved = 0 THEN 1 ELSE 0 END) as active,
                SUM(CASE WHEN resolved = 1 THEN 1 ELSE 0 END) as resolved
             FROM issues
             GROUP BY issue_type
             ORDER BY count DESC"
        )?;

        let mut frequency = Vec::new();
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })?;

        for row in rows {
            let (issue_type, count, active, resolved) = row?;
            frequency.push(serde_json::json!({
                "issue_type": issue_type,
                "count": count,
                "active": active,
                "resolved": resolved,
            }));
        }

        // Also get error frequency over time (last 30 days)
        let mut stmt_time = conn.prepare(
            "SELECT 
                date(timestamp) as date,
                issue_type,
                COUNT(*) as count
             FROM issues
             WHERE timestamp > datetime('now', '-30 days')
             GROUP BY date(timestamp), issue_type
             ORDER BY date ASC"
        )?;

        let mut time_series = Vec::new();
        let rows_time = stmt_time.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })?;

        for row in rows_time {
            let (date, issue_type, count) = row?;
            time_series.push(serde_json::json!({
                "date": date,
                "issue_type": issue_type,
                "count": count,
            }));
        }

        Ok(serde_json::json!({
            "by_type": frequency,
            "over_time": time_series,
        }))
    }

    /// Add entry to rip queue
    pub fn add_to_queue(&self, entry: &RipQueueEntry) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        
        conn.execute(
            "INSERT INTO rip_queue (created_at, drive, output_path, title, skip_metadata, skip_filebot, profile, priority, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                entry.created_at.to_rfc3339(),
                entry.drive,
                entry.output_path,
                entry.title,
                entry.skip_metadata as i64,
                entry.skip_filebot as i64,
                entry.profile,
                entry.priority,
                entry.status.to_string(),
            ],
        )?;

        Ok(conn.last_insert_rowid())
    }

    /// Get next pending queue entry (highest priority first, then oldest)
    #[allow(dead_code)]
    pub fn get_next_queue_entry(&self, drive: Option<&str>) -> Result<Option<RipQueueEntry>> {
        let conn = self.conn.lock().unwrap();
        
        let entry = if let Some(d) = drive {
            let mut stmt = conn.prepare(
                "SELECT id, created_at, drive, output_path, title, skip_metadata, skip_filebot, profile, priority, status, started_at
                 FROM rip_queue
                 WHERE status = 'pending' AND (drive IS NULL OR drive = ?1)
                 ORDER BY priority DESC, created_at ASC
                 LIMIT 1"
            )?;
            stmt.query_row([d], |row| {
                Ok(RipQueueEntry {
                    id: Some(row.get(0)?),
                    created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(1)?)
                        .unwrap()
                        .with_timezone(&Utc),
                    drive: row.get(2)?,
                    output_path: row.get(3)?,
                    title: row.get(4)?,
                    skip_metadata: row.get::<_, i64>(5)? != 0,
                    skip_filebot: row.get::<_, i64>(6)? != 0,
                    profile: row.get(7)?,
                    priority: row.get(8)?,
                    status: QueueStatus::from_string(&row.get::<_, String>(9)?),
                    started_at: row.get::<_, Option<String>>(10)?
                        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                        .map(|dt| dt.with_timezone(&Utc)),
                })
            }).ok()
        } else {
            let mut stmt = conn.prepare(
                "SELECT id, created_at, drive, output_path, title, skip_metadata, skip_filebot, profile, priority, status, started_at
                 FROM rip_queue
                 WHERE status = 'pending'
                 ORDER BY priority DESC, created_at ASC
                 LIMIT 1"
            )?;
            stmt.query_row([], |row| {
                Ok(RipQueueEntry {
                    id: Some(row.get(0)?),
                    created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(1)?)
                        .unwrap()
                        .with_timezone(&Utc),
                    drive: row.get(2)?,
                    output_path: row.get(3)?,
                    title: row.get(4)?,
                    skip_metadata: row.get::<_, i64>(5)? != 0,
                    skip_filebot: row.get::<_, i64>(6)? != 0,
                    profile: row.get(7)?,
                    priority: row.get(8)?,
                    status: QueueStatus::from_string(&row.get::<_, String>(9)?),
                    started_at: row.get::<_, Option<String>>(10)?
                        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                        .map(|dt| dt.with_timezone(&Utc)),
                })
            }).ok()
        };

        Ok(entry)
    }

    /// Update queue entry status
    pub fn update_queue_status(&self, id: i64, status: QueueStatus, started_at: Option<DateTime<Utc>>) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        
        if let Some(started) = started_at {
            conn.execute(
                "UPDATE rip_queue SET status = ?1, started_at = ?2 WHERE id = ?3",
                params![status.to_string(), started.to_rfc3339(), id],
            )?;
        } else {
            conn.execute(
                "UPDATE rip_queue SET status = ?1 WHERE id = ?2",
                params![status.to_string(), id],
            )?;
        }

        Ok(())
    }

    /// Get all queue entries
    pub fn get_queue_entries(&self, include_completed: bool) -> Result<Vec<RipQueueEntry>> {
        let conn = self.conn.lock().unwrap();
        
        let sql = if include_completed {
            "SELECT id, created_at, drive, output_path, title, skip_metadata, skip_filebot, profile, priority, status, started_at
             FROM rip_queue
             ORDER BY priority DESC, created_at ASC"
        } else {
            "SELECT id, created_at, drive, output_path, title, skip_metadata, skip_filebot, profile, priority, status, started_at
             FROM rip_queue
             WHERE status != 'completed'
             ORDER BY priority DESC, created_at ASC"
        };

        let mut stmt = conn.prepare(sql)?;
        let entries = stmt.query_map([], |row| {
            Ok(RipQueueEntry {
                id: Some(row.get(0)?),
                created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(1)?)
                    .unwrap()
                    .with_timezone(&Utc),
                drive: row.get(2)?,
                output_path: row.get(3)?,
                title: row.get(4)?,
                skip_metadata: row.get::<_, i64>(5)? != 0,
                skip_filebot: row.get::<_, i64>(6)? != 0,
                profile: row.get(7)?,
                priority: row.get(8)?,
                status: QueueStatus::from_string(&row.get::<_, String>(9)?),
                started_at: row.get::<_, Option<String>>(10)?
                    .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                    .map(|dt| dt.with_timezone(&Utc)),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

        Ok(entries)
    }

    /// Remove queue entry (for cancellation)
    #[allow(dead_code)]
    pub fn remove_queue_entry(&self, id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM rip_queue WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// Get recent rip history
    pub fn get_rip_history(&self, limit: i64) -> Result<Vec<RipHistory>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, timestamp, drive, disc, title, disc_type, status, duration_seconds, file_size_bytes, output_path, error_message, avg_speed_mbps, checksum
             FROM rip_history
             ORDER BY timestamp DESC
             LIMIT ?1"
        )?;

        let history = stmt.query_map([limit], |row| {
            Ok(RipHistory {
                id: Some(row.get(0)?),
                timestamp: DateTime::parse_from_rfc3339(&row.get::<_, String>(1)?)
                    .unwrap()
                    .with_timezone(&Utc),
                drive: row.get(2)?,
                disc: row.get(3)?,
                title: row.get(4)?,
                disc_type: row.get(5)?,
                status: RipStatus::from_string(&row.get::<_, String>(6)?),
                duration_seconds: row.get(7)?,
                file_size_bytes: row.get(8)?,
                output_path: row.get(9)?,
                error_message: row.get(10)?,
                avg_speed_mbps: row.get(11)?,
                checksum: row.get(12)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

        Ok(history)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to set test database for all tests
    fn setup_test_db() {
        std::env::set_var("RIPLEY_TEST_DB", ":memory:");
    }

    #[test]
    fn test_database_creation() {
        setup_test_db();
        let db = Database::new().unwrap();
        assert!(db.get_recent_logs(10).is_ok());
    }

    #[test]
    fn test_add_log() {
        setup_test_db();
        let db = Database::new().unwrap();
        let log = LogEntry {
            id: None,
            timestamp: Utc::now(),
            level: LogLevel::Info,
            message: "Test log".to_string(),
            drive: Some("/dev/disk2".to_string()),
            disc: None,
            title: None,
            context: None,
        };

        let id = db.add_log(&log).unwrap();
        assert!(id > 0);
    }

    #[test]
    fn test_get_recent_logs() {
        setup_test_db();
        let db = Database::new().unwrap();
        
        // Add multiple logs
        for i in 0..5 {
            let log = LogEntry {
                id: None,
                timestamp: Utc::now(),
                level: LogLevel::Info,
                message: format!("Test log {}", i),
                drive: None,
                disc: None,
                title: None,
                context: None,
            };
            db.add_log(&log).unwrap();
        }

        let logs = db.get_recent_logs(3).unwrap();
        assert_eq!(logs.len(), 3);
    }

    #[test]
    fn test_add_issue() {
        setup_test_db();
        let db = Database::new().unwrap();
        let issue = Issue {
            id: None,
            timestamp: Utc::now(),
            issue_type: IssueType::RipFailure,
            title: "Test issue".to_string(),
            description: "Test description".to_string(),
            drive: Some("/dev/disk2".to_string()),
            disc: None,
            resolved: false,
            resolved_at: None,
            assigned_to: None,
            resolution_notes: None,
        };

        let id = db.add_issue(&issue).unwrap();
        assert!(id > 0);
    }

    #[test]
    fn test_resolve_issue() {
        setup_test_db();
        let db = Database::new().unwrap();
        let issue = Issue {
            id: None,
            timestamp: Utc::now(),
            issue_type: IssueType::RipFailure,
            title: "Test issue".to_string(),
            description: "Test description".to_string(),
            drive: None,
            disc: None,
            resolved: false,
            resolved_at: None,
            assigned_to: None,
            resolution_notes: None,
        };

        let id = db.add_issue(&issue).unwrap();
        db.resolve_issue(id).unwrap();

        let active = db.get_active_issues().unwrap();
        assert!(!active.iter().any(|i| i.id == Some(id)));
    }

    #[test]
    fn test_assign_issue() {
        setup_test_db();
        let db = Database::new().unwrap();
        let issue = Issue {
            id: None,
            timestamp: Utc::now(),
            issue_type: IssueType::RipFailure,
            title: "Test issue".to_string(),
            description: "Test description".to_string(),
            drive: None,
            disc: None,
            resolved: false,
            resolved_at: None,
            assigned_to: None,
            resolution_notes: None,
        };

        let id = db.add_issue(&issue).unwrap();
        db.assign_issue(id, Some("test_user")).unwrap();

        let issues = db.get_all_issues(100).unwrap();
        let assigned = issues.iter().find(|i| i.id == Some(id)).unwrap();
        assert_eq!(assigned.assigned_to, Some("test_user".to_string()));
    }

    #[test]
    fn test_add_show() {
        setup_test_db();
        let db = Database::new().unwrap();
        // Use a unique name with timestamp to avoid conflicts
        let unique_name = format!("Test Show {}", chrono::Utc::now().timestamp_millis());
        let id = db.add_show(&unique_name).unwrap();
        assert!(id > 0);

        let show = db.get_show(id).unwrap().unwrap();
        assert_eq!(show.name, unique_name);
    }

    #[test]
    fn test_get_shows() {
        setup_test_db();
        let db = Database::new().unwrap();
        
        // Use unique names with timestamp to avoid conflicts
        let timestamp = chrono::Utc::now().timestamp_millis();
        db.add_show(&format!("Show 1 {}", timestamp)).unwrap();
        db.add_show(&format!("Show 2 {}", timestamp)).unwrap();

        let shows = db.get_shows().unwrap();
        assert!(shows.len() >= 2);
    }

    #[test]
    fn test_add_rip_history() {
        setup_test_db();
        let db = Database::new().unwrap();
        let history = RipHistory {
            id: None,
            timestamp: Utc::now(),
            drive: "/dev/disk2".to_string(),
            disc: Some("Test Disc".to_string()),
            title: Some("Test Title".to_string()),
            disc_type: Some("DVD".to_string()),
            status: RipStatus::Success,
            duration_seconds: Some(3600),
            file_size_bytes: Some(1000000000),
            output_path: Some("/tmp/test".to_string()),
            error_message: None,
            avg_speed_mbps: None, // May not be in schema yet
            checksum: None,
        };

        let id = db.add_rip_history(&history).unwrap();
        assert!(id > 0);
    }

    #[test]
    fn test_get_statistics() {
        setup_test_db();
        let db = Database::new().unwrap();
        
        // Add some rip history
        for i in 0..5 {
            let history = RipHistory {
                id: None,
                timestamp: Utc::now(),
                drive: "/dev/disk2".to_string(),
                disc: Some(format!("Disc {}", i)),
                title: None,
                disc_type: Some("DVD".to_string()),
                status: if i % 2 == 0 { RipStatus::Success } else { RipStatus::Failed },
                duration_seconds: Some(3600),
                file_size_bytes: if i % 2 == 0 { Some(1000000000) } else { None },
                output_path: Some("/tmp/test".to_string()),
                error_message: None,
                avg_speed_mbps: None,
                checksum: None,
            };
            db.add_rip_history(&history).unwrap();
        }

        let stats = db.get_statistics().unwrap();
        assert!(stats["total_rips"].as_i64().unwrap() >= 5);
    }

    #[test]
    fn test_episode_match_recording() {
        setup_test_db();
        let db = Database::new().unwrap();
        let match_result = EpisodeMatchResult {
            id: None,
            timestamp: Utc::now(),
            show_name: "Test Show".to_string(),
            season: 1,
            episode: 5,
            episode_title: Some("Test Episode".to_string()),
            match_method: "transcript".to_string(),
            confidence: Some(95.0),
            title_index: Some(1),
            rip_history_id: None,
        };

        let id = db.record_episode_match(&match_result).unwrap();
        assert!(id > 0);
    }

    #[test]
    fn test_queue_operations() {
        setup_test_db();
        let db = Database::new().unwrap();
        
        let entry = RipQueueEntry {
            id: None,
            created_at: Utc::now(),
            drive: Some("/dev/disk2".to_string()),
            output_path: Some("/tmp/output".to_string()),
            title: Some("Test Title".to_string()),
            skip_metadata: false,
            skip_filebot: false,
            profile: Some("Standard".to_string()),
            priority: 5,
            status: QueueStatus::Pending,
            started_at: None,
        };

        let id = db.add_to_queue(&entry).unwrap();
        assert!(id > 0);

        db.update_queue_status(id, QueueStatus::Processing, Some(Utc::now())).unwrap();
        
        let entries = db.get_queue_entries(false).unwrap();
        assert!(entries.iter().any(|e| e.id == Some(id)));
    }

    #[test]
    fn test_log_level_enum() {
        assert_eq!(LogLevel::from_string("info"), LogLevel::Info);
        assert_eq!(LogLevel::from_string("warning"), LogLevel::Warning);
        assert_eq!(LogLevel::from_string("error"), LogLevel::Error);
        assert_eq!(LogLevel::from_string("success"), LogLevel::Success);
        assert_eq!(LogLevel::from_string("unknown"), LogLevel::Info); // Default
    }

    #[test]
    fn test_rip_status_enum() {
        assert_eq!(RipStatus::from_string("success"), RipStatus::Success);
        assert_eq!(RipStatus::from_string("failed"), RipStatus::Failed);
        assert_eq!(RipStatus::from_string("cancelled"), RipStatus::Cancelled);
        assert_eq!(RipStatus::Success.to_string(), "success");
    }

    #[test]
    fn test_clear_logs() {
        setup_test_db();
        let db = Database::new().unwrap();
        
        // Add a log
        let log = LogEntry {
            id: None,
            timestamp: Utc::now(),
            level: LogLevel::Info,
            message: "Test log to clear".to_string(),
            drive: None,
            disc: None,
            title: None,
            context: None,
        };
        db.add_log(&log).unwrap();
        
        // Clear logs
        let count = db.clear_logs().unwrap();
        assert!(count >= 1);
        
        // Verify logs are cleared
        let logs = db.get_recent_logs(10).unwrap();
        assert_eq!(logs.len(), 0);
    }
}

use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use tracing::{debug, info};

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

#[derive(Debug, Clone, Serialize, Deserialize)]
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
        Ok(db)
    }

    fn get_db_path() -> PathBuf {
        if let Some(home) = dirs::home_dir() {
            home.join(".config").join("ripley").join("ripley.db")
        } else {
            PathBuf::from("ripley.db")
        }
    }

    /// Initialize database schema
    fn initialize_schema(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();

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
                resolved_at TEXT
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
                created_at TEXT NOT NULL
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
                error_message TEXT
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

        debug!("Database schema initialized");
        
        // Seed initial shows if the table is empty
        Self::seed_initial_shows(&conn)?;
        
        Ok(())
    }

    /// Seed initial shows if the table is empty
    fn seed_initial_shows(conn: &Connection) -> Result<()> {
        
        // Check if shows table is empty
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM shows",
            [],
            |row| row.get(0),
        )?;
        
        if count == 0 {
            let initial_shows = vec![
                "Foster's Home For Imaginary Friends",
                "Power Puff Girls",
                "Johnny Bravo",
                "Pinky And The Brain",
                "Batman Begins",
                "Batman The Animated Series",
                "King Of The Hill",
                "Animaniacs",
                "Rocko's Modern Life"
            ];
            
            let now = Utc::now().to_rfc3339();
            let show_count = initial_shows.len();
            
            for show_name in initial_shows {
                conn.execute(
                    "INSERT INTO shows (name, created_at) VALUES (?1, ?2)",
                    params![show_name, now],
                )?;
            }
            
            info!("Seeded {} initial shows", show_count);
        }
        
        Ok(())
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

    /// Search logs with filters
    pub fn search_logs(
        &self,
        query: Option<&str>,
        level: Option<&str>,
        drive: Option<&str>,
        limit: usize,
    ) -> Result<Vec<LogEntry>> {
        let conn = self.conn.lock().unwrap();
        
        let mut sql = "SELECT id, timestamp, level, message, drive, disc, title, context FROM logs WHERE 1=1".to_string();
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(q) = query {
            sql.push_str(" AND message LIKE ?");
            params.push(Box::new(format!("%{}%", q)));
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

    /// Add an issue
    pub fn add_issue(&self, issue: &Issue) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        
        conn.execute(
            "INSERT INTO issues (timestamp, issue_type, title, description, drive, disc, resolved, resolved_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                issue.timestamp.to_rfc3339(),
                issue.issue_type.to_string(),
                issue.title,
                issue.description,
                issue.drive,
                issue.disc,
                issue.resolved as i32,
                issue.resolved_at.as_ref().map(|dt| dt.to_rfc3339()),
            ],
        )?;

        Ok(conn.last_insert_rowid())
    }

    /// Get active issues (unresolved)
    pub fn get_active_issues(&self) -> Result<Vec<Issue>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, timestamp, issue_type, title, description, drive, disc, resolved, resolved_at
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
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

        Ok(issues)
    }

    /// Get all issues (including resolved)
    pub fn get_all_issues(&self, limit: usize) -> Result<Vec<Issue>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, timestamp, issue_type, title, description, drive, disc, resolved, resolved_at
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
            "SELECT id, name, created_at FROM shows ORDER BY name ASC"
        )?;

        let shows = stmt.query_map([], |row| {
            Ok(Show {
                id: Some(row.get(0)?),
                name: row.get(1)?,
                created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(2)?)
                    .unwrap()
                    .with_timezone(&Utc),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

        Ok(shows)
    }

    /// Get a show by ID
    pub fn get_show(&self, id: i64) -> Result<Option<Show>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, created_at FROM shows WHERE id = ?1"
        )?;

        let result = stmt.query_row([id], |row| {
            Ok(Show {
                id: Some(row.get(0)?),
                name: row.get(1)?,
                created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(2)?)
                    .unwrap()
                    .with_timezone(&Utc),
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

    /// Add a rip history entry
    pub fn add_rip_history(&self, entry: &RipHistory) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        
        conn.execute(
            "INSERT INTO rip_history (timestamp, drive, disc, title, disc_type, status, duration_seconds, file_size_bytes, output_path, error_message)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
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
            ],
        )?;

        Ok(conn.last_insert_rowid())
    }

    /// Get statistics summary
    pub fn get_statistics(&self) -> Result<serde_json::Value> {
        let conn = self.conn.lock().unwrap();
        
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

    /// Get recent rip history
    pub fn get_rip_history(&self, limit: i64) -> Result<Vec<RipHistory>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, timestamp, drive, disc, title, disc_type, status, duration_seconds, file_size_bytes, output_path, error_message
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
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

        Ok(history)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_database_creation() {
        let db = Database::new().unwrap();
        assert!(db.get_recent_logs(10).is_ok());
    }

    #[test]
    fn test_add_log() {
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
    fn test_add_issue() {
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
        };

        let id = db.add_issue(&issue).unwrap();
        assert!(id > 0);
    }
}

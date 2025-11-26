use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph},
    Frame, Terminal,
};
use std::io;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::ripper::RipProgress;

#[derive(Debug, Clone)]
pub struct DriveState {
    pub device: String,
    pub progress: Option<RipProgress>,
    pub album_info: Option<String>,
    pub logs: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum InputMode {
    Normal,
    AwaitingTitleInput { device: String, default_title: Option<String> },
    #[allow(dead_code)]
    AwaitingEpisodeInput { device: String, title: String },
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub drives: Vec<DriveState>,
    pub rsync_logs: Vec<String>,
    pub rename_logs: Vec<String>,
    pub should_quit: bool,
    pub input_mode: InputMode,
    pub current_input: String,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            drives: Vec::new(),
            rsync_logs: Vec::new(),
            rename_logs: Vec::new(),
            should_quit: false,
            input_mode: InputMode::Normal,
            current_input: String::new(),
        }
    }
}

impl AppState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_drive_log(&mut self, device: &str, message: String) {
        let formatted = format!("[{}] {}", chrono::Local::now().format("%H:%M:%S"), message);
        
        if let Some(drive) = self.drives.iter_mut().find(|d| d.device == device) {
            drive.logs.push(formatted);
            // Keep only last 50 logs per drive
            if drive.logs.len() > 50 {
                drive.logs.remove(0);
            }
        } else {
            // Create drive if it doesn't exist
            self.drives.push(DriveState {
                device: device.to_string(),
                progress: None,
                album_info: None,
                logs: vec![formatted],
            });
        }
    }
    
    pub fn add_rsync_log(&mut self, message: String) {
        let formatted = format!("[{}] {}", chrono::Local::now().format("%H:%M:%S"), message);
        self.rsync_logs.push(formatted);
        // Keep only last 100 rsync logs
        if self.rsync_logs.len() > 100 {
            self.rsync_logs.remove(0);
        }
    }
    
    pub fn add_rename_log(&mut self, device: &str, message: String) {
        let formatted = format!("[{}] [{}] {}", chrono::Local::now().format("%H:%M:%S"), device, message);
        self.rename_logs.push(formatted);
        // Keep only last 100 rename logs
        if self.rename_logs.len() > 100 {
            self.rename_logs.remove(0);
        }
    }
    
    // Keep for backward compatibility
    pub fn add_log(&mut self, message: String) {
        // Add to first drive or do nothing if no drives
        if let Some(drive) = self.drives.first_mut() {
            let formatted = format!("[{}] {}", chrono::Local::now().format("%H:%M:%S"), message);
            drive.logs.push(formatted);
            if drive.logs.len() > 50 {
                drive.logs.remove(0);
            }
        }
    }
}

pub struct Tui {
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
    pub state: Arc<Mutex<AppState>>,
}

impl Tui {
    pub fn new() -> Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;

        Ok(Self {
            terminal,
            state: Arc::new(Mutex::new(AppState::new())),
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        loop {
            self.draw().await?;

            if event::poll(std::time::Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        let mut state = self.state.lock().await;
                        
                        match &state.input_mode {
                            InputMode::Normal => {
                                match key.code {
                                    KeyCode::Char('q') | KeyCode::Esc => {
                                        state.should_quit = true;
                                        break;
                                    }
                                    _ => {}
                                }
                            }
                            InputMode::AwaitingTitleInput { .. } | InputMode::AwaitingEpisodeInput { .. } => {
                                match key.code {
                                    KeyCode::Char(c) => {
                                        state.current_input.push(c);
                                    }
                                    KeyCode::Backspace => {
                                        state.current_input.pop();
                                    }
                                    KeyCode::Enter => {
                                        // Submit input - transition to Normal mode
                                        state.input_mode = InputMode::Normal;
                                    }
                                    KeyCode::Esc => {
                                        // Cancel input
                                        state.input_mode = InputMode::Normal;
                                        state.current_input.clear();
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }

            let state = self.state.lock().await;
            if state.should_quit {
                break;
            }
        }

        Ok(())
    }
    
    /// Prompt for TV show title
    #[allow(dead_code)]
    pub async fn prompt_title(&self, device: &str, default_title: Option<String>) -> Result<String> {
        {
            let mut state = self.state.lock().await;
            state.input_mode = InputMode::AwaitingTitleInput { 
                device: device.to_string(), 
                default_title: default_title.clone() 
            };
            state.current_input = default_title.clone().unwrap_or_default();
        }
        
        // Wait for Enter key
        loop {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            let state = self.state.lock().await;
            
            if matches!(state.input_mode, InputMode::Normal) {
                // User cancelled
                return Err(anyhow::anyhow!("Input cancelled"));
            }
            
            // Check if Enter was pressed by polling events
            if event::poll(std::time::Duration::from_millis(10))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press && key.code == KeyCode::Enter {
                        let result = state.current_input.clone();
                        drop(state);
                        
                        let mut state = self.state.lock().await;
                        state.input_mode = InputMode::Normal;
                        state.current_input.clear();
                        
                        return Ok(result);
                    }
                }
            }
        }
    }
    
    /// Prompt for starting episode number
    #[allow(dead_code)]
    pub async fn prompt_episode(&self, device: &str, title: &str) -> Result<u32> {
        {
            let mut state = self.state.lock().await;
            state.input_mode = InputMode::AwaitingEpisodeInput { 
                device: device.to_string(), 
                title: title.to_string() 
            };
            state.current_input = "1".to_string(); // Default to episode 1
        }
        
        // Wait for Enter key
        loop {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            let state = self.state.lock().await;
            
            if matches!(state.input_mode, InputMode::Normal) {
                // User cancelled
                return Err(anyhow::anyhow!("Input cancelled"));
            }
            
            // Check if Enter was pressed
            if event::poll(std::time::Duration::from_millis(10))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press && key.code == KeyCode::Enter {
                        let result = state.current_input.parse::<u32>().unwrap_or(1);
                        drop(state);
                        
                        let mut state = self.state.lock().await;
                        state.input_mode = InputMode::Normal;
                        state.current_input.clear();
                        
                        return Ok(result);
                    }
                }
            }
        }
    }

    async fn draw(&mut self) -> Result<()> {
        let state = self.state.lock().await;
        let state_clone = state.clone();
        drop(state);

        self.terminal.draw(|f| {
            ui(f, &state_clone);
        })?;

        Ok(())
    }

    pub async fn add_log(&self, message: String) {
        let mut state = self.state.lock().await;
        state.add_log(message);
    }
}

impl Drop for Tui {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(
            self.terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        );
        let _ = self.terminal.show_cursor();
    }
}

fn ui(f: &mut Frame, state: &AppState) {
    let has_rsync_logs = !state.rsync_logs.is_empty();
    let has_rename_logs = !state.rename_logs.is_empty();
    
    let chunks = match (has_rsync_logs, has_rename_logs) {
        (true, true) => {
            // Both rsync and rename logs active
            Layout::default()
                .direction(Direction::Vertical)
                .margin(0)
                .constraints([
                    Constraint::Length(3),          // Header
                    Constraint::Percentage(40),     // Drives + logs section
                    Constraint::Percentage(30),     // Rename logs
                    Constraint::Percentage(30),     // Rsync logs
                ])
                .split(f.area())
        }
        (true, false) => {
            // Only rsync logs active
            Layout::default()
                .direction(Direction::Vertical)
                .margin(0)
                .constraints([
                    Constraint::Length(3),          // Header
                    Constraint::Percentage(60),     // Drives + logs section
                    Constraint::Percentage(40),     // Rsync logs
                ])
                .split(f.area())
        }
        (false, true) => {
            // Only rename logs active
            Layout::default()
                .direction(Direction::Vertical)
                .margin(0)
                .constraints([
                    Constraint::Length(3),          // Header
                    Constraint::Percentage(60),     // Drives + logs section
                    Constraint::Percentage(40),     // Rename logs
                ])
                .split(f.area())
        }
        (false, false) => {
            // No extra logs active
            Layout::default()
                .direction(Direction::Vertical)
                .margin(0)
                .constraints([
                    Constraint::Length(3),          // Header
                    Constraint::Min(5),              // Drives + logs section (flexible)
                ])
                .split(f.area())
        }
    };

    // Header
    render_header(f, chunks[0], state);

    // Drives with their individual log windows
    render_drives_with_logs(f, chunks[1], state);
    
    // Rename and rsync log windows (if active)
    match (has_rsync_logs, has_rename_logs) {
        (true, true) => {
            render_rename_logs(f, chunks[2], state);
            render_rsync_logs(f, chunks[3], state);
        }
        (true, false) => {
            render_rsync_logs(f, chunks[2], state);
        }
        (false, true) => {
            render_rename_logs(f, chunks[2], state);
        }
        (false, false) => {}
    }
    
    // Render input dialog overlay if in input mode
    match &state.input_mode {
        InputMode::AwaitingTitleInput { device, default_title } => {
            render_input_dialog(f, "TV Show Title", &format!("Enter title for {} (or press Enter to use default)", device), &state.current_input, default_title.as_deref());
        }
        InputMode::AwaitingEpisodeInput { device: _, title } => {
            render_input_dialog(f, "Starting Episode", &format!("Disc starts with episode # for '{}'", title), &state.current_input, None);
        }
        InputMode::Normal => {}
    }
}

fn render_header(f: &mut Frame, area: Rect, state: &AppState) {
    let active_rips = state.drives.len();

    let title = vec![
        Span::styled("🎵 ", Style::default().fg(Color::Cyan)),
        Span::styled("Ripley", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw(" - Automated Optical Disc Ripper | "),
        Span::styled(format!("{} active", active_rips), Style::default().fg(Color::Green)),
        Span::raw(" | Press "),
        Span::styled("q", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw(" to quit"),
    ];

    let header = Paragraph::new(Line::from(title))
        .block(Block::default().borders(Borders::ALL).style(Style::default().fg(Color::White)));

    f.render_widget(header, area);
}

fn render_drives_with_logs(f: &mut Frame, area: Rect, state: &AppState) {
    if state.drives.is_empty() {
        let message = Paragraph::new("Waiting for audio CDs...\nInsert a CD into any drive to begin.")
            .block(Block::default().borders(Borders::ALL).title("Drives"))
            .style(Style::default().fg(Color::DarkGray));
        f.render_widget(message, area);
        return;
    }

    // Each drive gets: 4 lines for progress + 12 lines for logs = 16 lines
    let height_per_drive = 16;
    let constraints: Vec<Constraint> = state.drives.iter()
        .map(|_| Constraint::Length(height_per_drive))
        .chain(std::iter::once(Constraint::Min(0)))
        .collect();

    let drive_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    for (idx, drive) in state.drives.iter().enumerate() {
        render_drive_with_log(f, drive_chunks[idx], drive);
    }
}

fn render_drive_with_log(f: &mut Frame, area: Rect, drive: &DriveState) {
    // Split into progress section (4 lines) and log section (rest)
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),    // Progress
            Constraint::Min(8),       // Log
        ])
        .split(area);
    
    // Render progress
    let title = if let Some(ref info) = drive.album_info {
        format!("{} - {}", drive.device, info)
    } else {
        drive.device.clone()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .style(Style::default().fg(Color::Cyan));

    let inner_area = block.inner(sections[0]);
    f.render_widget(block, sections[0]);
    
    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(inner_area);

    if let Some(ref progress) = drive.progress {
        let status_color = match progress.status {
            crate::ripper::RipStatus::Complete => Color::Green,
            crate::ripper::RipStatus::Error(_) => Color::Red,
            _ => Color::Yellow,
        };

        let info_text = format!(
            "Track {}/{}: {} - {:?}",
            progress.current_track,
            progress.total_tracks,
            progress.track_name,
            progress.status
        );

        let info = Paragraph::new(info_text)
            .style(Style::default().fg(status_color));
        f.render_widget(info, inner[0]);

        let gauge = Gauge::default()
            .gauge_style(Style::default().fg(status_color).bg(Color::Black))
            .ratio((progress.percentage / 100.0) as f64)
            .label(format!("{:.1}%", progress.percentage));
        f.render_widget(gauge, inner[1]);
    }
    
    // Render log for this drive
    let log_block = Block::default()
        .borders(Borders::ALL)
        .title(format!("Log {}", drive.device))
        .style(Style::default().fg(Color::White));
    
    let log_inner = log_block.inner(sections[1]);
    f.render_widget(log_block, sections[1]);
    
    let available_height = log_inner.height as usize;
    let log_items: Vec<ListItem> = drive.logs.iter()
        .rev()
        .take(available_height)
        .rev()
        .map(|log| ListItem::new(log.as_str()))
        .collect();

    let logs_widget = List::new(log_items);
    f.render_widget(logs_widget, log_inner);
}

fn render_rsync_logs(f: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title("📤 Rsync to /Volumes/video/RawRips")
        .style(Style::default().fg(Color::Magenta));
    
    let inner = block.inner(area);
    f.render_widget(block, area);
    
    let available_height = inner.height as usize;
    let log_items: Vec<ListItem> = state.rsync_logs.iter()
        .rev()
        .take(available_height)
        .rev()
        .map(|log| ListItem::new(log.as_str()))
        .collect();

    let logs_widget = List::new(log_items);
    f.render_widget(logs_widget, inner);
}

fn render_rename_logs(f: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title("🎬 Episode Matching & Renaming")
        .style(Style::default().fg(Color::Cyan));
    
    let inner = block.inner(area);
    f.render_widget(block, area);
    
    let available_height = inner.height as usize;
    let log_items: Vec<ListItem> = state.rename_logs.iter()
        .rev()
        .take(available_height)
        .rev()
        .map(|log| ListItem::new(log.as_str()))
        .collect();

    let logs_widget = List::new(log_items);
    f.render_widget(logs_widget, inner);
}

fn render_input_dialog(f: &mut Frame, title: &str, prompt: &str, current_input: &str, default_hint: Option<&str>) {
    // Center the dialog
    let area = f.area();
    let dialog_width = 80.min(area.width - 4);
    let dialog_height = 9;
    
    let x = (area.width.saturating_sub(dialog_width)) / 2;
    let y = (area.height.saturating_sub(dialog_height)) / 2;
    
    let dialog_area = Rect {
        x,
        y,
        width: dialog_width,
        height: dialog_height,
    };
    
    // Clear the area first (render a filled block)
    let clear_block = Block::default()
        .style(Style::default().bg(Color::Black));
    f.render_widget(clear_block, dialog_area);
    
    // Render dialog
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .style(Style::default().fg(Color::Yellow).bg(Color::Black));
    
    let inner = block.inner(dialog_area);
    f.render_widget(block, dialog_area);
    
    // Layout for dialog content
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),  // Prompt text
            Constraint::Length(1),  // Spacer
            Constraint::Length(1),  // Input box
            Constraint::Length(1),  // Spacer
            Constraint::Length(1),  // Hint/default
            Constraint::Length(1),  // Help text
        ])
        .split(inner);
    
    // Prompt
    let prompt_widget = Paragraph::new(prompt)
        .style(Style::default().fg(Color::White));
    f.render_widget(prompt_widget, chunks[0]);
    
    // Input box
    let input_widget = Paragraph::new(format!("> {}", current_input))
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
    f.render_widget(input_widget, chunks[2]);
    
    // Hint/default
    if let Some(default) = default_hint {
        let hint_widget = Paragraph::new(format!("Default: {}", default))
            .style(Style::default().fg(Color::DarkGray));
        f.render_widget(hint_widget, chunks[4]);
    }
    
    // Help text
    let help_widget = Paragraph::new("Press Enter to confirm, Esc to cancel")
        .style(Style::default().fg(Color::DarkGray));
    f.render_widget(help_widget, chunks[5]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_state_creation() {
        let state = AppState::default();
        assert_eq!(state.drives.len(), 0);
        assert_eq!(state.rsync_logs.len(), 0);
        assert_eq!(state.rename_logs.len(), 0);
        assert_eq!(state.current_input, "");
        assert!(matches!(state.input_mode, InputMode::Normal));
    }

    #[test]
    fn test_drive_state_creation() {
        let drive = DriveState {
            device: "/dev/sr0".to_string(),
            progress: None,
            album_info: None,
            logs: vec!["Test log".to_string()],
        };
        assert_eq!(drive.device, "/dev/sr0");
        assert_eq!(drive.logs.len(), 1);
        assert!(drive.progress.is_none());
        assert!(drive.album_info.is_none());
    }

    #[test]
    fn test_add_drive_log() {
        let mut state = AppState::default();
        state.add_drive_log("/dev/sr0", "First log".to_string());
        
        assert_eq!(state.drives.len(), 1);
        assert_eq!(state.drives[0].device, "/dev/sr0");
        assert_eq!(state.drives[0].logs.len(), 1);
        
        state.add_drive_log("/dev/sr0", "Second log".to_string());
        assert_eq!(state.drives[0].logs.len(), 2);
    }

    #[test]
    fn test_add_multiple_drives() {
        let mut state = AppState::default();
        state.add_drive_log("/dev/sr0", "Drive 1 log".to_string());
        state.add_drive_log("/dev/sr1", "Drive 2 log".to_string());
        
        assert_eq!(state.drives.len(), 2);
        assert_eq!(state.drives[0].device, "/dev/sr0");
        assert_eq!(state.drives[1].device, "/dev/sr1");
    }

    #[test]
    fn test_add_rsync_log() {
        let mut state = AppState::default();
        state.add_rsync_log("Rsync started".to_string());
        
        assert_eq!(state.rsync_logs.len(), 1);
        assert!(state.rsync_logs[0].contains("Rsync started"));
    }

    #[test]
    fn test_add_rename_log() {
        let mut state = AppState::default();
        state.add_rename_log("/dev/sr0", "Matched episode".to_string());
        
        assert_eq!(state.rename_logs.len(), 1);
        assert!(state.rename_logs[0].contains("/dev/sr0"));
        assert!(state.rename_logs[0].contains("Matched episode"));
    }

    #[test]
    fn test_rsync_log_limit() {
        let mut state = AppState::default();
        
        // Add 101 logs
        for i in 0..101 {
            state.add_rsync_log(format!("Log {}", i));
        }
        
        // Should only keep last 100
        assert_eq!(state.rsync_logs.len(), 100);
        assert!(state.rsync_logs[0].contains("Log 1")); // First one removed
    }

    #[test]
    fn test_rename_log_limit() {
        let mut state = AppState::default();
        
        // Add 101 logs
        for i in 0..101 {
            state.add_rename_log("/dev/sr0", format!("Rename {}", i));
        }
        
        // Should only keep last 100
        assert_eq!(state.rename_logs.len(), 100);
        assert!(state.rename_logs[0].contains("Rename 1")); // First one removed
    }

    #[test]
    fn test_input_mode_variants() {
        let normal = InputMode::Normal;
        let title = InputMode::AwaitingTitleInput {
            device: "/dev/sr0".to_string(),
            default_title: Some("Test Show".to_string()),
        };
        
        assert!(matches!(normal, InputMode::Normal));
        match title {
            InputMode::AwaitingTitleInput { device, default_title } => {
                assert_eq!(device, "/dev/sr0");
                assert_eq!(default_title, Some("Test Show".to_string()));
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_log_timestamping() {
        let mut state = AppState::default();
        state.add_rsync_log("Test message".to_string());
        
        // Check that log contains timestamp format [HH:MM:SS]
        let log = &state.rsync_logs[0];
        assert!(log.contains("["));
        assert!(log.contains("]"));
        assert!(log.contains("Test message"));
    }
}


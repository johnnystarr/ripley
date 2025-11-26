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
pub struct AppState {
    pub drives: Vec<DriveState>,
    pub should_quit: bool,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            drives: Vec::new(),
            should_quit: false,
        }
    }

    pub fn add_drive_log(&mut self, device: &str, message: String) {
        if let Some(drive) = self.drives.iter_mut().find(|d| d.device == device) {
            let formatted = format!("[{}] {}", chrono::Local::now().format("%H:%M:%S"), message);
            drive.logs.push(formatted);
            // Keep only last 50 logs per drive
            if drive.logs.len() > 50 {
                drive.logs.remove(0);
            }
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
                        match key.code {
                            KeyCode::Char('q') | KeyCode::Esc => {
                                let mut state = self.state.lock().await;
                                state.should_quit = true;
                                break;
                            }
                            _ => {}
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
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(0)
        .constraints([
            Constraint::Length(3),          // Header
            Constraint::Min(5),              // Drives + logs section (flexible)
        ])
        .split(f.area());

    // Header
    render_header(f, chunks[0], state);

    // Drives with their individual log windows
    render_drives_with_logs(f, chunks[1], state);
}

fn render_header(f: &mut Frame, area: Rect, state: &AppState) {
    let active_rips = state.drives.iter()
        .filter(|d| d.progress.is_some())
        .count();

    let title = vec![
        Span::styled("🎵 ", Style::default().fg(Color::Cyan)),
        Span::styled("Ripley", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw(" - Automated CD Ripper | "),
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



use anyhow::Result;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph, Wrap},
    Frame, Terminal,
};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::io;
use std::time::Duration;
use tokio::time;
use crate::agent::AgentClient;
use crate::config::AgentConfig;
use crate::job_worker::JobWorker;
use std::sync::Arc;
use std::path::Path;

pub struct TuiApp {
    agent_client: Arc<AgentClient>,
    config: AgentConfig,
    job_worker: Arc<JobWorker>,
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
    should_quit: bool,
    status: String,
    instructions: Vec<String>,
}

impl TuiApp {
    pub fn new(agent_client: Arc<AgentClient>, config: AgentConfig, job_worker: Arc<JobWorker>) -> Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        
        Ok(Self {
            agent_client,
            config,
            job_worker,
            terminal,
            should_quit: false,
            status: "Initializing...".to_string(),
            instructions: vec![],
        })
    }
    
    pub async fn run(&mut self) -> Result<()> {
        // Start background heartbeat task
        {
            let client = self.agent_client.clone();
            let interval = self.config.heartbeat_interval_seconds;
            tokio::spawn(async move {
                let mut interval = time::interval(Duration::from_secs(interval));
                loop {
                    interval.tick().await;
                    if let Err(e) = client.heartbeat().await {
                        tracing::warn!("Heartbeat failed: {}", e);
                    }
                }
            });
        }
        
        // Main event loop
        loop {
            // Update status and instructions before drawing
            if let Some(agent_id) = self.agent_client.agent_id() {
                self.status = format!("Connected as {}", agent_id);
            } else {
                self.status = "Not registered".to_string();
            }
            
            // Get current job
            let current_job = {
                let current_job_arc = self.job_worker.current_job();
                let job_guard = current_job_arc.lock().await;
                job_guard.clone()
            };
            
            // Poll for instructions
            match self.agent_client.get_instructions().await {
                Ok(instructions) => {
                    self.instructions = instructions.iter()
                        .map(|i| format!("[{}] {} - {}", i.id, i.instruction_type, i.status))
                        .collect();
                }
                Err(e) => {
                    self.status = format!("Error: {}", e);
                }
            }
            
            // Draw UI
            self.terminal.draw(|f| {
                let status = &self.status;
                let instructions: Vec<ListItem> = self.instructions.iter()
                    .map(|i| ListItem::new(i.as_str()))
                    .collect();
                
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(3),  // Status header
                        Constraint::Length(8),  // Current job info
                        Constraint::Min(0),     // Instructions
                    ])
                    .split(f.size());
                
                // Header
                let header = Paragraph::new(vec![
                    Line::from(vec![
                        Span::styled(
                            "Ripley Agent",
                            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                        ),
                        Span::raw(" - "),
                        Span::raw(status),
                    ]),
                ])
                .block(Block::default().borders(Borders::ALL).title("Status"))
                .alignment(Alignment::Left);
                f.render_widget(header, chunks[0]);
                
                // Current job panel
                if let Some(ref job) = current_job {
                    let progress = job.progress;
                    let status_color = match job.status.as_str() {
                        "processing" => Color::Yellow,
                        "completed" => Color::Green,
                        "failed" => Color::Red,
                        _ => Color::White,
                    };
                    
                    // Store formatted strings in variables to avoid temporary value issues
                    let progress_text = format!("{:.1}%", progress);
                    let input_filename = Path::new(&job.input_file_path)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(&job.input_file_path)
                        .to_string();
                    
                    let job_info = vec![
                        Line::from(vec![
                            Span::styled("Job ID: ", Style::default().fg(Color::Cyan)),
                            Span::raw(&job.job_id),
                        ]),
                        Line::from(vec![
                            Span::styled("Status: ", Style::default().fg(Color::Cyan)),
                            Span::styled(&job.status, Style::default().fg(status_color)),
                        ]),
                        Line::from(vec![
                            Span::styled("Progress: ", Style::default().fg(Color::Cyan)),
                            Span::raw(&progress_text),
                        ]),
                        Line::from(vec![
                            Span::styled("Input: ", Style::default().fg(Color::Cyan)),
                            Span::raw(&input_filename),
                        ]),
                    ];
                    
                    let inner_chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([
                            Constraint::Length(4),
                            Constraint::Length(1),
                        ])
                        .split(chunks[1]);
                    
                    let info_block = Paragraph::new(job_info)
                        .block(Block::default().borders(Borders::NONE))
                        .wrap(Wrap { trim: true });
                    f.render_widget(info_block, inner_chunks[0]);
                    
                    // Progress bar
                    let progress_label = format!("{:.1}%", progress);
                    let progress_gauge = Gauge::default()
                        .block(Block::default().borders(Borders::NONE))
                        .gauge_style(Style::default().fg(Color::Green))
                        .percent((progress as u16).min(100))
                        .label(&progress_label);
                    f.render_widget(progress_gauge, inner_chunks[1]);
                    
                    // Draw border around the whole job panel
                    let job_block = Block::default()
                        .borders(Borders::ALL)
                        .title("Current Job");
                    f.render_widget(job_block, chunks[1]);
                } else {
                    let no_job = Paragraph::new(vec![
                        Line::from(vec![
                            Span::styled("No active job", Style::default().fg(Color::DarkGray)),
                        ]),
                        Line::from(vec![
                            Span::raw("Waiting for upscaling jobs..."),
                        ]),
                    ])
                    .block(Block::default().borders(Borders::ALL).title("Current Job"))
                    .alignment(Alignment::Center);
                    f.render_widget(no_job, chunks[1]);
                }
                
                // Instructions list
                let list = List::new(instructions)
                    .block(Block::default().borders(Borders::ALL).title("Instructions"))
                    .style(Style::default().fg(Color::White));
                f.render_widget(list, chunks[2]);
            })?;
            
            if crossterm::event::poll(Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        match key.code {
                            KeyCode::Char('q') | KeyCode::Esc => {
                                self.should_quit = true;
                            }
                            _ => {}
                        }
                    }
                }
            }
            
            if self.should_quit {
                break;
            }
        }
        
        // Cleanup
        disable_raw_mode()?;
        execute!(
            self.terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        self.terminal.show_cursor()?;
        
        Ok(())
    }
    
}


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

pub struct TuiApp {
    agent_client: AgentClient,
    config: AgentConfig,
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
    should_quit: bool,
    status: String,
    instructions: Vec<String>,
}

impl TuiApp {
    pub fn new(agent_client: AgentClient, config: AgentConfig) -> Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        
        Ok(Self {
            agent_client,
            config,
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
                        Constraint::Length(3),
                        Constraint::Min(0),
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
                
                // Instructions list
                let list = List::new(instructions)
                    .block(Block::default().borders(Borders::ALL).title("Instructions"))
                    .style(Style::default().fg(Color::White));
                f.render_widget(list, chunks[1]);
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


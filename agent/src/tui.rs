use anyhow::Result;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph, Wrap},
    Terminal,
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

pub struct TuiApp {
    agent_client: Option<Arc<AgentClient>>,
    config: AgentConfig,
    job_worker: Option<Arc<JobWorker>>,
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
    should_quit: bool,
    status: String,
    instructions: Vec<String>,
    connection_state: ConnectionState,
    server_url_input: String,
    agent_name_input: String,
    editing_field: EditingField,
    connection_logs: Vec<String>,
    connection_in_progress: bool,
    job_history: Vec<(String, String, f32)>, // (job_id, status, progress)
}

#[derive(Clone, Copy, PartialEq)]
enum EditingField {
    ServerUrl,
    AgentName,
    None,
}

#[derive(Clone)]
enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Failed(String),
}

impl TuiApp {
    pub fn new(config: AgentConfig) -> Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        
        Ok(Self {
            agent_client: None,
            config: config.clone(),
            job_worker: None,
            terminal,
            should_quit: false,
            status: "Disconnected - Configure connection".to_string(),
            instructions: vec![],
            connection_state: ConnectionState::Disconnected,
            server_url_input: config.server_url.clone(),
            agent_name_input: config.agent_name.clone(),
            editing_field: if config.server_url.is_empty() {
                EditingField::ServerUrl
            } else if config.agent_name.is_empty() || config.agent_name == "agent" || config.agent_name.starts_with("agent-") {
                EditingField::AgentName
            } else {
                EditingField::None
            },
            connection_logs: vec![],
            connection_in_progress: false,
            job_history: vec![],
        })
    }
    
    fn add_log(&mut self, message: String) {
        self.connection_logs.push(message);
        if self.connection_logs.len() > 50 {
            self.connection_logs.remove(0);
        }
    }
    
    async fn connect_to_server(&mut self) {
        // Prevent multiple simultaneous connection attempts
        if self.connection_in_progress {
            return;
        }
        
        // Validate both fields
        if self.server_url_input.trim().is_empty() {
            self.connection_state = ConnectionState::Failed("Server URL cannot be empty".to_string());
            self.connection_in_progress = false;
            return;
        }
        
        if self.agent_name_input.trim().is_empty() {
            self.connection_state = ConnectionState::Failed("Agent name cannot be empty".to_string());
            self.connection_in_progress = false;
            return;
        }
        
        self.connection_in_progress = true;
        
        // Update config with both values
        self.config.server_url = self.server_url_input.trim().to_string();
        self.config.agent_name = self.agent_name_input.trim().to_string();
        if let Err(e) = self.config.save() {
            self.add_log(format!("Failed to save config: {}", e));
        }
        
        self.connection_state = ConnectionState::Connecting;
        self.status = format!("Connecting to {} as {}...", self.config.server_url, self.config.agent_name);
        self.add_log(format!("Connecting to: {} as {}", self.config.server_url, self.config.agent_name));
        
        // Create agent client
        match AgentClient::new(self.config.clone()) {
            Ok(client) => {
                let agent_client = Arc::new(client);
                self.add_log("Agent client created".to_string());
                
                // Try to register
                self.add_log("Registering agent...".to_string());
                match agent_client.register().await {
                    Ok(_) => {
                        self.add_log("Registration successful".to_string());
                        if let Some(agent_id) = agent_client.agent_id() {
                            self.add_log(format!("Agent ID: {}", agent_id));
                            self.status = format!("Connected as {}", agent_id);
                        }
                        
                        // Create job worker
                        match JobWorker::new(Arc::clone(&agent_client), None) {
                            Ok(worker) => {
                                let job_worker = Arc::new(worker);
                                self.add_log("Job worker initialized".to_string());
                                
                                // Start job worker
                                let job_worker_clone = Arc::clone(&job_worker);
                                tokio::spawn(async move {
                                    if let Err(e) = job_worker_clone.run().await {
                                        tracing::error!("Job worker failed: {}", e);
                                    }
                                });
                                
                                // Start heartbeat
                                let client_clone = Arc::clone(&agent_client);
                                let interval = self.config.heartbeat_interval_seconds;
                                tokio::spawn(async move {
                                    let mut interval = time::interval(Duration::from_secs(interval));
                                    loop {
                                        interval.tick().await;
                                        if let Err(e) = client_clone.heartbeat().await {
                                            tracing::warn!("Heartbeat failed: {}", e);
                                        }
                                    }
                                });
                                
                                self.agent_client = Some(agent_client);
                                self.job_worker = Some(job_worker);
                                self.connection_state = ConnectionState::Connected;
                                self.editing_field = EditingField::None;
                                self.connection_in_progress = false;
                                self.add_log("Connection established".to_string());
                            }
                            Err(e) => {
                                self.connection_state = ConnectionState::Failed(format!("Failed to create job worker: {}", e));
                                self.connection_in_progress = false;
                                self.add_log(format!("Job worker error: {}", e));
                            }
                        }
                    }
                    Err(e) => {
                        self.connection_state = ConnectionState::Failed(format!("Registration failed: {}", e));
                        self.connection_in_progress = false;
                        self.add_log(format!("Registration error: {}", e));
                        self.status = format!("Connection failed: {}", e);
                    }
                }
            }
            Err(e) => {
                self.connection_state = ConnectionState::Failed(format!("Failed to create agent client: {}", e));
                self.connection_in_progress = false;
                self.add_log(format!("Client creation error: {}", e));
                self.status = format!("Connection failed: {}", e);
            }
        }
    }
    
    pub async fn run(&mut self) -> Result<()> {
        // Main event loop
        loop {
            // Handle connection if in connecting state
            if matches!(self.connection_state, ConnectionState::Connecting) {
                self.connect_to_server().await;
            }
            
            // Update status based on connection state
            match &self.connection_state {
                ConnectionState::Disconnected => {
                    self.status = "Disconnected - Enter server URL and press Enter to connect".to_string();
                }
                ConnectionState::Connecting => {
                    self.status = "Connecting...".to_string();
                }
                ConnectionState::Connected => {
                    if let Some(ref client) = self.agent_client {
                        if let Some(agent_id) = client.agent_id() {
                            self.status = format!("Connected as {}", agent_id);
                        }
                    }
                }
                ConnectionState::Failed(ref error) => {
                    self.status = format!("Connection failed: {}", error);
                }
            }
            
            // Update instructions if connected
            if let Some(ref client) = self.agent_client {
                match client.get_instructions().await {
                    Ok(instructions) => {
                        self.instructions = instructions.iter()
                            .map(|i| format!("[{}] {} - {}", i.id, i.instruction_type, i.status))
                            .collect();
                    }
                    Err(_e) => {
                        // Don't spam error messages
                    }
                }
            }
            
            // Get current job and pause state if connected
            let (current_job, is_paused) = if let Some(ref worker) = self.job_worker {
                let current_job_arc = worker.current_job();
                let job_guard = current_job_arc.lock().await;
                let job = job_guard.clone();
                let paused = worker.is_paused().await;
                
                // Update job history when job completes
                if let Some(ref j) = job {
                    if j.status == "completed" || j.status == "failed" {
                        // Check if this job is already in history
                        if !self.job_history.iter().any(|(id, _, _)| id == &j.job_id) {
                            self.job_history.push((j.job_id.clone(), j.status.clone(), j.progress));
                            // Keep only last 10 jobs
                            if self.job_history.len() > 10 {
                                self.job_history.remove(0);
                            }
                        }
                    }
                }
                
                (job, paused)
            } else {
                (None, false)
            };
            
            // Draw UI - prepare data for drawing
            let status_str = self.status.clone();
            let instructions_clone = self.instructions.clone();
            let connection_state_clone = self.connection_state.clone();
            let server_url_input_clone = self.server_url_input.clone();
            let editing_url = self.editing_field == EditingField::ServerUrl;
            let logs_clone = self.connection_logs.clone();
            let current_job_clone = current_job.clone();
            let job_history_clone = self.job_history.clone();
            
            self.terminal.draw(|f| {
                Self::draw_ui(
                    f,
                    &status_str,
                    &instructions_clone,
                    &connection_state_clone,
                    &server_url_input_clone,
                    &self.agent_name_input,
                    self.editing_field,
                    &logs_clone,
                    &current_job_clone,
                    self.agent_client.as_ref().map(|_c| None::<String>),
                    &job_history_clone,
                    is_paused,
                );
            })?;
            
            if crossterm::event::poll(Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        match key.code {
                            KeyCode::Char('q') | KeyCode::Esc => {
                                if self.editing_field == EditingField::None || matches!(self.connection_state, ConnectionState::Connected) {
                                    self.should_quit = true;
                                } else {
                                    // Cancel editing
                                    self.editing_field = EditingField::None;
                                }
                            }
                            KeyCode::Tab => {
                                // Switch between editing fields
                                if matches!(self.connection_state, ConnectionState::Disconnected | ConnectionState::Failed(_)) {
                                    self.editing_field = match self.editing_field {
                                        EditingField::ServerUrl => EditingField::AgentName,
                                        EditingField::AgentName => EditingField::ServerUrl,
                                        EditingField::None => EditingField::ServerUrl,
                                    };
                                }
                            }
                            KeyCode::Char('d') => {
                                // Disconnect from server
                                if matches!(self.connection_state, ConnectionState::Connected) {
                                    self.add_log("Disconnecting from server...".to_string());
                                    self.connection_state = ConnectionState::Disconnected;
                                    self.status = "Disconnected".to_string();
                                    self.agent_client = None;
                                    self.job_worker = None;
                                    self.add_log("Disconnected successfully".to_string());
                                }
                            }
                            KeyCode::Char('p') => {
                                // Pause job processing
                                if let Some(ref worker) = self.job_worker {
                                    if !worker.is_paused().await {
                                        worker.pause().await;
                                        self.add_log("Job processing paused".to_string());
                                    }
                                }
                            }
                            KeyCode::Char('r') => {
                                // Resume job processing
                                if let Some(ref worker) = self.job_worker {
                                    if worker.is_paused().await {
                                        worker.resume().await;
                                        self.add_log("Job processing resumed".to_string());
                                    }
                                }
                            }
                            KeyCode::Enter => {
                                if matches!(self.connection_state, ConnectionState::Disconnected | ConnectionState::Failed(_)) {
                                    match self.editing_field {
                                        EditingField::ServerUrl => {
                                            // Move to agent name if URL is entered
                                            if !self.server_url_input.trim().is_empty() {
                                                self.config.server_url = self.server_url_input.trim().to_string();
                                                self.editing_field = EditingField::AgentName;
                                                self.add_log(format!("Server URL set: {}", self.config.server_url));
                                            }
                                        }
                                        EditingField::AgentName => {
                                            // Save agent name and attempt connection
                                            if !self.agent_name_input.trim().is_empty() {
                                                self.config.agent_name = self.agent_name_input.trim().to_string();
                                                if let Err(e) = self.config.save() {
                                                    self.add_log(format!("Failed to save config: {}", e));
                                                } else {
                                                    self.add_log(format!("Agent name set: {}", self.config.agent_name));
                                                }
                                                
                                                // Validate both fields are set
                                                if !self.config.server_url.trim().is_empty() && !self.config.agent_name.trim().is_empty() {
                                                    self.editing_field = EditingField::None;
                                                    self.connection_state = ConnectionState::Connecting;
                                                    self.status = "Connecting...".to_string();
                                                    self.add_log(format!("Connecting to: {} as {}", self.config.server_url, self.config.agent_name));
                                                } else {
                                                    self.add_log("Please enter both server URL and agent name".to_string());
                                                }
                                            }
                                        }
                                        EditingField::None => {
                                            // Try to connect if both fields are set
                                            if !self.config.server_url.trim().is_empty() && !self.config.agent_name.trim().is_empty() {
                                                self.connection_state = ConnectionState::Connecting;
                                                self.status = "Connecting...".to_string();
                                                self.add_log(format!("Connecting to: {} as {}", self.config.server_url, self.config.agent_name));
                                            }
                                        }
                                    }
                                }
                            }
                            KeyCode::Char('c') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                                match self.editing_field {
                                    EditingField::ServerUrl => self.server_url_input.clear(),
                                    EditingField::AgentName => self.agent_name_input.clear(),
                                    EditingField::None => {}
                                }
                            }
                            KeyCode::Char(c) => {
                                if matches!(self.connection_state, ConnectionState::Disconnected | ConnectionState::Failed(_)) {
                                    match self.editing_field {
                                        EditingField::ServerUrl => {
                                            // Allow all characters for URL/IP (including dots, colons, slashes, etc.)
                                            self.server_url_input.push(c);
                                        }
                                        EditingField::AgentName => {
                                            // Allow alphanumeric, dash, underscore for agent name
                                            if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' {
                                                self.agent_name_input.push(c);
                                            }
                                        }
                                        EditingField::None => {}
                                    }
                                }
                            }
                            KeyCode::Backspace => {
                                if matches!(self.connection_state, ConnectionState::Disconnected | ConnectionState::Failed(_)) {
                                    match self.editing_field {
                                        EditingField::ServerUrl => {
                                            self.server_url_input.pop();
                                        }
                                        EditingField::AgentName => {
                                            self.agent_name_input.pop();
                                        }
                                        EditingField::None => {}
                                    }
                                }
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
        
        // Graceful shutdown
        if let Some(ref worker) = self.job_worker {
            // Signal shutdown to worker (will need to add shutdown flag)
            // For now, just clear current job
            let current_job_arc = worker.current_job();
            let job_guard = current_job_arc.lock().await;
            if job_guard.is_some() {
                // Job is running, wait a bit for it to finish or cancel
                drop(job_guard);
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
            }
        }
        
        if let Some(ref client) = self.agent_client {
            // Send final heartbeat to mark as offline (heartbeat method doesn't take status, but we can disconnect)
            let _ = client.heartbeat().await;
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
    
    fn draw_ui(
        f: &mut ratatui::Frame,
        status: &str,
        instructions: &[String],
        connection_state: &ConnectionState,
        server_url_input: &str,
        agent_name_input: &str,
        editing_field: EditingField,
        connection_logs: &[String],
        current_job: &Option<crate::agent::UpscalingJob>,
        _agent_id: Option<Option<String>>,
        job_history: &[(String, String, f32)],
        is_paused: bool,
    ) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),  // Status header
                Constraint::Length(1),  // Controls/Help
                Constraint::Length(8),  // Connection/Job info
                Constraint::Length(6),  // Job history
                Constraint::Min(0),     // Instructions/Logs
            ])
            .split(f.area());
        
        // Header
        let status_color = match connection_state {
            ConnectionState::Connected => Color::Green,
            ConnectionState::Connecting => Color::Yellow,
            ConnectionState::Failed(_) => Color::Red,
            ConnectionState::Disconnected => Color::Gray,
        };
        
        let header = Paragraph::new(vec![
            Line::from(vec![
                Span::styled(
                    "Ripley Agent",
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                ),
                Span::raw(" - "),
                Span::styled(status, Style::default().fg(status_color)),
            ]),
        ])
        .block(Block::default().borders(Borders::ALL).title("Status"))
        .alignment(Alignment::Left);
        f.render_widget(header, chunks[0]);
        
        // Controls/Help bar
        let controls_text = if matches!(connection_state, ConnectionState::Connected) {
            if is_paused {
                format!("Controls: [P]ause (paused) | [R]esume | [D]isconnect | [Q]uit")
            } else {
                format!("Controls: [P]ause | [R]esume | [D]isconnect | [Q]uit")
            }
        } else {
            format!("Controls: [Q]uit | Enter to connect")
        };
        
        let controls = Paragraph::new(controls_text)
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().borders(Borders::NONE));
        f.render_widget(controls, chunks[1]);
        
        // Connection panel or Job panel
        if matches!(connection_state, ConnectionState::Disconnected | ConnectionState::Connecting | ConnectionState::Failed(_)) {
            // Show connection UI
            let inner_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(4),
                    Constraint::Length(4),
                    Constraint::Length(1),
                    Constraint::Min(0),
                ])
                .split(chunks[2]);
            
            // Server URL input
            let editing_url = editing_field == EditingField::ServerUrl;
            let url_prompt = Paragraph::new(vec![
                Line::from(vec![
                    Span::styled("Server URL/IP: ", Style::default().fg(Color::Cyan)),
                    Span::styled(
                        if server_url_input.is_empty() { "http://..." } else { server_url_input },
                        Style::default()
                            .fg(if editing_url { Color::Yellow } else { Color::White })
                            .add_modifier(if editing_url { Modifier::BOLD | Modifier::UNDERLINED } else { Modifier::empty() }),
                    ),
                    if editing_url {
                        Span::styled("_", Style::default().fg(Color::Yellow))
                    } else {
                        Span::raw("")
                    },
                ]),
                Line::from(vec![
                    Span::styled("Example: http://192.168.1.100:3000", Style::default().fg(Color::DarkGray)),
                ]),
            ])
            .block(Block::default()
                .borders(Borders::ALL)
                .title(if editing_url { "Server URL (Press Tab for Agent Name)" } else { "Server URL" }))
            .wrap(Wrap { trim: true });
            f.render_widget(url_prompt, inner_chunks[0]);
            
            // Agent Name input
            let editing_name = editing_field == EditingField::AgentName;
            let name_prompt = Paragraph::new(vec![
                Line::from(vec![
                    Span::styled("Agent Name: ", Style::default().fg(Color::Cyan)),
                    Span::styled(
                        if agent_name_input.is_empty() { "Enter agent name..." } else { agent_name_input },
                        Style::default()
                            .fg(if editing_name { Color::Yellow } else { Color::White })
                            .add_modifier(if editing_name { Modifier::BOLD | Modifier::UNDERLINED } else { Modifier::empty() }),
                    ),
                    if editing_name {
                        Span::styled("_", Style::default().fg(Color::Yellow))
                    } else {
                        Span::raw("")
                    },
                ]),
                Line::from(vec![
                    Span::styled("Press Enter after entering name to connect", Style::default().fg(Color::DarkGray)),
                ]),
            ])
            .block(Block::default()
                .borders(Borders::ALL)
                .title(if editing_name { "Agent Name (Press Tab for Server URL)" } else { "Agent Name" }))
            .wrap(Wrap { trim: true });
            f.render_widget(name_prompt, inner_chunks[1]);
            
            // Connection logs
            let log_items: Vec<ListItem> = connection_logs.iter()
                .map(|log| ListItem::new(log.as_str()))
                .collect();
            let log_list = List::new(log_items)
                .block(Block::default().borders(Borders::ALL).title("Connection Log"))
                .style(Style::default().fg(Color::White));
            f.render_widget(log_list, inner_chunks[3]);
        } else if matches!(connection_state, ConnectionState::Connected) {
            // Show connected but no job
            let no_job_text = if is_paused {
                vec![
                    Line::from(vec![
                        Span::styled("Status: ", Style::default().fg(Color::Cyan)),
                        Span::styled("PAUSED", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                    ]),
                    Line::from(vec![
                        Span::raw("Waiting for upscaling jobs..."),
                    ]),
                    Line::from(vec![
                        Span::raw("Press 'R' to resume processing"),
                    ]),
                ]
            } else {
                vec![
                    Line::from(vec![
                        Span::styled("Status: ", Style::default().fg(Color::Cyan)),
                        Span::styled("Connected", Style::default().fg(Color::Green)),
                    ]),
                    Line::from(vec![
                        Span::raw("Waiting for upscaling jobs..."),
                    ]),
                ]
            };
            
            let no_job = Paragraph::new(no_job_text)
                .block(Block::default().borders(Borders::ALL).title("Job Status"))
                .alignment(Alignment::Center);
            f.render_widget(no_job, chunks[2]);
        } else if let Some(ref job) = current_job {
            // Show job info
            let progress = job.progress;
            let status_color = match job.status.as_str() {
                "processing" => Color::Yellow,
                "completed" => Color::Green,
                "failed" => Color::Red,
                _ => Color::White,
            };
            
            let progress_text = format!("{:.1}%", progress);
            let input_filename = std::path::Path::new(&job.input_file_path)
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
                .split(chunks[2]);
            
            let info_block = Paragraph::new(job_info)
                .block(Block::default().borders(Borders::NONE))
                .wrap(Wrap { trim: true });
            f.render_widget(info_block, inner_chunks[0]);
            
            let progress_label = format!("{:.1}%", progress);
            let progress_gauge = Gauge::default()
                .block(Block::default().borders(Borders::NONE))
                .gauge_style(Style::default().fg(Color::Green))
                .percent((progress as u16).min(100))
                .label(&progress_label);
            f.render_widget(progress_gauge, inner_chunks[1]);
            
            let job_block = Block::default()
                .borders(Borders::ALL)
                .title("Current Job");
            f.render_widget(job_block, chunks[1]);
        } else {
            // No job
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
        
        // Job history
        let history_items: Vec<ListItem> = job_history.iter()
            .rev()
            .take(5)
            .map(|(job_id, status, progress)| {
                let status_color = match status.as_str() {
                    "completed" => Color::Green,
                    "failed" => Color::Red,
                    _ => Color::White,
                };
                let short_id = if job_id.len() > 12 {
                    &job_id[..12]
                } else {
                    job_id
                };
                ListItem::new(vec![
                    Line::from(vec![
                        Span::raw(format!("{} ", short_id)),
                        Span::styled(status.clone(), Style::default().fg(status_color)),
                        Span::raw(format!(" {:.1}%", progress)),
                    ]),
                ])
            })
            .collect();
        let history_list = List::new(history_items)
            .block(Block::default().borders(Borders::ALL).title("Recent Job History (Last 5)"))
            .style(Style::default().fg(Color::White));
        f.render_widget(history_list, chunks[4]);
        
        // Instructions list
        let instruction_items: Vec<ListItem> = instructions.iter()
            .map(|i| ListItem::new(i.as_str()))
            .collect();
        let list = List::new(instruction_items)
            .block(Block::default().borders(Borders::ALL).title("Instructions"))
            .style(Style::default().fg(Color::White));
        f.render_widget(list, chunks[3]);
    }
}

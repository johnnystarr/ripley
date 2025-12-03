use iced::widget::{button, column, container, row, scrollable, text, text_input, Column, image};
use iced::widget::image::Handle;
use iced::{Alignment, Application, Command, Element, Length, Settings, Theme, Color};
use iced::widget::container::Appearance;
use ripley::config::{Config, ShowSeed};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

fn main() -> iced::Result {
    RenameApp::run(Settings::with_flags(()))
}

#[derive(Debug, Clone)]
enum Message {
    #[allow(dead_code)]
    LoadFiles,
    FilesLoaded(Vec<FileEntry>),
    FileSelected(usize),
    PreviewFramesExtracted(Vec<PathBuf>),
    ShowNameChanged(String),
    EpisodeNameChanged(String),
    ChangeClicked,
    EpisodeInfoLoaded(Result<EpisodeInfo, String>),
    FrameSelected(usize), // Select a frame for large preview
    RenameClicked,
    RenameComplete(Result<(), String>),
    SuccessMessageDismissed, // Auto-dismiss success message
    PlayPreview,
    SeekForward,
    SeekBackward,
}

struct RenameApp {
    files: Vec<FileEntry>,
    selected_index: Option<usize>,
    show_name: String,
    inferred_show_name: String,
    episode_name: String,
    episode_info: Option<EpisodeInfo>,
    preview_frames: Vec<PathBuf>, // Extracted still frames
    selected_frame_index: Option<usize>, // Selected frame for large preview
    preview_position: u32, // Current position in seconds
    error_message: Option<String>,
    success_message: Option<String>, // Success message that auto-dismisses
    config: Config,
    show_preview_settings: HashMap<String, (u32, u32)>, // show_name -> (start_time, duration)
    show_name_map: HashMap<String, String>, // normalized -> original name
}

#[derive(Debug, Clone)]
struct FileEntry {
    path: PathBuf,
    relative_path: String,
    name: String,
}

#[derive(Debug, Clone)]
struct EpisodeInfo {
    season: u32,
    episode: u32,
    title: String,
}

impl Application for RenameApp {
    type Message = Message;
    type Theme = Theme;
    type Executor = iced::executor::Default;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Command<Message>) {
        // Load config
        let config = Config::load().unwrap_or_else(|_| Config::default());
        
        // Build show preview settings map
        // Also create a normalized name map for fuzzy matching
        let mut show_preview_settings = HashMap::new();
        let mut show_name_map = HashMap::new(); // normalized -> original
        
        for show in &config.seed.shows {
            let show_name = match show {
                ShowSeed::Simple(name) => name.clone(),
                ShowSeed::WithConfig { name, preview, .. } => {
                    if let Some(preview) = preview {
                        show_preview_settings.insert(name.clone(), (preview.start_time, preview.duration));
                    }
                    name.clone()
                }
            };
            // Default preview settings if not specified
            if !show_preview_settings.contains_key(&show_name) {
                show_preview_settings.insert(show_name.clone(), (0, 10));
            }
            // Create normalized version for matching (uppercase, no apostrophes, no extra spaces)
            let normalized = show_name.to_uppercase()
                .replace("'", "")
                .replace("  ", " ")
                .trim()
                .to_string();
            show_name_map.insert(normalized, show_name);
        }
        
        let app = RenameApp {
            files: Vec::new(),
            selected_index: None,
            show_name: String::new(),
            inferred_show_name: String::new(),
            episode_name: String::new(),
            episode_info: None,
            preview_frames: Vec::new(),
            selected_frame_index: None,
            preview_position: 0,
            error_message: None,
            success_message: None,
            config,
            show_preview_settings,
            show_name_map,
        };
        
        (app, Command::perform(load_files(), Message::FilesLoaded))
    }

    fn title(&self) -> String {
        "Ripley Rename".to_string()
    }

    fn theme(&self) -> Theme {
        Theme::Dark
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::LoadFiles => {
                Command::perform(load_files(), Message::FilesLoaded)
            }
            Message::FilesLoaded(files) => {
                self.files = files;
                Command::none()
            }
            Message::FileSelected(index) => {
                if index < self.files.len() {
                    self.selected_index = Some(index);
                    let file = &self.files[index];
                    
                    // Extract show name from filename
                    let inferred = extract_show_name(&file.name);
                    
                    // Try to match with config shows using fuzzy matching
                    let normalized_inferred = inferred.to_uppercase()
                        .replace("'", "")
                        .replace("  ", " ")
                        .trim()
                        .to_string();
                    
                    // Find best match from config
                    let matched_name = self.show_name_map.iter()
                        .find(|(normalized, _)| {
                            normalized_inferred.contains(normalized.as_str()) || 
                            normalized.contains(normalized_inferred.as_str()) ||
                            normalized_inferred.as_str() == normalized.as_str()
                        })
                        .map(|(_, original)| original.clone())
                        .unwrap_or_else(|| inferred.clone());
                    
                    self.inferred_show_name = inferred;
                    self.show_name = matched_name;
                    self.episode_name = String::new();
                    self.episode_info = None;
                    self.error_message = None;
                    self.preview_position = 0;
                    self.preview_frames.clear();
                    self.selected_frame_index = None;
                    self.success_message = None;
                    
                    // Extract preview
                    let _file_path = file.path.clone();
                    let _show_name = self.show_name.clone();
                    let _preview_settings = self.show_preview_settings.clone();
                    
                    // Extract preview in background
                    let file_path_clone = file.path.clone();
                    let show_name_clone = self.show_name.clone();
                    let preview_settings_clone = self.show_preview_settings.clone();
                    
                    Command::perform(
                        async move {
                            extract_preview_frames(file_path_clone, show_name_clone, &preview_settings_clone).await
                        },
                        |result| {
                            match result {
                                Ok(frames) => Message::PreviewFramesExtracted(frames),
                                Err(_) => Message::FileSelected(0)
                            }
                        }
                    )
                } else {
                    Command::none()
                }
            }
            Message::ShowNameChanged(name) => {
                self.show_name = name;
                Command::none()
            }
            Message::EpisodeNameChanged(name) => {
                self.episode_name = name;
                Command::none()
            }
            Message::ChangeClicked => {
                if self.show_name.is_empty() {
                    self.error_message = Some("Please enter a show name".to_string());
                    return Command::none();
                }
                
                // Extract episode title from episode_name field (everything after " - " or just the text)
                let episode_title = if let Some(dash_pos) = self.episode_name.find(" - ") {
                    self.episode_name[dash_pos + 3..].trim().to_string()
                } else if self.episode_name.starts_with("S") && self.episode_name.contains("E") {
                    // Already has S##E## format, extract title part
                    if let Some(dash_pos) = self.episode_name.find(" - ") {
                        self.episode_name[dash_pos + 3..].trim().to_string()
                    } else {
                        // Try to find episode title after S##E## pattern
                        let re = regex::Regex::new(r"(?i)S\d+E\d+\s*[-.]?\s*").ok();
                        if let Some(re) = re {
                            re.replace(&self.episode_name, "").trim().to_string()
                        } else {
                            self.episode_name.clone()
                        }
                    }
                } else {
                    // Use the whole episode_name as the title
                    self.episode_name.trim().to_string()
                };
                
                if episode_title.is_empty() {
                    self.error_message = Some("Please enter an episode title (what you see in the video frames)".to_string());
                    return Command::none();
                }
                
                let show_name = self.show_name.clone();
                let config = self.config.clone();
                Command::perform(
                    find_episode_by_title(show_name, episode_title, config),
                    Message::EpisodeInfoLoaded
                )
            }
            Message::EpisodeInfoLoaded(result) => {
                match result {
                    Ok(ep_info) => {
                        // Update episode info and episode name field with the matched episode
                        self.episode_info = Some(ep_info.clone());
                        self.episode_name = format!("S{:02}E{:02} - {}", ep_info.season, ep_info.episode, ep_info.title);
                        self.error_message = None;
                    }
                    Err(e) => {
                        self.error_message = Some(format!("Could not find episode: {}", e));
                        self.episode_info = None;
                    }
                }
                Command::none()
            }
            Message::RenameClicked => {
                // Validate that we have episode information
                if self.episode_name.is_empty() {
                    self.error_message = Some("Please enter or select an episode name".to_string());
                    return Command::none();
                }
                
                if let Some(index) = self.selected_index {
                    if index < self.files.len() {
                        let file = self.files[index].path.clone();
                        let new_name = self.generate_new_filename();
                        
                        if new_name.is_empty() {
                            self.error_message = Some("Could not generate filename. Please check show name and episode name format (S##E## - Title)".to_string());
                            return Command::none();
                        }
                        
                        let rename_cmd = Command::perform(
                            rename_file(file, new_name),
                            Message::RenameComplete
                        );
                        return rename_cmd;
                    }
                }
                Command::none()
            }
            Message::RenameComplete(result) => {
                match result {
                    Ok(_) => {
                        // Show success message
                        self.success_message = Some("SUCCESS: File renamed successfully!".to_string());
                        self.error_message = None;
                        
                        // Schedule auto-dismiss and move to next file after 2 seconds
                        Command::perform(
                            async {
                                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                            },
                            |_| Message::SuccessMessageDismissed
                        )
                    }
                    Err(e) => {
                        self.error_message = Some(format!("Rename failed: {}", e));
                        self.success_message = None;
                        Command::none()
                    }
                }
            }
            Message::SuccessMessageDismissed => {
                // Remove file from list and move to next
                if let Some(index) = self.selected_index {
                    if index < self.files.len() {
                        self.files.remove(index);
                        self.success_message = None;
                        
                        // Select next file if available
                        if !self.files.is_empty() {
                            let next_index = if index < self.files.len() {
                                index
                            } else {
                                index.saturating_sub(1)
                            };
                            
                            // Reset state
                            self.selected_index = Some(next_index);
                            let file = &self.files[next_index];
                            
                            // Extract show name from filename
                            let inferred = extract_show_name(&file.name);
                            
                            // Try to match with config shows using fuzzy matching
                            let normalized_inferred = inferred.to_uppercase()
                                .replace("'", "")
                                .replace("  ", " ")
                                .trim()
                                .to_string();
                            
                            // Find best match from config
                            let matched_name = self.show_name_map.iter()
                                .find(|(normalized, _)| {
                                    normalized_inferred.contains(normalized.as_str()) || 
                                    normalized.contains(normalized_inferred.as_str()) ||
                                    normalized_inferred.as_str() == normalized.as_str()
                                })
                                .map(|(_, original)| original.clone())
                                .unwrap_or_else(|| inferred.clone());
                            
                            self.inferred_show_name = inferred;
                            self.show_name = matched_name;
                            self.episode_name = String::new();
                            self.episode_info = None;
                            self.error_message = None;
                            self.preview_position = 0;
                            self.preview_frames.clear();
                            self.selected_frame_index = None;
                            
                            // Extract preview
                            let file_path = file.path.clone();
                            let show_name = self.show_name.clone();
                            let preview_settings = self.show_preview_settings.clone();
                            
                            return Command::perform(
                                async move {
                                    extract_preview_frames(file_path, show_name, &preview_settings).await
                                },
                                |result| {
                                    match result {
                                        Ok(frames) => Message::PreviewFramesExtracted(frames),
                                        Err(_) => Message::FileSelected(0)
                                    }
                                }
                            );
                        } else {
                            // No more files
                            self.selected_index = None;
                            self.show_name.clear();
                            self.episode_name.clear();
                            self.episode_info = None;
                        }
                    }
                }
                Command::none()
            }
            Message::FrameSelected(index) => {
                if index < self.preview_frames.len() {
                    self.selected_frame_index = Some(index);
                }
                Command::none()
            }
            Message::PlayPreview => {
                // Preview frames are already displayed in the app
                Command::none()
            }
            Message::PreviewFramesExtracted(frames) => {
                self.preview_frames = frames;
                Command::none()
            }
            Message::SeekForward => {
                self.preview_position += 1;
                // Re-extract preview at new position
                if let Some(index) = self.selected_index {
                    if index < self.files.len() {
                        let file = &self.files[index];
                        let file_path = file.path.clone();
                        let show_name = self.show_name.clone();
                        let preview_settings = self.show_preview_settings.clone();
                        let seek_pos = self.preview_position;
                        
                        return Command::perform(
                            async move {
                                extract_preview_frames_at_position(file_path, show_name, &preview_settings, seek_pos).await
                            },
                            |result| {
                                match result {
                                    Ok(frames) => Message::PreviewFramesExtracted(frames),
                                    Err(_) => Message::FileSelected(0)
                                }
                            }
                        );
                    }
                }
                Command::none()
            }
            Message::SeekBackward => {
                if self.preview_position > 0 {
                    self.preview_position -= 1;
                    // Re-extract preview at new position
                    if let Some(index) = self.selected_index {
                        if index < self.files.len() {
                            let file = &self.files[index];
                            let file_path = file.path.clone();
                            let show_name = self.show_name.clone();
                            let preview_settings = self.show_preview_settings.clone();
                            let seek_pos = self.preview_position;
                            
                            return Command::perform(
                                async move {
                                    extract_preview_frames_at_position(file_path, show_name, &preview_settings, seek_pos).await
                                },
                                |result| {
                                    match result {
                                        Ok(frames) => Message::PreviewFramesExtracted(frames),
                                        Err(_) => Message::FileSelected(0)
                                    }
                                }
                            );
                        }
                    }
                }
                Command::none()
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let left_panel = self.view_file_list();
        let right_panel = self.view_file_details();
        
        container(
            row![left_panel, right_panel]
                .spacing(10)
                .padding(10)
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }
}

impl RenameApp {
    fn view_file_list(&self) -> Element<'_, Message> {
        let mut file_list = Column::new().spacing(8);
        
        for (index, file) in self.files.iter().enumerate() {
            let is_selected = self.selected_index == Some(index);
            let file_text = text(format!("📄 {}", &file.relative_path))
                .size(13);
            
            let file_row = container(file_text)
                .padding(10)
                .style(if is_selected {
                    iced::theme::Container::Custom(Box::new(SelectedEpisodeStyle))
                } else {
                    iced::theme::Container::Box
                });
            
            file_list = file_list.push(
                button(file_row)
                    .style(iced::theme::Button::Text)
                    .on_press(Message::FileSelected(index))
            );
        }
        
        container(
            column![
                text("Files").size(16)
                    .style(iced::theme::Text::Color(Color::from_rgb(0.4, 0.7, 1.0))),
                scrollable(file_list)
                    .width(Length::Fill)
                    .height(Length::Fill)
            ]
            .spacing(10)
            .padding(10)
        )
        .width(Length::Fixed(400.0))
        .height(Length::Fill)
        .into()
    }
    
    fn view_file_details(&self) -> Element<'_, Message> {
        if let Some(index) = self.selected_index {
            if index < self.files.len() {
                let file = &self.files[index];
                
                let show_name_input = row![
                    text("Show Name:").size(14).width(Length::Fixed(140.0))
                        .style(iced::theme::Text::Color(Color::from_rgb(0.85, 0.85, 0.85))),
                    text_input("Enter show name", &self.show_name)
                        .on_input(Message::ShowNameChanged)
                        .width(Length::Fill)
                        .padding(10)
                ]
                .spacing(15)
                .align_items(Alignment::Center);
                
                let episode_name_input = row![
                    text("Episode Title:").size(14).width(Length::Fixed(140.0))
                        .style(iced::theme::Text::Color(Color::from_rgb(0.85, 0.85, 0.85))),
                    text_input("Enter episode title (what you see in frames)", &self.episode_name)
                        .on_input(Message::EpisodeNameChanged)
                        .width(Length::Fill)
                        .padding(10),
                    button("Lookup")
                        .padding([10, 20])
                        .style(iced::theme::Button::Secondary)
                        .on_press(Message::ChangeClicked)
                ]
                .spacing(15)
                .align_items(Alignment::Center);
                
                // Show matched episode info if available
                let episode_info_display = if let Some(ref ep_info) = self.episode_info {
                    container(
                        row![
                            text("Matched: ").size(14)
                                .style(iced::theme::Text::Color(Color::from_rgb(0.5, 0.8, 0.5))),
                            text(format!("S{:02}E{:02} - {}", ep_info.season, ep_info.episode, ep_info.title))
                                .size(14)
                                .style(iced::theme::Text::Color(Color::from_rgb(0.9, 0.9, 0.9)))
                        ]
                        .spacing(5)
                        .align_items(Alignment::Center)
                    )
                    .padding(12)
                    .style(iced::theme::Container::Box)
                } else {
                    container(text(""))
                        .width(Length::Fill)
                        .height(Length::Fixed(0.0))
                };
                
                // Video preview area - display still frames
                let (preview_start, preview_duration) = self.show_preview_settings.get(&self.show_name)
                    .copied()
                    .unwrap_or((0, 10));
                let current_start = preview_start + self.preview_position;
                let current_end = current_start + preview_duration;
                
                let preview_status = if !self.preview_frames.is_empty() {
                    format!("Preview frames: {}s - {}s", current_start, current_end)
                } else {
                    "Extracting preview frames...".to_string()
                };
                
                // Display frames in a grid (thumbnails)
                let frame_display: Element<Message> = if !self.preview_frames.is_empty() {
                    let mut frame_row = row![].spacing(5);
                    for (idx, frame_path) in self.preview_frames.iter().enumerate() {
                        if frame_path.exists() {
                            // Load image handle
                            let img_handle = Handle::from_path(frame_path.clone());
                            let img = image(img_handle);
                            let is_selected = self.selected_frame_index == Some(idx);
                            
                            frame_row = frame_row.push(
                                button(
                                    container(img)
                                        .width(Length::Fixed(120.0))
                                        .height(Length::Fixed(90.0))
                                        .style(if is_selected {
                                            iced::theme::Container::Custom(Box::new(SelectedFrameStyle))
                                        } else {
                                            iced::theme::Container::Box
                                        })
                                )
                                .style(iced::theme::Button::Text)
                                .on_press(Message::FrameSelected(idx))
                            );
                        }
                    }
                    scrollable(frame_row)
                        .width(Length::Fill)
                        .height(Length::Fixed(100.0))
                        .into()
                } else {
                    container(text("Extracting frames..."))
                        .width(Length::Fill)
                        .height(Length::Fixed(100.0))
                        .into()
                };
                
                // Large preview of selected frame
                let large_preview: Element<Message> = if let Some(frame_idx) = self.selected_frame_index {
                    if frame_idx < self.preview_frames.len() {
                        let frame_path = &self.preview_frames[frame_idx];
                        if frame_path.exists() {
                            let img_handle = Handle::from_path(frame_path.clone());
                            let img = image(img_handle);
                            container(img)
                                .width(Length::Fill)
                                .height(Length::Fixed(300.0))
                                .style(iced::theme::Container::Box)
                                .into()
                        } else {
                            container(text("Frame not found"))
                                .width(Length::Fill)
                                .height(Length::Fixed(300.0))
                                .into()
                        }
                    } else {
                        container(text(""))
                            .width(Length::Fill)
                            .height(Length::Fixed(300.0))
                            .into()
                    }
                } else if !self.preview_frames.is_empty() {
                    // Show first frame by default
                    let frame_path = &self.preview_frames[0];
                    if frame_path.exists() {
                        let img_handle = Handle::from_path(frame_path.clone());
                        let img = image(img_handle);
                        container(img)
                            .width(Length::Fill)
                            .height(Length::Fixed(300.0))
                            .style(iced::theme::Container::Box)
                            .into()
                    } else {
                        container(text(""))
                            .width(Length::Fill)
                            .height(Length::Fixed(300.0))
                            .into()
                    }
                } else {
                    container(text(""))
                        .width(Length::Fill)
                        .height(Length::Fixed(300.0))
                        .into()
                };
                
                let preview_area = container(
                    column![
                        text("Video Preview").size(16).style(iced::theme::Text::Color(iced::Color::from_rgb(0.4, 0.7, 1.0))),
                        text(&preview_status).size(12).style(iced::theme::Text::Color(iced::Color::from_rgb(0.7, 0.7, 0.7))),
                        frame_display,
                        large_preview
                    ]
                    .spacing(10)
                )
                .width(Length::Fill)
                .height(Length::Fill)
                .padding(15)
                .style(iced::theme::Container::Box);
                
                let preview_controls = row![
                    button("<<").padding(8).on_press(Message::SeekBackward),
                    button("Play").padding(8).on_press(Message::PlayPreview),
                    button(">>").padding(8).on_press(Message::SeekForward),
                    text(format!("Position: {}s", self.preview_position))
                        .size(12)
                        .style(iced::theme::Text::Color(Color::from_rgb(0.7, 0.7, 0.7)))
                ]
                .spacing(10)
                .align_items(Alignment::Center);
                
                let rename_button = button(
                    container(
                        text("Rename File").size(14)
                    )
                    .padding([10, 20])
                )
                .style(iced::theme::Button::Primary)
                .on_press(Message::RenameClicked);
                
                let error_text = if let Some(ref error) = self.error_message {
                    container(
                        text(format!("ERROR: {}", error))
                            .size(13)
                            .style(iced::theme::Text::Color(Color::from_rgb(1.0, 0.4, 0.4)))
                    )
                    .padding(10)
                    .style(iced::theme::Container::Box)
                } else {
                    container(text("")).height(Length::Fixed(0.0))
                };
                
                let success_text = if let Some(ref msg) = self.success_message {
                    container(
                        text(msg)
                            .size(14)
                            .style(iced::theme::Text::Color(Color::from_rgb(0.4, 1.0, 0.4)))
                    )
                    .padding(12)
                    .style(iced::theme::Container::Box)
                } else {
                    container(text("")).height(Length::Fixed(0.0))
                };
                
                column![
                    container(
                        text(&file.name)
                            .size(18)
                            .style(iced::theme::Text::Color(Color::from_rgb(1.0, 1.0, 1.0)))
                    )
                    .padding(12)
                    .style(iced::theme::Container::Box),
                    show_name_input,
                    episode_name_input,
                    episode_info_display,
                    preview_area,
                    preview_controls,
                    rename_button,
                    success_text,
                    error_text
                ]
                .spacing(15)
                .padding(25)
                .into()
            } else {
                container(text("No file selected"))
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into()
            }
        } else {
            container(text("Select a file from the list"))
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        }
    }
    
    fn generate_new_filename(&self) -> String {
        // Parse episode info from episode_name field (format: "S##E## - Episode Title")
        // Or use episode_info if available
        if let Some(ref info) = self.episode_info {
            // Format: Show.S##E##.Episode.Title.ext
            let show_name = self.show_name.replace(' ', ".");
            let episode_title = info.title.split('/').next().unwrap_or(&info.title).trim();
            let episode_title_clean = episode_title.replace(' ', ".");
            
            if let Some(index) = self.selected_index {
                if index < self.files.len() {
                    let file = &self.files[index];
                    if let Some(ext) = file.path.extension().and_then(|s| s.to_str()) {
                        return format!("{}.S{:02}E{:02}.{}.{}", 
                            show_name, info.season, info.episode, episode_title_clean, ext);
                    }
                }
            }
        } else if !self.episode_name.is_empty() {
            // Try to parse from episode_name field
            // Format: "S##E## - Episode Title" or just "Episode Title"
            let re = regex::Regex::new(r"(?i)S(\d+)E(\d+)").ok();
            if let Some(re) = re {
                if let Some(caps) = re.captures(&self.episode_name) {
                    if let (Ok(season), Ok(episode)) = (caps[1].parse::<u32>(), caps[2].parse::<u32>()) {
                        // Extract episode title (everything after " - " or after "S##E##")
                        let episode_title = if let Some(dash_pos) = self.episode_name.find(" - ") {
                            self.episode_name[dash_pos + 3..].trim()
                        } else if let Some(m) = re.find(&self.episode_name) {
                            self.episode_name[m.end()..].trim_start_matches(|c: char| c == ' ' || c == '-' || c == '.')
                        } else {
                            &self.episode_name
                        };
                        
                        let show_name = self.show_name.replace(' ', ".");
                        let episode_title_clean = episode_title.split('/').next().unwrap_or(episode_title).trim().replace(' ', ".");
                        
                        if let Some(index) = self.selected_index {
                            if index < self.files.len() {
                                let file = &self.files[index];
                                if let Some(ext) = file.path.extension().and_then(|s| s.to_str()) {
                                    return format!("{}.S{:02}E{:02}.{}.{}", 
                                        show_name, season, episode, episode_title_clean, ext);
                                }
                            }
                        }
                    }
                }
            }
        }
        String::new()
    }
}

async fn load_files() -> Vec<FileEntry> {
    let rips_dir = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Desktop")
        .join("Rips");
    
    let mut files = Vec::new();
    
    for entry in WalkDir::new(&rips_dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                if matches!(ext.to_lowercase().as_str(), "mkv" | "mp4" | "mov") {
                    if let Ok(relative) = path.strip_prefix(&rips_dir) {
                        files.push(FileEntry {
                            path: path.to_path_buf(),
                            relative_path: relative.to_string_lossy().to_string(),
                            name: path.file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("unknown")
                                .to_string(),
                        });
                    }
                }
            }
        }
    }
    
    files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    files
}

fn extract_show_name(filename: &str) -> String {
    // Try to extract show name from filename
    // Common patterns:
    // - Show.Name.S01E01.Episode.Title.mkv
    // - Show Name - S01E01 - Episode Title.mkv
    // - Show_Name_S01E01_Episode_Title.mkv
    // - FOSTERS HOME FOR IMAGINARY FRIENDS VOLUME 2 DISC 1-C2_t02.mkv
    
    // Remove extension
    let name = Path::new(filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(filename);
    
    // Try to find S##E## pattern and extract everything before it
    if let Some(caps) = regex::Regex::new(r"(?i)S(\d+)E(\d+)")
        .ok()
        .and_then(|re| re.find(name))
    {
        let before = &name[..caps.start()];
        // Clean up: remove trailing dots, dashes, underscores, spaces, and volume/disc info
        let cleaned = before.trim_end_matches(|c: char| c == '.' || c == '-' || c == '_' || c.is_whitespace())
            .replace('_', " ")
            .replace('.', " ");
        
        // Remove common patterns like "VOLUME 2 DISC 1", "DISC 1", "VOLUME 2", etc.
        let re_volume = regex::Regex::new(r"(?i)\s*(VOLUME|DISC|V|D)\s*\d+.*$").ok();
        let cleaned = if let Some(ref re) = re_volume {
            re.replace(&cleaned, "").trim().to_string()
        } else {
            cleaned.trim().to_string()
        };
        
        // Normalize: title case and handle apostrophes
        cleaned
    } else {
        // Fallback: try to extract from common patterns
        // Remove volume/disc info
        let re_volume = regex::Regex::new(r"(?i)\s*(VOLUME|DISC|V|D)\s*\d+.*$").ok();
        let cleaned = if let Some(ref re) = re_volume {
            re.replace(name, "").trim().to_string()
        } else {
            name.split('-')
                .next()
                .unwrap_or(name)
                .trim()
                .to_string()
        };
        
        cleaned.replace('_', " ")
            .replace('.', " ")
            .trim()
            .to_string()
    }
}

async fn extract_preview_frames(
    file_path: PathBuf,
    show_name: String,
    preview_settings: &HashMap<String, (u32, u32)>,
) -> Result<Vec<PathBuf>, String> {
    extract_preview_frames_at_position(file_path, show_name, preview_settings, 0).await
}

async fn extract_preview_frames_at_position(
    file_path: PathBuf,
    show_name: String,
    preview_settings: &HashMap<String, (u32, u32)>,
    position_offset: u32,
) -> Result<Vec<PathBuf>, String> {
    // Get preview settings for this show (default to 0s start, 10s duration)
    let (start_time, duration) = preview_settings
        .get(&show_name)
        .copied()
        .unwrap_or((0, 10));
    
    // Add position offset for seeking
    let actual_start = start_time + position_offset;
    
    // Create temp directory for frames
    let temp_dir = std::env::temp_dir();
    let frames_dir = temp_dir.join(format!("ripley-frames-{}-{}", 
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        position_offset));
    
    tokio::fs::create_dir_all(&frames_dir)
        .await
        .map_err(|e| format!("Failed to create frames directory: {}", e))?;
    
    // Extract 10 frames (one per second) using ffmpeg
    let mut frames = Vec::new();
    
    for i in 0..duration.min(10) {
        let frame_time = actual_start + i;
        let frame_path = frames_dir.join(format!("frame_{:02}.jpg", i));
        
        let output = tokio::process::Command::new("ffmpeg")
            .args([
                "-i", file_path.to_str().unwrap(),
                "-ss", &frame_time.to_string(),
                "-vframes", "1",
                "-q:v", "2", // High quality JPEG
                "-y", // Overwrite output
                frame_path.to_str().unwrap(),
            ])
            .output()
            .await
            .map_err(|e| format!("Failed to run ffmpeg: {}", e))?;
        
        if output.status.success() && frame_path.exists() {
            frames.push(frame_path);
        }
    }
    
    if frames.is_empty() {
        return Err("Failed to extract any frames".to_string());
    }
    
    Ok(frames)
}

async fn find_episode_by_title(show_name: String, episode_title: String, config: Config) -> Result<EpisodeInfo, String> {
    let api_key = config.tmdb_api_key
        .ok_or_else(|| "TMDB API key not configured".to_string())?;
    
    let client = reqwest::Client::builder()
        .user_agent("Ripley/0.1.0")
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
    
    // Search for TV show
    let search_url = format!(
        "https://api.themoviedb.org/3/search/tv?api_key={}&query={}",
        api_key,
        urlencoding::encode(&show_name)
    );
    
    let response = client.get(&search_url)
        .send()
        .await
        .map_err(|e| format!("TMDB request failed: {}", e))?;
    
    if !response.status().is_success() {
        return Err(format!("TMDB API returned error: {}", response.status()));
    }
    
    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse TMDB response: {}", e))?;
    
    let results = json.get("results")
        .and_then(|r| r.as_array())
        .ok_or_else(|| "No results found".to_string())?;
    
    if results.is_empty() {
        return Err("Show not found in TMDB".to_string());
    }
    
    let show = results[0].as_object()
        .ok_or_else(|| "Invalid show data".to_string())?;
    
    let show_id = show.get("id")
        .and_then(|id| id.as_i64())
        .ok_or_else(|| "Invalid show ID".to_string())?;
    
    // Search through seasons to find matching episode by title
    // Try first 5 seasons (covers most shows)
    let episode_title_lower = episode_title.to_lowercase();
    
    for season_num in 1..=5 {
        let season_url = format!(
            "https://api.themoviedb.org/3/tv/{}/season/{}?api_key={}",
            show_id, season_num, api_key
        );
        
        if let Ok(season_response) = client.get(&season_url).send().await {
            if season_response.status().is_success() {
                if let Ok(season_json) = season_response.json::<serde_json::Value>().await {
                    if let Some(episodes) = season_json.get("episodes").and_then(|e| e.as_array()) {
                        for ep in episodes {
                            if let Some(ep_obj) = ep.as_object() {
                                let ep_title = ep_obj.get("name")
                                    .and_then(|n| n.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                
                                // Check if episode title matches (case-insensitive, partial match)
                                let ep_title_lower = ep_title.to_lowercase();
                                if ep_title_lower.contains(&episode_title_lower) || 
                                   episode_title_lower.contains(&ep_title_lower) ||
                                   ep_title_lower == episode_title_lower {
                                    // Found matching episode!
                                    let episode_num = ep_obj.get("episode_number")
                                        .and_then(|n| n.as_u64())
                                        .map(|n| n as u32)
                                        .unwrap_or(0);
                                    
                                    return Ok(EpisodeInfo {
                                        season: season_num,
                                        episode: episode_num,
                                        title: ep_title,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    
    Err(format!("Episode '{}' not found in first 5 seasons of '{}'", episode_title, show_name))
}

// Custom style for selected frame
struct SelectedFrameStyle;

impl container::StyleSheet for SelectedFrameStyle {
    type Style = Theme;

    fn appearance(&self, _style: &Self::Style) -> Appearance {
        Appearance {
            text_color: None,
            background: Some(iced::Background::Color(Color::from_rgb(0.2, 0.5, 0.9))),
            border: iced::Border {
                color: Color::from_rgb(0.4, 0.7, 1.0),
                width: 2.0,
                radius: 4.0.into(),
            },
            shadow: Default::default(),
        }
    }
}

// Custom style for selected episode
struct SelectedEpisodeStyle;

impl container::StyleSheet for SelectedEpisodeStyle {
    type Style = Theme;

    fn appearance(&self, _style: &Self::Style) -> Appearance {
        Appearance {
            text_color: None,
            background: Some(iced::Background::Color(Color::from_rgb(0.15, 0.3, 0.5))),
            border: iced::Border {
                color: Color::from_rgb(0.4, 0.7, 1.0),
                width: 1.5,
                radius: 6.0.into(),
            },
            shadow: Default::default(),
        }
    }
}

async fn rename_file(file_path: PathBuf, new_name: String) -> Result<(), String> {
    if new_name.is_empty() {
        return Err("New filename is empty".to_string());
    }
    
    let parent = file_path.parent()
        .ok_or_else(|| "File has no parent directory".to_string())?;
    
    let new_path = parent.join(&new_name);
    
    tokio::fs::rename(&file_path, &new_path)
        .await
        .map_err(|e| format!("Failed to rename file: {}", e))?;
    
    Ok(())
}

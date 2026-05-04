// Copyright (c) 2026 Robert Simens. All Rights Reserved.
// Licensed under the Tempest AI Source-Available License.
// See the LICENSE file in the project root for full license text.

use miette::{Result, IntoDiagnostic};
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style, Modifier},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Sparkline},
    Frame, Terminal,
};
use syntect::parsing::SyntaxSet;
use syntect::highlighting::{ThemeSet, Style as SyntectStyle};
use syntect::easy::HighlightLines;
use syntect::util::LinesWithEndings;
use unicode_width::UnicodeWidthStr;
use std::io::stdout;
use std::time::{Duration, Instant};

pub enum AgentEvent {
    SystemUpdate(String), // Telemetry
    #[allow(dead_code)] Thinking(Option<String>),
    RequestInput(String, String), // (tool_name, question)
    RequestPrivileges {
        rationale: String,
        response_tx: tokio::sync::mpsc::Sender<ToolResponse>,
    },
    StreamToken(String),
    ReasoningToken(String),
    SubagentStatus(Option<String>),
    ContextStatus { used: usize, total: u64 },
    SentinelUpdate { active: Vec<String>, log: String },
    TelemetryMetrics { cpu: Option<u64>, gpu: Option<u64>, tps: Option<u64> },
    CommandOutput(String),
    EditorEdit { path: String, content: String },
}

pub enum ToolResponse {
    Confirmed(bool),
    Text(String),
    #[allow(dead_code)]
    Error(String),
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum FocusedPane {
    Chat,
    Reasoning,
    Explorer,
    CommandPalette,
}

pub struct App {
    pub input_buffer: String,
    pub messages: Vec<String>,
    pub current_stream: String,
    pub active_tool: Option<String>,
    pub telemetry_text: String,
    pub should_quit: bool,
    pub agent_mode: String,
    pub thinking_msg: Option<String>,
    pub reasoning_buffer: String,
    pub reasoning_lines: usize,
    pub reasoning_scroll: u16,
    pub list_state: ratatui::widgets::ListState,
    pub auto_scroll: bool,
    pub animation_tick: u32,
    pub pending_input: Option<(String, String)>,
    pub input_response_buffer: String,
    pub pending_privilege_request: Option<(String, tokio::sync::mpsc::Sender<ToolResponse>)>,
    pub context_used: usize,
    pub context_total: u64,
    pub active_sentinels: Vec<String>,
    pub sentinel_log: Vec<String>,
    pub engine_status: Option<String>,
    pub focused_pane: FocusedPane,
    pub show_reasoning: bool,
    pub show_explorer: bool,
    pub explorer_files: Vec<(String, bool)>, // (path, is_dir)
    pub explorer_state: ratatui::widgets::ListState,
    pub current_explorer_dir: std::path::PathBuf,
    pub command_output: Vec<String>,
    pub syntax_set: SyntaxSet,
    pub theme_set: ThemeSet,
    // --- 📊 SPARKLINE STATE ---
    pub cpu_history: Vec<u64>,
    pub gpu_history: Vec<u64>,
    pub tps_history: Vec<u64>, // Tokens Per Second
    // --- ⌨️ COMMAND PALETTE STATE ---
    pub show_command_palette: bool,
    pub command_palette_query: String,
    pub command_palette_options: Vec<String>,
    pub command_palette_state: ratatui::widgets::ListState,
    pub current_theme: String,
}

impl App {
    pub fn new(initial_theme: String) -> Self {
        Self {
            input_buffer: String::new(),
            messages: vec![
                "Welcome to Tempest AI TUI.".to_string(),
                "Type your request below and press Enter.".to_string(),
                "Press Esc to stop agent, Ctrl+C to exit.".to_string(),
            ],
            current_stream: String::new(),
            active_tool: None,
            telemetry_text: "Initializing systems...".to_string(),
            should_quit: false,
            agent_mode: "IDLE".to_string(),
            thinking_msg: None,
            reasoning_buffer: String::new(),
            reasoning_lines: 0,
            reasoning_scroll: 0,
            list_state: ratatui::widgets::ListState::default(),
            auto_scroll: true,
            animation_tick: 0,
            pending_input: None,
            input_response_buffer: String::new(),
            pending_privilege_request: None,
                context_used: 0,
            context_total: 0,
            active_sentinels: Vec::new(),
            sentinel_log: Vec::new(),
            engine_status: None,
            focused_pane: FocusedPane::Chat,
            show_reasoning: false,
            show_explorer: false,
            explorer_files: Vec::new(),
            explorer_state: ratatui::widgets::ListState::default(),
            current_explorer_dir: std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
            command_output: Vec::new(),
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
            cpu_history: vec![0; 100],
            gpu_history: vec![0; 100],
            tps_history: vec![0; 100],
            show_command_palette: false,
            command_palette_query: String::new(),
            command_palette_options: vec![
                "Hot-Swap: DeepSeek R1 (Distill)".to_string(),
                "Hot-Swap: Qwen 2.5 Coder".to_string(),
                "Toggle Safe Mode: ON/OFF".to_string(),
                "Recall: Latest Memory Item".to_string(),
                "System: Compact Context".to_string(),
                "Sentinel: Toggle Hardcore Mode".to_string(),
                "Theme: Base16 Ocean (Dark)".to_string(),
                "Theme: Base16 Mocha".to_string(),
                "Theme: Base16 Eighties".to_string(),
                "Theme: Solarized (Dark)".to_string(),
                "Theme: Solarized (Light)".to_string(),
            ],
            command_palette_state: ratatui::widgets::ListState::default(),
            current_theme: initial_theme,
        }
    }

    pub fn refresh_explorer(&mut self) {
        if let Ok(entries) = std::fs::read_dir(&self.current_explorer_dir) {
            let mut files = entries.filter_map(|e| e.ok())
                .map(|e| {
                    let path = e.path().file_name().unwrap_or_default().to_string_lossy().into_owned();
                    let is_dir = e.path().is_dir();
                    (path, is_dir)
                })
                .collect::<Vec<_>>();
            
            // Sort: Dirs first, then alpha
            files.sort_by(|a, b| {
                if a.1 != b.1 {
                    b.1.cmp(&a.1)
                } else {
                    a.0.to_lowercase().cmp(&b.0.to_lowercase())
                }
            });
            self.explorer_files = files;
        }
    }

    pub fn push_message(&mut self, msg: String) {
        self.messages.push(msg);
        if self.messages.len() > 1000 {
            self.messages.remove(0);
        }
        // Ensure scroll follows new messages if focused
        if self.focused_pane == FocusedPane::Chat {
            self.list_state.select(Some(self.messages.len().saturating_sub(1)));
        }
    }
}

pub async fn run_tui(
    mut agent_rx: tokio::sync::mpsc::Receiver<AgentEvent>, 
    user_tx: tokio::sync::mpsc::Sender<String>, 
    tool_tx: tokio::sync::mpsc::Sender<ToolResponse>,
    stop_flag: std::sync::Arc<std::sync::atomic::AtomicBool>,
    initial_theme: String,
) -> Result<()> {
    enable_raw_mode().into_diagnostic()?;
    stdout().execute(EnterAlternateScreen).into_diagnostic()?;
    stdout().execute(crossterm::event::EnableMouseCapture).into_diagnostic()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout())).into_diagnostic()?;

    let mut app = App::new(initial_theme);
    let tick_rate = Duration::from_millis(50);
    let mut last_tick = Instant::now();

    loop {
        terminal.draw(|f| ui(f, &mut app)).into_diagnostic()?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or(Duration::from_secs(0));

        if event::poll(timeout).into_diagnostic()? {
            let ev = event::read().into_diagnostic()?;
            
            // --- 🖱️ MOUSE HIT-TESTING ---
            if let Event::Mouse(mev) = &ev {
                if let crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left) = mev.kind {
                    let size = terminal.size().into_diagnostic()?;
                    let horizontal_chunks = Layout::default()
                        .direction(Direction::Horizontal)
                        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                        .split(size.into());

                    if horizontal_chunks[0].contains(ratatui::layout::Position { x: mev.column, y: mev.row }) {
                        app.focused_pane = FocusedPane::Chat;
                    } else if horizontal_chunks[1].contains(ratatui::layout::Position { x: mev.column, y: mev.row }) {
                        app.focused_pane = FocusedPane::Reasoning;
                    }
                }
                
                // MOUSE WHEEL SCROLLING
                if app.focused_pane == FocusedPane::Chat {
                    match mev.kind {
                        crossterm::event::MouseEventKind::ScrollUp => {
                            let cur = app.list_state.selected().unwrap_or(0);
                            app.list_state.select(Some(cur.saturating_sub(1)));
                            app.auto_scroll = false;
                        }
                        crossterm::event::MouseEventKind::ScrollDown => {
                            let cur = app.list_state.selected().unwrap_or(0);
                            app.list_state.select(Some(cur + 1));
                        }
                        _ => {}
                    }
                } else {
                    match mev.kind {
                        crossterm::event::MouseEventKind::ScrollUp => {
                            app.reasoning_scroll = app.reasoning_scroll.saturating_sub(1);
                        }
                        crossterm::event::MouseEventKind::ScrollDown => {
                            app.reasoning_scroll = app.reasoning_scroll.saturating_add(1);
                        }
                        _ => {}
                    }
                }
            }

            if let Event::Key(key) = ev {
                if let Some((_tool, _question)) = &app.pending_input {
                    match key.code {
                        KeyCode::Enter => {
                            let mut resp = app.input_response_buffer.clone();
                            if resp.is_empty() {
                                resp = "y".to_string(); // Default to approval on Enter
                            }
                            let _ = tool_tx.send(ToolResponse::Text(resp)).await;
                            app.pending_input = None;
                            app.input_response_buffer.clear();
                        }
                        KeyCode::Char(c) => app.input_response_buffer.push(c),
                        KeyCode::Backspace => { app.input_response_buffer.pop(); }
                        KeyCode::Esc => { 
                            let _ = tool_tx.send(ToolResponse::Text("n".to_string())).await; 
                            app.pending_input = None;
                            app.input_response_buffer.clear();
                        }
                        _ => {}
                    }
                } else if let Some((_rationale, resp_tx)) = &app.pending_privilege_request {
                    match key.code {
                        KeyCode::Char('y') | KeyCode::Char('Y') => {
                             let tx = resp_tx.clone();
                             tokio::spawn(async move {
                                 let _ = tx.send(ToolResponse::Confirmed(true)).await;
                             });
                             app.pending_privilege_request = None;
                        }
                        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                             let tx = resp_tx.clone();
                             tokio::spawn(async move {
                                 let _ = tx.send(ToolResponse::Confirmed(false)).await;
                             });
                             app.pending_privilege_request = None;
                        }
                        _ => {}
                    }
                } else {
                    match key.code {
                        KeyCode::Char(c) => {
                            if key.modifiers.contains(KeyModifiers::CONTROL) && (c == 'c' || c == 'C') {
                                app.should_quit = true;
                                stop_flag.store(true, std::sync::atomic::Ordering::Relaxed);
                            } else if key.modifiers.contains(KeyModifiers::CONTROL) && (c == 'e' || c == 'E') {
                                app.show_explorer = !app.show_explorer;
                                if app.show_explorer {
                                    app.refresh_explorer();
                                    app.focused_pane = FocusedPane::Explorer;
                                    app.explorer_state.select(Some(0));
                                } else if app.focused_pane == FocusedPane::Explorer {
                                    app.focused_pane = FocusedPane::Chat;
                                }
                            } else if key.modifiers.contains(KeyModifiers::CONTROL) && (c == 'p' || c == 'P') {
                                app.show_command_palette = !app.show_command_palette;
                                if app.show_command_palette {
                                    app.focused_pane = FocusedPane::CommandPalette;
                                    app.command_palette_query.clear();
                                    app.command_palette_state.select(Some(0));
                                } else if app.focused_pane == FocusedPane::CommandPalette {
                                    app.focused_pane = FocusedPane::Chat;
                                }
                            } else if app.focused_pane == FocusedPane::CommandPalette {
                                app.command_palette_query.push(c);
                            } else {
                                app.input_buffer.push(c);
                            }
                        }
                        KeyCode::Backspace => {
                            if app.focused_pane == FocusedPane::CommandPalette {
                                app.command_palette_query.pop();
                            } else {
                                app.input_buffer.pop();
                            }
                        }
                        KeyCode::Enter => {
                            if app.focused_pane == FocusedPane::CommandPalette {
                                if let Some(idx) = app.command_palette_state.selected() {
                                    let option = app.command_palette_options[idx].clone();
                                    app.push_message(format!("⚡ [PALETTE]: Executing '{}'", option));
                                    
                                    if option.starts_with("Theme: ") {
                                        let theme_name = match option.as_str() {
                                            "Theme: Base16 Ocean (Dark)" => "base16-ocean.dark",
                                            "Theme: Base16 Mocha" => "base16-mocha.dark",
                                            "Theme: Base16 Eighties" => "base16-eighties.dark",
                                            "Theme: Solarized (Dark)" => "Solarized (dark)",
                                            "Theme: Solarized (Light)" => "Solarized (light)",
                                            _ => "base16-ocean.dark",
                                        };
                                        app.current_theme = theme_name.to_string();
                                        app.push_message(format!("🎨 Aesthetic updated to {}", theme_name));
                                        
                                        // --- 💾 PERSIST TO CONFIG.TOML ---
                                        if let Ok(mut content) = std::fs::read_to_string("config.toml") {
                                            let re = regex::Regex::new(r#"(?m)^tui_theme\s*=\s*".*""#).unwrap();
                                            if re.is_match(&content) {
                                                content = re.replace(&content, &format!(r#"tui_theme = "{}""#, theme_name)).into_owned();
                                            } else {
                                                // If not found, append it to the [🧹 BASE SETTINGS] section or end of file
                                                if content.contains("[🧹 BASE SETTINGS]") {
                                                    content = content.replace("[🧹 BASE SETTINGS]", &format!("[🧹 BASE SETTINGS]\ntui_theme = \"{}\"", theme_name));
                                                } else {
                                                    content.push_str(&format!("\ntui_theme = \"{}\"\n", theme_name));
                                                }
                                            }
                                            let _ = std::fs::write("config.toml", content);
                                        }
                                    }

                                    app.show_command_palette = false;
                                    app.focused_pane = FocusedPane::Chat;
                                }
                            } else if app.focused_pane == FocusedPane::Explorer {
                                if let Some(idx) = app.explorer_state.selected() {
                                    if let Some((name, is_dir)) = app.explorer_files.get(idx) {
                                        if *is_dir {
                                            if name == ".." {
                                                app.current_explorer_dir.pop();
                                            } else {
                                                app.current_explorer_dir.push(name);
                                            }
                                            app.refresh_explorer();
                                            app.explorer_state.select(Some(0));
                                        } else {
                                            // Select file for context
                                            let full_path = app.current_explorer_dir.join(name);
                                            app.input_buffer.push_str(&format!(" [CONTEXT: {}] ", full_path.to_string_lossy()));
                                            app.focused_pane = FocusedPane::Chat;
                                        }
                                    }
                                }
                            } else if !app.input_buffer.is_empty() {
                                let msg = app.input_buffer.drain(..).collect::<String>();
                                app.push_message(format!("You: {}", msg));
                                let _ = user_tx.send(msg).await;
                                app.auto_scroll = true;
                            }
                        }
                        KeyCode::Tab => {
                            app.focused_pane = match app.focused_pane {
                                FocusedPane::Chat => {
                                    if app.show_reasoning { FocusedPane::Reasoning } else if app.show_explorer { FocusedPane::Explorer } else { FocusedPane::Chat }
                                },
                                FocusedPane::Reasoning => if app.show_explorer { FocusedPane::Explorer } else { FocusedPane::Chat },
                                FocusedPane::Explorer => FocusedPane::Chat,
                                FocusedPane::CommandPalette => FocusedPane::Chat,
                            };
                        }
                        KeyCode::Up => {
                            if app.focused_pane == FocusedPane::Chat {
                                let cur = app.list_state.selected().unwrap_or(0);
                                app.list_state.select(Some(cur.saturating_sub(1)));
                                app.auto_scroll = false;
                            } else if app.focused_pane == FocusedPane::Explorer {
                                let cur = app.explorer_state.selected().unwrap_or(0);
                                app.explorer_state.select(Some(cur.saturating_sub(1)));
                            } else if app.focused_pane == FocusedPane::CommandPalette {
                                let cur = app.command_palette_state.selected().unwrap_or(0);
                                app.command_palette_state.select(Some(cur.saturating_sub(1)));
                            } else {
                                app.reasoning_scroll = app.reasoning_scroll.saturating_sub(1);
                            }
                        }
                        KeyCode::Down => {
                            if app.focused_pane == FocusedPane::Chat {
                                let cur = app.list_state.selected().unwrap_or(0);
                                app.list_state.select(Some(cur + 1));
                            } else if app.focused_pane == FocusedPane::Explorer {
                                let cur = app.explorer_state.selected().unwrap_or(0);
                                app.explorer_state.select(Some(cur + 1));
                            } else if app.focused_pane == FocusedPane::CommandPalette {
                                let cur = app.command_palette_state.selected().unwrap_or(0);
                                app.command_palette_state.select(Some(cur + 1));
                            } else {
                                app.reasoning_scroll = app.reasoning_scroll.saturating_add(1);
                            }
                        }
                        KeyCode::PageUp => {
                            if app.focused_pane == FocusedPane::Chat {
                                let cur = app.list_state.selected().unwrap_or(0);
                                app.list_state.select(Some(cur.saturating_sub(15)));
                                app.auto_scroll = false;
                            } else {
                                app.reasoning_scroll = app.reasoning_scroll.saturating_sub(15);
                            }
                        }
                        KeyCode::PageDown => {
                            if app.focused_pane == FocusedPane::Chat {
                                let cur = app.list_state.selected().unwrap_or(0);
                                app.list_state.select(Some(cur + 15));
                            } else {
                                app.reasoning_scroll = app.reasoning_scroll.saturating_add(15);
                            }
                        }
                        KeyCode::Home => {
                            app.list_state.select(Some(0));
                            app.auto_scroll = false;
                        }
                        KeyCode::End => {
                            app.auto_scroll = true;
                        }
                        KeyCode::Esc => {
                            if app.agent_mode == "PLANNING" || app.agent_mode == "EXECUTING" || app.thinking_msg.is_some() {
                                stop_flag.store(true, std::sync::atomic::Ordering::Relaxed);
                                app.push_message("⚠️ [INTERRUPTED]: Stopping agent...".to_string());
                            } else {
                                // In IDLE mode, Esc can clear input or do nothing. 
                                // Let's have it clear input for better UX.
                                app.input_buffer.clear();
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        while let Ok(event) = agent_rx.try_recv() {
            match event {
                AgentEvent::SystemUpdate(u) => {
                    if u.contains("PLANNING mode") {
                        app.agent_mode = "PLANNING".to_string();
                    } else if u.contains("EXECUTION mode") {
                        app.agent_mode = "EXECUTING".to_string();
                    }
                    app.telemetry_text = u;
                }
                AgentEvent::Thinking(msg) => app.thinking_msg = msg,
                AgentEvent::RequestInput(tool, question) => {
                    app.pending_input = Some((tool.clone(), question.clone()));
                    app.input_response_buffer.clear();
                    // High-visibility alert in chat pane too
                    app.push_message(format!("⚠️ [ACTION REQUIRED]: Approval needed for {} in the input bar below.", tool.to_uppercase()));
                }
                AgentEvent::RequestPrivileges { rationale, response_tx } => {
                    app.pending_privilege_request = Some((rationale, response_tx));
                }
                AgentEvent::StreamToken(token) => {
                    if token.is_empty() {
                        if !app.current_stream.is_empty() {
                            app.push_message(format!("Tempest: {}", app.current_stream));
                            app.current_stream.clear();
                        }
                    } else {
                        // Clear reasoning pane at the start of each new response
                        if app.current_stream.is_empty() {
                            app.reasoning_buffer.clear();
                            app.reasoning_lines = 0;
                            app.reasoning_scroll = 0;
                            app.show_reasoning = false;
                        }
                        app.current_stream.push_str(&token);
                    }
                }
                AgentEvent::ReasoningToken(token) => {
                    app.show_reasoning = true;
                    if token.is_empty() { continue; } // Marker for reasoning start
                    app.reasoning_buffer.push_str(&token);
                    app.reasoning_lines = app.reasoning_buffer.lines().count();
                    if app.auto_scroll {
                        app.reasoning_scroll = app.reasoning_lines.saturating_sub(10) as u16;
                    }
                }
                AgentEvent::CommandOutput(line) => {
                    app.command_output.push(line);
                    if app.command_output.len() > 100 {
                        app.command_output.remove(0);
                    }
                }
                AgentEvent::SentinelUpdate { active, log } => {
                    app.active_sentinels = active;
                    if !log.is_empty() {
                        app.sentinel_log.push(log);
                        if app.sentinel_log.len() > 10 {
                            app.sentinel_log.remove(0);
                        }
                    }
                }
                AgentEvent::TelemetryMetrics { cpu, gpu, tps } => {
                    if let Some(c) = cpu {
                        app.cpu_history.push(c);
                        if app.cpu_history.len() > 100 { app.cpu_history.remove(0); }
                    }
                    if let Some(g) = gpu {
                        app.gpu_history.push(g);
                        if app.gpu_history.len() > 100 { app.gpu_history.remove(0); }
                    }
                    if let Some(t) = tps {
                        app.tps_history.push(t);
                        if app.tps_history.len() > 100 { app.tps_history.remove(0); }
                    }
                }
                AgentEvent::SubagentStatus(msg) => {
                    app.engine_status = msg;
                }
                AgentEvent::ContextStatus { used, total } => {
                    app.context_used = used;
                    app.context_total = total;
                }
                AgentEvent::EditorEdit { path, .. } => {
                    app.push_message(format!("📝 [EDITOR SYNC]: Applied changes to {}", path));
                }
            }
        }

        if app.should_quit {
            break;
        }
        
        if last_tick.elapsed() >= tick_rate {
            app.animation_tick = app.animation_tick.wrapping_add(1);
            last_tick = Instant::now();
        }
    }

    disable_raw_mode().into_diagnostic()?;
    stdout().execute(LeaveAlternateScreen).into_diagnostic()?;
    stdout().execute(crossterm::event::DisableMouseCapture).into_diagnostic()?;
    Ok(())
}

fn highlight_text(text: &str, syntax_set: &SyntaxSet, theme_set: &ThemeSet, theme_name: &str) -> Vec<Line<'static>> {
    let syntax = syntax_set.find_syntax_by_extension("rs").unwrap(); // Default to Rust
    let mut h = HighlightLines::new(syntax, &theme_set.themes[theme_name]);
    let mut lines = Vec::new();

    for line in LinesWithEndings::from(text) {
        let ranges: Vec<(SyntectStyle, &str)> = h.highlight_line(line, syntax_set).unwrap();
        let mut spans = Vec::new();
        for (style, content) in ranges {
            let color = Color::Rgb(style.foreground.r, style.foreground.g, style.foreground.b);
            spans.push(Span::styled(content.to_string(), Style::default().fg(color)));
        }
        lines.push(Line::from(spans));
    }
    lines
}

fn ui(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8), // Header/Logo area
            Constraint::Min(3),    // Main Content Area
            Constraint::Length(3), // Input Box
        ].as_ref())
        .split(f.area());

    // Main Content Area Layout Logic
    let mut main_constraints = Vec::new();
    if app.show_explorer {
        main_constraints.push(Constraint::Percentage(20)); // Explorer
    }
    main_constraints.push(Constraint::Percentage(if app.show_reasoning { 40 } else { 80 })); // Chat
    if app.show_reasoning {
        main_constraints.push(Constraint::Percentage(40)); // Reasoning
    }

    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(main_constraints)
        .split(chunks[1]);

    let mut pane_idx = 0;

    // 📂 FILE EXPLORER (Optional Sidebar)
    if app.show_explorer {
        let explorer_area = main_chunks[pane_idx];
        pane_idx += 1;
        
        let explorer_border_color = if app.focused_pane == FocusedPane::Explorer { Color::Yellow } else { Color::DarkGray };
        let explorer_title = if app.focused_pane == FocusedPane::Explorer { " 📂 EXPLORER [FOCUS] " } else { " 📂 EXPLORER " };

        let mut items = vec![ListItem::new(Span::styled(".. [UP]", Style::default().fg(Color::Blue)))];
        for (name, is_dir) in &app.explorer_files {
            let icon = if *is_dir { "📁 " } else { "📄 " };
            let style = if *is_dir { Style::default().fg(Color::Cyan) } else { Style::default().fg(Color::White) };
            items.push(ListItem::new(Span::styled(format!("{}{}", icon, name), style)));
        }

        let explorer = List::new(items)
            .block(Block::default().borders(Borders::ALL).title(explorer_title).border_style(Style::default().fg(explorer_border_color)))
            .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
            .highlight_symbol(">> ");
        
        f.render_stateful_widget(explorer, explorer_area, &mut app.explorer_state);
    }

    // Header area for Logo
    let logo = vec![
        Line::from(Span::styled("  _______ ______ __  __ _____  ______  _____ _______ ", Style::default().fg(Color::Cyan))),
        Line::from(Span::styled(" |__   __|  ____|  \\/  |  __ \\|  ____|/ ____|__   __|", Style::default().fg(Color::Cyan))),
        Line::from(Span::styled("    | |  | |__  | \\  / | |__) | |__  | (___    | |   ", Style::default().fg(Color::Cyan))),
        Line::from(Span::styled("    | |  |  __| | |\\/| |  ___/|  __|  \\___ \\   | |   ", Style::default().fg(Color::Cyan))),
        Line::from(Span::styled("    | |  | |____| |  | | |    | |____ ____) |  | |   ", Style::default().fg(Color::Cyan))),
        Line::from(Span::styled("    |_|  |______|_|  |_|_|    |______|_____/   |_|   ", Style::default().fg(Color::Cyan))),
        Line::from(Span::styled(" 🌪️    AUTONOMOUS SYSTEMS ENGINEERING    🌪️ ", Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD))),
    ];
    let header_block = Paragraph::new(logo)
        .alignment(ratatui::layout::Alignment::Center)
        .block(Block::default().borders(Borders::NONE));
    f.render_widget(header_block, chunks[0]);

    let chat_area = main_chunks[pane_idx];
    pane_idx += 1;

    let top_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(70),
            Constraint::Percentage(30),
        ].as_ref())
        .split(chat_area);

    let mut list_items = Vec::new();
    let chat_width = top_chunks[0].width.saturating_sub(2) as usize;

    let push_wrapped = |text: &str, items: &mut Vec<ListItem>, is_user: bool, show_header: bool| {
        if chat_width == 0 { return; }
        let (prefix, color) = if is_user { ("You: ", Color::Blue) } else { ("Tempest: ", Color::Cyan) };
        
        let has_prefix = text.starts_with(prefix);
        let content_to_wrap = if has_prefix { &text[prefix.len()..] } else { text };
        
        // --- 🌈 SYNTAX HIGHLIGHTING FOR CODE BLOCKS IN CHAT ---
        if !is_user && content_to_wrap.contains("```") {
             // Basic detection and highlighting for code blocks
             let highlighted = highlight_text(content_to_wrap, &app.syntax_set, &app.theme_set, &app.current_theme);
             for line in highlighted {
                 items.push(ListItem::new(line));
             }
             return;
        }

        let mut first_line = true;
        for line in content_to_wrap.split('\n') {
            let mut current = line;
            let mut first_chunk = true;

            if current.is_empty() && !first_line {
                items.push(ListItem::new(Line::from("")));
                continue;
            }

            while !current.is_empty() || (first_line && first_chunk) {
                let mut width = 0;
                let mut split_idx = 0;
                let available_width = if first_line && first_chunk && show_header && has_prefix {
                    chat_width.saturating_sub(UnicodeWidthStr::width(prefix))
                } else {
                    chat_width
                };

                for (i, c) in current.char_indices() {
                    let c_width = UnicodeWidthStr::width(c.to_string().as_str());
                    if width + c_width > available_width {
                        break;
                    }
                    width += c_width;
                    split_idx = i + c.len_utf8();
                }

                if split_idx == 0 && !current.is_empty() {
                    split_idx = current.chars().next().unwrap().len_utf8();
                }

                let (chunk, rest) = current.split_at(split_idx);
                
                let line_content = if first_line && first_chunk && show_header && has_prefix {
                    Line::from(vec![
                        Span::styled(prefix, Style::default().fg(color).add_modifier(Modifier::BOLD)),
                        Span::raw(chunk.to_string()),
                    ])
                } else {
                    Line::from(chunk.to_string())
                };
                
                items.push(ListItem::new(line_content));
                current = rest;
                first_chunk = false;
                if current.is_empty() { break; }
            }
            first_line = false;
        }
    };

    for msg in &app.messages {
        let is_user = msg.starts_with("You: ");
        push_wrapped(msg, &mut list_items, is_user, true);
    }

    if !app.current_stream.is_empty() {
        push_wrapped(&format!("Tempest: {}", app.current_stream), &mut list_items, false, true);
    }

    let core_border_color = if app.focused_pane == FocusedPane::Chat { Color::Yellow } else { Color::DarkGray };
    let core_title = if app.focused_pane == FocusedPane::Chat { " 🦾 CORE SESSION [FOCUS] " } else { " 🦾 CORE SESSION " };
    
    let item_count = list_items.len();
    let list = List::new(list_items)
        .block(Block::default()
            .borders(Borders::ALL)
            .title(core_title)
            .border_style(Style::default().fg(core_border_color)))
        .style(Style::default().fg(Color::White));

    if app.auto_scroll && item_count > 0 {
        app.list_state.select(Some(item_count.saturating_sub(1)));
    }
    
    f.render_stateful_widget(list, top_chunks[0], &mut app.list_state);

    let status_title = format!(" ⚙️ STATUS [{}] ", app.agent_mode);
    let mut status_lines = Vec::new();
    
    let spinner = if app.thinking_msg.is_some() {
        let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        frames[(app.animation_tick as usize) % frames.len()]
    } else {
        " "
    };

    for line_text in app.telemetry_text.split('\n') {
        if line_text.trim().is_empty() { continue; }
        let mut spans = Vec::new();
        if let Some((label, value)) = line_text.split_once(':') {
             spans.push(Span::styled(format!("{}:", label), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)));
             spans.push(Span::styled(value.to_string(), Style::default().fg(Color::Green)));
        } else {
             spans.push(Span::raw(line_text.to_string()));
        }
        status_lines.push(Line::from(spans));
    }

    if let Some(tool) = &app.active_tool {
        status_lines.push(Line::from(vec![
            Span::raw("🔧 Executing: "),
            Span::styled(tool, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        ]));
    }

    if let Some(thinking) = &app.thinking_msg {
        status_lines.push(Line::from(vec![
            Span::styled(format!("{} ", spinner), Style::default().fg(Color::Yellow)),
            Span::styled(thinking, Style::default().fg(Color::Magenta).add_modifier(Modifier::ITALIC)),
        ]));
    }

    // --- 🛡️ SENTINEL FLEET HUD ---
    status_lines.push(Line::from(""));
    if !app.active_sentinels.is_empty() {
        let mut spans = vec![Span::styled("🛡️ FLEET: ", Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD))];
        for s in &app.active_sentinels {
            spans.push(Span::styled(format!("{} ", s.chars().next().unwrap_or('S')), Style::default().fg(Color::Cyan)));
        }
        status_lines.push(Line::from(spans));
    }

    for log in &app.sentinel_log {
        status_lines.push(Line::from(vec![
            Span::styled(" ⤷ ", Style::default().fg(Color::DarkGray)),
            Span::styled(log, Style::default().fg(Color::Red).add_modifier(Modifier::ITALIC)),
        ]));
    }

    // --- CONTEXT WINDOW TRACKER ---
    if app.context_total > 0 {
        status_lines.push(Line::from(""));
        status_lines.push(Line::from(Span::styled("🧠 CONTEXT", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD))));
        let pct = (app.context_used as f64 / app.context_total as f64).min(1.0);
        let bar_width = 12;
        let filled = (pct * bar_width as f64) as usize;
        let bar_color = if pct > 0.9 { Color::Red } else if pct > 0.75 { Color::Yellow } else { Color::Green };
        status_lines.push(Line::from(vec![
            Span::styled("[", Style::default().fg(Color::Gray)),
            Span::styled("|".repeat(filled), Style::default().fg(bar_color)),
            Span::styled(".".repeat(bar_width - filled), Style::default().fg(Color::DarkGray)),
            Span::styled("]", Style::default().fg(Color::Gray)),
            Span::raw(format!(" {}k / {}k", app.context_used / 1024, app.context_total / 1024)),
        ]));
    }

    // --- STATUS PANE LAYOUT (Split for Sparklines) ---
    let status_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1), // Text metrics
            Constraint::Length(10), // Sparklines area
        ])
        .split(top_chunks[1]);

    let status_block = Paragraph::new(status_lines)
        .block(Block::default().borders(Borders::ALL).title(status_title))
        .style(Style::default().fg(Color::Gray))
        .wrap(ratatui::widgets::Wrap { trim: true });
    f.render_widget(status_block, status_chunks[0]);

    // --- 📊 REAL-TIME TELEMETRY SPARKLINES (Boxed) ---
    let pulse_block = Block::default()
        .title(Span::styled(" 📊 TELEMETRY PULSE ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    
    let pulse_inner = pulse_block.inner(status_chunks[1]);
    f.render_widget(pulse_block, status_chunks[1]);

    let pulse_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Ratio(1, 3), // CPU
            Constraint::Ratio(1, 3), // GPU
            Constraint::Ratio(1, 3), // TPS
        ])
        .split(pulse_inner);

    let cpu_spark = Sparkline::default()
        .block(Block::default().title(" CPU ").style(Style::default().fg(Color::Green)))
        .data(&app.cpu_history)
        .max(100);
    f.render_widget(cpu_spark, pulse_chunks[0]);
    
    let gpu_spark = Sparkline::default()
        .block(Block::default().title(" GPU ").style(Style::default().fg(Color::Blue)))
        .data(&app.gpu_history)
        .max(100);
    f.render_widget(gpu_spark, pulse_chunks[1]);

    let tps_spark = Sparkline::default()
        .block(Block::default().title(" TPS ").style(Style::default().fg(Color::Magenta)))
        .data(&app.tps_history)
        .max(50);
    f.render_widget(tps_spark, pulse_chunks[2]);

    // --- REASONING TRACE PANE (With Syntax Highlighting) ---
    if app.show_reasoning {
        let reasoning_area = main_chunks[pane_idx];
        let reasoning_border_color = if app.focused_pane == FocusedPane::Reasoning { Color::Yellow } else { Color::Magenta };
        let reasoning_title = if app.focused_pane == FocusedPane::Reasoning { " 🧠 REASONING [FOCUS] " } else { " 🧠 REASONING " };

        let highlighted_lines = highlight_text(&app.reasoning_buffer, &app.syntax_set, &app.theme_set, &app.current_theme);

        let reasoning_para = Paragraph::new(highlighted_lines)
            .block(Block::default()
                .title(Span::styled(reasoning_title, Style::default().fg(reasoning_border_color).add_modifier(Modifier::BOLD)))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(reasoning_border_color)))
            .wrap(ratatui::widgets::Wrap { trim: false })
            .scroll((app.reasoning_scroll, 0));
        f.render_widget(reasoning_para, reasoning_area);
    }

    let mut input_title = " 🗨️ INPUT ".to_string();
    let mut input_text = app.input_buffer.clone();
    let mut input_style = Style::default();

    if let Some((tool, question)) = &app.pending_input {
        input_title = format!(" ⚠️  APPROVAL REQUIRED for {} ", tool);
        input_text = format!("{} >> {}", question, app.input_response_buffer);
        input_style = Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD);
    } else if let Some((rationale, _resp_tx)) = &app.pending_privilege_request {
        input_title = format!(" 🔒 PRIVILEGE ESCALATION ");
        input_text = format!("Rationale: {} | Accept root? (y/n)", rationale);
        input_style = Style::default().fg(Color::Red).add_modifier(Modifier::BOLD);
    }

    let input = Paragraph::new(input_text.clone())
        .style(input_style)
        .block(Block::default().borders(Borders::ALL).title(input_title));
    f.render_widget(input, chunks[2]);

    f.set_cursor_position((
        chunks[2].x + (UnicodeWidthStr::width(input_text.as_str()) as u16) + 1,
        chunks[2].y + 1,
    ));

    // --- ⌨️ FUZZY COMMAND PALETTE OVERLAY ---
    if app.show_command_palette {
        let area = centered_rect(60, 40, f.area());
        f.render_widget(ratatui::widgets::Clear, area); // Clear background

        let block = Block::default()
            .title(" ⌨️ COMMAND PALETTE [Fuzzy Search] ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));
        
        let filtered_options: Vec<ListItem> = app.command_palette_options.iter()
            .filter(|opt| opt.to_lowercase().contains(&app.command_palette_query.to_lowercase()))
            .map(|opt| ListItem::new(Span::raw(opt.clone())))
            .collect();

        let list = List::new(filtered_options)
            .block(block)
            .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
            .highlight_symbol(">> ");

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(1)])
            .split(area);

        let query_box = Paragraph::new(app.command_palette_query.clone())
            .block(Block::default().borders(Borders::ALL).title(" 🔍 Search "));
        
        f.render_widget(query_box, chunks[0]);
        f.render_stateful_widget(list, chunks[1], &mut app.command_palette_state);
    }
}

// Helper for centering the command palette
fn centered_rect(percent_x: u16, percent_y: u16, r: ratatui::layout::Rect) -> ratatui::layout::Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ].as_ref())
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ].as_ref())
        .split(popup_layout[1])[1]
}

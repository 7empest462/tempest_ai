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
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame, Terminal,
};
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
}

pub enum ToolResponse {
    Confirm,
    Confirmed(bool),
    Text(String),
    #[allow(dead_code)]
    Error(String),
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum FocusedPane {
    Chat,
    Reasoning,
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
    pub reasoning_scroll: u16,
    pub list_state: ratatui::widgets::ListState,
    pub auto_scroll: bool,
    pub animation_tick: u32,
    pub pending_input: Option<(String, String)>,
    pub input_response_buffer: String,
    pub pending_confirmation: Option<(String, String)>,
    pub pending_privilege_request: Option<(String, tokio::sync::mpsc::Sender<ToolResponse>)>,
    pub subagent_notification: Option<String>,
    pub context_used: usize,
    pub context_total: u64,
    pub active_sentinels: Vec<String>,
    pub sentinel_log: Vec<String>,
    pub focused_pane: FocusedPane,
}

impl App {
    pub fn new() -> Self {
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
            reasoning_scroll: 0,
            list_state: ratatui::widgets::ListState::default(),
            auto_scroll: true,
            animation_tick: 0,
            pending_input: None,
            input_response_buffer: String::new(),
            pending_confirmation: None,
            pending_privilege_request: None,
            subagent_notification: None,
            context_used: 0,
            context_total: 0,
            active_sentinels: Vec::new(),
            sentinel_log: Vec::new(),
            focused_pane: FocusedPane::Chat,
        }
    }
}

pub async fn run_tui(
    mut agent_rx: tokio::sync::mpsc::Receiver<AgentEvent>, 
    user_tx: tokio::sync::mpsc::Sender<String>, 
    tool_tx: tokio::sync::mpsc::Sender<ToolResponse>,
    stop_flag: std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> Result<()> {
    enable_raw_mode().into_diagnostic()?;
    stdout().execute(EnterAlternateScreen).into_diagnostic()?;
    stdout().execute(crossterm::event::EnableMouseCapture).into_diagnostic()?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout())).into_diagnostic()?;

    let mut app = App::new();
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
                            let resp = app.input_response_buffer.clone();
                            let _ = tool_tx.send(ToolResponse::Text(resp)).await;
                            app.pending_input = None;
                            app.input_response_buffer.clear();
                        }
                        KeyCode::Char(c) => app.input_response_buffer.push(c),
                        KeyCode::Backspace => { app.input_response_buffer.pop(); }
                        KeyCode::Esc => { 
                            let _ = tool_tx.send(ToolResponse::Text("Cancelled".to_string())).await; 
                            app.pending_input = None;
                        }
                        _ => {}
                    }
                } else if app.pending_confirmation.is_some() {
                    match key.code {
                        KeyCode::Char('y') | KeyCode::Char('Y') => {
                            let _ = tool_tx.send(ToolResponse::Confirm).await;
                            app.pending_confirmation = None;
                        }
                        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                            app.pending_confirmation = None;
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
                            } else {
                                app.input_buffer.push(c);
                            }
                        }
                        KeyCode::Backspace => {
                            app.input_buffer.pop();
                        }
                        KeyCode::Enter => {
                            if !app.input_buffer.is_empty() {
                                let msg = app.input_buffer.drain(..).collect::<String>();
                                app.messages.push(format!("You: {}", msg));
                                let _ = user_tx.send(msg).await;
                                app.auto_scroll = true;
                            }
                        }
                        KeyCode::Tab => {
                            app.focused_pane = match app.focused_pane {
                                FocusedPane::Chat => FocusedPane::Reasoning,
                                FocusedPane::Reasoning => FocusedPane::Chat,
                            };
                        }
                        KeyCode::Up => {
                            if app.focused_pane == FocusedPane::Chat {
                                let cur = app.list_state.selected().unwrap_or(0);
                                app.list_state.select(Some(cur.saturating_sub(1)));
                                app.auto_scroll = false;
                            } else {
                                app.reasoning_scroll = app.reasoning_scroll.saturating_sub(1);
                            }
                        }
                        KeyCode::Down => {
                            if app.focused_pane == FocusedPane::Chat {
                                let cur = app.list_state.selected().unwrap_or(0);
                                app.list_state.select(Some(cur + 1));
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
                                app.messages.push("⚠️ [INTERRUPTED]: Stopping agent...".to_string());
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
                    app.pending_input = Some((tool.clone(), question));
                    app.input_response_buffer.clear();
                }
                AgentEvent::RequestPrivileges { rationale, response_tx } => {
                    app.pending_privilege_request = Some((rationale, response_tx));
                }
                AgentEvent::StreamToken(token) => {
                    if token.is_empty() {
                        if !app.current_stream.is_empty() {
                            app.messages.push(format!("Tempest: {}", app.current_stream));
                            app.current_stream.clear();
                        }
                    } else {
                        app.current_stream.push_str(&token);
                    }
                }
                AgentEvent::ReasoningToken(token) => {
                    app.reasoning_buffer.push_str(&token);
                    // Basic heuristic for line counting to auto-scroll
                    let lines = app.reasoning_buffer.split('\n').count();
                    if lines > 20 {
                         app.reasoning_scroll = (lines as u16).saturating_sub(15);
                    }
                }
                AgentEvent::SubagentStatus(msg) => {
                    app.subagent_notification = msg;
                }
                AgentEvent::ContextStatus { used, total } => {
                    app.context_used = used;
                    app.context_total = total;
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

fn ui(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8), // Header/Logo area
            Constraint::Min(3),    // Main Content Area
            Constraint::Length(3), // Input Box
        ].as_ref())
        .split(f.area());

    // Main Content Area: Split horizontally if there's reasoning content
    let main_chunks = if !app.reasoning_buffer.is_empty() {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(50), // Chat
                Constraint::Percentage(50), // Reasoning
            ].as_ref())
            .split(chunks[1])
    } else {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(100),
            ].as_ref())
            .split(chunks[1])
    };

    // Header area for Logo
    let logo = vec![
        Line::from(Span::styled("  _______ ______ __  __ _____  ______  _____ _______ ", Style::default().fg(Color::Cyan))),
        Line::from(Span::styled(" |__   __|  ____|  \\/  |  __ \\|  ____|/ ____|__   __|", Style::default().fg(Color::Cyan))),
        Line::from(Span::styled("    | |  | |__  | \\  / | |__) | |__  | (___    | |   ", Style::default().fg(Color::Cyan))),
        Line::from(Span::styled("    | |  |  __| | |\\/| |  ___/|  __|  \\___ \\   | |   ", Style::default().fg(Color::Cyan))),
        Line::from(Span::styled("    | |  | |____| |  | | |    | |____ ____) |  | |   ", Style::default().fg(Color::Cyan))),
        Line::from(Span::styled("    |_|  |______|_|  |_|_|    |______|_____/   |_|   ", Style::default().fg(Color::Cyan))),
        Line::from(Span::styled("🌪️  AUTONOMOUS AGENTIC CORE  🌪️", Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD))),
    ];
    let header_block = Paragraph::new(logo)
        .alignment(ratatui::layout::Alignment::Center)
        .block(Block::default().borders(Borders::NONE));
    f.render_widget(header_block, chunks[0]);

    let main_area = main_chunks[0]; // All chat/telemetry happens in the left panel
    let top_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(70),
            Constraint::Percentage(30),
        ].as_ref())
        .split(main_area);

    let mut list_items = Vec::new();
    let chat_width = top_chunks[0].width.saturating_sub(2) as usize;

    let push_wrapped = |text: &str, items: &mut Vec<ListItem>, is_user: bool, show_header: bool| {
        if chat_width == 0 { return; }
        let (prefix, color) = if is_user { ("You: ", Color::Blue) } else { ("Tempest: ", Color::Cyan) };
        
        let mut first = true;
        let mut prefix_added = false;
        
        for line in text.split('\n') {
            let content = if first && text.starts_with(prefix) {
                first = false;
                &text[prefix.len()..]
            } else {
                first = false;
                line
            };

            let mut current = content;

            while current.len() > chat_width {
                let split_idx = current.char_indices().nth(chat_width).map(|(i, _)| i).unwrap_or(current.len());
                let (chunk, rest) = current.split_at(split_idx);
                
                let line_content = if show_header && !prefix_added && text.starts_with(prefix) {
                    prefix_added = true;
                    Line::from(vec![
                        Span::styled(prefix, Style::default().fg(color).add_modifier(Modifier::BOLD)),
                        Span::raw(chunk.to_string()),
                    ])
                } else {
                    Line::from(chunk.to_string())
                };
                
                items.push(ListItem::new(line_content));
                current = rest;
            }
            
            let final_line = if show_header && !prefix_added && text.starts_with(prefix) {
                prefix_added = true;
                Line::from(vec![
                    Span::styled(prefix, Style::default().fg(color).add_modifier(Modifier::BOLD)),
                    Span::raw(current.to_string()),
                ])
            } else {
                Line::from(current.to_string())
            };
            items.push(ListItem::new(final_line));
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
    let core_title = if app.focused_pane == FocusedPane::Chat { " 🦾 TEMPEST CORE SESSION [FOCUS] " } else { " 🦾 TEMPEST CORE SESSION " };
    
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

    let status_title = format!(" ⚙️ SYSTEM STATUS [{}] ", app.agent_mode);
    let mut status_lines = Vec::new();
    
    // Animate spinner if thinking
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

    // --- SENTINEL FLEET HUD (Compact) ---
    if !app.active_sentinels.is_empty() {
        let mut spans = vec![
            Span::styled("🛡️ FLEET: ", Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)),
        ];
        
        for sentinel in &app.active_sentinels {
            let tag = match sentinel.as_str() {
                "Context Runway" => "[C]",
                "Privilege Escalator" => "[P]",
                "Compiler Guard" => "[G]",
                "Build Watcher" => "[B]",
                "Thermal Guard" => "[T]",
                _ => "[?]",
            };
            spans.push(Span::styled(format!("{} ", tag), Style::default().fg(Color::Cyan)));
        }
        status_lines.push(Line::from(spans));
    }

    for log in &app.sentinel_log {
        status_lines.push(Line::from(vec![
            Span::styled(" ⤷ ", Style::default().fg(Color::Gray)),
            Span::styled(log, Style::default().fg(Color::Red).add_modifier(Modifier::ITALIC)),
        ]));
    }

    if let Some(msg) = &app.subagent_notification {
        status_lines.push(Line::from(""));
        status_lines.push(Line::from(Span::styled("🤖 SUBAGENT ACTIVE", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))));
        for line in msg.split('\n') {
            status_lines.push(Line::from(Span::styled(line.to_string(), Style::default().fg(Color::White))));
        }
    }

    // --- CONTEXT WINDOW TRACKER ---
    if app.context_total > 0 {
        status_lines.push(Line::from(""));
        status_lines.push(Line::from(Span::styled("🧠 CONTEXT WINDOW", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD))));
        
        let pct = (app.context_used as f64 / app.context_total as f64).min(1.0);
        let bar_width = 20;
        let filled = (pct * bar_width as f64) as usize;
        let empty = bar_width - filled;
        
        let bar_color = if pct > 0.9 { Color::Red } else if pct > 0.75 { Color::Yellow } else { Color::Green };
        
        status_lines.push(Line::from(vec![
            Span::styled("[", Style::default().fg(Color::Gray)),
            Span::styled("|".repeat(filled), Style::default().fg(bar_color)),
            Span::styled(".".repeat(empty), Style::default().fg(Color::DarkGray)),
            Span::styled("]", Style::default().fg(Color::Gray)),
            Span::raw(format!(" {} / {}k", app.context_used, app.context_total / 1024)),
        ]));
    }

    let status_block = Paragraph::new(status_lines)
        .block(Block::default().borders(Borders::ALL).title(status_title))
        .style(Style::default().fg(Color::Gray))
        .wrap(ratatui::widgets::Wrap { trim: true });
    f.render_widget(status_block, top_chunks[1]);

    // --- REASONING TRACE PANE (Right Panel) ---
    if !app.reasoning_buffer.is_empty() {
        let reasoning_border_color = if app.focused_pane == FocusedPane::Reasoning { Color::Yellow } else { Color::Magenta };
        let reasoning_title = if app.focused_pane == FocusedPane::Reasoning { " 🧠 THOUGHT PROCESS [FOCUS] " } else { " 🧠 THOUGHT PROCESS (Reasoning Trace) " };

        let reasoning_para = Paragraph::new(app.reasoning_buffer.clone())
            .block(Block::default()
                .title(Span::styled(reasoning_title, Style::default().fg(reasoning_border_color).add_modifier(Modifier::BOLD)))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(reasoning_border_color)))
            .style(Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC))
            .wrap(ratatui::widgets::Wrap { trim: true })
            .scroll((app.reasoning_scroll, 0));
        f.render_widget(reasoning_para, main_chunks[1]);
    }

    let mut input_title = " 🗨️ INPUT ".to_string();
    let mut input_text = app.input_buffer.clone();
    let mut input_style = Style::default();

    if let Some((tool, question)) = &app.pending_input {
        input_title = format!(" ❓ INPUT REQUIRED for {} ", tool);
        input_text = format!("{}: {} >> {}", "Question", question, app.input_response_buffer);
        input_style = Style::default().fg(Color::Cyan);
    } else if let Some((rationale, _resp_tx)) = &app.pending_privilege_request {
        input_title = " 🔒 SECURE ESCALATION REQUIRED ".to_string();
        input_text = format!("Rationale: {} | Accept root privileges? (y/n)", rationale);
        input_style = Style::default().fg(Color::Red).add_modifier(Modifier::BOLD);
    } else if let Some((tool, args)) = &app.pending_confirmation {
        input_title = format!(" ⚠️ CONFIRMATION REQUIRED for {} ", tool);
        input_text = format!("Confirm {}? (y/n) >> {}", args, " ");
        input_style = Style::default().fg(Color::Yellow);
    }

    let input = Paragraph::new(input_text.clone())
        .style(input_style)
        .block(Block::default().borders(Borders::ALL).title(input_title));
    f.render_widget(input, chunks[2]);

    // Set cursor position for typing feedback
    f.set_cursor_position((
        chunks[2].x + (input_text.chars().count() as u16) + 1,
        chunks[2].y + 1,
    ));
}

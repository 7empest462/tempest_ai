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
}

pub enum ToolResponse {
    Confirm,
    Confirmed(bool),
    Text(String),
    #[allow(dead_code)]
    Error(String),
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
    pub list_state: ratatui::widgets::ListState,
    pub auto_scroll: bool,
    pub animation_tick: u32,
    pub pending_input: Option<(String, String)>,
    pub input_response_buffer: String,
    pub pending_confirmation: Option<(String, String)>,
    pub pending_privilege_request: Option<(String, tokio::sync::mpsc::Sender<ToolResponse>)>,
}

impl App {
    pub fn new() -> Self {
        Self {
            input_buffer: String::new(),
            messages: vec![
                "Welcome to Tempest AI TUI.".to_string(),
                "Type your request below and press Enter.".to_string(),
                "Press Esc to exit.".to_string(),
            ],
            current_stream: String::new(),
            active_tool: None,
            telemetry_text: "Initializing systems...".to_string(),
            should_quit: false,
            agent_mode: "IDLE".to_string(),
            thinking_msg: None,
            list_state: ratatui::widgets::ListState::default(),
            auto_scroll: true,
            animation_tick: 0,
            pending_input: None,
            input_response_buffer: String::new(),
            pending_confirmation: None,
            pending_privilege_request: None,
        }
    }
}

pub async fn run_tui(mut agent_rx: tokio::sync::mpsc::Receiver<AgentEvent>, user_tx: tokio::sync::mpsc::Sender<String>, tool_tx: tokio::sync::mpsc::Sender<ToolResponse>) -> Result<()> {
    enable_raw_mode().into_diagnostic()?;
    stdout().execute(EnterAlternateScreen).into_diagnostic()?;
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
            if let Event::Key(key) = event::read().into_diagnostic()? {
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
                        KeyCode::Up => {
                            let cur = app.list_state.selected().unwrap_or(0);
                            app.list_state.select(Some(cur.saturating_sub(1)));
                            app.auto_scroll = false;
                        }
                        KeyCode::Down => {
                            let cur = app.list_state.selected().unwrap_or(0);
                            app.list_state.select(Some(cur + 1));
                        }
                        KeyCode::PageUp => {
                            let cur = app.list_state.selected().unwrap_or(0);
                            app.list_state.select(Some(cur.saturating_sub(15)));
                            app.auto_scroll = false;
                        }
                        KeyCode::PageDown => {
                            let cur = app.list_state.selected().unwrap_or(0);
                            app.list_state.select(Some(cur + 15));
                        }
                        KeyCode::Home => {
                            app.list_state.select(Some(0));
                            app.auto_scroll = false;
                        }
                        KeyCode::End => {
                            app.auto_scroll = true;
                        }
                        KeyCode::Esc => {
                            app.should_quit = true;
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
    Ok(())
}

fn ui(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8), // Header/Logo area
            Constraint::Min(3),    // Chat Area
            Constraint::Length(3), // Input Box
        ].as_ref())
        .split(f.area());

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

    let main_area = chunks[1];
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

    let item_count = list_items.len();
    let list = List::new(list_items)
        .block(Block::default().borders(Borders::ALL).title(" 🦾 TEMPEST CORE SESSION "))
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

    let status_block = Paragraph::new(status_lines)
        .block(Block::default().borders(Borders::ALL).title(status_title))
        .style(Style::default().fg(Color::Gray));
    f.render_widget(status_block, top_chunks[1]);

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

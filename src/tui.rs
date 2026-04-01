use anyhow::Result;
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
// Removed unused tokio::sync::mpsc
use std::time::{Duration, Instant};

pub enum AgentEvent {
    StreamToken(String),
    ToolStart(String),
    ToolFinish,
    SystemUpdate(String), // Telemetry
    Done,
    Thinking(bool),
    RequestConfirmation(String, String),
    RequestInput(String, String), // (tool_name, question)
}

/// Unified response type for the tool-approval channel.
/// Carries either a boolean confirmation or freeform text input.
pub enum ToolResponse {
    Confirm(bool),
    Text(String),
}

pub struct App {
    pub input_buffer: String,
    pub messages: Vec<String>,
    pub current_stream: String,
    pub active_tool: Option<String>,
    pub telemetry_text: String,
    pub should_quit: bool,
    pub auto_scroll: bool,
    pub list_state: ratatui::widgets::ListState,
    pub pending_confirmation: Option<(String, String)>,
    pub pending_input: Option<(String, String)>,
    pub input_response_buffer: String,
    pub agent_mode: String,
    pub is_thinking: bool,
    pub animation_tick: u32,
}

impl App {
    pub fn new() -> Self {
        App {
            input_buffer: String::new(),
            messages: Vec::new(),
            current_stream: String::new(),
            active_tool: None,
            telemetry_text: "Waiting for telemetry...".to_string(),
            should_quit: false,
            auto_scroll: true,
            list_state: ratatui::widgets::ListState::default(),
            pending_confirmation: None,
            pending_input: None,
            input_response_buffer: String::new(),
            agent_mode: "PLANNING".to_string(),
            is_thinking: false,
            animation_tick: 0,
        }
    }
}

pub async fn run_tui(mut app: App,    mut agent_rx: tokio::sync::mpsc::Receiver<AgentEvent>,
    user_tx: tokio::sync::mpsc::Sender<String>,
    tool_tx: tokio::sync::mpsc::Sender<ToolResponse>,
    stop_flag: std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> Result<()> {
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    let tick_rate = Duration::from_millis(50);
    let mut last_tick = Instant::now();

    loop {
        terminal.draw(|f| ui(f, &mut app))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                // Priority 1: Confirmation modal (Y/N)
                if app.pending_confirmation.is_some() {
                    match key.code {
                        KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                            let _ = tool_tx.send(ToolResponse::Confirm(true)).await;
                            app.pending_confirmation = None;
                        }
                        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                            let _ = tool_tx.send(ToolResponse::Confirm(false)).await;
                            app.pending_confirmation = None;
                        }
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            app.should_quit = true;
                        }
                        _ => {}
                    }
                // Priority 2: Text input modal (AskUser)
                } else if app.pending_input.is_some() {
                    match key.code {
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            app.should_quit = true;
                        }
                        KeyCode::Char(c) => {
                            app.input_response_buffer.push(c);
                        }
                        KeyCode::Backspace => {
                            app.input_response_buffer.pop();
                        }
                        KeyCode::Enter => {
                            if !app.input_response_buffer.is_empty() {
                                let response = app.input_response_buffer.clone();
                                app.messages.push(format!("You: {}", response));
                                let _ = tool_tx.send(ToolResponse::Text(response)).await;
                                app.input_response_buffer.clear();
                                app.pending_input = None;
                                app.auto_scroll = true;
                            }
                        }
                        KeyCode::Esc => {
                            let _ = tool_tx.send(ToolResponse::Text("[User cancelled input]".to_string())).await;
                            app.input_response_buffer.clear();
                            app.pending_input = None;
                        }
                        _ => {}
                    }
                // Priority 3: Normal input
                } else {
                    match key.code {
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            app.should_quit = true;
                        }
                        KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            stop_flag.store(true, std::sync::atomic::Ordering::SeqCst);
                        }
                        KeyCode::Char(c) => {
                            app.input_buffer.push(c);
                        }
                        KeyCode::Backspace => {
                            app.input_buffer.pop();
                        }
                        KeyCode::Enter => {
                            if !app.input_buffer.is_empty() {
                                let msg = app.input_buffer.clone();
                                app.messages.push(format!("You: {}", msg));
                                let _ = user_tx.send(msg).await;
                                app.input_buffer.clear();
                                app.auto_scroll = true; // Re-enable follow on send
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
                            // Check if we hit the bottom to re-enable auto-scroll later
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

        // Process Agent Events
        while let Ok(event) = agent_rx.try_recv() {
            match event {
                AgentEvent::StreamToken(t) => app.current_stream.push_str(&t),
                AgentEvent::ToolStart(t) => app.active_tool = Some(t),
                AgentEvent::ToolFinish => app.active_tool = None,
                AgentEvent::SystemUpdate(u) => {
                    if u.contains("PLANNING mode") {
                        app.agent_mode = "PLANNING".to_string();
                    } else if u.contains("EXECUTION mode") {
                        app.agent_mode = "EXECUTING".to_string();
                    }
                    app.telemetry_text = u;
                }
                AgentEvent::Done => {
                    app.messages.push(format!("Tempest: {}", app.current_stream));
                    app.current_stream.clear();
                }
                AgentEvent::Thinking(b) => app.is_thinking = b,
                AgentEvent::RequestConfirmation(tool, args) => {
                    app.pending_confirmation = Some((tool, args));
                }
                AgentEvent::RequestInput(tool, question) => {
                    app.pending_input = Some((tool, question));
                    app.input_response_buffer.clear();
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

    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;
    Ok(())
}

fn ui(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),    // Chat Area
            Constraint::Length(3), // Input Box
        ].as_ref())
        .split(f.area());

    // Split Top Pane into Chat (Left) and Telemetry (Right)
    let top_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(70),
            Constraint::Percentage(30),
        ].as_ref())
        .split(chunks[0]);

    // 1. Chat History
    let mut list_items = Vec::new();
    let chat_width = top_chunks[0].width.saturating_sub(2) as usize;

    let push_wrapped = |text: &str, items: &mut Vec<ListItem>, is_user: bool| {
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
                
                let line_content = if !prefix_added && text.starts_with(prefix) {
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
            
            let final_line = if !prefix_added && text.starts_with(prefix) {
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
        push_wrapped(msg, &mut list_items, is_user);
        list_items.push(ListItem::new(Line::from(""))); // Spacing between messages
    }
    
    if !app.current_stream.is_empty() {
        push_wrapped(&format!("Tempest: {}█", app.current_stream), &mut list_items, false);
    } else if app.is_thinking {
        let spinner = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        let frame = spinner[(app.animation_tick % spinner.len() as u32) as usize];
        list_items.push(ListItem::new(Line::from(vec![
            Span::styled("Tempest: ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled(format!("{} Thinking...", frame), Style::default().fg(Color::Yellow)),
        ])));
    } else if let Some(ref tool) = app.active_tool {
        list_items.push(ListItem::new(Line::from(Span::styled(
            format!(" ⚙️  EXECUTING: {}...", tool.to_uppercase()),
            Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)
        ))));
    }
    
    let scroll_status = if app.auto_scroll { " [ FOLLOW: ON ] " } else { " [ SCROLL LOCK: ON ] " };
    let mode_indicator = if app.agent_mode == "PLANNING" {
        Span::styled(" [\u{1f9e0} PLANNING] ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
    } else {
        Span::styled(" [\u{26a1} EXECUTING] ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
    };
    let chat_list = List::new(list_items.clone())
        .block(Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray))
            .title(vec![
                Span::styled(" TEMPEST AI ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                mode_indicator,
                Span::styled(scroll_status, Style::default().fg(Color::Yellow)),
                Span::styled(" [Ctrl+S: Stop] ", Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM)),
            ]));
    
    // Auto-scroll logic: Anchor to bottom if follow mode is active
    if app.auto_scroll && !list_items.is_empty() {
        let last_idx = list_items.len().saturating_sub(1);
        app.list_state.select(Some(last_idx));
    } else if !list_items.is_empty() && app.list_state.selected().is_none() {
        // Fallback for first load
        app.list_state.select(Some(0));
    }


    f.render_stateful_widget(chat_list, top_chunks[0], &mut app.list_state);

    // 2. Telemetry (Premium Visuals)
    let telemetry_lines: Vec<Line> = app.telemetry_text.lines()
        .map(|l| {
            if l.contains("---") {
                Line::from(Span::styled(l, Style::default().fg(Color::DarkGray)))
            } else if l.contains("🔥") || l.contains("🚀") || l.contains("💾") || l.contains("🌡️") || l.contains("⚙️") || l.contains("⏱️") {
                Line::from(Span::styled(l, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)))
            } else {
                Line::from(l.to_string())
            }
        })
        .collect();

    let telemetry_para = Paragraph::new(telemetry_lines)
        .block(Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray))
            .title(Span::styled(" HARDWARE VECTORS ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))));
    f.render_widget(telemetry_para, top_chunks[1]);

    // 3. Input Buffer (Interactive Look)
    let input_para = Paragraph::new(vec![
        Line::from(vec![
            Span::styled(" >> ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::raw(app.input_buffer.as_str()),
        ])
    ])
    .block(Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green))
        .title(Span::styled(" COMMAND TERMINAL ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))));
    
    f.render_widget(input_para, chunks[1]);

    // Hardware Cursor
    f.set_cursor_position((
        chunks[1].x + app.input_buffer.chars().count() as u16 + 5,
        chunks[1].y + 1,
    ));

    // 4. Confirmation Modal Popup
    if let Some((tool, args)) = &app.pending_confirmation {
        let popup_area = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([
                ratatui::layout::Constraint::Percentage(20),
                ratatui::layout::Constraint::Percentage(60),
                ratatui::layout::Constraint::Percentage(20),
            ])
            .split(f.area())[1];

        let popup_area = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Horizontal)
            .constraints([
                ratatui::layout::Constraint::Percentage(15),
                ratatui::layout::Constraint::Percentage(70),
                ratatui::layout::Constraint::Percentage(15),
            ])
            .split(popup_area)[1];

        f.render_widget(ratatui::widgets::Clear, popup_area);

        let content = vec![
            Line::from(vec![Span::styled(format!(" ⚙️ TOOL: {} ", tool), Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD))]),
            Line::from(""),
            Line::from(Span::styled(args.as_str(), Style::default().fg(Color::Cyan))),
            Line::from(""),
            Line::from(vec![Span::styled(" Allow Execution? [Y/n] ", Style::default().bg(Color::Red).fg(Color::White).add_modifier(Modifier::BOLD))]),
        ];
        
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Red))
            .title(Span::styled(" SECURITY OVERRIDE REQUIRED ", Style::default().bg(Color::Red).fg(Color::White).add_modifier(Modifier::BOLD)));
            
        f.render_widget(Paragraph::new(content).block(block).wrap(ratatui::widgets::Wrap { trim: true }), popup_area);
        
        // Hide Hardware Cursor when Modal is open
        f.set_cursor_position((chunks[1].x, chunks[1].y));
    }

    // 5. User Input Modal (AskUser)
    if let Some((_tool, question)) = &app.pending_input {
        let popup_area = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([
                ratatui::layout::Constraint::Percentage(25),
                ratatui::layout::Constraint::Percentage(50),
                ratatui::layout::Constraint::Percentage(25),
            ])
            .split(f.area())[1];

        let popup_area = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Horizontal)
            .constraints([
                ratatui::layout::Constraint::Percentage(15),
                ratatui::layout::Constraint::Percentage(70),
                ratatui::layout::Constraint::Percentage(15),
            ])
            .split(popup_area)[1];

        f.render_widget(ratatui::widgets::Clear, popup_area);

        let content = vec![
            Line::from(vec![Span::styled(" 🤔 AI REQUIRES YOUR INPUT ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))]),
            Line::from(""),
            Line::from(Span::styled(question.as_str(), Style::default().fg(Color::White))),
            Line::from(""),
            Line::from(vec![
                Span::styled(" >> ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                Span::raw(app.input_response_buffer.as_str()),
                Span::styled("█", Style::default().fg(Color::Green)),
            ]),
            Line::from(""),
            Line::from(Span::styled(" [Enter: Submit | Esc: Cancel] ", Style::default().fg(Color::DarkGray))),
        ];
        
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow))
            .title(Span::styled(" USER INPUT REQUIRED ", Style::default().bg(Color::Yellow).fg(Color::Black).add_modifier(Modifier::BOLD)));
            
        f.render_widget(Paragraph::new(content).block(block).wrap(ratatui::widgets::Wrap { trim: true }), popup_area);
        
        // Position cursor in the input area of the modal
        let cursor_x = popup_area.x + 4 + app.input_response_buffer.chars().count() as u16;
        let cursor_y = popup_area.y + 5; // Line where the input field is
        f.set_cursor_position((cursor_x, cursor_y));
    }
}


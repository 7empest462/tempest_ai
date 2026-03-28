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
use tokio::sync::mpsc;
use std::time::{Duration, Instant};

pub enum AgentEvent {
    StreamToken(String),
    ToolStart(String),
    ToolFinish,
    SystemUpdate(String), // Telemetry
    Done,
}

pub struct App {
    pub input_buffer: String,
    pub messages: Vec<String>,
    pub current_stream: String,
    pub active_tool: Option<String>,
    pub telemetry_text: String,
    pub should_quit: bool,
    pub list_state: ratatui::widgets::ListState,
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
            list_state: ratatui::widgets::ListState::default(),
        }
    }
}

pub async fn run_tui(mut app: App, mut agent_rx: mpsc::Receiver<AgentEvent>, user_tx: mpsc::Sender<String>) -> Result<()> {
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
                match key.code {
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        app.should_quit = true;
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
                        }
                    }
                    KeyCode::Esc => {
                        app.should_quit = true;
                    }
                    _ => {}
                }
            }
        }

        // Process Agent Events
        while let Ok(event) = agent_rx.try_recv() {
            match event {
                AgentEvent::StreamToken(t) => app.current_stream.push_str(&t),
                AgentEvent::ToolStart(t) => app.active_tool = Some(t),
                AgentEvent::ToolFinish => app.active_tool = None,
                AgentEvent::SystemUpdate(u) => app.telemetry_text = u,
                AgentEvent::Done => {
                    app.messages.push(format!("Tempest: {}", app.current_stream));
                    app.current_stream.clear();
                }
            }
        }

        if app.should_quit {
            break;
        }
        
        if last_tick.elapsed() >= tick_rate {
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

    let mut push_wrapped = |text: &str, items: &mut Vec<ListItem>, is_user: bool| {
        if chat_width == 0 { return; }
        let (prefix, color) = if is_user { ("You: ", Color::Blue) } else { ("Tempest: ", Color::Cyan) };
        
        let mut first = true;
        for line in text.split('\n') {
            let content = if first && text.starts_with(prefix) {
                first = false;
                text[prefix.len()..].to_string()
            } else {
                line.to_string()
            };

            let mut current = content.as_str();
            let mut prefix_added = false;

            while current.len() > chat_width {
                let split_idx = current.char_indices().nth(chat_width).map(|(i, _)| i).unwrap_or(current.len());
                let (chunk, rest) = current.split_at(split_idx);
                
                let line_content = if !prefix_added && is_user == (text.starts_with(prefix)) {
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
    } else if let Some(ref tool) = app.active_tool {
        list_items.push(ListItem::new(Line::from(Span::styled(
            format!(" ⚙️  EXECUTING: {}...", tool.to_uppercase()),
            Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)
        ))));
    }
    
    let chat_list = List::new(list_items.clone())
        .block(Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray))
            .title(Span::styled(" TEMPEST AI - COMMUNICATION LINK ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))));
    
    // Auto-scroll logic: Anchor to bottom
    if !list_items.is_empty() {
        let last_idx = list_items.len().saturating_sub(1);
        app.list_state.select(Some(last_idx));
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
}


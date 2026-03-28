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
        terminal.draw(|f| ui(f, &app))?;

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

fn ui(f: &mut Frame, app: &App) {
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

    let push_wrapped = |text: &str, items: &mut Vec<ListItem>| {
        if chat_width == 0 { return; }
        for line in text.split('\n') {
            let mut current = line;
            while current.len() > chat_width {
                // To avoid breaking multi-byte UTF-8, we use char indices
                let split_idx = current.char_indices().nth(chat_width).map(|(i, _)| i).unwrap_or(current.len());
                let (chunk, rest) = current.split_at(split_idx);
                items.push(ListItem::new(Line::from(chunk.to_string())));
                current = rest;
            }
            items.push(ListItem::new(Line::from(current.to_string())));
        }
    };

    for msg in &app.messages {
        push_wrapped(msg, &mut list_items);
        list_items.push(ListItem::new(Line::from(""))); // Spacing between messages
    }
    
    if !app.current_stream.is_empty() {
        push_wrapped(&format!("Tempest: {}█", app.current_stream), &mut list_items);
    } else if let Some(ref tool) = app.active_tool {
        list_items.push(ListItem::new(Line::from(Span::styled(
            format!("⚙️ Executing: {}...", tool),
            Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)
        ))));
    }
    
    let chat_list = List::new(list_items.clone())
        .block(Block::default().borders(Borders::ALL).title("Tempest AI - Communication Link"));
    
    let mut state = ratatui::widgets::ListState::default();
    if !list_items.is_empty() {
        state.select(Some(list_items.len() - 1));
    }
    f.render_stateful_widget(chat_list, top_chunks[0], &mut state);

    // 2. Telemetry
    let telemetry_para = Paragraph::new(app.telemetry_text.as_str())
        .block(Block::default().borders(Borders::ALL).title("Hardware Vectors"));
    f.render_widget(telemetry_para, top_chunks[1]);

    // 3. Input Buffer
    let input_para = Paragraph::new(app.input_buffer.as_str())
        .block(Block::default().borders(Borders::ALL).title(">> Command Terminal [Press Enter to Submit | ESC to Quit]"));
    f.render_widget(input_para, chunks[1]);

    // Track the actual hardware cursor so the user can see where they type
    f.set_cursor_position((
        chunks[1].x + app.input_buffer.chars().count() as u16 + 1,
        chunks[1].y + 1,
    ));
}

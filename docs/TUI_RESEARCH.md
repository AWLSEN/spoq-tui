# Command Deck TUI - Rust Architecture Document
## Building a JARVIS-style Terminal Interface with Full System Control

---

## Table of Contents
1. [Executive Summary](#executive-summary)
2. [Technology Stack](#technology-stack)
3. [Core Architecture](#core-architecture)
4. [Layout System](#layout-system)
5. [Multi-Screen Navigation](#multi-screen-navigation)
6. [Streaming & Real-time Updates](#streaming--real-time-updates)
7. [Chat Interface Implementation](#chat-interface-implementation)
8. [File System Indexing](#file-system-indexing)
9. [macOS System Integration](#macos-system-integration)
10. [Browser Control](#browser-control)
11. [Window Animations](#window-animations)
12. [System-Level Control](#system-level-control)
13. [Permissions Management](#permissions-management)
14. [Styling & Theming](#styling--theming)
15. [Project Structure](#project-structure)
16. [Implementation Roadmap](#implementation-roadmap)

---

## Executive Summary

The Command Deck is a JARVIS-style terminal interface built in Rust that provides:
- **Multi-panel dashboard** with dynamic layouts
- **Real-time chat** with streaming responses
- **File system indexing** for intelligent code search
- **Full macOS system control** via AppleScript
- **Browser orchestration** with smooth animations
- **Window choreography** across terminal and external apps

### Why Rust?

| Factor | Advantage |
|--------|-----------|
| **Performance** | Native speed, zero GC pauses, ~60fps rendering |
| **Memory Safety** | No crashes from null pointers or buffer overflows |
| **Async** | First-class Tokio support for streaming/WebSockets |
| **Ecosystem** | Ratatui + Crossterm = battle-tested TUI stack |
| **Binary Size** | Single ~5MB executable, no runtime dependencies |
| **System Access** | Easy FFI for AppleScript, file indexing |

---

## Technology Stack

### Cargo.toml

```toml
[package]
name = "command_deck"
version = "0.1.0"
edition = "2021"

[dependencies]
# TUI Framework
ratatui = "0.29"
crossterm = "0.28"

# Async Runtime
tokio = { version = "1.40", features = ["full"] }
tokio-util = "0.7"
futures-util = "0.3"

# Networking
tokio-tungstenite = "0.24"     # WebSocket
reqwest = { version = "0.12", features = ["json", "stream"] }

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# File Indexing
tantivy = "0.22"               # Full-text search
walkdir = "2.5"                # Directory traversal
ignore = "0.4"                 # .gitignore support
notify = "6.1"                 # File watching

# Utilities
anyhow = "1.0"                 # Error handling
tracing = "0.1"                # Logging
chrono = "0.4"                 # Time handling
unicode-width = "0.1"          # Text width calculation

[profile.release]
lto = true
codegen-units = 1
strip = true
```

---

## Core Architecture

### The Elm Architecture (TEA)

The Command Deck uses the Elm architecture pattern for predictable state management:

```
┌─────────────────────────────────────────────────────┐
│                      Model                           │
│            (Complete Application State)              │
└─────────────────────────────────────────────────────┘
              │                           ▲
              │ render()                  │ update()
              ▼                           │
┌──────────────────────┐    ┌──────────────────────────┐
│        View          │    │         Update           │
│  (Ratatui Rendering) │    │    (State Mutations)     │
└──────────────────────┘    └──────────────────────────┘
              │                           ▲
              │                           │ Message
              ▼                           │
┌─────────────────────────────────────────────────────┐
│                    Event Bus                         │
│  (Keys, Mouse, Backend, Timers, System Events)       │
└─────────────────────────────────────────────────────┘
```

### Core Types

```rust
use anyhow::Result;
use tokio::sync::mpsc;

// === Messages ===
pub enum Message {
    // User Input
    Key(crossterm::event::KeyEvent),
    Mouse(crossterm::event::MouseEvent),
    Resize(u16, u16),

    // Navigation
    SwitchScreen(Screen),
    PushScreen(Screen),
    PopScreen,

    // Chat
    SendMessage(String),
    StreamChunk(String),
    StreamComplete,

    // Backend
    BackendConnected,
    BackendDisconnected,
    BackendMessage(BackendEvent),

    // System
    Tick,
    Quit,
}

// === Application State ===
pub struct Model {
    pub screen_stack: Vec<Screen>,
    pub current_screen: Screen,
    pub chat: ChatState,
    pub dashboard: DashboardState,
    pub search: SearchState,
    pub system: SystemState,
    pub browser: BrowserState,
    pub should_quit: bool,
}

// === Screens ===
#[derive(Clone, PartialEq)]
pub enum Screen {
    Dashboard,
    Chat,
    Search,
    Settings,
    Browser,
}

// === App Runner ===
pub struct App {
    model: Model,
    event_rx: mpsc::UnboundedReceiver<Message>,
    event_tx: mpsc::UnboundedSender<Message>,
}

impl App {
    pub async fn run(&mut self) -> Result<()> {
        let mut terminal = ratatui::init();

        loop {
            // Render
            terminal.draw(|frame| self.view(frame))?;

            // Handle events
            if let Some(msg) = self.event_rx.recv().await {
                self.update(msg);

                if self.model.should_quit {
                    break;
                }
            }
        }

        ratatui::restore();
        Ok(())
    }

    fn update(&mut self, msg: Message) {
        match msg {
            Message::Key(key) => self.handle_key(key),
            Message::SwitchScreen(screen) => {
                self.model.current_screen = screen;
            }
            Message::PushScreen(screen) => {
                self.model.screen_stack.push(self.model.current_screen.clone());
                self.model.current_screen = screen;
            }
            Message::PopScreen => {
                if let Some(screen) = self.model.screen_stack.pop() {
                    self.model.current_screen = screen;
                }
            }
            Message::StreamChunk(chunk) => {
                self.model.chat.append_chunk(&chunk);
            }
            Message::Quit => {
                self.model.should_quit = true;
            }
            _ => {}
        }
    }

    fn view(&self, frame: &mut ratatui::Frame) {
        match self.model.current_screen {
            Screen::Dashboard => self.render_dashboard(frame),
            Screen::Chat => self.render_chat(frame),
            Screen::Search => self.render_search(frame),
            Screen::Settings => self.render_settings(frame),
            Screen::Browser => self.render_browser_control(frame),
        }
    }
}
```

### Event Handler

```rust
use crossterm::event::{self, Event, KeyCode};
use std::time::Duration;
use tokio::sync::mpsc;

pub struct EventHandler {
    tx: mpsc::UnboundedSender<Message>,
}

impl EventHandler {
    pub fn new(tx: mpsc::UnboundedSender<Message>) -> Self {
        Self { tx }
    }

    pub fn spawn(self) {
        // Tick events (60fps)
        let tick_tx = self.tx.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(16));
            loop {
                interval.tick().await;
                if tick_tx.send(Message::Tick).is_err() {
                    break;
                }
            }
        });

        // Terminal events
        let term_tx = self.tx.clone();
        tokio::spawn(async move {
            loop {
                if event::poll(Duration::from_millis(10)).unwrap() {
                    match event::read().unwrap() {
                        Event::Key(key) => {
                            term_tx.send(Message::Key(key)).ok();
                        }
                        Event::Mouse(mouse) => {
                            term_tx.send(Message::Mouse(mouse)).ok();
                        }
                        Event::Resize(w, h) => {
                            term_tx.send(Message::Resize(w, h)).ok();
                        }
                        _ => {}
                    }
                }
            }
        });
    }

    pub fn sender(&self) -> mpsc::UnboundedSender<Message> {
        self.tx.clone()
    }
}
```

---

## Layout System

### Constraint-Based Layouts

Ratatui provides powerful constraint-based layouts:

```rust
use ratatui::layout::{Constraint, Direction, Layout, Rect};

// Constraint Types:
// - Length(n)      Fixed n characters
// - Percentage(n)  n% of available space
// - Min(n)         At least n characters
// - Max(n)         At most n characters
// - Fill(n)        Fill remaining space (ratio n)

fn create_layout(area: Rect) -> Vec<Rect> {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),      // Header: fixed 3 rows
            Constraint::Fill(1),        // Content: fills remaining
            Constraint::Length(3),      // Footer: fixed 3 rows
        ])
        .split(area)
        .to_vec()
}
```

### Command Deck Layout

```
┌─────────────────────────────────────────────────────────────────────┐
│                          COMMAND DECK v1.0                           │
├─────────────────────────────────────────────────────────────────────┤
│ ┌──────────────┐ ┌────────────────────────────┐ ┌──────────────────┐│
│ │   SYSTEM     │ │                            │ │    METRICS       ││
│ │   STATUS     │ │                            │ │    ────────      ││
│ │              │ │                            │ │    CPU: 45%      ││
│ │   ● Online   │ │      MAIN VIEWPORT         │ │    MEM: 2.1GB    ││
│ │   ● API OK   │ │                            │ │    NET: 125MB/s  ││
│ │   ● DB OK    │ │    [Chat / Dashboard]      │ │                  ││
│ │              │ │                            │ │    THREADS: 12   ││
│ ├──────────────┤ │                            │ ├──────────────────┤│
│ │   ACTIONS    │ │                            │ │    ALERTS        ││
│ │              │ │                            │ │                  ││
│ │   [1] Chat   │ │                            │ │    ⚠ Warning 1   ││
│ │   [2] Search │ │                            │ │    ⚠ Warning 2   ││
│ │   [3] Browse │ └────────────────────────────┘ │                  ││
│ └──────────────┘                                └──────────────────┘│
├─────────────────────────────────────────────────────────────────────┤
│ > Enter command...                          [Ctrl+1/2/3] [F1 Help]  │
└─────────────────────────────────────────────────────────────────────┘
```

### Implementation

```rust
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, BorderType, Paragraph, List, ListItem},
    Frame,
};

fn render_dashboard(&self, frame: &mut Frame) {
    let area = frame.area();

    // Main vertical layout
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),      // Header
            Constraint::Fill(1),        // Content
            Constraint::Length(3),      // Input bar
        ])
        .split(area);

    // Header
    let header = Paragraph::new(" COMMAND DECK v1.0 ")
        .style(Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD))
        .block(Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Double)
            .border_style(Style::default().fg(Color::Cyan)));
    frame.render_widget(header, main_chunks[0]);

    // Content: 3-column layout
    let content_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(16),     // Left sidebar
            Constraint::Fill(1),        // Main viewport
            Constraint::Length(20),     // Right sidebar
        ])
        .split(main_chunks[1]);

    // Left sidebar: stacked panels
    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ])
        .split(content_chunks[0]);

    // Render status panel
    self.render_status_panel(frame, left_chunks[0]);

    // Render actions panel
    self.render_actions_panel(frame, left_chunks[1]);

    // Render main viewport
    self.render_main_viewport(frame, content_chunks[1]);

    // Right sidebar: stacked panels
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ])
        .split(content_chunks[2]);

    // Render metrics panel
    self.render_metrics_panel(frame, right_chunks[0]);

    // Render alerts panel
    self.render_alerts_panel(frame, right_chunks[1]);

    // Render input bar
    self.render_input_bar(frame, main_chunks[2]);
}

fn render_status_panel(&self, frame: &mut Frame, area: Rect) {
    let items = vec![
        ListItem::new("● Online").style(Style::default().fg(Color::Green)),
        ListItem::new("● API OK").style(Style::default().fg(Color::Green)),
        ListItem::new("● DB OK").style(Style::default().fg(Color::Green)),
    ];

    let list = List::new(items)
        .block(Block::default()
            .title(" SYSTEM ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)));

    frame.render_widget(list, area);
}
```

---

## Multi-Screen Navigation

### Screen Stack Pattern

```rust
#[derive(Clone, PartialEq)]
pub enum Screen {
    Dashboard,
    Chat,
    Search,
    Settings,
    Browser,
    Help,
}

pub struct Navigator {
    stack: Vec<Screen>,
    current: Screen,
}

impl Navigator {
    pub fn new() -> Self {
        Self {
            stack: Vec::new(),
            current: Screen::Dashboard,
        }
    }

    pub fn push(&mut self, screen: Screen) {
        self.stack.push(self.current.clone());
        self.current = screen;
    }

    pub fn pop(&mut self) -> bool {
        if let Some(screen) = self.stack.pop() {
            self.current = screen;
            true
        } else {
            false
        }
    }

    pub fn switch(&mut self, screen: Screen) {
        self.stack.clear();
        self.current = screen;
    }

    pub fn current(&self) -> &Screen {
        &self.current
    }

    pub fn depth(&self) -> usize {
        self.stack.len()
    }
}
```

### Keybinding System

```rust
use crossterm::event::{KeyCode, KeyModifiers, KeyEvent};

pub struct KeyBindings {
    bindings: HashMap<(KeyCode, KeyModifiers), Action>,
}

pub enum Action {
    Navigate(Screen),
    SendMessage,
    CancelStream,
    Quit,
    Help,
    FocusInput,
    ScrollUp,
    ScrollDown,
    CycleThread,
}

impl KeyBindings {
    pub fn default_bindings() -> Self {
        let mut bindings = HashMap::new();

        // Global navigation (Ctrl+number for threads/screens)
        bindings.insert((KeyCode::Char('1'), KeyModifiers::CONTROL), Action::Navigate(Screen::Dashboard));
        bindings.insert((KeyCode::Char('2'), KeyModifiers::CONTROL), Action::Navigate(Screen::Chat));
        bindings.insert((KeyCode::Char('3'), KeyModifiers::CONTROL), Action::Navigate(Screen::Search));
        bindings.insert((KeyCode::Char('4'), KeyModifiers::CONTROL), Action::Navigate(Screen::Browser));

        // Actions
        bindings.insert((KeyCode::Enter, KeyModifiers::NONE), Action::SendMessage);
        bindings.insert((KeyCode::Esc, KeyModifiers::NONE), Action::CancelStream);
        bindings.insert((KeyCode::Char('q'), KeyModifiers::CONTROL), Action::Quit);
        bindings.insert((KeyCode::F(1), KeyModifiers::NONE), Action::Help);

        // Scrolling
        bindings.insert((KeyCode::Up, KeyModifiers::NONE), Action::ScrollUp);
        bindings.insert((KeyCode::Down, KeyModifiers::NONE), Action::ScrollDown);
        bindings.insert((KeyCode::PageUp, KeyModifiers::NONE), Action::ScrollUp);
        bindings.insert((KeyCode::PageDown, KeyModifiers::NONE), Action::ScrollDown);

        Self { bindings }
    }

    pub fn get_action(&self, key: &KeyEvent) -> Option<&Action> {
        self.bindings.get(&(key.code, key.modifiers))
    }
}
```

---

## Streaming & Real-time Updates

### Async Event Loop

```rust
use tokio::sync::mpsc;
use std::time::Duration;

pub async fn run_event_loop(
    mut app: App,
    mut event_rx: mpsc::UnboundedReceiver<Message>,
) -> Result<()> {
    let mut terminal = ratatui::init();
    let mut render_interval = tokio::time::interval(Duration::from_millis(16));
    let mut needs_render = true;

    loop {
        tokio::select! {
            // Render at 60fps
            _ = render_interval.tick() => {
                if needs_render {
                    terminal.draw(|frame| app.view(frame))?;
                    needs_render = false;
                }
            }

            // Handle events
            Some(msg) = event_rx.recv() => {
                app.update(msg);
                needs_render = true;

                if app.model.should_quit {
                    break;
                }
            }
        }
    }

    ratatui::restore();
    Ok(())
}
```

### WebSocket Backend Connection

```rust
use tokio_tungstenite::{connect_async, tungstenite::Message as WsMessage};
use futures_util::{StreamExt, SinkExt};

pub struct BackendConnection {
    tx: mpsc::UnboundedSender<Message>,
    write: futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>
        >,
        WsMessage
    >,
}

impl BackendConnection {
    pub async fn connect(
        url: &str,
        event_tx: mpsc::UnboundedSender<Message>,
    ) -> Result<Self> {
        let (ws_stream, _) = connect_async(url).await?;
        let (write, mut read) = ws_stream.split();

        // Spawn reader task
        let tx = event_tx.clone();
        tokio::spawn(async move {
            while let Some(msg) = read.next().await {
                match msg {
                    Ok(WsMessage::Text(text)) => {
                        if let Ok(event) = serde_json::from_str::<BackendEvent>(&text) {
                            match event {
                                BackendEvent::ChatChunk(chunk) => {
                                    tx.send(Message::StreamChunk(chunk)).ok();
                                }
                                BackendEvent::ChatComplete => {
                                    tx.send(Message::StreamComplete).ok();
                                }
                                BackendEvent::Metrics(m) => {
                                    tx.send(Message::BackendMessage(
                                        BackendEvent::Metrics(m)
                                    )).ok();
                                }
                                _ => {}
                            }
                        }
                    }
                    Err(_) => {
                        tx.send(Message::BackendDisconnected).ok();
                        break;
                    }
                    _ => {}
                }
            }
        });

        event_tx.send(Message::BackendConnected).ok();

        Ok(Self {
            tx: event_tx,
            write,
        })
    }

    pub async fn send(&mut self, msg: &str) -> Result<()> {
        self.write.send(WsMessage::Text(msg.to_string())).await?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BackendEvent {
    ChatChunk(String),
    ChatComplete,
    Metrics(SystemMetrics),
    Alert(Alert),
    Status(ServiceStatus),
}
```

---

## Chat Interface Implementation

### Chat State

```rust
#[derive(Default)]
pub struct ChatState {
    pub threads: Vec<ChatThread>,
    pub active_thread: usize,
    pub input: String,
    pub input_cursor: usize,
    pub streaming: Option<StreamingMessage>,
    pub scroll_offset: usize,
}

pub struct ChatThread {
    pub id: String,
    pub name: String,
    pub messages: Vec<ChatMessage>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub struct ChatMessage {
    pub role: Role,
    pub content: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Clone, PartialEq)]
pub enum Role {
    User,
    Assistant,
    System,
}

pub struct StreamingMessage {
    pub content: String,
    pub cursor_visible: bool,
    pub last_chunk_at: std::time::Instant,
}

impl ChatState {
    pub fn append_chunk(&mut self, chunk: &str) {
        if let Some(ref mut stream) = self.streaming {
            stream.content.push_str(chunk);
            stream.last_chunk_at = std::time::Instant::now();
        } else {
            self.streaming = Some(StreamingMessage {
                content: chunk.to_string(),
                cursor_visible: true,
                last_chunk_at: std::time::Instant::now(),
            });
        }
    }

    pub fn complete_stream(&mut self) {
        if let Some(stream) = self.streaming.take() {
            if let Some(thread) = self.threads.get_mut(self.active_thread) {
                thread.messages.push(ChatMessage {
                    role: Role::Assistant,
                    content: stream.content,
                    timestamp: chrono::Utc::now(),
                });
            }
        }
    }

    pub fn send_message(&mut self, content: String) {
        if let Some(thread) = self.threads.get_mut(self.active_thread) {
            thread.messages.push(ChatMessage {
                role: Role::User,
                content,
                timestamp: chrono::Utc::now(),
            });
        }
    }
}
```

### Chat Renderer

```rust
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap, Scrollbar, ScrollbarOrientation},
    Frame,
};

impl App {
    fn render_chat(&self, frame: &mut Frame) {
        let area = frame.area();

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),      // Thread tabs
                Constraint::Fill(1),        // Messages
                Constraint::Length(3),      // Input
            ])
            .split(area);

        // Thread tabs
        self.render_thread_tabs(frame, chunks[0]);

        // Messages
        self.render_messages(frame, chunks[1]);

        // Input
        self.render_input(frame, chunks[2]);
    }

    fn render_messages(&self, frame: &mut Frame, area: Rect) {
        let chat = &self.model.chat;
        let mut lines: Vec<Line> = Vec::new();

        if let Some(thread) = chat.threads.get(chat.active_thread) {
            for msg in &thread.messages {
                let (prefix, style) = match msg.role {
                    Role::User => (
                        "You: ",
                        Style::default().fg(Color::Cyan)
                    ),
                    Role::Assistant => (
                        "Assistant: ",
                        Style::default().fg(Color::Green)
                    ),
                    Role::System => (
                        "System: ",
                        Style::default().fg(Color::Yellow)
                    ),
                };

                lines.push(Line::from(vec![
                    Span::styled(prefix, style.add_modifier(Modifier::BOLD)),
                ]));

                for line in msg.content.lines() {
                    lines.push(Line::from(Span::styled(
                        format!("  {}", line),
                        style,
                    )));
                }

                lines.push(Line::from("")); // Spacing
            }
        }

        // Render streaming message with cursor
        if let Some(ref stream) = chat.streaming {
            lines.push(Line::from(vec![
                Span::styled(
                    "Assistant: ",
                    Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
                ),
            ]));

            for line in stream.content.lines() {
                lines.push(Line::from(Span::styled(
                    format!("  {}", line),
                    Style::default().fg(Color::Green),
                )));
            }

            // Blinking cursor
            if stream.cursor_visible {
                lines.push(Line::from(Span::styled(
                    "  \u{2588}", // Block cursor
                    Style::default().fg(Color::Green),
                )));
            }
        }

        let paragraph = Paragraph::new(lines)
            .block(Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)))
            .wrap(Wrap { trim: false })
            .scroll((chat.scroll_offset as u16, 0));

        frame.render_widget(paragraph, area);

        // Scrollbar
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
        frame.render_stateful_widget(
            scrollbar,
            area,
            &mut ScrollbarState::new(lines.len()).position(chat.scroll_offset),
        );
    }

    fn render_input(&self, frame: &mut Frame, area: Rect) {
        let chat = &self.model.chat;

        // Create input with cursor
        let input_text = if chat.input.is_empty() {
            Span::styled(
                "Type your message...",
                Style::default().fg(Color::DarkGray),
            )
        } else {
            let before = &chat.input[..chat.input_cursor];
            let cursor = "\u{2588}";
            let after = &chat.input[chat.input_cursor..];

            Span::raw(format!("{}{}{}", before, cursor, after))
        };

        let input = Paragraph::new(input_text)
            .block(Block::default()
                .title(" Message ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)));

        frame.render_widget(input, area);
    }
}
```

---

## File System Indexing

### Tantivy-Based Indexer

```rust
use tantivy::{
    schema::{Schema, STORED, TEXT, Field},
    Index, IndexWriter, Document, TantivyDocument,
    collector::TopDocs,
    query::QueryParser,
};
use walkdir::WalkDir;
use ignore::gitignore::Gitignore;
use std::path::{Path, PathBuf};

pub struct FileIndexer {
    index: Index,
    writer: IndexWriter,
    schema: Schema,
    path_field: Field,
    content_field: Field,
    name_field: Field,
}

impl FileIndexer {
    pub fn new(index_path: &Path) -> Result<Self> {
        let mut schema_builder = Schema::builder();

        let path_field = schema_builder.add_text_field("path", TEXT | STORED);
        let content_field = schema_builder.add_text_field("content", TEXT);
        let name_field = schema_builder.add_text_field("name", TEXT | STORED);

        let schema = schema_builder.build();
        let index = Index::create_in_dir(index_path, schema.clone())?;
        let writer = index.writer(50_000_000)?; // 50MB buffer

        Ok(Self {
            index,
            writer,
            schema,
            path_field,
            content_field,
            name_field,
        })
    }

    pub fn index_directory(&mut self, root: &Path) -> Result<IndexStats> {
        let gitignore = Gitignore::new(root.join(".gitignore")).0;
        let mut stats = IndexStats::default();

        for entry in WalkDir::new(root)
            .into_iter()
            .filter_entry(|e| !self.should_ignore(e.path(), &gitignore))
        {
            let entry = entry?;

            if entry.file_type().is_file() {
                if let Ok(content) = std::fs::read_to_string(entry.path()) {
                    let mut doc = TantivyDocument::default();
                    doc.add_text(self.path_field, entry.path().to_string_lossy());
                    doc.add_text(self.name_field, entry.file_name().to_string_lossy());
                    doc.add_text(self.content_field, &content);

                    self.writer.add_document(doc)?;
                    stats.files_indexed += 1;
                    stats.bytes_indexed += content.len();
                }
            }
        }

        self.writer.commit()?;
        stats.index_size = self.index.directory().total_size()?;

        Ok(stats)
    }

    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let reader = self.index.reader()?;
        let searcher = reader.searcher();

        let query_parser = QueryParser::for_index(
            &self.index,
            vec![self.content_field, self.name_field, self.path_field],
        );

        let query = query_parser.parse_query(query)?;
        let top_docs = searcher.search(&query, &TopDocs::with_limit(limit))?;

        let mut results = Vec::new();
        for (_score, doc_address) in top_docs {
            let doc: TantivyDocument = searcher.doc(doc_address)?;

            if let Some(path) = doc.get_first(self.path_field) {
                results.push(SearchResult {
                    path: PathBuf::from(path.as_str().unwrap()),
                    name: doc.get_first(self.name_field)
                        .map(|v| v.as_str().unwrap().to_string())
                        .unwrap_or_default(),
                    score: _score,
                });
            }
        }

        Ok(results)
    }

    fn should_ignore(&self, path: &Path, gitignore: &Gitignore) -> bool {
        // Always ignore these
        let ignore_patterns = [
            ".git", "node_modules", "target", ".build",
            "__pycache__", ".cache", "dist", ".next",
        ];

        if let Some(name) = path.file_name() {
            let name_str = name.to_string_lossy();
            if ignore_patterns.iter().any(|p| name_str == *p) {
                return true;
            }
        }

        gitignore.matched(path, path.is_dir()).is_ignore()
    }
}

#[derive(Default)]
pub struct IndexStats {
    pub files_indexed: usize,
    pub bytes_indexed: usize,
    pub index_size: u64,
}

pub struct SearchResult {
    pub path: PathBuf,
    pub name: String,
    pub score: f32,
}
```

### File Watcher

```rust
use notify::{Watcher, RecursiveMode, Event, EventKind};
use std::sync::mpsc as std_mpsc;

pub struct FileWatcher {
    watcher: notify::RecommendedWatcher,
    event_tx: mpsc::UnboundedSender<Message>,
}

impl FileWatcher {
    pub fn new(
        root: PathBuf,
        event_tx: mpsc::UnboundedSender<Message>,
    ) -> Result<Self> {
        let tx = event_tx.clone();

        let watcher = notify::recommended_watcher(move |res: Result<Event, _>| {
            if let Ok(event) = res {
                match event.kind {
                    EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) => {
                        for path in event.paths {
                            tx.send(Message::FileChanged(path.clone())).ok();
                        }
                    }
                    _ => {}
                }
            }
        })?;

        let mut w = Self { watcher, event_tx };
        w.watcher.watch(&root, RecursiveMode::Recursive)?;

        Ok(w)
    }
}
```

---

## macOS System Integration

### AppleScript Executor

```rust
use std::process::Command;
use anyhow::Result;

pub struct AppleScript;

impl AppleScript {
    /// Execute an AppleScript and return the result
    pub fn execute(script: &str) -> Result<String> {
        let output = Command::new("osascript")
            .arg("-e")
            .arg(script)
            .output()?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            Err(anyhow::anyhow!(
                "AppleScript error: {}",
                String::from_utf8_lossy(&output.stderr)
            ))
        }
    }

    /// Execute multi-line AppleScript
    pub fn execute_multi(lines: &[&str]) -> Result<String> {
        let script = lines.join("\n");
        Self::execute(&script)
    }

    /// Execute JavaScript for Automation (JXA)
    pub fn execute_jxa(script: &str) -> Result<String> {
        let output = Command::new("osascript")
            .arg("-l")
            .arg("JavaScript")
            .arg("-e")
            .arg(script)
            .output()?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            Err(anyhow::anyhow!(
                "JXA error: {}",
                String::from_utf8_lossy(&output.stderr)
            ))
        }
    }
}
```

### System Controller

```rust
pub struct SystemController;

impl SystemController {
    /// Get current display bounds
    pub fn get_screen_bounds() -> Result<ScreenBounds> {
        let script = r#"
            tell application "Finder"
                set screenBounds to bounds of window of desktop
                return (item 1 of screenBounds) & "," & (item 2 of screenBounds) & "," & (item 3 of screenBounds) & "," & (item 4 of screenBounds)
            end tell
        "#;

        let result = AppleScript::execute(script)?;
        let parts: Vec<i32> = result.split(',')
            .map(|s| s.trim().parse().unwrap_or(0))
            .collect();

        Ok(ScreenBounds {
            x: parts.get(0).copied().unwrap_or(0),
            y: parts.get(1).copied().unwrap_or(0),
            width: parts.get(2).copied().unwrap_or(1920),
            height: parts.get(3).copied().unwrap_or(1080),
        })
    }

    /// Get terminal window bounds
    pub fn get_terminal_bounds() -> Result<WindowBounds> {
        let script = r#"
            tell application "Terminal"
                set winBounds to bounds of front window
                return (item 1 of winBounds) & "," & (item 2 of winBounds) & "," & (item 3 of winBounds) & "," & (item 4 of winBounds)
            end tell
        "#;

        let result = AppleScript::execute(script)?;
        let parts: Vec<i32> = result.split(',')
            .map(|s| s.trim().parse().unwrap_or(0))
            .collect();

        Ok(WindowBounds {
            x: parts.get(0).copied().unwrap_or(0),
            y: parts.get(1).copied().unwrap_or(0),
            width: parts.get(2).copied().unwrap_or(800) - parts.get(0).copied().unwrap_or(0),
            height: parts.get(3).copied().unwrap_or(600) - parts.get(1).copied().unwrap_or(0),
        })
    }

    /// Set terminal window bounds
    pub fn set_terminal_bounds(bounds: &WindowBounds) -> Result<()> {
        let script = format!(r#"
            tell application "Terminal"
                set bounds of front window to {{{}, {}, {}, {}}}
            end tell
        "#, bounds.x, bounds.y, bounds.x + bounds.width, bounds.y + bounds.height);

        AppleScript::execute(&script)?;
        Ok(())
    }

    /// Set system volume (0-100)
    pub fn set_volume(level: u8) -> Result<()> {
        let level = level.min(100);
        let script = format!("set volume output volume {}", level);
        AppleScript::execute(&script)?;
        Ok(())
    }

    /// Show notification
    pub fn notify(title: &str, message: &str) -> Result<()> {
        let script = format!(r#"
            display notification "{}" with title "{}"
        "#, message.replace('"', r#"\""#), title.replace('"', r#"\""#));

        AppleScript::execute(&script)?;
        Ok(())
    }

    /// Switch to Mission Control space
    pub fn switch_space(space_number: u8) -> Result<()> {
        let script = format!(r#"
            tell application "System Events"
                key code {} using control down
            end tell
        "#, 17 + space_number); // key codes for 1-9

        AppleScript::execute(&script)?;
        Ok(())
    }

    /// Hide all other apps (focus mode)
    pub fn focus_mode() -> Result<()> {
        let script = r#"
            tell application "System Events"
                set visible of every process whose visible is true and name is not "Terminal" to false
            end tell
        "#;

        AppleScript::execute(script)?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct ScreenBounds {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Clone)]
pub struct WindowBounds {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}
```

---

## Browser Control

### Browser Controller

```rust
pub struct BrowserController {
    current_url: Option<String>,
    bounds: Option<WindowBounds>,
}

impl BrowserController {
    pub fn new() -> Self {
        Self {
            current_url: None,
            bounds: None,
        }
    }

    /// Open URL in Safari
    pub fn open_safari(&mut self, url: &str) -> Result<()> {
        let script = format!(r#"
            tell application "Safari"
                activate
                open location "{}"
            end tell
        "#, url);

        AppleScript::execute(&script)?;
        self.current_url = Some(url.to_string());
        Ok(())
    }

    /// Open URL in Chrome
    pub fn open_chrome(&mut self, url: &str) -> Result<()> {
        let script = format!(r#"
            tell application "Google Chrome"
                activate
                open location "{}"
            end tell
        "#, url);

        AppleScript::execute(&script)?;
        self.current_url = Some(url.to_string());
        Ok(())
    }

    /// Position browser beside terminal
    pub fn position_beside_terminal(&mut self) -> Result<()> {
        let script = r#"
            tell application "Terminal"
                set termBounds to bounds of front window
                set termRight to item 3 of termBounds
                set termTop to item 2 of termBounds
                set termBottom to item 4 of termBounds
            end tell

            tell application "Safari"
                set bounds of front window to {termRight, termTop, termRight + 800, termBottom}
            end tell
        "#;

        AppleScript::execute(script)?;
        Ok(())
    }

    /// Set browser window bounds
    pub fn set_bounds(&mut self, bounds: &WindowBounds) -> Result<()> {
        let script = format!(r#"
            tell application "Safari"
                set bounds of front window to {{{}, {}, {}, {}}}
            end tell
        "#, bounds.x, bounds.y, bounds.x + bounds.width, bounds.y + bounds.height);

        AppleScript::execute(&script)?;
        self.bounds = Some(bounds.clone());
        Ok(())
    }

    /// Get current browser bounds
    pub fn get_bounds(&self) -> Result<WindowBounds> {
        let script = r#"
            tell application "Safari"
                set winBounds to bounds of front window
                return (item 1 of winBounds) & "," & (item 2 of winBounds) & "," & (item 3 of winBounds) & "," & (item 4 of winBounds)
            end tell
        "#;

        let result = AppleScript::execute(script)?;
        let parts: Vec<i32> = result.split(',')
            .map(|s| s.trim().parse().unwrap_or(0))
            .collect();

        Ok(WindowBounds {
            x: parts.get(0).copied().unwrap_or(0),
            y: parts.get(1).copied().unwrap_or(0),
            width: parts.get(2).copied().unwrap_or(800) - parts.get(0).copied().unwrap_or(0),
            height: parts.get(3).copied().unwrap_or(600) - parts.get(1).copied().unwrap_or(0),
        })
    }

    /// Navigate to URL
    pub fn navigate(&mut self, url: &str) -> Result<()> {
        let script = format!(r#"
            tell application "Safari"
                set URL of current tab of front window to "{}"
            end tell
        "#, url);

        AppleScript::execute(&script)?;
        self.current_url = Some(url.to_string());
        Ok(())
    }

    /// Execute JavaScript in browser
    pub fn execute_js(&self, js: &str) -> Result<String> {
        let escaped = js.replace('"', r#"\""#);
        let script = format!(r#"
            tell application "Safari"
                do JavaScript "{}" in current tab of front window
            end tell
        "#, escaped);

        AppleScript::execute(&script)
    }

    /// Get page title
    pub fn get_title(&self) -> Result<String> {
        let script = r#"
            tell application "Safari"
                return name of current tab of front window
            end tell
        "#;

        AppleScript::execute(script)
    }

    /// Scroll page
    pub fn scroll(&self, pixels: i32) -> Result<()> {
        let js = format!("window.scrollBy(0, {})", pixels);
        self.execute_js(&js)?;
        Ok(())
    }

    /// Close browser
    pub fn close(&mut self) -> Result<()> {
        let script = r#"
            tell application "Safari"
                close front window
            end tell
        "#;

        AppleScript::execute(script)?;
        self.current_url = None;
        self.bounds = None;
        Ok(())
    }

    /// Hide browser (minimize)
    pub fn hide(&self) -> Result<()> {
        let script = r#"
            tell application "Safari"
                set miniaturized of front window to true
            end tell
        "#;

        AppleScript::execute(script)
    }

    /// Show browser (bring to front)
    pub fn show(&self) -> Result<()> {
        let script = r#"
            tell application "Safari"
                activate
                set miniaturized of front window to false
            end tell
        "#;

        AppleScript::execute(script)
    }
}
```

### AI-Driven Web Actions

```rust
/// Actions that AI can trigger based on conversation context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AIAction {
    /// Plain text response
    TextResponse(String),

    /// Open a web search
    WebSearch {
        query: String,
        engine: SearchEngine,
    },

    /// Open a specific URL
    OpenURL {
        url: String,
        position: Option<WindowPosition>,
    },

    /// Open documentation
    OpenDocs {
        topic: String,
        language: Option<String>,
    },

    /// Show image/preview
    ShowPreview {
        url: String,
    },

    /// Execute code in browser console
    BrowserAction {
        action: BrowserActionType,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SearchEngine {
    Google,
    DuckDuckGo,
    GitHub,
    StackOverflow,
    Docs,
}

impl SearchEngine {
    pub fn search_url(&self, query: &str) -> String {
        let encoded = urlencoding::encode(query);
        match self {
            SearchEngine::Google => format!("https://google.com/search?q={}", encoded),
            SearchEngine::DuckDuckGo => format!("https://duckduckgo.com/?q={}", encoded),
            SearchEngine::GitHub => format!("https://github.com/search?q={}", encoded),
            SearchEngine::StackOverflow => format!("https://stackoverflow.com/search?q={}", encoded),
            SearchEngine::Docs => format!("https://devdocs.io/#q={}", encoded),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WindowPosition {
    BesideTerminal,
    BottomRight,
    BottomLeft,
    TopRight,
    TopLeft,
    Center,
    Fullscreen,
    Custom(WindowBounds),
}

pub struct AIActionHandler {
    browser: BrowserController,
}

impl AIActionHandler {
    pub fn new() -> Self {
        Self {
            browser: BrowserController::new(),
        }
    }

    pub async fn handle(&mut self, action: AIAction) -> Result<()> {
        match action {
            AIAction::WebSearch { query, engine } => {
                let url = engine.search_url(&query);
                self.browser.open_safari(&url)?;
                self.browser.position_beside_terminal()?;
            }

            AIAction::OpenURL { url, position } => {
                self.browser.open_safari(&url)?;
                if let Some(pos) = position {
                    self.position_window(pos)?;
                }
            }

            AIAction::OpenDocs { topic, language } => {
                let url = match language.as_deref() {
                    Some("rust") => format!("https://docs.rs/{}", topic),
                    Some("python") => format!("https://docs.python.org/3/search.html?q={}", topic),
                    Some("js") | Some("javascript") => format!("https://developer.mozilla.org/en-US/search?q={}", topic),
                    _ => format!("https://devdocs.io/#q={}", topic),
                };
                self.browser.open_safari(&url)?;
                self.browser.position_beside_terminal()?;
            }

            AIAction::ShowPreview { url } => {
                self.browser.open_safari(&url)?;
                // Position in corner
                self.position_window(WindowPosition::BottomRight)?;
            }

            _ => {}
        }

        Ok(())
    }

    fn position_window(&mut self, position: WindowPosition) -> Result<()> {
        let screen = SystemController::get_screen_bounds()?;
        let terminal = SystemController::get_terminal_bounds()?;

        let bounds = match position {
            WindowPosition::BesideTerminal => {
                WindowBounds {
                    x: terminal.x + terminal.width,
                    y: terminal.y,
                    width: screen.width - terminal.x - terminal.width,
                    height: terminal.height,
                }
            }
            WindowPosition::BottomRight => {
                WindowBounds {
                    x: screen.width - 400,
                    y: screen.height - 300,
                    width: 400,
                    height: 300,
                }
            }
            WindowPosition::Center => {
                let w = 800;
                let h = 600;
                WindowBounds {
                    x: (screen.width - w) / 2,
                    y: (screen.height - h) / 2,
                    width: w,
                    height: h,
                }
            }
            WindowPosition::Fullscreen => {
                WindowBounds {
                    x: 0,
                    y: 0,
                    width: screen.width,
                    height: screen.height,
                }
            }
            WindowPosition::Custom(b) => b,
            _ => return Ok(()),
        };

        self.browser.set_bounds(&bounds)
    }
}
```

---

## Window Animations

### Easing Functions

```rust
use std::f64::consts::PI;

#[derive(Debug, Clone, Copy)]
pub enum Easing {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    EaseOutBack,
    EaseOutBounce,
    EaseOutElastic,
    EaseInQuad,
    EaseOutQuad,
    EaseInCubic,
    EaseOutCubic,
}

impl Easing {
    /// Apply easing function to t (0.0 to 1.0)
    pub fn apply(&self, t: f64) -> f64 {
        let t = t.clamp(0.0, 1.0);

        match self {
            Easing::Linear => t,

            Easing::EaseIn => t * t,

            Easing::EaseOut => 1.0 - (1.0 - t).powi(2),

            Easing::EaseInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
                }
            }

            Easing::EaseOutBack => {
                let c1 = 1.70158;
                let c3 = c1 + 1.0;
                1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
            }

            Easing::EaseOutBounce => {
                let n1 = 7.5625;
                let d1 = 2.75;

                if t < 1.0 / d1 {
                    n1 * t * t
                } else if t < 2.0 / d1 {
                    let t = t - 1.5 / d1;
                    n1 * t * t + 0.75
                } else if t < 2.5 / d1 {
                    let t = t - 2.25 / d1;
                    n1 * t * t + 0.9375
                } else {
                    let t = t - 2.625 / d1;
                    n1 * t * t + 0.984375
                }
            }

            Easing::EaseOutElastic => {
                if t == 0.0 || t == 1.0 {
                    t
                } else {
                    let c4 = (2.0 * PI) / 3.0;
                    2.0_f64.powf(-10.0 * t) * ((t * 10.0 - 0.75) * c4).sin() + 1.0
                }
            }

            Easing::EaseInQuad => t * t,

            Easing::EaseOutQuad => 1.0 - (1.0 - t) * (1.0 - t),

            Easing::EaseInCubic => t * t * t,

            Easing::EaseOutCubic => 1.0 - (1.0 - t).powi(3),
        }
    }
}
```

### Window Animator

```rust
use tokio::time::{sleep, Duration, Instant};

pub struct WindowAnimator {
    browser: BrowserController,
}

impl WindowAnimator {
    pub fn new(browser: BrowserController) -> Self {
        Self { browser }
    }

    /// Animate browser window to target bounds
    pub async fn animate_to(
        &mut self,
        target: WindowBounds,
        duration: Duration,
        easing: Easing,
    ) -> Result<()> {
        let start = self.browser.get_bounds()?;
        let start_time = Instant::now();

        // 60fps animation
        let frame_duration = Duration::from_millis(16);

        while start_time.elapsed() < duration {
            let elapsed = start_time.elapsed().as_secs_f64();
            let total = duration.as_secs_f64();
            let t = easing.apply(elapsed / total);

            let current = WindowBounds {
                x: lerp(start.x, target.x, t),
                y: lerp(start.y, target.y, t),
                width: lerp(start.width, target.width, t),
                height: lerp(start.height, target.height, t),
            };

            self.browser.set_bounds(&current)?;
            sleep(frame_duration).await;
        }

        // Ensure we end exactly at target
        self.browser.set_bounds(&target)?;
        Ok(())
    }

    /// Slide in from right
    pub async fn slide_in_from_right(&mut self, target: WindowBounds) -> Result<()> {
        let screen = SystemController::get_screen_bounds()?;

        let start = WindowBounds {
            x: screen.width, // Start offscreen right
            y: target.y,
            width: target.width,
            height: target.height,
        };

        self.browser.set_bounds(&start)?;
        self.browser.show()?;

        self.animate_to(target, Duration::from_millis(400), Easing::EaseOutCubic).await
    }

    /// Slide out to right
    pub async fn slide_out_to_right(&mut self) -> Result<()> {
        let screen = SystemController::get_screen_bounds()?;
        let current = self.browser.get_bounds()?;

        let target = WindowBounds {
            x: screen.width,
            y: current.y,
            width: current.width,
            height: current.height,
        };

        self.animate_to(target, Duration::from_millis(300), Easing::EaseInQuad).await?;
        self.browser.hide()?;
        Ok(())
    }

    /// Pop in with bounce
    pub async fn pop_in(&mut self, target: WindowBounds) -> Result<()> {
        // Start small and centered
        let center_x = target.x + target.width / 2;
        let center_y = target.y + target.height / 2;

        let start = WindowBounds {
            x: center_x - 10,
            y: center_y - 10,
            width: 20,
            height: 20,
        };

        self.browser.set_bounds(&start)?;
        self.browser.show()?;

        self.animate_to(target, Duration::from_millis(500), Easing::EaseOutBack).await
    }

    /// Fade and scale out
    pub async fn scale_out(&mut self) -> Result<()> {
        let current = self.browser.get_bounds()?;
        let center_x = current.x + current.width / 2;
        let center_y = current.y + current.height / 2;

        let target = WindowBounds {
            x: center_x,
            y: center_y,
            width: 0,
            height: 0,
        };

        self.animate_to(target, Duration::from_millis(300), Easing::EaseInQuad).await?;
        self.browser.close()?;
        Ok(())
    }
}

fn lerp(start: i32, end: i32, t: f64) -> i32 {
    (start as f64 + (end - start) as f64 * t).round() as i32
}
```

### Choreographed Window Scenes

```rust
pub struct WindowChoreographer {
    browser_animator: WindowAnimator,
    event_tx: mpsc::UnboundedSender<Message>,
}

impl WindowChoreographer {
    /// Split screen: terminal left, browser right
    pub async fn scene_split_screen(&mut self, url: &str) -> Result<()> {
        let screen = SystemController::get_screen_bounds()?;

        // Animate terminal to left half
        let terminal_target = WindowBounds {
            x: 0,
            y: 0,
            width: screen.width / 2,
            height: screen.height,
        };

        // Open and position browser
        self.browser_animator.browser.open_safari(url)?;

        tokio::time::sleep(Duration::from_millis(300)).await;

        let browser_target = WindowBounds {
            x: screen.width / 2,
            y: 0,
            width: screen.width / 2,
            height: screen.height,
        };

        // Animate both
        tokio::join!(
            async {
                SystemController::set_terminal_bounds(&terminal_target).ok();
            },
            self.browser_animator.slide_in_from_right(browser_target)
        );

        Ok(())
    }

    /// Picture-in-picture: small browser in corner
    pub async fn scene_pip(&mut self, url: &str) -> Result<()> {
        let screen = SystemController::get_screen_bounds()?;

        let pip_bounds = WindowBounds {
            x: screen.width - 420,
            y: screen.height - 320,
            width: 400,
            height: 300,
        };

        self.browser_animator.browser.open_safari(url)?;
        tokio::time::sleep(Duration::from_millis(200)).await;

        self.browser_animator.pop_in(pip_bounds).await
    }

    /// Full focus: browser fullscreen
    pub async fn scene_fullscreen(&mut self, url: &str) -> Result<()> {
        let screen = SystemController::get_screen_bounds()?;

        self.browser_animator.browser.open_safari(url)?;

        let target = WindowBounds {
            x: 0,
            y: 0,
            width: screen.width,
            height: screen.height,
        };

        self.browser_animator.animate_to(
            target,
            Duration::from_millis(400),
            Easing::EaseOutCubic
        ).await
    }

    /// Dismiss browser
    pub async fn scene_dismiss(&mut self) -> Result<()> {
        self.browser_animator.slide_out_to_right().await?;

        // Restore terminal to full width
        let screen = SystemController::get_screen_bounds()?;
        let terminal_target = WindowBounds {
            x: 50,
            y: 50,
            width: screen.width - 100,
            height: screen.height - 100,
        };

        SystemController::set_terminal_bounds(&terminal_target)?;
        Ok(())
    }
}
```

---

## System-Level Control

### Complete System Controller

```rust
pub struct FullSystemController;

impl FullSystemController {
    // === App Control ===

    /// Launch any application
    pub fn launch_app(app_name: &str) -> Result<()> {
        let script = format!(r#"
            tell application "{}" to activate
        "#, app_name);
        AppleScript::execute(&script)?;
        Ok(())
    }

    /// Quit any application
    pub fn quit_app(app_name: &str) -> Result<()> {
        let script = format!(r#"
            tell application "{}" to quit
        "#, app_name);
        AppleScript::execute(&script)?;
        Ok(())
    }

    /// Get list of running apps
    pub fn list_running_apps() -> Result<Vec<String>> {
        let script = r#"
            tell application "System Events"
                set appNames to name of every application process whose background only is false
                set output to ""
                repeat with appName in appNames
                    set output to output & appName & ","
                end repeat
                return output
            end tell
        "#;

        let result = AppleScript::execute(script)?;
        Ok(result.split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect())
    }

    // === Window Management ===

    /// Get any app's window bounds
    pub fn get_app_bounds(app_name: &str) -> Result<WindowBounds> {
        let script = format!(r#"
            tell application "{}"
                set winBounds to bounds of front window
                return (item 1 of winBounds) & "," & (item 2 of winBounds) & "," & (item 3 of winBounds) & "," & (item 4 of winBounds)
            end tell
        "#, app_name);

        let result = AppleScript::execute(&script)?;
        let parts: Vec<i32> = result.split(',')
            .map(|s| s.trim().parse().unwrap_or(0))
            .collect();

        Ok(WindowBounds {
            x: parts.get(0).copied().unwrap_or(0),
            y: parts.get(1).copied().unwrap_or(0),
            width: parts.get(2).copied().unwrap_or(800) - parts.get(0).copied().unwrap_or(0),
            height: parts.get(3).copied().unwrap_or(600) - parts.get(1).copied().unwrap_or(0),
        })
    }

    /// Set any app's window bounds
    pub fn set_app_bounds(app_name: &str, bounds: &WindowBounds) -> Result<()> {
        let script = format!(r#"
            tell application "{}"
                set bounds of front window to {{{}, {}, {}, {}}}
            end tell
        "#, app_name, bounds.x, bounds.y, bounds.x + bounds.width, bounds.y + bounds.height);

        AppleScript::execute(&script)?;
        Ok(())
    }

    // === Audio Control ===

    /// Get current volume (0-100)
    pub fn get_volume() -> Result<u8> {
        let script = "output volume of (get volume settings)";
        let result = AppleScript::execute(script)?;
        Ok(result.parse().unwrap_or(50))
    }

    /// Set volume (0-100)
    pub fn set_volume(level: u8) -> Result<()> {
        let script = format!("set volume output volume {}", level.min(100));
        AppleScript::execute(&script)?;
        Ok(())
    }

    /// Mute/unmute
    pub fn set_muted(muted: bool) -> Result<()> {
        let script = format!("set volume output muted {}", muted);
        AppleScript::execute(&script)?;
        Ok(())
    }

    // === Clipboard ===

    /// Get clipboard content
    pub fn get_clipboard() -> Result<String> {
        let script = "the clipboard";
        AppleScript::execute(script)
    }

    /// Set clipboard content
    pub fn set_clipboard(content: &str) -> Result<()> {
        let escaped = content.replace('"', r#"\""#);
        let script = format!(r#"set the clipboard to "{}""#, escaped);
        AppleScript::execute(&script)?;
        Ok(())
    }

    // === Display ===

    /// Set display brightness (0.0-1.0)
    pub fn set_brightness(level: f32) -> Result<()> {
        let level = level.clamp(0.0, 1.0);
        let script = format!(r#"
            tell application "System Events"
                tell process "SystemUIServer"
                    -- This requires additional setup
                end tell
            end tell
        "#);
        // Note: Brightness control requires additional privileges
        AppleScript::execute(&script)?;
        Ok(())
    }

    /// Toggle dark mode
    pub fn toggle_dark_mode() -> Result<()> {
        let script = r#"
            tell application "System Events"
                tell appearance preferences
                    set dark mode to not dark mode
                end tell
            end tell
        "#;
        AppleScript::execute(script)?;
        Ok(())
    }

    // === Finder ===

    /// Open folder in Finder
    pub fn open_folder(path: &str) -> Result<()> {
        let script = format!(r#"
            tell application "Finder"
                activate
                open POSIX file "{}"
            end tell
        "#, path);
        AppleScript::execute(&script)?;
        Ok(())
    }

    /// Get selected files in Finder
    pub fn get_finder_selection() -> Result<Vec<String>> {
        let script = r#"
            tell application "Finder"
                set selectedItems to selection
                set paths to ""
                repeat with item_ in selectedItems
                    set paths to paths & (POSIX path of (item_ as alias)) & ","
                end repeat
                return paths
            end tell
        "#;

        let result = AppleScript::execute(script)?;
        Ok(result.split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect())
    }

    // === Notifications ===

    /// Show system notification
    pub fn notify(title: &str, message: &str, sound: Option<&str>) -> Result<()> {
        let sound_part = sound
            .map(|s| format!(r#" sound name "{}""#, s))
            .unwrap_or_default();

        let script = format!(r#"
            display notification "{}" with title "{}"{}"#,
            message.replace('"', r#"\""#),
            title.replace('"', r#"\""#),
            sound_part
        );

        AppleScript::execute(&script)?;
        Ok(())
    }

    // === Keyboard Simulation ===

    /// Simulate key press
    pub fn key_press(key: &str, modifiers: &[&str]) -> Result<()> {
        let modifier_string = if modifiers.is_empty() {
            String::new()
        } else {
            format!(" using {{{}}}", modifiers.join(", "))
        };

        let script = format!(r#"
            tell application "System Events"
                keystroke "{}"{}"
            end tell
        "#, key, modifier_string);

        AppleScript::execute(&script)?;
        Ok(())
    }

    /// Simulate key code press
    pub fn key_code(code: u8, modifiers: &[&str]) -> Result<()> {
        let modifier_string = if modifiers.is_empty() {
            String::new()
        } else {
            format!(" using {{{}}}", modifiers.join(" down, ")) + " down"
        };

        let script = format!(r#"
            tell application "System Events"
                key code {}{}"
            end tell
        "#, code, modifier_string);

        AppleScript::execute(&script)?;
        Ok(())
    }
}
```

---

## Permissions Management

### Permission Checker

```rust
use std::process::Command;

#[derive(Debug, Clone, PartialEq)]
pub enum PermissionStatus {
    Granted,
    Denied,
    Unknown,
    NotDetermined,
}

pub struct PermissionManager;

impl PermissionManager {
    /// Check Accessibility permission (required for system control)
    pub fn check_accessibility() -> PermissionStatus {
        // Try to query accessibility
        let script = r#"
            tell application "System Events"
                return name of first process
            end tell
        "#;

        match AppleScript::execute(script) {
            Ok(_) => PermissionStatus::Granted,
            Err(_) => PermissionStatus::Denied,
        }
    }

    /// Check Automation permission for specific app
    pub fn check_automation(app_name: &str) -> PermissionStatus {
        let script = format!(r#"
            tell application "{}"
                return name
            end tell
        "#, app_name);

        match AppleScript::execute(&script) {
            Ok(_) => PermissionStatus::Granted,
            Err(e) => {
                if e.to_string().contains("not allowed") {
                    PermissionStatus::Denied
                } else {
                    PermissionStatus::Unknown
                }
            }
        }
    }

    /// Check Full Disk Access
    pub fn check_full_disk_access() -> PermissionStatus {
        // Try to read a protected location
        let test_path = std::path::Path::new("/Library/Application Support/com.apple.TCC/TCC.db");

        if test_path.exists() && std::fs::metadata(test_path).is_ok() {
            PermissionStatus::Granted
        } else {
            PermissionStatus::Denied
        }
    }

    /// Request Accessibility permission (opens System Preferences)
    pub fn request_accessibility() -> Result<()> {
        let script = r#"
            tell application "System Preferences"
                activate
                set current pane to pane "com.apple.preference.security"
                reveal anchor "Privacy_Accessibility" of current pane
            end tell
        "#;

        AppleScript::execute(script)?;
        Ok(())
    }

    /// Request Automation permission for app
    pub fn request_automation(app_name: &str) -> Result<()> {
        // Simply trying to control the app triggers the permission dialog
        let script = format!(r#"
            tell application "{}"
                -- This will trigger permission prompt
            end tell
        "#, app_name);

        AppleScript::execute(&script).ok();
        Ok(())
    }

    /// Open Security & Privacy preferences
    pub fn open_security_preferences() -> Result<()> {
        let script = r#"
            tell application "System Preferences"
                activate
                set current pane to pane "com.apple.preference.security"
            end tell
        "#;

        AppleScript::execute(script)?;
        Ok(())
    }

    /// Get all permission statuses
    pub fn get_all_permissions() -> PermissionReport {
        PermissionReport {
            accessibility: Self::check_accessibility(),
            automation_safari: Self::check_automation("Safari"),
            automation_chrome: Self::check_automation("Google Chrome"),
            automation_finder: Self::check_automation("Finder"),
            automation_terminal: Self::check_automation("Terminal"),
            full_disk_access: Self::check_full_disk_access(),
        }
    }
}

#[derive(Debug)]
pub struct PermissionReport {
    pub accessibility: PermissionStatus,
    pub automation_safari: PermissionStatus,
    pub automation_chrome: PermissionStatus,
    pub automation_finder: PermissionStatus,
    pub automation_terminal: PermissionStatus,
    pub full_disk_access: PermissionStatus,
}

impl PermissionReport {
    pub fn has_basic_control(&self) -> bool {
        self.accessibility == PermissionStatus::Granted
    }

    pub fn has_browser_control(&self) -> bool {
        self.accessibility == PermissionStatus::Granted
            && (self.automation_safari == PermissionStatus::Granted
                || self.automation_chrome == PermissionStatus::Granted)
    }

    pub fn has_full_control(&self) -> bool {
        self.accessibility == PermissionStatus::Granted
            && self.automation_finder == PermissionStatus::Granted
            && self.full_disk_access == PermissionStatus::Granted
    }
}
```

### Permission Request Flow

```rust
pub struct PermissionFlow {
    event_tx: mpsc::UnboundedSender<Message>,
}

impl PermissionFlow {
    /// Run permission setup flow
    pub async fn run_setup(&self) -> Result<PermissionReport> {
        // Check current permissions
        let mut report = PermissionManager::get_all_permissions();

        // Request Accessibility if needed
        if report.accessibility != PermissionStatus::Granted {
            self.event_tx.send(Message::ShowPermissionDialog(
                "Accessibility Access Required".to_string(),
                "Command Deck needs Accessibility access to control windows and apps.".to_string(),
            ))?;

            PermissionManager::request_accessibility()?;

            // Wait for user to grant permission
            loop {
                tokio::time::sleep(Duration::from_secs(1)).await;
                report.accessibility = PermissionManager::check_accessibility();

                if report.accessibility == PermissionStatus::Granted {
                    break;
                }
            }
        }

        // Request Automation permissions
        let apps_to_request = vec!["Safari", "Finder", "Terminal"];

        for app in apps_to_request {
            let status = PermissionManager::check_automation(app);
            if status != PermissionStatus::Granted {
                PermissionManager::request_automation(app)?;
                // Small delay between requests
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        }

        // Final check
        Ok(PermissionManager::get_all_permissions())
    }
}
```

---

## Styling & Theming

### Theme System

```rust
use ratatui::style::{Color, Modifier, Style};

#[derive(Clone)]
pub struct Theme {
    // Base colors
    pub background: Color,
    pub foreground: Color,
    pub muted: Color,

    // Accent colors
    pub primary: Color,
    pub secondary: Color,
    pub accent: Color,

    // Semantic colors
    pub success: Color,
    pub warning: Color,
    pub error: Color,
    pub info: Color,

    // Border styles
    pub border_focused: Color,
    pub border_unfocused: Color,

    // Chat colors
    pub user_message: Color,
    pub assistant_message: Color,
    pub system_message: Color,
}

impl Theme {
    /// Cyberpunk / Iron Man theme
    pub fn jarvis() -> Self {
        Self {
            background: Color::Rgb(10, 10, 20),
            foreground: Color::Rgb(200, 200, 220),
            muted: Color::Rgb(100, 100, 120),

            primary: Color::Rgb(0, 200, 255),      // Cyan
            secondary: Color::Rgb(255, 100, 50),   // Orange
            accent: Color::Rgb(255, 200, 0),       // Gold

            success: Color::Rgb(0, 255, 100),
            warning: Color::Rgb(255, 200, 0),
            error: Color::Rgb(255, 50, 50),
            info: Color::Rgb(100, 150, 255),

            border_focused: Color::Rgb(0, 200, 255),
            border_unfocused: Color::Rgb(50, 50, 80),

            user_message: Color::Rgb(100, 200, 255),
            assistant_message: Color::Rgb(100, 255, 150),
            system_message: Color::Rgb(255, 200, 100),
        }
    }

    /// Dark minimal theme
    pub fn minimal() -> Self {
        Self {
            background: Color::Rgb(20, 20, 25),
            foreground: Color::Rgb(220, 220, 230),
            muted: Color::Rgb(100, 100, 110),

            primary: Color::Rgb(150, 150, 255),
            secondary: Color::Rgb(150, 200, 150),
            accent: Color::Rgb(255, 150, 150),

            success: Color::Rgb(100, 200, 100),
            warning: Color::Rgb(200, 200, 100),
            error: Color::Rgb(200, 100, 100),
            info: Color::Rgb(100, 150, 200),

            border_focused: Color::Rgb(150, 150, 255),
            border_unfocused: Color::Rgb(60, 60, 70),

            user_message: Color::Rgb(150, 180, 220),
            assistant_message: Color::Rgb(150, 220, 180),
            system_message: Color::Rgb(220, 200, 150),
        }
    }

    // Style helpers
    pub fn title_style(&self) -> Style {
        Style::default()
            .fg(self.primary)
            .add_modifier(Modifier::BOLD)
    }

    pub fn border_style(&self, focused: bool) -> Style {
        Style::default().fg(if focused {
            self.border_focused
        } else {
            self.border_unfocused
        })
    }

    pub fn status_style(&self, status: &Status) -> Style {
        let color = match status {
            Status::Online => self.success,
            Status::Warning => self.warning,
            Status::Error => self.error,
            Status::Offline => self.muted,
        };
        Style::default().fg(color)
    }
}

pub enum Status {
    Online,
    Warning,
    Error,
    Offline,
}
```

### Styled Components

```rust
use ratatui::{
    widgets::{Block, Borders, BorderType, Paragraph},
    text::{Line, Span},
};

impl App {
    fn styled_panel(&self, title: &str, focused: bool) -> Block {
        Block::default()
            .title(format!(" {} ", title))
            .title_style(self.theme.title_style())
            .borders(Borders::ALL)
            .border_type(if focused {
                BorderType::Double
            } else {
                BorderType::Rounded
            })
            .border_style(self.theme.border_style(focused))
    }

    fn styled_status_line(&self, items: Vec<(&str, Status)>) -> Line {
        let spans: Vec<Span> = items
            .iter()
            .flat_map(|(text, status)| {
                vec![
                    Span::styled(
                        match status {
                            Status::Online => "● ",
                            Status::Warning => "◐ ",
                            Status::Error => "○ ",
                            Status::Offline => "○ ",
                        },
                        self.theme.status_style(status),
                    ),
                    Span::styled(
                        format!("{} ", text),
                        Style::default().fg(self.theme.foreground),
                    ),
                ]
            })
            .collect();

        Line::from(spans)
    }
}
```

---

## Project Structure

```
command_deck/
├── Cargo.toml
├── src/
│   ├── main.rs                 # Entry point
│   ├── app.rs                  # Main App struct and TEA loop
│   ├── lib.rs                  # Library exports
│   │
│   ├── core/
│   │   ├── mod.rs
│   │   ├── message.rs          # Message enum
│   │   ├── event.rs            # Event handler
│   │   └── config.rs           # App configuration
│   │
│   ├── ui/
│   │   ├── mod.rs
│   │   ├── theme.rs            # Theme definitions
│   │   ├── layout.rs           # Layout utilities
│   │   │
│   │   ├── screens/
│   │   │   ├── mod.rs
│   │   │   ├── dashboard.rs    # Main dashboard
│   │   │   ├── chat.rs         # Chat interface
│   │   │   ├── search.rs       # File search
│   │   │   ├── browser.rs      # Browser control panel
│   │   │   └── settings.rs     # Settings screen
│   │   │
│   │   └── components/
│   │       ├── mod.rs
│   │       ├── panel.rs        # Reusable panel
│   │       ├── status_bar.rs   # Status indicators
│   │       ├── input.rs        # Text input
│   │       ├── message_list.rs # Chat messages
│   │       └── metrics.rs      # Metrics display
│   │
│   ├── state/
│   │   ├── mod.rs
│   │   ├── model.rs            # Main model struct
│   │   ├── chat.rs             # Chat state
│   │   ├── search.rs           # Search state
│   │   └── browser.rs          # Browser state
│   │
│   ├── backend/
│   │   ├── mod.rs
│   │   ├── websocket.rs        # WebSocket client
│   │   ├── api.rs              # REST API client
│   │   └── streaming.rs        # Stream handling
│   │
│   ├── indexer/
│   │   ├── mod.rs
│   │   ├── tantivy.rs          # Tantivy wrapper
│   │   ├── watcher.rs          # File watcher
│   │   └── query.rs            # Search queries
│   │
│   └── system/
│       ├── mod.rs
│       ├── applescript.rs      # AppleScript executor
│       ├── browser.rs          # Browser controller
│       ├── animator.rs         # Window animations
│       ├── controller.rs       # System controller
│       └── permissions.rs      # Permission manager
│
├── assets/
│   └── themes/
│       ├── jarvis.toml
│       └── minimal.toml
│
└── tests/
    ├── integration/
    │   ├── chat_test.rs
    │   └── browser_test.rs
    └── unit/
        ├── animation_test.rs
        └── indexer_test.rs
```

---

## Implementation Roadmap

### Phase 1: Foundation
- [ ] Project setup with Cargo.toml
- [ ] Basic TUI with Ratatui + Crossterm
- [ ] Event loop with Tokio
- [ ] Screen navigation system
- [ ] Theme system

### Phase 2: Core UI
- [ ] Dashboard layout
- [ ] Panel components
- [ ] Status indicators
- [ ] Metrics display
- [ ] Input handling

### Phase 3: Chat System
- [ ] Chat state management
- [ ] Message rendering
- [ ] Streaming support
- [ ] Multi-thread chat
- [ ] Cursor animation

### Phase 4: Backend Integration
- [ ] WebSocket connection
- [ ] Stream handling
- [ ] Real-time updates
- [ ] Connection state management

### Phase 5: File Indexing
- [ ] Tantivy index setup
- [ ] Directory scanning
- [ ] Fuzzy search
- [ ] File watching
- [ ] Search UI

### Phase 6: System Integration
- [ ] AppleScript executor
- [ ] Permission checker
- [ ] Terminal control
- [ ] Notification system

### Phase 7: Browser Control
- [ ] Safari/Chrome control
- [ ] URL navigation
- [ ] Window positioning
- [ ] JavaScript execution

### Phase 8: Animations
- [ ] Easing functions
- [ ] Window animator
- [ ] Choreographed scenes
- [ ] Transition effects

### Phase 9: AI Integration
- [ ] Action parsing
- [ ] Web search triggers
- [ ] Documentation lookup
- [ ] Context-aware commands

### Phase 10: Polish
- [ ] Error handling
- [ ] Performance optimization
- [ ] User preferences
- [ ] Documentation

---

## References

### Documentation
- [Ratatui](https://ratatui.rs/) - Main TUI framework
- [Crossterm](https://docs.rs/crossterm/) - Terminal handling
- [Tokio](https://tokio.rs/) - Async runtime
- [Tantivy](https://docs.rs/tantivy/) - Search engine

### Example Projects
- [Claude Code](https://github.com/anthropics/claude-code) - Inspiration
- [Lazygit](https://github.com/jesseduffield/lazygit) - Git TUI
- [Bottom](https://github.com/ClementTsang/bottom) - System monitor

### AppleScript References
- [Mac Automation Scripting Guide](https://developer.apple.com/library/archive/documentation/AppleScript/Conceptual/AppleScriptX/)
- [JXA Cookbook](https://github.com/JXA-Cookbook/JXA-Cookbook)

---

*Document updated: 2025-01-13*
*Stack: Rust + Ratatui + Tokio + AppleScript*
*Project: Command Deck TUI*

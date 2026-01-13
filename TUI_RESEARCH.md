# TUI Command Deck Research Document
## Building Beautiful, Fast Terminal User Interfaces with Multiple Screens

---

## Table of Contents
1. [Executive Summary](#executive-summary)
2. [TUI Framework Comparison](#tui-framework-comparison)
3. [Architecture Patterns](#architecture-patterns)
4. [Multi-Screen Navigation](#multi-screen-navigation)
5. [Layout Systems](#layout-systems)
6. [Streaming & Real-time Updates](#streaming--real-time-updates)
7. [Chat Interface Implementation](#chat-interface-implementation)
8. [Dynamic Backend Updates](#dynamic-backend-updates)
9. [Styling & Theming](#styling--theming)
10. [Performance Considerations](#performance-considerations)
11. [Recommended Stack](#recommended-stack)
12. [Implementation Blueprint](#implementation-blueprint)

---

## Executive Summary

Building a command deck TUI (like Iron Man's JARVIS interface) requires:
- **Multi-panel layouts** with dynamic resizing
- **Real-time streaming** for chat functionality
- **Async event handling** for backend updates
- **Beautiful styling** with colors, borders, and animations
- **Responsive navigation** between screens/modes

The top frameworks for this are:
1. **Ratatui (Rust)** - Best performance, excellent layout system
2. **Bubbletea (Go)** - Great Elm architecture, easy streaming
3. **Textual (Python)** - Most feature-rich, CSS styling, rapid development
4. **Ink (TypeScript)** - React-based, used by Claude Code

---

## TUI Framework Comparison

### 1. Ratatui (Rust)
**Best for:** Maximum performance, complex layouts, production apps

```rust
use ratatui::{
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

fn ui(frame: &mut Frame) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Percentage(10),  // Header
            Constraint::Percentage(80),  // Main content
            Constraint::Percentage(10),  // Footer
        ])
        .split(frame.area());

    let block = Block::default()
        .title("Command Deck")
        .borders(Borders::ALL);
    frame.render_widget(block, chunks[0]);
}
```

**Pros:**
- Blazing fast performance
- Excellent constraint-based layout system
- Rich widget library
- Great async support with Tokio
- Large ecosystem (ratatui-widgets)

**Cons:**
- Steeper learning curve
- More boilerplate code

### 2. Bubbletea (Go)
**Best for:** Chat applications, streaming, simple state management

```go
package main

import (
    "github.com/charmbracelet/bubbles/textarea"
    "github.com/charmbracelet/bubbles/viewport"
    tea "github.com/charmbracelet/bubbletea"
)

type model struct {
    viewport viewport.Model
    messages []string
    textarea textarea.Model
}

func (m model) Init() tea.Cmd {
    return textarea.Blink
}

func (m model) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
    switch msg := msg.(type) {
    case tea.KeyMsg:
        switch msg.Type {
        case tea.KeyEnter:
            m.messages = append(m.messages, m.textarea.Value())
            m.textarea.Reset()
        }
    }
    return m, nil
}

func (m model) View() string {
    return fmt.Sprintf("%s\n%s", m.viewport.View(), m.textarea.View())
}
```

**Pros:**
- Elm architecture (predictable state)
- Excellent for chat/streaming
- Beautiful components (Bubbles library)
- Lipgloss for styling
- External message injection via `Program.Send()`

**Cons:**
- Go's paradigms can conflict with Elm model
- Less flexible layouts than Ratatui

### 3. Textual (Python)
**Best for:** Rapid development, complex UIs, multiple screens

```python
from textual.app import App, ComposeResult
from textual.screen import Screen
from textual.widgets import Header, Footer, Static, Input
from textual.containers import Container, Horizontal, Vertical

class DashboardScreen(Screen):
    def compose(self) -> ComposeResult:
        yield Header()
        with Horizontal():
            yield Static("Panel 1", classes="panel")
            yield Static("Panel 2", classes="panel")
            yield Static("Panel 3", classes="panel")
        yield Footer()

class ChatScreen(Screen):
    def compose(self) -> ComposeResult:
        yield Header()
        yield Static(id="messages")
        yield Input(placeholder="Type message...")
        yield Footer()

class CommandDeck(App):
    BINDINGS = [
        ("d", "switch_mode('dashboard')", "Dashboard"),
        ("c", "switch_mode('chat')", "Chat"),
    ]

    MODES = {
        "dashboard": DashboardScreen,
        "chat": ChatScreen,
    }

    def on_mount(self) -> None:
        self.switch_mode("dashboard")
```

**Pros:**
- CSS-like styling (TCSS)
- Built-in screen/mode management
- Rich widget library
- Reactive updates
- Async support
- Fastest development time

**Cons:**
- Python performance limitations
- Heavier resource usage

### 4. Ink (TypeScript/React)
**Best for:** React developers, Claude Code-style interfaces

```typescript
import React, { useState } from 'react';
import { render, Box, Text, useInput } from 'ink';
import TextInput from 'ink-text-input';

const App = () => {
    const [messages, setMessages] = useState<string[]>([]);
    const [input, setInput] = useState('');

    useInput((input, key) => {
        if (key.return) {
            setMessages([...messages, input]);
            setInput('');
        }
    });

    return (
        <Box flexDirection="column">
            <Box borderStyle="round" padding={1}>
                <Text>Command Deck</Text>
            </Box>
            <Box flexDirection="column">
                {messages.map((msg, i) => (
                    <Text key={i}>{msg}</Text>
                ))}
            </Box>
            <TextInput value={input} onChange={setInput} />
        </Box>
    );
};

render(<App />);
```

**Pros:**
- Familiar React paradigm
- Used by Claude Code, Gemini CLI, Quen Code
- Good for TypeScript developers
- Component composition

**Cons:**
- Performance overhead from React runtime
- Less efficient than native solutions

---

## Architecture Patterns

### The Elm Architecture (TEA)
Used by Bubbletea and recommended for TUI apps:

```
┌─────────────────────────────────────────┐
│                 Model                    │
│  (Application State)                     │
└─────────────────────────────────────────┘
           │                    ▲
           │ render             │ update
           ▼                    │
┌──────────────────┐  ┌────────────────────┐
│      View        │  │      Update        │
│  (UI Rendering)  │  │  (State Changes)   │
└──────────────────┘  └────────────────────┘
           │                    ▲
           │                    │ messages
           ▼                    │
┌─────────────────────────────────────────┐
│              Events/Commands             │
│  (User Input, Backend Messages, Timers)  │
└─────────────────────────────────────────┘
```

### Component-Based Architecture
For larger applications:

```
App
├── Router/Navigator
│   ├── DashboardScreen
│   │   ├── HeaderPanel
│   │   ├── StatusPanel
│   │   ├── MetricsPanel
│   │   └── LogPanel
│   ├── ChatScreen
│   │   ├── MessageList
│   │   ├── InputArea
│   │   └── StatusBar
│   └── SettingsScreen
├── StateManager
│   ├── AppState
│   ├── ChatState
│   └── ConfigState
└── EventBus
    ├── UserEvents
    ├── BackendEvents
    └── SystemEvents
```

---

## Multi-Screen Navigation

### Screen-Based Navigation (Textual Style)

```python
from textual.app import App
from textual.screen import Screen

class App(App):
    MODES = {
        "dashboard": DashboardScreen,
        "chat": ChatScreen,
        "settings": SettingsScreen,
    }

    BINDINGS = [
        ("1", "switch_mode('dashboard')", "Dashboard"),
        ("2", "switch_mode('chat')", "Chat"),
        ("3", "switch_mode('settings')", "Settings"),
        ("q", "quit", "Quit"),
    ]
```

### Stack-Based Navigation (Ratatui Style)

```rust
enum Screen {
    Dashboard,
    Chat,
    Settings,
}

struct App {
    screen_stack: Vec<Screen>,
    current_screen: Screen,
}

impl App {
    fn push_screen(&mut self, screen: Screen) {
        self.screen_stack.push(self.current_screen.clone());
        self.current_screen = screen;
    }

    fn pop_screen(&mut self) {
        if let Some(screen) = self.screen_stack.pop() {
            self.current_screen = screen;
        }
    }
}
```

### Navigator Pattern (ratapp Framework)

```rust
use ratapp::{App, Navigator, Screen, Screens};

#[derive(Screens)]
enum AppScreens {
    Dashboard(DashboardScreen),
    Chat(ChatScreen),
    Settings(SettingsScreen),
}

impl Screen<AppScreens> for DashboardScreen {
    async fn on_event(&mut self, event: Event, navigator: Navigator<AppScreens>) {
        if let Event::Key(key) = event {
            match key.code {
                KeyCode::Char('c') => navigator.push(ScreenID::Chat),
                KeyCode::Char('s') => navigator.push(ScreenID::Settings),
                KeyCode::Esc => navigator.back(),
                _ => {}
            }
        }
    }
}
```

---

## Layout Systems

### Constraint-Based Layouts (Ratatui)

```rust
// Horizontal split with fixed and flexible areas
let layout = Layout::default()
    .direction(Direction::Horizontal)
    .constraints([
        Constraint::Length(20),      // Fixed 20 chars
        Constraint::Percentage(50),  // 50% of remaining
        Constraint::Min(10),         // At least 10 chars
        Constraint::Fill(1),         // Fill remaining space
    ])
    .split(area);

// Nested layouts
let main_chunks = Layout::default()
    .direction(Direction::Vertical)
    .constraints([
        Constraint::Length(3),   // Header
        Constraint::Fill(1),     // Content
        Constraint::Length(3),   // Footer
    ])
    .split(frame.area());

let content_chunks = Layout::default()
    .direction(Direction::Horizontal)
    .constraints([
        Constraint::Percentage(30),  // Sidebar
        Constraint::Percentage(70),  // Main
    ])
    .split(main_chunks[1]);
```

### Iron Man Command Deck Layout Example

```
┌─────────────────────────────────────────────────────────────────┐
│                        COMMAND DECK                              │
├─────────────────────────────────────────────────────────────────┤
│ ┌─────────────┐ ┌─────────────────────────┐ ┌─────────────────┐ │
│ │  SYSTEM     │ │                         │ │   METRICS       │ │
│ │  STATUS     │ │      MAIN DISPLAY       │ │   CPU: 45%      │ │
│ │             │ │                         │ │   MEM: 2.1GB    │ │
│ │  ● Online   │ │   [Chat/Dashboard]      │ │   NET: 125MB/s  │ │
│ │  ○ API OK   │ │                         │ │                 │ │
│ │  ○ DB OK    │ │                         │ │   TASKS: 12     │ │
│ └─────────────┘ │                         │ └─────────────────┘ │
│ ┌─────────────┐ │                         │ ┌─────────────────┐ │
│ │  QUICK      │ │                         │ │   ALERTS        │ │
│ │  ACTIONS    │ │                         │ │                 │ │
│ │             │ │                         │ │   ⚠ Warning 1   │ │
│ │  [1] Chat   │ │                         │ │   ⚠ Warning 2   │ │
│ │  [2] Stats  │ │                         │ │                 │ │
│ │  [3] Logs   │ └─────────────────────────┘ │                 │ │
│ └─────────────┘                             └─────────────────┘ │
├─────────────────────────────────────────────────────────────────┤
│ > Enter command...                                    [F1 Help] │
└─────────────────────────────────────────────────────────────────┘
```

Implementation:

```rust
fn draw_command_deck(frame: &mut Frame) {
    // Main vertical layout
    let main_layout = Layout::vertical([
        Constraint::Length(1),   // Title bar
        Constraint::Fill(1),     // Content area
        Constraint::Length(3),   // Input area
    ]).split(frame.area());

    // Content: 3-column layout
    let content_layout = Layout::horizontal([
        Constraint::Length(15),   // Left sidebar
        Constraint::Fill(1),      // Main display
        Constraint::Length(20),   // Right sidebar
    ]).split(main_layout[1]);

    // Left sidebar: stacked panels
    let left_panels = Layout::vertical([
        Constraint::Percentage(50),
        Constraint::Percentage(50),
    ]).split(content_layout[0]);

    // Right sidebar: stacked panels
    let right_panels = Layout::vertical([
        Constraint::Percentage(50),
        Constraint::Percentage(50),
    ]).split(content_layout[2]);

    // Render widgets...
}
```

---

## Streaming & Real-time Updates

### Async Event Loop (Ratatui + Tokio)

```rust
use tokio::sync::mpsc;
use std::time::Duration;

pub enum Event {
    Tick,
    Key(KeyEvent),
    BackendMessage(String),
    Resize(u16, u16),
}

pub struct EventHandler {
    rx: mpsc::UnboundedReceiver<Event>,
    tx: mpsc::UnboundedSender<Event>,
}

impl EventHandler {
    pub fn new(tick_rate: Duration) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let event_tx = tx.clone();

        // Tick events
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tick_rate);
            loop {
                interval.tick().await;
                if event_tx.send(Event::Tick).is_err() {
                    break;
                }
            }
        });

        // Key events
        let key_tx = tx.clone();
        tokio::spawn(async move {
            loop {
                if crossterm::event::poll(Duration::from_millis(10)).unwrap() {
                    if let crossterm::event::Event::Key(key) = crossterm::event::read().unwrap() {
                        key_tx.send(Event::Key(key)).ok();
                    }
                }
            }
        });

        Self { rx, tx }
    }

    pub async fn next(&mut self) -> Option<Event> {
        self.rx.recv().await
    }

    // Inject events from external sources (backend)
    pub fn send(&self, event: Event) {
        self.tx.send(event).ok();
    }
}
```

### Streaming Text Updates (Chat-style)

```rust
struct ChatState {
    messages: Vec<Message>,
    current_stream: Option<StreamingMessage>,
}

struct StreamingMessage {
    content: String,
    is_complete: bool,
}

impl ChatState {
    fn append_stream_chunk(&mut self, chunk: &str) {
        if let Some(ref mut stream) = self.current_stream {
            stream.content.push_str(chunk);
        }
    }

    fn complete_stream(&mut self) {
        if let Some(stream) = self.current_stream.take() {
            self.messages.push(Message {
                content: stream.content,
                role: Role::Assistant,
            });
        }
    }
}
```

### Backend WebSocket Integration

```rust
use tokio_tungstenite::{connect_async, tungstenite::Message};
use futures_util::{StreamExt, SinkExt};

async fn connect_backend(event_tx: mpsc::UnboundedSender<Event>) {
    let (ws_stream, _) = connect_async("ws://localhost:8080/ws").await.unwrap();
    let (mut write, mut read) = ws_stream.split();

    // Read messages from backend
    tokio::spawn(async move {
        while let Some(msg) = read.next().await {
            if let Ok(Message::Text(text)) = msg {
                event_tx.send(Event::BackendMessage(text)).ok();
            }
        }
    });
}
```

---

## Chat Interface Implementation

### Claude Code-style Chat (Bubbletea)

```go
package main

import (
    "strings"

    "github.com/charmbracelet/bubbles/textarea"
    "github.com/charmbracelet/bubbles/viewport"
    tea "github.com/charmbracelet/bubbletea"
    "github.com/charmbracelet/lipgloss"
)

type model struct {
    viewport    viewport.Model
    messages    []string
    textarea    textarea.Model
    streaming   bool
    streamBuf   string
}

type StreamChunk string
type StreamComplete struct{}

func (m model) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
    switch msg := msg.(type) {
    case tea.KeyMsg:
        if msg.Type == tea.KeyEnter && !m.textarea.Focused() {
            return m, nil
        }
        if msg.Type == tea.KeyEnter {
            userMsg := m.textarea.Value()
            m.messages = append(m.messages, "You: " + userMsg)
            m.textarea.Reset()
            m.streaming = true
            m.streamBuf = "Assistant: "
            return m, sendToBackend(userMsg)
        }

    case StreamChunk:
        m.streamBuf += string(msg)
        m.viewport.SetContent(m.renderMessages())
        return m, nil

    case StreamComplete:
        m.messages = append(m.messages, m.streamBuf)
        m.streamBuf = ""
        m.streaming = false
        m.viewport.SetContent(m.renderMessages())
        return m, nil
    }

    var cmds []tea.Cmd
    m.textarea, _ = m.textarea.Update(msg)
    m.viewport, _ = m.viewport.Update(msg)
    return m, tea.Batch(cmds...)
}

func (m model) renderMessages() string {
    content := strings.Join(m.messages, "\n\n")
    if m.streaming {
        content += "\n\n" + m.streamBuf + "▋"  // Cursor
    }
    return content
}

func (m model) View() string {
    return lipgloss.JoinVertical(
        lipgloss.Left,
        m.viewport.View(),
        m.textarea.View(),
    )
}
```

### Streaming Message Handler (External Integration)

```go
// Inject messages from external source
func main() {
    p := tea.NewProgram(initialModel())

    // Goroutine to handle backend streaming
    go func() {
        for chunk := range backendStream {
            p.Send(StreamChunk(chunk))
        }
        p.Send(StreamComplete{})
    }()

    p.Run()
}
```

---

## Dynamic Backend Updates

### Pattern 1: Channel-Based Updates

```rust
use tokio::sync::broadcast;

struct App {
    state: AppState,
    update_rx: broadcast::Receiver<BackendUpdate>,
}

enum BackendUpdate {
    MetricsUpdate(Metrics),
    AlertNew(Alert),
    StatusChange(Status),
    ChatMessage(ChatMessage),
}

async fn run_app(mut app: App) {
    loop {
        tokio::select! {
            // Handle terminal events
            Some(event) = terminal_events.next() => {
                app.handle_terminal_event(event);
            }
            // Handle backend updates
            Ok(update) = app.update_rx.recv() => {
                match update {
                    BackendUpdate::MetricsUpdate(m) => app.state.metrics = m,
                    BackendUpdate::AlertNew(a) => app.state.alerts.push(a),
                    BackendUpdate::StatusChange(s) => app.state.status = s,
                    BackendUpdate::ChatMessage(msg) => app.state.chat.push(msg),
                }
            }
        }
        app.render()?;
    }
}
```

### Pattern 2: Reactive State (Textual)

```python
from textual.app import App
from textual.reactive import reactive
from textual.widgets import Static

class MetricsPanel(Static):
    cpu_usage = reactive(0.0)
    memory_usage = reactive(0.0)

    def watch_cpu_usage(self, value: float) -> None:
        self.update_display()

    def watch_memory_usage(self, value: float) -> None:
        self.update_display()

    def update_display(self) -> None:
        self.update(f"CPU: {self.cpu_usage:.1f}%\nMEM: {self.memory_usage:.1f}%")

class CommandDeckApp(App):
    def on_mount(self) -> None:
        # Start background worker for backend updates
        self.run_worker(self.poll_backend())

    async def poll_backend(self) -> None:
        async for update in backend_stream():
            metrics_panel = self.query_one(MetricsPanel)
            metrics_panel.cpu_usage = update.cpu
            metrics_panel.memory_usage = update.memory
```

---

## Styling & Theming

### Lipgloss Styling (Go/Bubbletea)

```go
import "github.com/charmbracelet/lipgloss"

var (
    // Colors
    primaryColor   = lipgloss.Color("#7D56F4")
    secondaryColor = lipgloss.Color("#3C3C3C")
    accentColor    = lipgloss.Color("#04B575")
    warningColor   = lipgloss.Color("#FFCC00")
    errorColor     = lipgloss.Color("#FF5555")

    // Styles
    titleStyle = lipgloss.NewStyle().
        Bold(true).
        Foreground(primaryColor).
        Background(secondaryColor).
        Padding(0, 1).
        MarginBottom(1)

    panelStyle = lipgloss.NewStyle().
        Border(lipgloss.RoundedBorder()).
        BorderForeground(primaryColor).
        Padding(1)

    statusOnline = lipgloss.NewStyle().
        Foreground(accentColor).
        SetString("● Online")

    statusOffline = lipgloss.NewStyle().
        Foreground(errorColor).
        SetString("○ Offline")
)
```

### TCSS Styling (Textual/Python)

```css
/* command_deck.tcss */

Screen {
    background: $surface;
}

#header {
    dock: top;
    height: 3;
    background: $primary;
    color: $text;
    text-align: center;
    text-style: bold;
}

.panel {
    border: solid $primary;
    padding: 1;
    margin: 1;
}

.panel:focus {
    border: double $accent;
}

#status-online {
    color: $success;
}

#status-offline {
    color: $error;
}

#chat-input {
    dock: bottom;
    height: 3;
    border: solid $secondary;
}

#chat-input:focus {
    border: solid $accent;
}

.message-user {
    background: $primary-darken-2;
    margin: 1 0;
    padding: 1;
}

.message-assistant {
    background: $secondary;
    margin: 1 0;
    padding: 1;
}
```

### Ratatui Styling

```rust
use ratatui::style::{Color, Modifier, Style, Stylize};

// Define theme
struct Theme {
    primary: Color,
    secondary: Color,
    accent: Color,
    success: Color,
    warning: Color,
    error: Color,
    text: Color,
    text_muted: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            primary: Color::Rgb(125, 86, 244),
            secondary: Color::Rgb(60, 60, 60),
            accent: Color::Rgb(4, 181, 117),
            success: Color::Rgb(4, 181, 117),
            warning: Color::Rgb(255, 204, 0),
            error: Color::Rgb(255, 85, 85),
            text: Color::White,
            text_muted: Color::Gray,
        }
    }
}

// Apply styles
fn styled_block(title: &str, theme: &Theme) -> Block {
    Block::default()
        .title(title)
        .title_style(Style::default().fg(theme.primary).bold())
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.primary))
        .border_type(BorderType::Rounded)
}
```

---

## Performance Considerations

### 1. Render Optimization

```rust
// Only redraw when state changes
struct App {
    state: AppState,
    last_rendered_state: Option<AppState>,
}

impl App {
    fn should_render(&self) -> bool {
        match &self.last_rendered_state {
            None => true,
            Some(last) => self.state != *last,
        }
    }

    fn render(&mut self, frame: &mut Frame) {
        if self.should_render() {
            // Render UI
            self.last_rendered_state = Some(self.state.clone());
        }
    }
}
```

### 2. Efficient List Rendering

```rust
// Use StatefulWidget for large lists
use ratatui::widgets::{List, ListState};

struct ChatView {
    messages: Vec<Message>,
    state: ListState,
    viewport_start: usize,
    viewport_size: usize,
}

impl ChatView {
    fn visible_messages(&self) -> &[Message] {
        let end = (self.viewport_start + self.viewport_size).min(self.messages.len());
        &self.messages[self.viewport_start..end]
    }
}
```

### 3. Async Non-Blocking Updates

```rust
// Use tokio::select! for non-blocking event handling
async fn main_loop(mut app: App, mut events: EventHandler) {
    let mut render_interval = tokio::time::interval(Duration::from_millis(16)); // ~60fps

    loop {
        tokio::select! {
            _ = render_interval.tick() => {
                if app.needs_render {
                    terminal.draw(|f| app.render(f))?;
                    app.needs_render = false;
                }
            }
            Some(event) = events.next() => {
                app.handle_event(event);
                app.needs_render = true;
            }
        }
    }
}
```

---

## Recommended Stack

### For Maximum Performance (Rust)
```
Framework: Ratatui
Async Runtime: Tokio
Terminal Backend: Crossterm
State Management: Custom Elm-style
Styling: Native Ratatui styles
```

### For Rapid Development (Python)
```
Framework: Textual
Async: asyncio (built-in)
Styling: TCSS
State Management: Reactive attributes
```

### For Chat-Heavy Apps (Go)
```
Framework: Bubbletea
Components: Bubbles
Styling: Lipgloss
State Management: Elm architecture
```

### For React Developers (TypeScript)
```
Framework: Ink
Components: ink-* packages
Styling: Flexbox-like
State Management: React hooks
```

---

## Implementation Blueprint

### Phase 1: Core Framework Setup
1. Choose framework based on requirements
2. Set up project structure
3. Implement basic terminal setup/teardown
4. Create event loop

### Phase 2: Layout System
1. Define screen hierarchy
2. Implement constraint-based layouts
3. Create reusable panel components
4. Add responsive resizing

### Phase 3: Navigation
1. Implement screen/mode system
2. Add keyboard navigation
3. Create transition animations (optional)
4. Add breadcrumb/status indicators

### Phase 4: Chat Interface
1. Create message list component
2. Implement text input with editing
3. Add streaming text support
4. Style messages by role

### Phase 5: Backend Integration
1. Set up WebSocket/HTTP connections
2. Implement event injection
3. Add real-time metric updates
4. Handle connection state

### Phase 6: Polish
1. Add theming support
2. Implement animations
3. Optimize performance
4. Add error handling

---

## Example Project Structure

```
command_deck/
├── src/
│   ├── main.rs
│   ├── app.rs              # Main application state
│   ├── event.rs            # Event handling
│   ├── ui/
│   │   ├── mod.rs
│   │   ├── screens/
│   │   │   ├── dashboard.rs
│   │   │   ├── chat.rs
│   │   │   └── settings.rs
│   │   ├── components/
│   │   │   ├── panel.rs
│   │   │   ├── status_bar.rs
│   │   │   ├── metrics.rs
│   │   │   └── message_list.rs
│   │   └── theme.rs
│   ├── backend/
│   │   ├── mod.rs
│   │   ├── api.rs
│   │   └── websocket.rs
│   └── state/
│       ├── mod.rs
│       ├── chat.rs
│       └── metrics.rs
├── Cargo.toml
└── README.md
```

---

## References

### Documentation
- [Ratatui Documentation](https://ratatui.rs/)
- [Bubbletea GitHub](https://github.com/charmbracelet/bubbletea)
- [Textual Documentation](https://textual.textualize.io/)
- [Ink GitHub](https://github.com/vadimdemedes/ink)

### Inspirations
- Claude Code terminal interface
- Iron Man JARVIS HUD
- htop/btop system monitors
- Lazygit TUI

### Community
- [Awesome TUIs](https://github.com/rothgar/awesome-tuis)
- [Ratatui Forum](https://forum.ratatui.rs/)
- [Charm Community](https://charm.sh/)

---

*Document generated: 2025-01-13*
*For: Command Deck TUI Project*

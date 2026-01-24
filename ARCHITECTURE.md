# Spoq TUI Architecture

This document describes the architecture of the Spoq TUI application following the v0.2.0 refactor. The refactor introduced trait-based abstractions, dependency injection, and clean module separation.

## High-Level Overview

```
+------------------+
|     main.rs      |  Entry point, CLI parsing
+--------+---------+
         |
+--------v---------+
|    Terminal      |  RAII terminal management
+--------+---------+
         |
+--------v---------+
|       App        |  Core application state
+--------+---------+
         |
    +----+----+
    |         |
+---v---+ +---v---+
|  UI   | | Input |  Rendering / Command handling
+-------+ +-------+
```

## Module Structure

### Public API Modules

These modules are part of the stable public API:

| Module | Purpose |
|--------|---------|
| `models` | Core data types: Thread, Message, ThreadType, Folder, Request |
| `app` | Application state (App), screens (Screen), focus (Focus), AppMessage |
| `cache` | Thread and message caching with reconciliation |
| `state` | SessionState, DashboardState, ToolState management |
| `ui` | User interface rendering components |
| `adapters` | Concrete trait implementations for DI |
| `sse` | Server-Sent Events parsing |
| `widgets` | Reusable UI widgets (TextAreaInput) |
| `prelude` | Convenient re-exports |

### Internal Modules

These modules support the binary but are not part of the stable API:

| Module | Purpose |
|--------|---------|
| `traits` | Trait abstractions for dependency injection |
| `auth` | Authentication and credential management |
| `cli` | CLI argument parsing and command handling |
| `conductor` | Backend communication client |
| `debug` | Development debugging system |
| `input` | Command pattern for keyboard handling |
| `terminal` | RAII terminal setup/cleanup |
| `websocket` | Real-time WebSocket communication |
| `startup` | Preflight checks and initialization |
| `update` | Application update checking |

## Architectural Patterns

### 1. Trait-Based Dependency Injection

The `traits` module defines abstract interfaces:

```rust
// src/traits/mod.rs
pub trait HttpClient: Clone + Send + Sync + 'static {
    async fn get(&self, url: &str, headers: &Headers) -> Result<Response, HttpError>;
    async fn post(&self, url: &str, body: &str, headers: &Headers) -> Result<Response, HttpError>;
    async fn post_stream(&self, url: &str, body: &str, headers: &Headers)
        -> Result<Pin<Box<dyn Stream<Item = Result<Bytes, HttpError>> + Send>>, HttpError>;
}

pub trait WebSocketConnection: Clone + Send + Sync + 'static {
    fn send(&self, message: WsMessage) -> Result<(), WsError>;
    fn subscribe(&self) -> broadcast::Receiver<WsMessage>;
    fn state(&self) -> ConnectionState;
}

pub trait CredentialsProvider: Clone + Send + Sync + 'static {
    async fn load(&self) -> Result<Option<Credentials>, CredentialsError>;
    async fn save(&self, creds: &Credentials) -> Result<(), CredentialsError>;
    async fn clear(&self) -> Result<(), CredentialsError>;
}

pub trait SseParserTrait: Clone + Send + Sync + 'static {
    fn feed_line(&mut self, line: &str) -> Result<Option<SseEvent>, SseParseError>;
    fn reset(&mut self);
}

pub trait TerminalBackend {
    fn draw<F>(&mut self, f: F) -> Result<()>;
    fn size(&self) -> Result<Rect>;
}
```

### 2. Adapter Pattern

The `adapters` module provides concrete implementations:

| Trait | Production Adapter | Mock Adapter |
|-------|-------------------|--------------|
| `HttpClient` | `ReqwestHttpClient` | `MockHttpClient` |
| `WebSocketConnection` | `TungsteniteWsConnection` | `MockWebSocket` |
| `CredentialsProvider` | `FileCredentialsProvider` | `InMemoryCredentials` |
| `SseParserTrait` | `DefaultSseParser` | - |

Example usage:

```rust
// Production
let client = ReqwestHttpClient::new();
let api = CentralApiClient::new(client);

// Testing
let mock = MockHttpClient::new();
mock.set_response("/api/threads", MockResponse::Success(response));
let api = CentralApiClient::new(mock);
```

### 3. Command Pattern for Input

The `input` module implements a command pattern:

```
KeyEvent -> CommandRegistry::dispatch() -> Command -> Handler -> App mutation
```

Key components:
- `Command` enum - All possible user actions
- `InputContext` - Current UI state for dispatch decisions
- `CommandRegistry` - Maps key events to commands
- `handlers` - Execute commands by mutating App

```rust
// src/input/mod.rs
impl App {
    pub fn build_input_context(&self) -> InputContext { ... }
    pub fn execute_command(&mut self, cmd: Command) -> bool { ... }
}
```

### 4. RAII Terminal Management

The `terminal` module provides automatic cleanup:

```rust
// src/terminal/mod.rs
pub struct TerminalManager {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    _guard: TerminalGuard,  // Cleanup on drop
}

pub struct TerminalGuard {
    cleaned_up: bool,
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        self.cleanup();  // Always restores terminal state
    }
}
```

Benefits:
- Terminal always restored, even on panic
- No manual cleanup code needed
- Panic hook provides additional safety

## Layer Architecture

```
+---------------------------------------------------+
|                    UI Layer                        |
|  (ui/, widgets/, view_state/)                     |
+---------------------------------------------------+
                        |
+---------------------------------------------------+
|                Application Layer                   |
|  (app/, input/, state/)                           |
+---------------------------------------------------+
                        |
+---------------------------------------------------+
|                  Domain Layer                      |
|  (models/, cache/, domain/)                       |
+---------------------------------------------------+
                        |
+---------------------------------------------------+
|               Infrastructure Layer                 |
|  (adapters/, traits/, auth/, websocket/)          |
+---------------------------------------------------+
```

### UI Layer
- `ui/` - Screen rendering (command_deck, conversation, dashboard)
- `widgets/` - Reusable components (TextAreaInput)
- `view_state/` - Transient view state (scroll, hit registry)

### Application Layer
- `app/` - Core App struct, AppMessage handling
- `input/` - Command dispatch and handlers
- `state/` - SessionState, question state, tool tracking

### Domain Layer
- `models/` - Thread, Message, Folder, Request types
- `cache/` - ThreadCache, message reconciliation
- `domain/` - Business logic types

### Infrastructure Layer
- `traits/` - Abstract interfaces
- `adapters/` - Concrete implementations
- `auth/` - CentralApiClient, credentials
- `websocket/` - Real-time communication

## Testing Strategy

### Unit Tests
Every module has co-located tests in `#[cfg(test)]` blocks.

### Mock Adapters
The `adapters::mock` module enables testing without network:

```rust
#[test]
fn test_api_call() {
    let mock = MockHttpClient::new();
    mock.set_response("/api/test", MockResponse::Success(response));

    let client = ApiClient::new(mock);
    let result = client.get_test().await;

    assert_eq!(mock.get_requests().len(), 1);
}
```

### Integration Tests
Located in `tests/` directory for cross-module testing.

## Key Design Decisions

1. **Traits over concrete types** - Enables testing and future flexibility
2. **Command pattern** - Centralizes input handling, enables keybinding customization
3. **RAII for terminal** - Guarantees cleanup in all exit paths
4. **Module documentation** - Clear public/internal API boundary
5. **Clean separation** - UI knows nothing about network, adapters know nothing about UI

## Module Dependencies

```
main.rs
  -> cli (argument parsing)
  -> terminal (RAII management)
  -> startup (preflight checks)
  -> app (core state)
     -> traits (abstractions)
     -> adapters (implementations)
     -> models (data types)
     -> cache (thread cache)
     -> state (session state)
  -> ui (rendering)
     -> widgets
     -> view_state
  -> input (command handling)
  -> websocket (real-time)
  -> auth (credentials)
```

## Version History

- **v0.1.x** - Initial implementation with tight coupling
- **v0.2.0** - Deep architectural refactor:
  - Trait-based abstractions for all external dependencies
  - Command pattern for input handling
  - RAII terminal management
  - Clean module separation with public/internal API boundary
  - Comprehensive test coverage with mock adapters

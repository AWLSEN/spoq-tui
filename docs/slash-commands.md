# Spoq CLI Slash Commands Specification

Minimal, essential slash commands for spoq-cli based on architecture and design specifications.

**Last Updated:** January 20, 2026

---

## Design Principles

1. **Fallback to Bash**: If no slash command matches, execute as bash command
2. **Context-Aware**: Commands adapt to current screen/state
3. **Consistent Syntax**: Similar commands use similar patterns
4. **Minimal & Essential**: Only commands that earn their place

---

## Slash Commands (29 total)

### 🎯 Core Thread Management (5)

| Command | Description | Example |
|---------|-------------|---------|
| `/new` | Start new thread | `/new` |
| `/threads` | List/search all threads | `/threads` |
| `/resume <id>` | Resume specific thread | `/resume auth-refactor` |
| `/archive` | Archive current thread | `/archive` |
| `/recent` | Recent threads picker (same as Tab) | `/recent` |

**Note:** `/new` replaces both "new" and "clear" - starting a new thread is the same as clearing.

### 🤖 Agent & Plan Control (6)

| Command | Description | Example |
|---------|-------------|---------|
| `/plan` | Enter plan mode (Nova) | `/plan` |
| `/exec` | Execute approved plan (Pulsar) | `/exec` |
| `/agents` | List all running agents | `/agents` |
| `/stop [id]` | Stop agent/thread | `/stop agent-42` |
| `/approve` | Approve waiting plan | `/approve` |
| `/reject [reason]` | Reject waiting plan | `/reject too risky` |

**Aliases:**
- `/nova` → `/plan`
- `/pulsar` → `/exec`

### 🏠 Navigation (3)

| Command | Description | Example |
|---------|-------------|---------|
| `/home` | Go to command deck | `/home` |
| `/conv` | Go to conversation view | `/conv` |
| `/cd <path>` | Change working directory | `/cd ~/projects/api` |

**Alias:**
- `/deck` → `/home`

### 📊 Status & Monitoring (4)

| Command | Description | Example |
|---------|-------------|---------|
| `/status` | System status (CPU, memory, agents) | `/status` |
| `/health` | VPS/conductor health check | `/health` |
| `/logs [n]` | Show recent logs | `/logs 50` |
| `/debug` | Toggle debug mode | `/debug` |

### 🔐 Authentication (3)

| Command | Description | Example |
|---------|-------------|---------|
| `/login` | Start device flow authentication | `/login` |
| `/logout` | Sign out and clear credentials | `/logout` |
| `/whoami` | Show current user info | `/whoami` |

### 🖥️ VPS & Infrastructure (3)

| Command | Description | Example |
|---------|-------------|---------|
| `/vps` | VPS info & status | `/vps` |
| `/ssh-info` | SSH connection details | `/ssh-info` |
| `/conductor-logs` | View conductor logs | `/conductor-logs` |

### ❓ Help & Info (3)

| Command | Description | Example |
|---------|-------------|---------|
| `/help [cmd]` | Show help (or for specific command) | `/help plan` |
| `/commands` | List all slash commands | `/commands` |
| `/shortcuts` | Show keyboard shortcuts | `/shortcuts` |

### 🔧 Utility (2)

| Command | Description | Example |
|---------|-------------|---------|
| `/config` | Open configuration editor | `/config` |
| `/version` | Show spoq version | `/version` |

---

## Bash Fallback Behavior

Any command that doesn't match a slash command executes as bash:

```bash
/ls -la              → executes: ls -la
/git status          → executes: git status
/cargo test          → executes: cargo test
/curl https://...    → executes: curl https://...
/python script.py    → executes: python script.py
```

**Implementation:**
1. Parse input: `/command [args]`
2. Check if `command` exists in slash commands registry
3. If **found**: Execute slash command handler
4. If **not found**: Execute as bash: `bash -c "command args"`
5. Display output in conversation

---

## What We Intentionally Cut

These are NOT in MVP but can be added later when users request them:

**Thread Management:**
- `/fork`, `/rename`, `/delete`, `/export` → can add later
- `/compact` → conductor can auto-compact when needed

**Code & Review:**
- `/review`, `/diff`, `/commit`, `/test` → use bash: `/git diff`, `/cargo test`
- `/security-review`, `/verify`, `/issue` → add when needed

**Advanced Features:**
- Aliases, macros, snippets → add when users ask
- `/benchmark`, `/profile`, `/trace` → dev tools for later
- `/backup`, `/restore` → overkill for MVP
- Theme switching, font size → config file is fine
- Model switching → auto-select or config
- `/extract`, `/summarize` → nice-to-have

**Why cut them?**
- Simpler to learn and use
- Less maintenance burden
- Can add incrementally based on user feedback
- Many operations work fine via bash fallback

---

## Context-Specific Commands

### When in Plan Mode (Nova)

Additional commands available:

| Command | Description |
|---------|-------------|
| `/phases` | Show all planned phases |
| `/phase <n>` | Jump to specific phase |
| `/edit-plan` | Edit current plan |

### When on Command Deck

Additional filtering:

| Command | Description |
|---------|-------------|
| `/filter <state>` | Filter threads by state |
| `/sort <by>` | Sort threads |

---

## Priority Implementation Order

### Phase 1: MVP (Core functionality)
- ✅ `/new`, `/threads`, `/recent`
- ✅ `/help`, `/commands`
- ✅ `/status`, `/health`
- ✅ `/login`, `/logout`
- ✅ Bash fallback

### Phase 2: Agent Control
- `/plan`, `/exec`
- `/agents`, `/stop`
- `/approve`, `/reject`

### Phase 3: VPS Management
- `/vps`, `/ssh-info`
- `/conductor-logs`

### Phase 4: Polish
- `/home`, `/conv`, `/cd`
- `/debug`, `/logs`
- Autocomplete UI
- Command history

---

## UI Integration

### Slash Command Autocomplete

When user types `/`, show fuzzy-searchable list:

```
┌─────────────────────────────────────┐
│ /n                                  │
│ ──────────────────────────────────  │
│ /new         Start new thread       │
│ /nova        Enter plan mode        │
│ ...                                 │
└─────────────────────────────────────┘
```

Features:
- Fuzzy search as user types
- Show command + brief description
- Arrow keys to navigate
- Enter to select, Esc to cancel
- Tab to autocomplete

### Command Feedback

**Success:**
```
✓ New thread created
✓ Thread archived: "auth-refactor"
✓ Logged out successfully
```

**Error:**
```
✗ Failed to stop agent: not found
✗ Thread ID required: /resume <id>
```

**Progress:**
```
⏳ Provisioning VPS... (3/5)
⏳ Executing plan... (phase 2/4)
```

### Command History

- Store last 100 commands in `~/.spoq/command_history`
- Arrow up/down to cycle through history
- Persist across sessions

---

## Command Syntax

### Arguments

**Required:** `<arg>`
```
/resume <thread-id>
/cd <path>
```

**Optional:** `[arg]`
```
/stop [agent-id]
/logs [n]
/help [command]
```

### Quoted Arguments

For arguments with spaces:

```bash
/reject "too complex, needs more detail"
/resume "API Refactor Phase 2"
```

---

## Error Handling

### Unknown Command with Bash Fallback

```
Input: /notacommand

→ No slash command: 'notacommand'
→ Executing as bash: notacommand
→ bash: notacommand: command not found
```

### Missing Required Arguments

```
Input: /resume

✗ Missing required argument: <thread-id>
Usage: /resume <thread-id>
```

### Invalid Arguments

```
Input: /stop invalid-id

✗ Agent not found: invalid-id
Hint: Use /agents to list running agents
```

---

## Configuration

Store user preferences in `~/.spoq/config.toml`:

```toml
[commands]
# Custom aliases
[commands.aliases]
s = "status"
h = "health"
a = "agents"

[behavior]
auto_compact = true
approval_mode = "auto"  # auto, manual, selective

[display]
theme = "dark"
show_timestamps = true
```

---

## Implementation: Command Registry

```rust
pub struct SlashCommand {
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub description: &'static str,
    pub usage: &'static str,
    pub category: CommandCategory,
    pub handler: CommandHandler,
    pub min_args: usize,
    pub max_args: Option<usize>,
}

pub enum CommandCategory {
    Thread,
    Agent,
    Navigation,
    Status,
    Auth,
    VPS,
    Help,
    Utility,
}

// Example registration
SlashCommand {
    name: "new",
    aliases: &[],
    description: "Start new thread",
    usage: "/new",
    category: CommandCategory::Thread,
    handler: handle_new_thread,
    min_args: 0,
    max_args: Some(0),
}
```

### Parser

```rust
pub enum ParsedCommand {
    SlashCommand {
        name: String,
        args: Vec<String>,
        handler: CommandHandler,
    },
    BashFallback {
        command: String,
    },
    Empty,
}

pub fn parse_command(input: &str) -> ParsedCommand {
    if !input.starts_with('/') {
        return ParsedCommand::Empty;
    }

    let parts: Vec<&str> = input[1..].split_whitespace().collect();
    if parts.is_empty() {
        return ParsedCommand::Empty;
    }

    let cmd = parts[0];
    let args: Vec<String> = parts[1..].iter().map(|s| s.to_string()).collect();

    // Check registry
    if let Some(handler) = COMMAND_REGISTRY.get(cmd) {
        return ParsedCommand::SlashCommand {
            name: cmd.to_string(),
            args,
            handler: handler.handler,
        };
    }

    // Fallback to bash
    ParsedCommand::BashFallback {
        command: input[1..].to_string(),
    }
}
```

### Execution Flow

```
User input: "/exec"
    │
    ▼
Parse command
    │
    ▼
Validate arguments
    │
    ▼
Execute handler
    │
    ├─→ Success: Show confirmation
    ├─→ Error: Show error message
    └─→ Async: Show progress
```

---

## Special Keys (Not Slash Commands)

Built into the UI:

| Key | Action |
|-----|--------|
| `Tab` | Open recent threads picker (MRU) |
| `Ctrl+N` | New thread (same as `/new`) |
| `Ctrl+K` | Command palette |
| `Ctrl+/` | Show slash command help |
| `Esc` | Go back / cancel |
| `?` | Show keyboard shortcuts |

---

## Examples by Use Case

### Starting New Work

```bash
/new
"Help me implement user authentication"
```

### Planning & Executing

```bash
/plan
# ... AI creates plan ...
/approve
/exec
```

### Managing Threads

```bash
/threads           # List all
# select one
/resume thread-42
# ... work on it ...
/archive
```

### Checking Status

```bash
/status            # Quick overview
/agents            # Running agents
/health            # VPS health
/conductor-logs    # Detailed logs
```

### VPS Management

```bash
/vps               # Info & status
/ssh-info          # Connection details
/whoami            # Current user
```

### Using Bash Fallback

```bash
/git status
/cargo build
/ls -la src/
/curl -I https://api.spoq.dev
```

---

## Why This Is Better

**Minimal (29 commands):**
- ✅ Easy to learn
- ✅ Easy to remember
- ✅ Each command earns its place

**Bash Fallback:**
- ✅ Don't need `/diff`, `/test`, etc. - just use git/cargo directly
- ✅ No command explosion
- ✅ Users already know bash commands

**Room to Grow:**
- ✅ Can add commands incrementally
- ✅ Based on actual user feedback
- ✅ No premature optimization

**Clean Architecture:**
- ✅ Clear categories
- ✅ Consistent naming
- ✅ Predictable behavior

---

## Future Additions (Based on User Requests)

If users frequently do:
- `/git diff` → add built-in `/diff` with better UI
- `/cargo test` → add built-in `/test` with progress
- Export threads → add `/export`
- Fork threads → add `/fork`
- Custom commands → add command system like OpenCode/Droid

But for MVP: **29 commands + bash fallback is perfect**.

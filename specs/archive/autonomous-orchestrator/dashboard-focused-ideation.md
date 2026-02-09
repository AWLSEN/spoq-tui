# Command Center Dashboard - Final Ideation

> Target: 13" Mac terminal (~100×35 characters)
> Touch-first, non-blocking, minimal UI

---

## Design Principles

1. **Minimal chrome** - Indicators over text labels where possible
2. **Touch-first** - All interactions via click/tap; keyboard as accelerators
3. **KISS** - Simple categories, no unnecessary sub-filters
4. **Non-blocking** - Never trap user in a modal
5. **No displacement** - Expanded content overlays, doesn't push rows

---

## Logo

SPOQ in pixel style (2 lines):

```
▄▄▄ ▄▄▄ ▄▄▄ ▄▄▄
▀▀█ █▀▀ █▄█ █▄█
```

Alternative compact:
```
╭──────╮
│ SPOQ │
╰──────╯
```

---

## Header

### Layout

```
┌─────────────────────────────────────────────────────────────────────────────────────────────┐
│                                                                                             │
│   [System Status]                    [Logo]                           [Work Stats]          │
│                                                                                             │
└─────────────────────────────────────────────────────────────────────────────────────────────┘
```

### System Status (Left)

```
●  cpu ▓▓▓░░  4.2/8g
```

| Element | Description |
|---------|-------------|
| `●` | Connection indicator (green = connected, red = disconnected) |
| `cpu ▓▓▓░░` | CPU usage as visual bar with label |
| `4.2/8g` | RAM used / total allocated |

### Work Stats (Right)

```
47 threads · 12 repos
```

- Shows thread count and repository count
- Agents omitted (implementation detail, not user-facing metric)
- Hover/tooltip can reveal additional details if needed

### Complete Header

```
●  cpu ▓▓▓░░  4.2/8g                    ▄▄▄ ▄▄▄ ▄▄▄ ▄▄▄                   47 threads · 12 repos
                                        ▀▀█ █▀▀ █▄█ █▄█
```

---

## Status Bar

### Purpose

Shows aggregate thread state at a glance. Each segment is clickable to filter.

### Categories

| Segment | Meaning | Contains |
|---------|---------|----------|
| **working** | Active threads | Executing, waiting for approval, has question |
| **ready to test** | Completed work | Done, awaiting user verification |
| **idle** | Inactive threads | Paused, archived |

### Visual

```
████████████████████████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░
working 24              ready to test 8                                idle 15
```

- Proportional bar representing distribution
- Click segment to filter view
- `✕` appears when filtered; click to clear

### Filtered State

```
▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░
▶ working 24            ready to test 8                                idle 15   ✕
```

- `▶` indicates active filter
- Selected segment highlighted
- `✕` button to clear filter

---

## Thread List

### Separator

Short solid line (~10% width) on left side separates:

- **Above the line:** Threads needing user action (waiting, question)
- **Below the line:** Threads working autonomously

```
  Auth Refactor          ~/api       plan       waiting              [approve] [reject]
  Settings Page          ~/tui       normal     question             [answer]
  Payment Flow           ~/pay       plan       waiting              [approve] [reject]

  ────────

  API Endpoints          ~/api       exec       Edit: handlers.rs                  12m
  Test Suite             ~/tui       exec       Bash: npm test                      3m
  Docs Generator         ~/docs      exec       Read: index.md                      8m
```

### Thread Row Structure

**Non-Action Threads** (Working autonomously below separator):

| Field | Width | Description | Examples |
|-------|-------|-------------|----------|
| **Title** | 30% | Thread name | `Auth Refactor`, `API Endpoints` |
| **Repo** | 14% | Repository path | `~/api`, `~/tui`, `~/docs` |
| **Mode** | 9% | Thread type | `plan`, `normal`, `exec` |
| **Activity** | 27% | Current activity or status | `Read: schema.ts`, `Thinking...`, `done` |
| **Time** | 10% | Duration or age | `12m`, `3h`, `2d` |

**Action Threads** (Above separator, needs user input):

| Field | Width | Description | Examples |
|-------|-------|-------------|----------|
| **Title** | 30% | Thread name | `Auth Refactor`, `Settings Page` |
| **Repo** | 14% | Repository path | `~/api`, `~/tui` |
| **Mode** | 9% | Thread type | `plan`, `normal` |
| **Status** | 17% | Current state | `waiting`, `question` |
| **Actions** | 20% | Context-specific buttons | `[approve] [reject]`, `[answer]` |

### Activity Column Display Logic

**For non-action threads** (working autonomously):

| Thread State | Activity Shows | Data Source |
|--------------|----------------|-------------|
| Running + tool active | `Read: main.rs` | ToolExecuting.display_name |
| Running + tool active | `Edit: schema.ts` | ToolExecuting.display_name |
| Running + tool active | `Bash: npm test` | ToolExecuting.display_name |
| Running + no tool | `Thinking...` | Fallback when no active tool |
| Idle | `idle` | Status |
| Done | `done` | Status |
| Error | `error` | Status |

**For action threads** (above separator):

| Thread State | Display Mode |
|--------------|--------------|
| Waiting (plan approval) | Uses old layout: Status column + `[approve] [reject]` actions |
| Question | Uses old layout: Status column + `[answer]` action |
| Ready to test | Uses old layout: Status column + `[verify] [issue] [archive]` actions |

### Example: New Column Layout for Non-Action Threads

```
Title (30%)       | Repo (14%) | Mode (9%) | Activity (27%)            | Time (10%)
──────────────────────────────────────────────────────────────────────────────────────
API Endpoints       ~/api        exec        Edit: handlers.rs              12m
Auth Refactor       ~/api        plan        Read: schema.ts                 3m
Settings Page       ~/tui        normal      Thinking...                     1m
Test Runner         ~/test       exec        Bash: npm test                  5m
DB Migration        ~/db         normal      idle                            3h
Cache Layer         ~/api        done        done                            4h
```

### Action Buttons by State

| Thread State | Available Actions |
|--------------|-------------------|
| `waiting` (plan approval) | `[approve]` `[reject]` |
| `question` | `[answer]` |
| `exec` (running) | (click row to view details) |
| `done` (ready to test) | `[verify]` `[issue]` `[archive]` |
| `paused` / `archived` | `[resume]` `[archive]` or `[delete]` |

---

## Footer

### Input Area

```
╭────────────────────────────────────────────────────────────────────────────────────────╮
│                                                                                        │
╰────────────────────────────────────────────────────────────────────────────────────────╯
```

Standard text input for commands, search, or new thread creation.

### Hints

Context-aware, minimal hints below input:

| State | Hint |
|-------|------|
| Default view | `click status to filter` |
| Filtered view | `✕ clear` |
| Overlay open | `esc close` |

---

## Interaction Model

All interactions are touch/click-based. Keyboard shortcuts exist as accelerators.

| Element | Interaction |
|---------|-------------|
| Status bar segment | Click to filter |
| Thread row | Click to expand/view details |
| Action button | Click to execute action |
| `✕` button | Click to clear filter |
| Overlay | `esc` or click outside to close |
| Scroll | Mouse scroll / trackpad |

---

## Full Mockups

### Default View

```
╔════════════════════════════════════════════════════════════════════════════════════════════════╗
║                                                                                                ║
║  ●  cpu ▓▓▓░░  4.2/8g                 ▄▄▄ ▄▄▄ ▄▄▄ ▄▄▄                   47 threads · 12 repos  ║
║                                       ▀▀█ █▀▀ █▄█ █▄█                                          ║
║                                                                                                ║
║  ════════════════════════════════════════════════════════════════════════════════════════     ║
║                                                                                                ║
║  ████████████████████████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░     ║
║  working 24              ready to test 8                                        idle 15       ║
║                                                                                                ║
║  ────────────────────────────────────────────────────────────────────────────────────────     ║
║                                                                                                ║
║  Auth Refactor          ~/api       plan       waiting              [approve] [reject]        ║
║  Settings Page          ~/tui       normal     question             [answer]                  ║
║  Payment Flow           ~/pay       plan       waiting              [approve] [reject]        ║
║                                                                                                ║
║  ────────                                                                                     ║
║                                                                                                ║
║  API Endpoints          ~/api       exec       Edit: handlers.rs                      12m     ║
║  Test Suite             ~/tui       exec       Bash: npm test                          3m     ║
║  Docs Generator         ~/docs      exec       Read: index.md                          8m     ║
║  Search Index           ~/search    exec       Thinking...                             5m     ║
║                                                                                                ║
║                                                                                                ║
║  ────────────────────────────────────────────────────────────────────────────────────────     ║
║  ╭────────────────────────────────────────────────────────────────────────────────────────╮   ║
║  │                                                                                        │   ║
║  ╰────────────────────────────────────────────────────────────────────────────────────────╯   ║
║  click status to filter                                                                       ║
║                                                                                                ║
╚════════════════════════════════════════════════════════════════════════════════════════════════╝
```

### Filtered: Working

```
╔════════════════════════════════════════════════════════════════════════════════════════════════╗
║                                                                                                ║
║  ●  cpu ▓▓▓░░  4.2/8g                 ▄▄▄ ▄▄▄ ▄▄▄ ▄▄▄                   47 threads · 12 repos  ║
║                                       ▀▀█ █▀▀ █▄█ █▄█                                          ║
║                                                                                                ║
║  ════════════════════════════════════════════════════════════════════════════════════════     ║
║                                                                                                ║
║  ▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░     ║
║  ▶ working 24            ready to test 8                                        idle 15   ✕   ║
║                                                                                                ║
║  ────────────────────────────────────────────────────────────────────────────────────────     ║
║                                                                                                ║
║  Auth Refactor          ~/api       plan       waiting              [approve] [reject]        ║
║  Settings Page          ~/tui       normal     question             [answer]                  ║
║  Payment Flow           ~/pay       plan       waiting              [approve] [reject]        ║
║                                                                                                ║
║  ────────                                                                                     ║
║                                                                                                ║
║  API Endpoints          ~/api       exec       Edit: handlers.rs                      12m     ║
║  Test Suite             ~/tui       exec       Bash: npm test                          3m     ║
║  Docs Generator         ~/docs      exec       Read: index.md                          8m     ║
║  Search Index           ~/search    exec       Thinking...                             5m     ║
║  Lint Fixes             ~/lib       exec       Bash: cargo clippy                      1m     ║
║  DB Optimize            ~/db        exec       Edit: migrations.sql                    6m     ║
║  + 15 more                                                                                    ║
║                                                                                                ║
║  ────────────────────────────────────────────────────────────────────────────────────────     ║
║  ╭────────────────────────────────────────────────────────────────────────────────────────╮   ║
║  │                                                                                        │   ║
║  ╰────────────────────────────────────────────────────────────────────────────────────────╯   ║
║  ✕ clear                                                                                      ║
║                                                                                                ║
╚════════════════════════════════════════════════════════════════════════════════════════════════╝
```

### Filtered: Ready to Test

```
╔════════════════════════════════════════════════════════════════════════════════════════════════╗
║                                                                                                ║
║  ●  cpu ▓▓░░░  2.1/8g                 ▄▄▄ ▄▄▄ ▄▄▄ ▄▄▄                   47 threads · 12 repos  ║
║                                       ▀▀█ █▀▀ █▄█ █▄█                                          ║
║                                                                                                ║
║  ════════════════════════════════════════════════════════════════════════════════════════     ║
║                                                                                                ║
║  ████████████████████████▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░     ║
║  working 24              ▶ ready to test 8                                      idle 15   ✕   ║
║                                                                                                ║
║  ────────────────────────────────────────────────────────────────────────────────────────     ║
║                                                                                                ║
║  DB Migration           ~/db        done       2h ago         [verify] [issue] [archive]      ║
║  Cache Layer            ~/api       done       4h ago         [verify] [issue] [archive]      ║
║  User Auth v2           ~/auth      done       1d ago         [verify] [issue] [archive]      ║
║  Rate Limiter           ~/api       done       3h ago         [verify] [issue] [archive]      ║
║  Search Refactor        ~/search    done       5h ago         [verify] [issue] [archive]      ║
║  Notification API       ~/api       done       6h ago         [verify] [issue] [archive]      ║
║  Mobile SDK             ~/mobile    done       1d ago         [verify] [issue] [archive]      ║
║  Webhook Handler        ~/api       done       2d ago         [verify] [issue] [archive]      ║
║                                                                                                ║
║                                                                                                ║
║                                                                                                ║
║  ────────────────────────────────────────────────────────────────────────────────────────     ║
║  ╭────────────────────────────────────────────────────────────────────────────────────────╮   ║
║  │                                                                                        │   ║
║  ╰────────────────────────────────────────────────────────────────────────────────────────╯   ║
║  ✕ clear                                                                                      ║
║                                                                                                ║
╚════════════════════════════════════════════════════════════════════════════════════════════════╝
```

### Filtered: Idle

```
╔════════════════════════════════════════════════════════════════════════════════════════════════╗
║                                                                                                ║
║  ●  cpu ▓▓░░░  2.1/8g                 ▄▄▄ ▄▄▄ ▄▄▄ ▄▄▄                   47 threads · 12 repos  ║
║                                       ▀▀█ █▀▀ █▄█ █▄█                                          ║
║                                                                                                ║
║  ════════════════════════════════════════════════════════════════════════════════════════     ║
║                                                                                                ║
║  ████████████████████████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓     ║
║  working 24              ready to test 8                                    ▶ idle 15     ✕   ║
║                                                                                                ║
║  ────────────────────────────────────────────────────────────────────────────────────────     ║
║                                                                                                ║
║  Dashboard v1           ~/web       paused     3d ago                   [resume] [archive]    ║
║  Legacy API             ~/api       archived   1w ago                   [resume] [delete]     ║
║  Email Templates        ~/email     paused     2d ago                   [resume] [archive]    ║
║  Admin Panel            ~/admin     archived   2w ago                   [resume] [delete]     ║
║  Analytics Setup        ~/data      paused     4d ago                   [resume] [archive]    ║
║  + 10 more                                                                                    ║
║                                                                                                ║
║                                                                                                ║
║                                                                                                ║
║                                                                                                ║
║                                                                                                ║
║  ────────────────────────────────────────────────────────────────────────────────────────     ║
║  ╭────────────────────────────────────────────────────────────────────────────────────────╮   ║
║  │                                                                                        │   ║
║  ╰────────────────────────────────────────────────────────────────────────────────────────╯   ║
║  ✕ clear                                                                                      ║
║                                                                                                ║
╚════════════════════════════════════════════════════════════════════════════════════════════════╝
```

### All Clear State

When no threads need user attention:

```
╔════════════════════════════════════════════════════════════════════════════════════════════════╗
║                                                                                                ║
║  ●  cpu ▓▓░░░  2.1/8g                 ▄▄▄ ▄▄▄ ▄▄▄ ▄▄▄                   47 threads · 12 repos  ║
║                                       ▀▀█ █▀▀ █▄█ █▄█                                          ║
║                                                                                                ║
║  ════════════════════════════════════════════════════════════════════════════════════════     ║
║                                                                                                ║
║  ████████████████████████████████████████████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░     ║
║  working 32                                                                     idle 15       ║
║                                                                                                ║
║  ────────────────────────────────────────────────────────────────────────────────────────     ║
║                                                                                                ║
║                                                                                                ║
║                                      all clear                                                 ║
║                                                                                                ║
║                                 nothing needs your attention                                   ║
║                                 32 threads working autonomously                                ║
║                                                                                                ║
║  ────────                                                                                     ║
║                                                                                                ║
║  API Endpoints          ~/api       exec       Edit: handlers.rs                      12m     ║
║  Test Suite             ~/tui       exec       Bash: npm test                          3m     ║
║  Docs Generator         ~/docs      exec       Read: index.md                          8m     ║
║                                                                                                ║
║  ────────────────────────────────────────────────────────────────────────────────────────     ║
║  ╭────────────────────────────────────────────────────────────────────────────────────────╮   ║
║  │                                                                                        │   ║
║  ╰────────────────────────────────────────────────────────────────────────────────────────╯   ║
║  click status to filter                                                                       ║
║                                                                                                ║
╚════════════════════════════════════════════════════════════════════════════════════════════════╝
```

### Heavy Load Warning

```
╔════════════════════════════════════════════════════════════════════════════════════════════════╗
║                                                                                                ║
║  ●  cpu ▓▓▓▓▓▓  7.8/8g  ⚠            ▄▄▄ ▄▄▄ ▄▄▄ ▄▄▄                  127 threads · 24 repos  ║
║                                      ▀▀█ █▀▀ █▄█ █▄█                                           ║
║                                                                                                ║
║  ════════════════════════════════════════════════════════════════════════════════════════     ║
║                                                                                                ║
║  ████████████████████████████████████████████████████████████████████████████████████████     ║
║  working 127                                                                                  ║
║                                                                                                ║
║  ⚠ heavy load                                                                                 ║
║                                                                                                ║
║  ────────────────────────────────────────────────────────────────────────────────────────     ║
║                                                                                                ║
║  Auth Refactor          ~/api       plan       waiting              [approve] [reject]        ║
║  Settings Page          ~/tui       normal     question             [answer]                  ║
║  Payment Flow           ~/api       plan       waiting              [approve] [reject]        ║
║  + 9 more                                                                                     ║
║                                                                                                ║
║  ────────                                                                                     ║
║                                                                                                ║
║  API Endpoints          ~/api       exec       Edit: handlers.rs                      12m     ║
║  + 114 more                                                                                   ║
║                                                                                                ║
║  ────────────────────────────────────────────────────────────────────────────────────────     ║
║  ╭────────────────────────────────────────────────────────────────────────────────────────╮   ║
║  │                                                                                        │   ║
║  ╰────────────────────────────────────────────────────────────────────────────────────────╯   ║
║  click status to filter                                                                       ║
║                                                                                                ║
╚════════════════════════════════════════════════════════════════════════════════════════════════╝
```

---

## Inline Expansion (Overlay)

### Design Principle

When a thread row expands (to show question, plan details, etc.), it **overlays** the rows below rather than pushing them down. This prevents jarring text displacement.

### Collapsed State

```
  Auth Refactor          ~/api       plan       waiting              [approve] [reject]
  Settings Page          ~/tui       normal     question             [answer]
  Payment Flow           ~/pay       plan       waiting              [approve] [reject]

  ────────

  API Endpoints          ~/api       exec       Edit: handlers.rs                      12m
```

### Expanded: Question with Options

After clicking `[answer]` on Settings Page:

```
╔════════════════════════════════════════════════════════════════════════════════════════════════╗
║                                                                                                ║
║  ●  cpu ▓▓▓░░  4.2/8g                 ▄▄▄ ▄▄▄ ▄▄▄ ▄▄▄                   47 threads · 12 repos  ║
║                                       ▀▀█ █▀▀ █▄█ █▄█                                          ║
║                                                                                                ║
║  ════════════════════════════════════════════════════════════════════════════════════════     ║
║                                                                                                ║
║  ████████████████████████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░     ║
║  working 24              ready to test 8                                        idle 15       ║
║                                                                                                ║
║  ────────────────────────────────────────────────────────────────────────────────────────     ║
║                                                                                                ║
║  Auth Refactor          ~/api       plan       waiting              [approve] [reject]        ║
║  ┌─ Settings Page · ~/tui ───────────────────────────────────────────────────────────────╮    ║
║  │                                                                                       │    ║
║  │  which auth provider should I use for the settings page?                              │    ║
║  │  I found both OAuth and JWT implementations in your codebase.                         │    ║
║  │                                                                                       │    ║
║  │  [OAuth (Recommended)]    [JWT]    [Other...]                                         │    ║
║  │                                                                                       │    ║
║  └───────────────────────────────────────────────────────────────────────────────────────╯    ║
║  ░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░     ║
║  ░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░     ║
║                                                                                                ║
║  ────────────────────────────────────────────────────────────────────────────────────────     ║
║  ╭────────────────────────────────────────────────────────────────────────────────────────╮   ║
║  │                                                                                        │   ║
║  ╰────────────────────────────────────────────────────────────────────────────────────────╯   ║
║  esc close                                                                                    ║
║                                                                                                ║
╚════════════════════════════════════════════════════════════════════════════════════════════════╝
```

- `░░░` = dimmed/occluded rows behind the overlay
- Rows above overlay remain fully visible
- Card anchored to its original row position

### Expanded: Free-Form Input

After clicking `[Other...]`:

```
║  ┌─ Settings Page · ~/tui ───────────────────────────────────────────────────────────────╮    ║
║  │                                                                                       │    ║
║  │  which auth provider should I use for the settings page?                              │    ║
║  │  I found both OAuth and JWT implementations in your codebase.                         │    ║
║  │                                                                                       │    ║
║  │  ╭─────────────────────────────────────────────────────────────────────────────────╮  │    ║
║  │  │ Use session-based auth with refresh tokens...                                   │  │    ║
║  │  ╰─────────────────────────────────────────────────────────────────────────────────╯  │    ║
║  │                                                                      [← back] [send]  │    ║
║  │                                                                                       │    ║
║  └───────────────────────────────────────────────────────────────────────────────────────╯    ║
```

### Expanded: Text-Only Question

```
║  ┌─ API Endpoints · ~/api ───────────────────────────────────────────────────────────────╮    ║
║  │                                                                                       │    ║
║  │  what should the rate limit be for the /users endpoint?                               │    ║
║  │                                                                                       │    ║
║  │  ╭─────────────────────────────────────────────────────────────────────────────────╮  │    ║
║  │  │                                                                                 │  │    ║
║  │  ╰─────────────────────────────────────────────────────────────────────────────────╯  │    ║
║  │                                                                              [send]   │    ║
║  │                                                                                       │    ║
║  └───────────────────────────────────────────────────────────────────────────────────────╯    ║
```

### Expanded: Plan Approval

After clicking `[approve]` or the row on a `waiting` plan thread:

```
║  ┌─ Auth Refactor · ~/api ───────────────────────────────────────────────────────────────╮    ║
║  │                                                                                       │    ║
║  │  plan ready · 7 phases · 12 files · ~45k tokens                                       │    ║
║  │                                                                                       │    ║
║  │  1. Research existing auth patterns                                                   │    ║
║  │  2. Create new AuthProvider interface                                                 │    ║
║  │  3. Implement OAuth2 adapter                                                          │    ║
║  │  4. Migrate existing endpoints                                                        │    ║
║  │  5. Add refresh token support                                                    ↓    │    ║
║  │                                                                                       │    ║
║  │                                                    [view full] [reject] [approve]     │    ║
║  │                                                                                       │    ║
║  └───────────────────────────────────────────────────────────────────────────────────────╯    ║
```

### Overlay Behavior

| Action | Result |
|--------|--------|
| Press `esc` | Close overlay, return to list |
| Click outside overlay | Close overlay |
| Click action button | Execute action, then close |
| Scroll list | Overlay stays anchored to row |

### Benefits

1. **No displacement** - Other rows don't jump around
2. **Instant collapse** - Returns to exact previous state
3. **Context preserved** - Rows above remain visible
4. **Non-blocking** - User can close anytime and navigate elsewhere

---

## Navigation Reference

| Action | How |
|--------|-----|
| Filter by status | Click status bar segment |
| Clear filter | Click `✕` |
| Open thread | Click thread row |
| Execute action | Click action button |
| Close overlay | `esc` or click outside |
| Scroll list | Mouse scroll / trackpad |

---

## Visual Language

### Colors (Semantic)

| Element | Meaning |
|---------|---------|
| Green `●` | Connected |
| Red `●` | Disconnected |
| `⚠` | Warning (high load) |
| `▶` | Active filter |

### Progress Indicators

```
●●●●○○○  4/7    (4 of 7 phases complete)
●●○○○○○  2/5    (2 of 5 phases complete)
```

### Status Labels

| Label | Meaning |
|-------|---------|
| `waiting` | Plan awaiting approval |
| `question` | Thread has pending question |
| `exec` | Currently executing |
| `done` | Completed, awaiting verification |
| `paused` | Manually paused |
| `archived` | Archived/inactive |

---

## Summary

This design prioritizes:

- **Clarity** - User immediately sees what needs attention
- **Efficiency** - One-click actions, no deep navigation
- **Non-intrusiveness** - Overlays don't disrupt flow
- **Scalability** - Works for 5 threads or 500 threads

The dashboard serves as command center for managing multiple autonomous coding threads, surfacing only what requires human decision while letting the rest run autonomously.

# Plan + Execution Mode Fix Spec (TUI + Conductor)

## Problem (Plan + Execute)
- For **existing threads**, toggling into Plan or Execute is local‑only (UI indicator changes), but the backend thread stays `mode=normal` and `permission_mode=default`.
- This prevents Plan/Execute from being authoritative across clients and breaks the expected “plan‑approved → execute” workflow.

## Goals
- Make Plan and Execute authoritative for **existing threads** by persisting to backend immediately.
- Ensure mode updates propagate to all clients via WebSocket `ThreadModeUpdate`.
- Preserve current flow for new threads (Plan/Execute still set at creation).

## Non‑Goals
- Changing Pulsar status monitoring or plan execution progress.
- Backend schema changes unrelated to thread mode.
- Changing the behavior of Claude CLI permission flags.

## Proposed Fix (Mode‑centric, modular)
### 1) Add a ThreadModeSync module (frontend)
Create a small module responsible for **authoritative mode updates**. It should:
- Accept `(thread_id, PermissionMode)` and compute both:
  - `thread_mode = normal | plan | exec`
  - `permission_mode = default | plan | execution`
- Coalesce updates: keep only the latest requested mode per thread.
- Send updates via HTTP to backend.

**Suggested location (TUI):**
- New file: `src/app/thread_mode_sync.rs`
- Wire into `src/app/mod.rs` and `src/app/navigation.rs`

### 2) Emit updates on mode toggle (Shift+Tab)
When Shift+Tab changes permission mode:
- If `active_thread_id` exists, call `ThreadModeSync::enqueue(thread_id, permission_mode)`.
- If no active thread, do nothing (new thread creation still sets mode).

**Integration points (TUI):**
- Shift+Tab handler in `src/main.rs` (calls `app.cycle_permission_mode()` today)
- `App::cycle_permission_mode()` in `src/app/navigation.rs` (add sync hook or delegate from handler)

### 3) Robustness: last‑intent wins
ThreadModeSync should:
- Debounce rapid toggles (e.g., 150–300ms).
- Cancel or supersede in‑flight updates when a newer mode arrives.
- Retry once on transient network errors, then give up quietly (UI remains local).

### 4) Persist both “mode” and “permission_mode”
Use both endpoints:
- `PUT /v1/threads/{id}/mode` → `mode = plan` (or `normal` / `exec`)
- `PUT /v1/threads/{id}/permission` → `permission_mode = plan` (or `default` / `execution`)
This keeps dashboard mode and request metadata aligned.

### 2) Mapping rules (frontend)
- `PermissionMode::Default` → thread mode `normal`
- `PermissionMode::Plan` → thread mode `plan`
- `PermissionMode::Execution` → thread mode `exec`

### 3) UI behavior
- Keep local UI mode indicator behavior as-is (shows `[PLAN]` or `[EXECUTE]`).
- After a successful mode update, no extra UI change required; server WebSocket should broadcast mode updates to all clients.

## API Details (unchanged)
- Endpoint already exists in backend routes:
  - `PUT /v1/threads/{thread_id}/mode`
  - `PUT /v1/threads/{thread_id}/permission`
- Frontend conductor client already has `update_thread_mode()` in `src/conductor.rs`.
- If `update_thread_permission()` is not currently implemented, add it or confirm it exists.

### Payload/Response Compatibility (TUI ↔ Conductor)
**Thread mode update (TUI → Conductor):**
- Request: `PUT /v1/threads/{id}/mode`
  - JSON body: `{"mode":"normal" | "plan" | "exec"}`
- Response: should reflect updated thread mode (backend currently returns a Thread DTO).

**Permission mode update (TUI → Conductor):**
- Request: `PUT /v1/threads/{id}/permission`
  - JSON body: `{"permission_mode":"default" | "plan" | "execution"}`
- Response: should reflect updated permission_mode in thread metadata.

**Stream request (TUI → Conductor):**
- Request: `POST /v1/stream`
  - JSON body includes:
    - `permission_mode`: `"default" | "plan" | "execution"` (TUI serialization in `src/models/request.rs`)
    - `plan_mode`: `true` when `permission_mode == plan`
  - Conductor expects these values in `AgentRequest` (`../spoq-conductor/src/api/agent_stream.rs`).

**Critical mismatch to avoid:**
- TUI must send `"execution"` (not `"bypassPermissions"`). This is already fixed in `src/models/request.rs` via `#[serde(rename="execution")]`.
- Conductor maps thread mode from `permission_mode` strings; it recognizes `"plan"` and `"execution"`.

## Implementation Steps (modular)
1) Add `src/app/thread_mode_sync.rs` with:
   - `ThreadModeSync::enqueue(thread_id, permission_mode)`
   - debounce/coalesce per thread
   - call `update_thread_mode()` then `update_thread_permission()`
2) Wire `App::cycle_permission_mode()` or Shift+Tab handler to call `ThreadModeSync`.
3) Add mapping unit tests in the new module.
4) Add a focused test ensuring toggle on existing thread calls the sync path.

## Tests
- Unit: verify mapping `PermissionMode → thread mode` and `permission_mode`.
- Integration (if available):
  - Create thread, toggle to Plan, verify backend returns `mode=plan` in `/v1/threads`.
  - Toggle to Execute, verify `mode=exec`.

## Risks
- If backend ignores updates or requires auth, calls may fail; UI remains local‑only.
- Multiple clients toggling: last write wins. Acceptable for plan/execute semantics.
@@
## Architecture Notes
- ThreadModeSync isolates backend persistence so UI stays simple and testable.
- Debounce/coalesce makes the HTTP approach robust under rapid switching.
- Keeping both `mode` and `permission_mode` in sync avoids dashboard/UI drift.

## HTTP Update Considerations (Edge Cases)
Using HTTP `PUT /v1/threads/{id}/mode` is fine, but there are some edge cases to account for:
- **Rapid toggling (Shift+Tab spam):** multiple in‑flight requests can arrive out of order. Mitigation: debounce or coalesce mode updates; only keep the latest intent and drop older ones.
- **Last‑write wins:** even with debouncing, the backend will persist the last request that arrives. This is acceptable if the UI is optimistic and the latest toggle is what the user wants.
- **Network latency / offline:** HTTP failures should not block UI; keep local state and retry on next mode change or next stream.
- **Consistency across clients:** if multiple clients toggle, HTTP updates are the authoritative source; the WebSocket mode update should reflect the last applied change.

If lower latency or ordering guarantees become important, a WebSocket command could replace HTTP, but HTTP is sufficient with debouncing and “latest intent” coalescing.

## Success Criteria
- For an existing thread, toggling to Plan or Execute updates backend thread mode (`mode=plan/exec`) immediately.
- `/v1/threads` reflects the correct mode after toggle.
- UI shows consistent mode across clients.

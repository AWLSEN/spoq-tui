# Ultimate Autonomous Loop Orchestrator Spec (Local-First, Server-Ready)

## Spec Status
- Status: canonical source of truth
- Scope: local-first orchestrator with server-ready boundaries
- Branch model: one shared branch, no git worktrees (V1)
- Write model: monitor-only writer, worker patch intents

## Supersedes
This spec supersedes ideation/research artifacts now archived at:
- `specs/archive/autonomous-orchestrator/command-center-ideations.md`
- `specs/archive/autonomous-orchestrator/dashboard-focused-ideation.md`
- `specs/archive/autonomous-orchestrator/TUI_RESEARCH.md`
- `specs/archive/autonomous-orchestrator/osforagents-ideation.md`
- `specs/archive/autonomous-orchestrator/osforagents-implementation.md`

## Summary
Build a local-first autonomous coding loop system inside `spoq-local` that combines:
- A Rust TUI for clarity-first operation and reduced cognitive context switching.
- A separate local Rust orchestrator daemon (`spoqd`) as source of truth.
- Engine adapters for both Claude Code (`claude`) and Codex (`codex`).
- Sandboxed autonomous execution via Gondolin.
- Fully programmable DAG-based agent teams (swarms).
- Single-branch execution with one monitor-controlled writer (no git worktrees in V1).

The system must be modular so that a remote orchestrator server can be introduced later without rewriting the TUI or core execution model.

## Problem
Current repo has strong TUI, state, event, and websocket foundations, but no dedicated autonomous-loop orchestrator layer:
- No durable loop scheduler with resumable runs.
- No common execution abstraction for both Claude Code and Codex.
- No sandbox-first autonomous execution engine.
- No first-class multi-agent DAG workflow execution.

## Goals
- Provide one local orchestrator daemon that can execute autonomous loops reliably.
- Support Claude Code and Codex in a unified engine contract.
- Enforce plan-first autonomous flow with allowlists and sandbox controls.
- Implement fully programmable DAG workflows for agent teams.
- Keep TUI as client-only so daemon can later become remote.
- Persist all loop state and events for replay, audit, and recovery.

## Non-Goals (V1)
- Distributed multi-machine scheduler.
- Kubernetes-native control plane.
- Full visual parity with external HEIC reference before a PNG/JPG is provided.
- Replacing existing SPOQ features unrelated to autonomous loops.

## Design Principles
- Simple over easy: one clear source of truth (`spoqd`) and explicit boundaries.
- Event-first state: all important transitions emit durable events.
- Fail-closed safety: autonomous execution denied unless policy permits.
- Detach-safe operation: runs continue when TUI exits.
- Engine neutrality: Claude and Codex are interchangeable execution backends.

## Hickey Simplicity Frame
Apply Rich Hickey's "simple != easy" framing by removing complected concerns:

- One branch, one writer, many thinkers:
  - all agents can think in parallel
  - only monitor agent can apply writes
- One protocol for cross-agent communication:
  - named mailbox messages, durable and replayable
- One verification contract:
  - mandatory baseline self-verification evidence before apply
- One source of truth:
  - append-only event log + SQLite state
- One control center:
  - all blockers, approvals, verify evidence, and apply queue in a single TUI workspace

Complexity removed (explicitly out of V1):
- per-agent worktrees
- multiple write heads
- ad-hoc cross-agent chat transport
- optional/untracked verification before branch apply

## High-Level Architecture

```text
┌─────────────────────────────┐
│      spoq TUI (client)      │
│ loop dashboards + controls  │
└──────────────┬──────────────┘
               │ HTTP + SSE (local)
┌──────────────v──────────────┐
│     spoqd orchestrator      │
│ scheduler + DAG executor    │
│ approvals + policy + store  │
└───────┬───────────┬─────────┘
        │           │
┌───────v──────┐ ┌──v───────────────────┐
│Engine Adapters│ │ Sandbox Adapter      │
│ claude/codex  │ │ Gondolin micro-VM    │
└───────────────┘ └──────────────────────┘
```

## OpenCode-Inspired Shape (Rust Adaptation)
Adopt these OpenCode-style ideas in Rust:
- Client/server split (TUI as client, runtime as orchestrator).
- Agent modes and sub-agent workflows.
- Session + event stream semantics.

Do not copy implementation details directly; implement contracts in Rust and align with existing SPOQ app/module style.

## Gondolin Sandbox Integration
Gondolin is used as execution sandbox for autonomous nodes:
- Micro-VM execution for bash/tool steps.
- Host/path/network allowlist enforcement.
- Secret-to-host binding for egress-limited credentials.
- Policy level:
  - `sandbox_required` (default autonomous mode).
  - `host_allowed` (explicit override for trusted operations).

## Runtime Topology

### 1) `spoqd` Local Daemon (new binary/crate)
Responsibilities:
- Loop lifecycle management.
- DAG scheduling and execution.
- Approval gate management.
- Event fanout via SSE.
- Durable state persistence and replay.

### 2) TUI Client Extension (existing app)
Responsibilities:
- Display loop progress, DAG state, approvals, event stream.
- Send user intents (start/pause/resume/approve/deny/cancel).
- Provide integrated shell interpreter panel (through orchestrator APIs).

### 3) Engine Workers
Adapters:
- `ClaudeCodeEngine`: wraps local `claude` execution.
- `CodexEngine`: wraps local `codex` execution.

Contract:
- Stream stdout/stderr/status as structured events.
- Honor cancellation and timeouts.
- Emit recoverable vs fatal failure classification.

### 4) Swarm Executor
Fully programmable DAG workflow execution:
- Arbitrary nodes and dependency edges.
- Parallel branches with configurable concurrency caps.
- Child team nodes (`subteam`) spawn nested DAG scopes.

### 5) Team Coordination + Branch Control
Team agents coordinate through one message protocol and one write gate:
- Named mailbox bus (durable):
  - `task`
  - `update`
  - `blocker`
  - `request_review`
  - `review_result`
  - `artifact_ref`
  - `verify_result`
- Monitor agent:
  - owns the single write token for the shared branch
  - validates verification evidence
  - applies patch intents in strict serial order
- Worker agents:
  - never write directly to git
  - submit patch intents + evidence bundles

### 6) Shell Interpreter Service (PTY-primary)
The orchestrator exposes a shell interpreter subsystem used by both humans and agents:
- Primary backend: native PTY supervisor managed by orchestrator.
- Secondary backend: tmux adapter fallback when needed.
- Session model:
  - `shell_session_id`
  - working directory
  - command history
  - bounded output ring buffer
- Agent nodes may execute commands in:
  - isolated one-shot sandbox shell
  - persistent task shell session (policy-controlled).

## Public Interfaces (New)

### Core IDs
- `LoopId`, `RunId`, `NodeId`, `TeamId`, `ApprovalId`, `SessionId` (UUID newtypes).
- `PatchIntentId`, `VerificationEvidenceId`, `MessageId`.

### LoopSpec
- `name: String`
- `goal: String`
- `working_dir: PathBuf`
- `engine_policy: EnginePolicy`
- `sandbox_policy: SandboxPolicy`
- `approval_policy: ApprovalPolicy`
- `dag: DagSpec`
- `context_files: Vec<PathBuf>`

### DagSpec
- `nodes: Vec<NodeSpec>`
- `edges: Vec<(NodeId, NodeId)>`
- Validation:
  - graph must be acyclic.
  - all dependencies must resolve.
  - no orphan node without execution path from root.

### NodeSpec
- `id: NodeId`
- `kind: NodeKind` (`plan|implement|review|shell|custom_tool|subteam`)
- `agent_profile: AgentProfile`
- `engine_hint: Option<EngineKind>`
- `deps: Vec<NodeId>`
- `inputs: serde_json::Value`
- `retry: RetryPolicy`
- `timeout_secs: u64`
- `requires_approval: bool` (policy may elevate this to true).

### EventEnvelope
- `event_id: String`
- `seq: u64` (monotonic per run)
- `ts: DateTime<Utc>`
- `loop_id: LoopId`
- `run_id: RunId`
- `node_id: Option<NodeId>`
- `kind: EventKind`
- `payload: serde_json::Value`

Key event kinds:
- `loop.created`
- `run.started`
- `node.runnable`
- `node.running`
- `node.output.chunk`
- `approval.requested`
- `approval.granted`
- `approval.denied`
- `engine.switched`
- `sandbox.denied`
- `run.paused`
- `run.resumed`
- `run.completed`
- `run.failed`
- `mailbox.message.sent`
- `patch.submitted`
- `patch.validated`
- `patch.applied`
- `verify.baseline.passed`
- `verify.baseline.failed`

### PatchIntent
- `intent_id: PatchIntentId`
- `run_id: RunId`
- `node_id: NodeId`
- `agent_id: String`
- `base_commit: String`
- `touched_files: Vec<PathBuf>`
- `diff: String` (or artifact reference)
- `verification_evidence_id: VerificationEvidenceId`

### VerificationEvidence
- `evidence_id: VerificationEvidenceId`
- `run_id: RunId`
- `node_id: NodeId`
- `agent_id: String`
- `checks: Vec<CheckResult>`
- `artifacts: Vec<ArtifactRef>`

Mandatory baseline checks:
- `format_check`
- `lint_check`
- `type_check`
- `test_targeted`
- `policy_check`

## Local API Contract (Daemon)

### Command endpoints (HTTP)
- `POST /v1/loops`
- `POST /v1/loops/{loop_id}/runs`
- `POST /v1/runs/{run_id}/pause`
- `POST /v1/runs/{run_id}/resume`
- `POST /v1/runs/{run_id}/cancel`
- `POST /v1/approvals/{approval_id}/grant`
- `POST /v1/approvals/{approval_id}/deny`
- `POST /v1/mailbox/send`
- `POST /v1/patch-intents`
- `POST /v1/patch-intents/{intent_id}/validate`
- `POST /v1/patch-intents/{intent_id}/apply` (monitor only)

### Query endpoints
- `GET /v1/runs/{run_id}`
- `GET /v1/loops/{loop_id}`
- `GET /v1/health`

### Streaming
- `GET /v1/runs/{run_id}/events` (SSE, ordered by `seq`)
- `GET /v1/runs/{run_id}/mailbox/events` (SSE, ordered by `seq`)

## State Model

### Run state machine
```text
created -> starting -> running -> paused -> running
running -> completed
running -> failed
running -> cancelled
running -> blocked_approval -> running
```

### Node state machine
```text
pending -> runnable -> running -> succeeded
running -> failed
running -> blocked_approval -> running
running -> cancelled
```

## Persistence
- SQLite as canonical local database.
- Append-only event log table for replay and audit.
- Required tables:
  - `loops`
  - `runs`
  - `nodes`
  - `events`
  - `approvals`
  - `policies`
- Migrations are versioned and run at daemon startup.

## Safety and Policy Defaults
- Plan-first gating is required.
- Autonomous execution defaults to sandbox.
- Command/path/host allowlists apply to all non-read-only actions.
- Dangerous operations trigger approval unless explicitly allowlisted.
- Policy violations return deny events (no silent fallback).
- Single-branch mode is default.
- Only monitor agent can apply branch writes.
- Baseline self-verification evidence is mandatory for every patch apply.

## TUI UX Requirements (Clarity-First)
The interface must optimize for lower cognitive switching:
- Always show a single “next best action.”
- Show why a run is blocked in plain language.
- Keep event stream visible but secondary to decisions.
- Keep DAG and approvals in one navigable workspace.
- Surface shell session status, mailbox blockers, verification evidence, and apply queue without leaving loop context.

Reference wireframe:

```text
┌────────────────────────────────────────────────────────────┐
│ LOOP: auth-refactor      STATUS: RUNNING      ELAPSED: 42m │
├───────────────────────┬────────────────────────────────────┤
│ Next Action           │ DAG / Team Graph                  │
│ - Approve N7          │ N1 -> N3 -> N7 (blocked)          │
│ - Reason: write+bash  │  \-> N4 -> N8 (running)           │
├───────────────────────┼────────────────────────────────────┤
│ Cognitive Load        │ Event Stream                       │
│ - open blockers: 2    │ node.output.chunk ...              │
│ - pending approvals:1 │ approval.requested ...             │
├───────────────────────┴────────────────────────────────────┤
│ [A]pprove [D]eny [P]ause [R]esume [S]hell [T]eam [Q]uit   │
└────────────────────────────────────────────────────────────┘
```

## Server-Ready Modularity Plan
Introduce interfaces now so local daemon can be swapped later:
- `OrchestratorApi` trait for TUI client.
- `EngineExecutor` trait for engine adapters.
- `SandboxExecutor` trait for sandbox adapters.
- `EventStore` trait for persistence backend.

Migration path:
- V1: TUI -> local `spoqd`.
- V2: TUI can target remote orchestrator with same API contract.
- No TUI business logic rewrite required.

## Implementation Phases

### Phase 0: Scaffolding
- Add new crates/modules:
  - `crates/spoq-orchestrator`
  - `crates/spoq-engines`
  - `crates/spoq-sandbox`
  - shared core types module.

### Phase 1: Daemon + Store
- Boot daemon, config, health endpoint.
- SQLite schema + event append path.
- Basic loop/run CRUD and SSE.

### Phase 2: Engine Adapters
- Implement Claude and Codex process adapters.
- Streaming, cancel, timeout, retry semantics.
- Engine fallback policy support.

### Phase 3: DAG Executor + Swarms
- DAG validation and scheduler.
- Parallel branch execution.
- Subteam/nested DAG execution.

### Phase 4: Gondolin Sandbox
- VM lifecycle integration.
- Egress + secret policy controls.
- Sandbox deny/allow events.

### Phase 5: TUI Integration
- Add orchestrator client.
- Add loop dashboard, approvals queue, DAG pane.
- Add shell interpreter panel routed through orchestrator.

### Phase 6: Remote-Ready Hardening
- Enforce transport-neutral client traits.
- Add compatibility tests for local vs remote API behavior.

## Testing Plan

### Unit tests
- DAG cycle detection and topological ordering.
- Node/run state machine transitions.
- Policy evaluation (allow/ask/deny).
- Event sequence monotonicity and replay determinism.
- Engine adapter parsing and cancellation behavior.

### Integration tests
- Daemon startup + persistence migration.
- Full loop run with Claude adapter.
- Full loop run with Codex adapter.
- Sandbox deny/approval/continue path.
- Pause/resume across daemon restart.

### End-to-end scenarios
1. Plan-first autonomous flow with approval checkpoint.
2. Parallel DAG branches with one failing node and retry policy.
3. TUI disconnect/reconnect while run continues.
4. Mixed engine workflow: Claude node then Codex node fallback.
5. PTY-backed shell session survives TUI restart and resumes with history intact.
6. Worker patch intent rejected when mandatory baseline verification is missing.
7. Concurrent worker outputs are serialized by monitor with no concurrent writes.

## Acceptance Criteria
- User can create and run a loop from TUI and see live progress.
- Runs continue if TUI exits and resume on reconnect.
- Claude and Codex execute through same orchestrator contracts.
- DAG swarms execute with dependencies and parallelism.
- Sandbox and allowlists are enforced with visible policy events.
- Event history is queryable and replayable from local store.

## Risks
- CLI behavior drift in `claude`/`codex` output formats.
- Overly strict allowlists can stall workflows.
- DAG complexity may increase debugging burden without good observability.
- Sandbox startup latency can impact UX if no pooling strategy is used.

## Mitigations
- Normalize adapter output via robust parser + fallback event envelopes.
- Provide policy diagnostics and recommended remediations in TUI.
- Include run explainability view (why node blocked/failed/retried).
- Add configurable sandbox pool for warm VM reuse.

## Assumptions and Defaults
- Build target: current repo (`spoq-local`).
- Runtime mode: local daemon first.
- Store: SQLite + append-only event model.
- Safety: plan-first + allowlist + sandbox-default for autonomous nodes.
- Swarms: fully programmable DAG workflows in V1.
- Git model: one shared branch in V1; no git worktrees.
- Write model: monitor-only writer; workers submit patch intents.
- Visual reference: functional clarity-first UI now; high-fidelity styling after PNG/JPG is supplied.

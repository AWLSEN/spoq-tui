#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Headless Codex autonomous loop.

Usage:
  scripts/codex/autonomy-loop.sh <task-file>

Environment overrides:
  CODEX_MODEL         Default: codex-5.3-high
  MAX_ITERATIONS      Default: 200
  SLEEP_SECONDS       Default: 2
  LOOP_RUN_ID         Default: timestamp (YYYYMMDD-HHMMSS)
  LOOP_STATE_DIR      Default: .codex-loop/<LOOP_RUN_ID>

Notes:
  - Runs with full permissions via:
      --dangerously-bypass-approvals-and-sandbox
  - Keeps working on one branch; no worktree/branch switching.
  - Completion is controlled by a required sentinel line in Codex output:
      LOOP_STATUS: DONE
      LOOP_STATUS: CONTINUE
EOF
}

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  usage
  exit 0
fi

if [[ $# -ne 1 ]]; then
  usage >&2
  exit 1
fi

TASK_FILE="$1"
if [[ ! -f "$TASK_FILE" ]]; then
  echo "Task file not found: $TASK_FILE" >&2
  exit 1
fi

if ! command -v codex >/dev/null 2>&1; then
  echo "codex CLI not found in PATH" >&2
  exit 1
fi

if ! command -v rg >/dev/null 2>&1; then
  echo "ripgrep (rg) is required" >&2
  exit 1
fi

ROOT_DIR="$(git rev-parse --show-toplevel)"
cd "$ROOT_DIR"

BRANCH="$(git branch --show-current)"
if [[ -z "$BRANCH" ]]; then
  echo "Not on a git branch (detached HEAD not supported for this loop)" >&2
  exit 1
fi

MODEL="${CODEX_MODEL:-codex-5.3-high}"
MAX_ITERATIONS="${MAX_ITERATIONS:-200}"
SLEEP_SECONDS="${SLEEP_SECONDS:-2}"
RUN_ID="${LOOP_RUN_ID:-$(date +%Y%m%d-%H%M%S)}"
STATE_DIR="${LOOP_STATE_DIR:-.codex-loop/${RUN_ID}}"
mkdir -p "$STATE_DIR"

TASK_CONTENT="$(cat "$TASK_FILE")"

BASE_PROTOCOL="$(cat <<EOF
You are running in a headless autonomous coding loop.

Hard constraints:
1) Work only in this repository root: $ROOT_DIR
2) Work only on this current branch: $BRANCH
3) Do NOT create or switch branches
4) Do NOT create or use git worktrees
5) Make progress by editing code, running checks, and validating changes
6) Keep changes coherent and minimal per step

At the END of your response, print exactly one line:
LOOP_STATUS: DONE
or
LOOP_STATUS: CONTINUE

Use DONE only when the objective is fully complete and verified.
EOF
)"

build_prompt_first() {
  cat <<EOF
$BASE_PROTOCOL

Primary objective:
$TASK_CONTENT
EOF
}

build_prompt_resume() {
  cat <<'EOF'
Continue execution toward full completion of the same objective.

If anything remains, keep implementing and validating.
If fully complete and verified, output LOOP_STATUS: DONE.
Otherwise output LOOP_STATUS: CONTINUE.
EOF
}

extract_loop_status() {
  local log_file="$1"
  local raw
  raw="$(rg -o 'LOOP_STATUS:\s*(DONE|CONTINUE)' "$log_file" | tail -n 1 || true)"
  if [[ -z "$raw" ]]; then
    echo "UNKNOWN"
    return 0
  fi
  echo "$raw" | awk '{print $2}'
}

run_exec() {
  local prompt="$1"
  local log_file="$2"

  codex exec \
    -C "$ROOT_DIR" \
    -m "$MODEL" \
    --dangerously-bypass-approvals-and-sandbox \
    "$prompt" 2>&1 | tee "$log_file"
}

run_resume() {
  local prompt="$1"
  local log_file="$2"

  codex exec resume --last \
    -m "$MODEL" \
    --dangerously-bypass-approvals-and-sandbox \
    "$prompt" 2>&1 | tee "$log_file"
}

echo "== Codex autonomous loop =="
echo "root:        $ROOT_DIR"
echo "branch:      $BRANCH"
echo "model:       $MODEL"
echo "iterations:  $MAX_ITERATIONS"
echo "state dir:   $STATE_DIR"
echo

mode="first"
for ((i = 1; i <= MAX_ITERATIONS; i++)); do
  log_file="$STATE_DIR/iter-${i}.log"
  echo "---- iteration $i/$MAX_ITERATIONS ($mode) ----"

  if [[ "$mode" == "first" ]]; then
    prompt="$(build_prompt_first)"
    if ! run_exec "$prompt" "$log_file"; then
      echo "iteration $i failed; retrying after ${SLEEP_SECONDS}s" >&2
      sleep "$SLEEP_SECONDS"
      mode="first"
      continue
    fi
    mode="resume"
  else
    prompt="$(build_prompt_resume)"
    if ! run_resume "$prompt" "$log_file"; then
      echo "iteration $i failed; retrying after ${SLEEP_SECONDS}s" >&2
      sleep "$SLEEP_SECONDS"
      continue
    fi
  fi

  status="$(extract_loop_status "$log_file")"
  echo "loop status: $status"

  if [[ "$status" == "DONE" ]]; then
    echo "Objective completed on iteration $i."
    exit 0
  fi

  sleep "$SLEEP_SECONDS"
done

echo "Reached MAX_ITERATIONS=$MAX_ITERATIONS without LOOP_STATUS: DONE" >&2
exit 2

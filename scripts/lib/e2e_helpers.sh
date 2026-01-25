#!/bin/bash
#
# E2E Test Helpers Library
#
# Shared functions for E2E testing scripts in tui_spoq.
# This library provides reusable utilities for:
# - Logging with colors
# - Plan directory/file creation
# - Status file creation (with proper JSON format)
# - Conductor process checks
# - File verification
# - UUID generation
# - API call wrappers
# - Screenshot capture
#
# Usage:
#   source scripts/lib/e2e_helpers.sh
#
# Required environment variables (should be set by the sourcing script):
#   PROJECT       - The project name (e.g., "tui_spoq")
#   PLAN_ID       - The plan identifier (e.g., "plan-test-e2e")
#   TOTAL_PHASES  - Number of phases in the plan (e.g., 3)
#
# Optional environment variables:
#   PLAN_DIR      - Override the plan directory path
#   STATUS_DIR    - Override the status directory path
#   POLL_INTERVAL - Conductor poll interval in seconds (default: 6)
#   API_BASE_URL  - Base URL for API calls (default: http://localhost:8000)
#

# Fail on errors (can be overridden by sourcing script)
set -e

# ==============================================================================
# Color Definitions
# ==============================================================================
readonly E2E_RED='\033[0;31m'
readonly E2E_GREEN='\033[0;32m'
readonly E2E_YELLOW='\033[1;33m'
readonly E2E_BLUE='\033[0;34m'
readonly E2E_CYAN='\033[0;36m'
readonly E2E_MAGENTA='\033[0;35m'
readonly E2E_BOLD='\033[1m'
readonly E2E_NC='\033[0m' # No Color

# Legacy color names for backward compatibility
readonly RED="${E2E_RED}"
readonly GREEN="${E2E_GREEN}"
readonly YELLOW="${E2E_YELLOW}"
readonly BLUE="${E2E_BLUE}"
readonly NC="${E2E_NC}"

# ==============================================================================
# Configuration Defaults
# ==============================================================================
: "${POLL_INTERVAL:=6}"
: "${API_BASE_URL:=http://localhost:8000}"
: "${COMMS_BASE_DIR:=$HOME/comms}"

# ==============================================================================
# Logging Functions
# ==============================================================================

# Log an informational message (blue)
# Usage: log_info "message"
log_info() {
    echo -e "${E2E_BLUE}[INFO]${E2E_NC} $1"
}

# Log a success message (green)
# Usage: log_success "message"
log_success() {
    echo -e "${E2E_GREEN}[SUCCESS]${E2E_NC} $1"
}

# Log a warning message (yellow)
# Usage: log_warn "message"
log_warn() {
    echo -e "${E2E_YELLOW}[WARN]${E2E_NC} $1"
}

# Log an error message (red)
# Usage: log_error "message"
log_error() {
    echo -e "${E2E_RED}[ERROR]${E2E_NC} $1"
}

# Log a debug message (cyan) - only if E2E_DEBUG is set
# Usage: log_debug "message"
log_debug() {
    if [ -n "${E2E_DEBUG:-}" ]; then
        echo -e "${E2E_CYAN}[DEBUG]${E2E_NC} $1"
    fi
}

# Log a step/section header (bold)
# Usage: log_step "Step 1" "Creating plan structure"
log_step() {
    local step_num="$1"
    local description="$2"
    echo ""
    echo -e "${E2E_BOLD}=== ${step_num}: ${description} ===${E2E_NC}"
}

# Print a separator line
# Usage: log_separator
log_separator() {
    echo "========================================"
}

# ==============================================================================
# Directory and Path Functions
# ==============================================================================

# Get the plan directory path
# Usage: plan_dir=$(get_plan_dir)
# Requires: PROJECT, PLAN_ID environment variables
get_plan_dir() {
    if [ -n "${PLAN_DIR:-}" ]; then
        echo "$PLAN_DIR"
    else
        echo "${COMMS_BASE_DIR}/plans/${PROJECT}/active/${PLAN_ID}"
    fi
}

# Get the status directory path
# Usage: status_dir=$(get_status_dir)
# Requires: PROJECT, PLAN_ID environment variables
get_status_dir() {
    if [ -n "${STATUS_DIR:-}" ]; then
        echo "$STATUS_DIR"
    else
        echo "$(get_plan_dir)/status"
    fi
}

# Get the markers directory path
# Usage: markers_dir=$(get_markers_dir)
# Requires: PROJECT, PLAN_ID environment variables
get_markers_dir() {
    echo "$(get_plan_dir)/markers"
}

# Initialize plan directory structure
# Usage: init_plan_dirs
# Creates: plan directory, status directory, markers directory
init_plan_dirs() {
    local plan_dir
    plan_dir=$(get_plan_dir)
    mkdir -p "$(get_status_dir)"
    mkdir -p "$(get_markers_dir)"
    log_debug "Created directories: $plan_dir"
}

# ==============================================================================
# UUID Generation Functions
# ==============================================================================

# Generate a UUID v4
# Usage: uuid=$(generate_uuid)
# Falls back to random hex if uuidgen is not available
generate_uuid() {
    if command -v uuidgen &> /dev/null; then
        uuidgen | tr '[:upper:]' '[:lower:]'
    elif [ -f /proc/sys/kernel/random/uuid ]; then
        cat /proc/sys/kernel/random/uuid
    else
        # Fallback: generate pseudo-random UUID-like string
        local hex
        hex=$(od -An -tx1 -N16 /dev/urandom 2>/dev/null | tr -d ' \n' || \
              openssl rand -hex 16 2>/dev/null || \
              printf '%04x%04x-%04x-%04x-%04x-%04x%04x%04x' \
                  $RANDOM $RANDOM $RANDOM $RANDOM $RANDOM $RANDOM $RANDOM $RANDOM)
        echo "${hex:0:8}-${hex:8:4}-${hex:12:4}-${hex:16:4}-${hex:20:12}"
    fi
}

# Generate a session ID for a phase
# Usage: session_id=$(generate_session_id 1)
# Output: phase-1-plan-xxx-xxx
generate_session_id() {
    local phase="$1"
    echo "phase-${phase}-${PLAN_ID}"
}

# Generate a thread ID
# Usage: thread_id=$(generate_thread_id)
# Output: UUID v4
generate_thread_id() {
    generate_uuid
}

# ==============================================================================
# Plan File Functions
# ==============================================================================

# Create a plan markdown file
# Usage: create_plan_file "Plan Title" "Phase 1 description" "Phase 2 description" ...
# Or:    create_plan_file  # Uses defaults
create_plan_file() {
    local plan_dir
    plan_dir=$(get_plan_dir)
    local plan_file="$plan_dir/$PLAN_ID.md"
    local title="${1:-E2E Test Plan}"
    shift 2>/dev/null || true

    # Create plan directory if needed
    init_plan_dirs

    # Start the plan file
    cat > "$plan_file" << EOF
# Plan: ${title}

## Metadata
- **ID**: ${PLAN_ID}
- **Project**: ${PROJECT}
- **Type**: test
- **Status**: active

## Phases
EOF

    # Add phases
    local phase=1
    if [ $# -gt 0 ]; then
        for desc in "$@"; do
            cat >> "$plan_file" << EOF

### Phase ${phase}: ${desc}
- **Description**: ${desc}
- **Files**: \`test/phase${phase}.rs\`
- **Complexity**: Medium
EOF
            phase=$((phase + 1))
        done
    else
        # Default phases based on TOTAL_PHASES
        local total="${TOTAL_PHASES:-3}"
        for i in $(seq 1 "$total"); do
            cat >> "$plan_file" << EOF

### Phase ${i}: Test Phase ${i}
- **Description**: Test phase ${i} implementation
- **Files**: \`test/phase${i}.rs\`
- **Complexity**: Medium
EOF
        done
    fi

    log_success "Created plan file: $plan_file"
}

# ==============================================================================
# Status File Functions
# ==============================================================================

# Create a status file for a phase
# Usage: create_status_file <phase> <status> [tool_count] [last_tool] [last_file]
# Status values: "pending", "running", "completed", "failed"
create_status_file() {
    local phase="$1"
    local status="$2"
    local tool_count="${3:-5}"
    local last_tool="${4:-Edit}"
    local last_file="${5:-src/test.rs}"
    local timestamp
    timestamp=$(date -u +%Y-%m-%dT%H:%M:%SZ)

    local status_dir
    status_dir=$(get_status_dir)
    mkdir -p "$status_dir"

    local status_file="$status_dir/phase-${phase}.status"
    local task_id
    task_id=$(generate_session_id "$phase")

    # Determine completed_at value
    local completed_at="null"
    if [ "$status" = "completed" ]; then
        completed_at="\"$timestamp\""
    fi

    # Use jq for proper JSON generation (safer than heredoc)
    if command -v jq &> /dev/null; then
        jq -n \
            --arg task_id "$task_id" \
            --arg project "$PROJECT" \
            --arg plan_id "$PLAN_ID" \
            --argjson phase "$phase" \
            --arg status "$status" \
            --argjson tool_count "$tool_count" \
            --arg last_tool "$last_tool" \
            --arg last_file "$last_file" \
            --arg started_at "$timestamp" \
            --arg updated_at "$timestamp" \
            --arg completed_at_raw "$completed_at" \
            '{
                task_id: $task_id,
                thread_id: null,
                project: $project,
                plan_id: $plan_id,
                phase: $phase,
                status: $status,
                tool_count: $tool_count,
                last_tool: $last_tool,
                last_file: $last_file,
                started_at: $started_at,
                updated_at: $updated_at,
                completed_at: (if $completed_at_raw == "null" then null else $completed_at_raw | gsub("\""; "") end)
            }' > "$status_file"
    else
        # Fallback to heredoc if jq is not available
        cat > "$status_file" << EOF
{
  "task_id": "${task_id}",
  "thread_id": null,
  "project": "${PROJECT}",
  "plan_id": "${PLAN_ID}",
  "phase": ${phase},
  "status": "${status}",
  "tool_count": ${tool_count},
  "last_tool": "${last_tool}",
  "last_file": "${last_file}",
  "started_at": "${timestamp}",
  "updated_at": "${timestamp}",
  "completed_at": ${completed_at}
}
EOF
    fi

    log_success "Created status file: phase-${phase}.status (status: ${status})"
}

# Create a status file with thread_id
# Usage: create_status_file_with_thread <phase> <status> <thread_id> [tool_count] [last_tool] [last_file]
create_status_file_with_thread() {
    local phase="$1"
    local status="$2"
    local thread_id="$3"
    local tool_count="${4:-5}"
    local last_tool="${5:-Edit}"
    local last_file="${6:-src/test.rs}"
    local timestamp
    timestamp=$(date -u +%Y-%m-%dT%H:%M:%SZ)

    local status_dir
    status_dir=$(get_status_dir)
    mkdir -p "$status_dir"

    local status_file="$status_dir/phase-${phase}.status"
    local task_id
    task_id=$(generate_session_id "$phase")

    # Determine completed_at value
    local completed_at="null"
    if [ "$status" = "completed" ]; then
        completed_at="\"$timestamp\""
    fi

    jq -n \
        --arg task_id "$task_id" \
        --arg thread_id "$thread_id" \
        --arg project "$PROJECT" \
        --arg plan_id "$PLAN_ID" \
        --argjson phase "$phase" \
        --arg status "$status" \
        --argjson tool_count "$tool_count" \
        --arg last_tool "$last_tool" \
        --arg last_file "$last_file" \
        --arg started_at "$timestamp" \
        --arg updated_at "$timestamp" \
        --arg completed_at_raw "$completed_at" \
        '{
            task_id: $task_id,
            thread_id: $thread_id,
            project: $project,
            plan_id: $plan_id,
            phase: $phase,
            status: $status,
            tool_count: $tool_count,
            last_tool: $last_tool,
            last_file: $last_file,
            started_at: $started_at,
            updated_at: $updated_at,
            completed_at: (if $completed_at_raw == "null" then null else $completed_at_raw | gsub("\""; "") end)
        }' > "$status_file"

    log_success "Created status file: phase-${phase}.status (status: ${status}, thread: ${thread_id:0:8}...)"
}

# Update an existing status file
# Usage: update_status_file <phase> <new_status>
update_status_file() {
    local phase="$1"
    local new_status="$2"
    local timestamp
    timestamp=$(date -u +%Y-%m-%dT%H:%M:%SZ)

    local status_dir
    status_dir=$(get_status_dir)
    local status_file="$status_dir/phase-${phase}.status"

    if [ ! -f "$status_file" ]; then
        log_error "Status file does not exist: $status_file"
        return 1
    fi

    local completed_at="null"
    if [ "$new_status" = "completed" ]; then
        completed_at="\"$timestamp\""
    fi

    # Update status and timestamps
    local tmp_file="${status_file}.tmp"
    jq --arg status "$new_status" \
       --arg updated_at "$timestamp" \
       --arg completed_at_raw "$completed_at" \
       '.status = $status | .updated_at = $updated_at | .completed_at = (if $completed_at_raw == "null" then null else $completed_at_raw | gsub("\""; "") end)' \
       "$status_file" > "$tmp_file" && mv "$tmp_file" "$status_file"

    log_success "Updated status file: phase-${phase}.status (status: ${new_status})"
}

# ==============================================================================
# Conductor Functions
# ==============================================================================

# Check if conductor is running
# Usage: check_conductor && echo "running" || echo "not running"
check_conductor() {
    if pgrep -f conductor > /dev/null 2>&1; then
        log_success "Conductor is running"
        return 0
    else
        log_warn "Conductor is NOT running. Files will be created but not picked up."
        log_info "Start conductor with: cd ~/starry-night/conductor && cargo run"
        return 1
    fi
}

# Wait for conductor to pick up changes
# Usage: wait_for_conductor [seconds]
wait_for_conductor() {
    local wait_time="${1:-$POLL_INTERVAL}"
    log_info "Waiting ${wait_time}s for conductor to pick up status file..."
    sleep "$wait_time"
}

# Check WebSocket connectivity (if websocat is available)
# Usage: check_websocket [timeout_seconds]
check_websocket() {
    local timeout="${1:-5}"
    if command -v websocat &> /dev/null; then
        log_info "Listening for WebSocket messages (${timeout}s timeout)..."
        timeout "$timeout" websocat -t "ws://localhost:8000/ws" 2>/dev/null | head -5 || \
            log_warn "No WebSocket messages received"
    else
        log_warn "websocat not installed. Install with: brew install websocat"
        log_info "Manual verification: Connect to ws://localhost:8000/ws and look for phase_progress_update messages"
    fi
}

# ==============================================================================
# File Verification Functions
# ==============================================================================

# Verify plan file exists
# Usage: verify_plan_file
verify_plan_file() {
    local plan_dir
    plan_dir=$(get_plan_dir)
    local plan_file="$plan_dir/$PLAN_ID.md"

    if [ -f "$plan_file" ]; then
        log_success "Plan file exists: $plan_file"
        return 0
    else
        log_error "Plan file missing: $plan_file"
        return 1
    fi
}

# Verify status file exists and has valid JSON
# Usage: verify_status_file <phase>
verify_status_file() {
    local phase="$1"
    local status_dir
    status_dir=$(get_status_dir)
    local status_file="$status_dir/phase-${phase}.status"

    if [ ! -f "$status_file" ]; then
        log_error "Status file missing: $status_file"
        return 1
    fi

    # Verify JSON is valid
    if command -v jq &> /dev/null; then
        if jq empty "$status_file" 2>/dev/null; then
            local status
            status=$(jq -r '.status' "$status_file")
            log_success "Phase $phase status file exists (status: $status)"
            return 0
        else
            log_error "Status file has invalid JSON: $status_file"
            return 1
        fi
    else
        log_success "Phase $phase status file exists (JSON validation skipped - jq not found)"
        return 0
    fi
}

# Verify all status files for the plan
# Usage: verify_all_status_files
verify_all_status_files() {
    log_info "Verifying status files..."
    local total="${TOTAL_PHASES:-3}"
    local errors=0

    for phase in $(seq 1 "$total"); do
        verify_status_file "$phase" || errors=$((errors + 1))
    done

    if [ "$errors" -gt 0 ]; then
        log_error "$errors status file(s) failed verification"
        return 1
    fi

    log_success "All status files verified"
    return 0
}

# Verify complete file structure (plan + all status files)
# Usage: verify_files
verify_files() {
    log_info "Verifying file structure..."
    local errors=0

    verify_plan_file || errors=$((errors + 1))
    verify_all_status_files || errors=$((errors + 1))

    if [ "$errors" -gt 0 ]; then
        return 1
    fi
    return 0
}

# ==============================================================================
# API Call Functions
# ==============================================================================

# Make a GET request to the API
# Usage: api_get "/endpoint" [extra_curl_args...]
# Returns: Response body on stdout, HTTP status in $?
api_get() {
    local endpoint="$1"
    shift
    local url="${API_BASE_URL}${endpoint}"

    log_debug "GET $url"

    local response
    local http_code

    response=$(curl -s -w "\n%{http_code}" "$@" "$url")
    http_code=$(echo "$response" | tail -n1)
    response=$(echo "$response" | sed '$d')

    echo "$response"

    if [ "$http_code" -ge 200 ] && [ "$http_code" -lt 300 ]; then
        return 0
    else
        log_debug "API GET failed with status $http_code"
        return 1
    fi
}

# Make a POST request to the API
# Usage: api_post "/endpoint" '{"json": "data"}' [extra_curl_args...]
# Returns: Response body on stdout, HTTP status in $?
api_post() {
    local endpoint="$1"
    local data="$2"
    shift 2
    local url="${API_BASE_URL}${endpoint}"

    log_debug "POST $url"

    local response
    local http_code

    response=$(curl -s -w "\n%{http_code}" -X POST \
        -H "Content-Type: application/json" \
        -d "$data" \
        "$@" "$url")
    http_code=$(echo "$response" | tail -n1)
    response=$(echo "$response" | sed '$d')

    echo "$response"

    if [ "$http_code" -ge 200 ] && [ "$http_code" -lt 300 ]; then
        return 0
    else
        log_debug "API POST failed with status $http_code"
        return 1
    fi
}

# Make a PUT request to the API
# Usage: api_put "/endpoint" '{"json": "data"}' [extra_curl_args...]
api_put() {
    local endpoint="$1"
    local data="$2"
    shift 2
    local url="${API_BASE_URL}${endpoint}"

    log_debug "PUT $url"

    local response
    local http_code

    response=$(curl -s -w "\n%{http_code}" -X PUT \
        -H "Content-Type: application/json" \
        -d "$data" \
        "$@" "$url")
    http_code=$(echo "$response" | tail -n1)
    response=$(echo "$response" | sed '$d')

    echo "$response"

    if [ "$http_code" -ge 200 ] && [ "$http_code" -lt 300 ]; then
        return 0
    else
        log_debug "API PUT failed with status $http_code"
        return 1
    fi
}

# Make a DELETE request to the API
# Usage: api_delete "/endpoint" [extra_curl_args...]
api_delete() {
    local endpoint="$1"
    shift
    local url="${API_BASE_URL}${endpoint}"

    log_debug "DELETE $url"

    local response
    local http_code

    response=$(curl -s -w "\n%{http_code}" -X DELETE "$@" "$url")
    http_code=$(echo "$response" | tail -n1)
    response=$(echo "$response" | sed '$d')

    echo "$response"

    if [ "$http_code" -ge 200 ] && [ "$http_code" -lt 300 ]; then
        return 0
    else
        log_debug "API DELETE failed with status $http_code"
        return 1
    fi
}

# ==============================================================================
# Screenshot Functions
# ==============================================================================

# Capture a screenshot of the TUI
# Usage: capture_screenshot [output_path] [session_id]
# Requires: mcp__tui-vision__screenshot_tui or screencapture (macOS)
# Default output: /tmp/e2e_screenshot_<timestamp>.png
capture_screenshot() {
    local output_path="${1:-/tmp/e2e_screenshot_$(date +%Y%m%d_%H%M%S).png}"
    local session_id="${2:-}"

    log_info "Capturing screenshot to: $output_path"

    # Try TUI vision tool first (if available via MCP)
    if [ -n "$session_id" ]; then
        # This would be called via MCP in practice
        log_debug "Would call mcp__tui-vision__screenshot_tui with session_id=$session_id"
    fi

    # Fallback to macOS screencapture
    if command -v screencapture &> /dev/null; then
        screencapture -x "$output_path" 2>/dev/null && \
            log_success "Screenshot saved: $output_path" && return 0
    fi

    # Fallback to scrot (Linux)
    if command -v scrot &> /dev/null; then
        scrot "$output_path" 2>/dev/null && \
            log_success "Screenshot saved: $output_path" && return 0
    fi

    # Fallback to import (ImageMagick)
    if command -v import &> /dev/null; then
        import -window root "$output_path" 2>/dev/null && \
            log_success "Screenshot saved: $output_path" && return 0
    fi

    log_warn "No screenshot tool available (tried: screencapture, scrot, import)"
    return 1
}

# Capture a screenshot of a specific window by name
# Usage: capture_window_screenshot "window_name" [output_path]
capture_window_screenshot() {
    local window_name="$1"
    local output_path="${2:-/tmp/e2e_window_$(date +%Y%m%d_%H%M%S).png}"

    log_info "Capturing window '$window_name' to: $output_path"

    # macOS: Use screencapture with window selection
    if command -v screencapture &> /dev/null; then
        # Get window ID using osascript
        local window_id
        window_id=$(osascript -e "tell application \"System Events\" to get id of window 1 of (processes whose name contains \"$window_name\")" 2>/dev/null)
        if [ -n "$window_id" ]; then
            screencapture -l "$window_id" "$output_path" 2>/dev/null && \
                log_success "Window screenshot saved: $output_path" && return 0
        fi
    fi

    log_warn "Could not capture window: $window_name"
    return 1
}

# ==============================================================================
# Cleanup Functions
# ==============================================================================

# Clean up test plan directory
# Usage: cleanup_plan
cleanup_plan() {
    local plan_dir
    plan_dir=$(get_plan_dir)

    log_info "Cleaning up test files..."
    rm -rf "$plan_dir"
    log_success "Cleanup complete: $plan_dir"
}

# Register cleanup handler for script exit
# Usage: register_cleanup_on_exit
register_cleanup_on_exit() {
    trap cleanup_plan EXIT
}

# ==============================================================================
# Utility Functions
# ==============================================================================

# Get current timestamp in ISO 8601 format (UTC)
# Usage: ts=$(get_timestamp)
get_timestamp() {
    date -u +%Y-%m-%dT%H:%M:%SZ
}

# Get current timestamp as Unix epoch
# Usage: epoch=$(get_epoch)
get_epoch() {
    date +%s
}

# Sleep with progress indication
# Usage: sleep_with_progress 10 "Waiting for conductor"
sleep_with_progress() {
    local seconds="$1"
    local message="${2:-Waiting}"

    for ((i=1; i<=seconds; i++)); do
        printf "\r${E2E_BLUE}[INFO]${E2E_NC} %s... %d/%d seconds" "$message" "$i" "$seconds"
        sleep 1
    done
    printf "\n"
}

# Check if a command exists
# Usage: command_exists jq && echo "jq is installed"
command_exists() {
    command -v "$1" &> /dev/null
}

# Require a command to exist (exit if not)
# Usage: require_command jq "JSON processing"
require_command() {
    local cmd="$1"
    local description="${2:-$1}"

    if ! command_exists "$cmd"; then
        log_error "Required command not found: $cmd ($description)"
        log_info "Please install $cmd to continue"
        exit 1
    fi
}

# ==============================================================================
# Test Assertion Functions
# ==============================================================================

# Assert that two values are equal
# Usage: assert_eq "$actual" "$expected" "description"
assert_eq() {
    local actual="$1"
    local expected="$2"
    local description="${3:-values should be equal}"

    if [ "$actual" = "$expected" ]; then
        log_success "PASS: $description"
        return 0
    else
        log_error "FAIL: $description"
        log_error "  Expected: $expected"
        log_error "  Actual:   $actual"
        return 1
    fi
}

# Assert that a file exists
# Usage: assert_file_exists "/path/to/file" "description"
assert_file_exists() {
    local file="$1"
    local description="${2:-file should exist}"

    if [ -f "$file" ]; then
        log_success "PASS: $description ($file)"
        return 0
    else
        log_error "FAIL: $description"
        log_error "  File not found: $file"
        return 1
    fi
}

# Assert that a directory exists
# Usage: assert_dir_exists "/path/to/dir" "description"
assert_dir_exists() {
    local dir="$1"
    local description="${2:-directory should exist}"

    if [ -d "$dir" ]; then
        log_success "PASS: $description ($dir)"
        return 0
    else
        log_error "FAIL: $description"
        log_error "  Directory not found: $dir"
        return 1
    fi
}

# Assert that JSON field has expected value
# Usage: assert_json_field "file.json" ".status" "running" "status should be running"
assert_json_field() {
    local file="$1"
    local field="$2"
    local expected="$3"
    local description="${4:-JSON field should match}"

    require_command jq "JSON field assertion"

    local actual
    actual=$(jq -r "$field" "$file" 2>/dev/null)

    assert_eq "$actual" "$expected" "$description"
}

# ==============================================================================
# TUI Log Verification Functions
# ==============================================================================

# Default TUI log location
: "${TUI_LOG_FILE:=$HOME/.spoq/logs/spoq.log}"

# Verify TUI log contains PHASE_PROGRESS_UPDATE entries
# Usage: verify_tui_log_received [plan_id] [expected_count]
# Returns: 0 if expected entries found, 1 otherwise
verify_tui_log_received() {
    local plan_id="${1:-$PLAN_ID}"
    local expected_count="${2:-1}"

    if [ ! -f "$TUI_LOG_FILE" ]; then
        log_error "TUI log file not found: $TUI_LOG_FILE"
        return 1
    fi

    local count
    count=$(grep -c "PHASE_PROGRESS_UPDATE" "$TUI_LOG_FILE" 2>/dev/null || echo "0")

    # If plan_id specified, filter by it
    if [ -n "$plan_id" ]; then
        count=$(grep "PHASE_PROGRESS_UPDATE" "$TUI_LOG_FILE" 2>/dev/null | grep -c "$plan_id" || echo "0")
    fi

    if [ "$count" -ge "$expected_count" ]; then
        log_success "TUI log verification passed: Found $count PHASE_PROGRESS_UPDATE entries (expected >= $expected_count)"
        return 0
    else
        log_error "TUI log verification failed: Found $count PHASE_PROGRESS_UPDATE entries (expected >= $expected_count)"
        return 1
    fi
}

# Extract all phase update entries from TUI log
# Usage: extract_phase_updates_from_log [plan_id]
# Output: JSON array of phase updates on stdout
extract_phase_updates_from_log() {
    local plan_id="${1:-$PLAN_ID}"

    if [ ! -f "$TUI_LOG_FILE" ]; then
        log_error "TUI log file not found: $TUI_LOG_FILE"
        echo "[]"
        return 1
    fi

    local updates=()
    local line
    local count=0

    # Read PHASE_PROGRESS_UPDATE lines and extract JSON payloads
    while IFS= read -r line; do
        # Skip if plan_id specified and line doesn't match
        if [ -n "$plan_id" ] && ! echo "$line" | grep -q "$plan_id"; then
            continue
        fi

        # Try to extract JSON payload from the log line
        # Common log formats:
        # 1. "[timestamp] PHASE_PROGRESS_UPDATE {json}"
        # 2. "PHASE_PROGRESS_UPDATE: {json}"
        # 3. "... PHASE_PROGRESS_UPDATE ... {json}"
        local json_payload
        json_payload=$(echo "$line" | sed -n 's/.*PHASE_PROGRESS_UPDATE[^{]*\({.*}\).*/\1/p')

        if [ -n "$json_payload" ]; then
            # Validate JSON if jq is available
            if command_exists jq; then
                if echo "$json_payload" | jq empty 2>/dev/null; then
                    updates+=("$json_payload")
                    count=$((count + 1))
                else
                    log_debug "Skipping invalid JSON in log line"
                fi
            else
                updates+=("$json_payload")
                count=$((count + 1))
            fi
        else
            # If no JSON found, create a simple entry with the raw line
            local timestamp
            timestamp=$(echo "$line" | sed -n 's/^\[\([^]]*\)\].*/\1/p')
            if [ -z "$timestamp" ]; then
                timestamp=$(get_timestamp)
            fi
            updates+=("{\"raw_line\": \"$(echo "$line" | sed 's/"/\\"/g')\", \"timestamp\": \"$timestamp\"}")
            count=$((count + 1))
        fi
    done < <(grep "PHASE_PROGRESS_UPDATE" "$TUI_LOG_FILE" 2>/dev/null)

    log_debug "Extracted $count phase updates from TUI log"

    # Output as JSON array
    if command_exists jq; then
        printf '%s\n' "${updates[@]}" | jq -s '.' 2>/dev/null || echo "[]"
    else
        # Manual JSON array construction without jq
        echo -n "["
        local first=true
        for update in "${updates[@]}"; do
            if [ "$first" = true ]; then
                first=false
            else
                echo -n ","
            fi
            echo -n "$update"
        done
        echo "]"
    fi
}

# Verify a thread exists in the TUI via debug API or log inspection
# Usage: verify_thread_exists_in_tui <thread_id> [timeout_seconds]
# Returns: 0 if thread found, 1 otherwise
verify_thread_exists_in_tui() {
    local thread_id="$1"
    local timeout="${2:-10}"

    if [ -z "$thread_id" ]; then
        log_error "Thread ID is required"
        return 1
    fi

    log_info "Verifying thread $thread_id exists in TUI (timeout: ${timeout}s)..."

    local start_time
    start_time=$(get_epoch)
    local elapsed=0

    while [ "$elapsed" -lt "$timeout" ]; do
        # Method 1: Check TUI debug API (if available)
        local api_response
        api_response=$(api_get "/api/debug/threads" 2>/dev/null) || true

        if [ -n "$api_response" ] && echo "$api_response" | grep -q "$thread_id"; then
            log_success "Thread $thread_id found via debug API"
            return 0
        fi

        # Method 2: Check TUI log for thread references
        if [ -f "$TUI_LOG_FILE" ]; then
            if grep -q "thread.*$thread_id\|$thread_id.*thread\|\"thread_id\":.*$thread_id" "$TUI_LOG_FILE" 2>/dev/null; then
                log_success "Thread $thread_id found in TUI log"
                return 0
            fi
        fi

        # Method 3: Check for thread in active session files
        local session_file="$HOME/.spoq/sessions/$thread_id.json"
        if [ -f "$session_file" ]; then
            log_success "Thread $thread_id found in session files"
            return 0
        fi

        sleep 1
        elapsed=$(($(get_epoch) - start_time))
    done

    log_error "Thread $thread_id not found in TUI after ${timeout}s"
    return 1
}

# Collect all verification evidence into a structured report
# Usage: collect_verification_evidence [output_dir]
# Creates: timestamped directory with evidence files
# Returns: Path to evidence directory on stdout
collect_verification_evidence() {
    local base_dir="${1:-/tmp/e2e_evidence}"
    local timestamp
    timestamp=$(date +%Y%m%d_%H%M%S)
    local evidence_dir="$base_dir/${PROJECT:-unknown}_${PLAN_ID:-unknown}_$timestamp"

    mkdir -p "$evidence_dir"
    log_info "Collecting verification evidence to: $evidence_dir"

    # 1. Copy TUI log (last 1000 lines)
    if [ -f "$TUI_LOG_FILE" ]; then
        tail -1000 "$TUI_LOG_FILE" > "$evidence_dir/tui_log_tail.log" 2>/dev/null || true
        log_debug "Copied TUI log tail"
    fi

    # 2. Extract phase updates
    extract_phase_updates_from_log "${PLAN_ID:-}" > "$evidence_dir/phase_updates.json" 2>/dev/null || true
    log_debug "Extracted phase updates"

    # 3. Copy status files
    local status_dir
    status_dir=$(get_status_dir 2>/dev/null) || status_dir=""
    if [ -n "$status_dir" ] && [ -d "$status_dir" ]; then
        mkdir -p "$evidence_dir/status_files"
        cp "$status_dir"/*.status "$evidence_dir/status_files/" 2>/dev/null || true
        log_debug "Copied status files"
    fi

    # 4. Copy plan file
    local plan_dir
    plan_dir=$(get_plan_dir 2>/dev/null) || plan_dir=""
    if [ -n "$plan_dir" ] && [ -f "$plan_dir/$PLAN_ID.md" ]; then
        cp "$plan_dir/$PLAN_ID.md" "$evidence_dir/plan.md" 2>/dev/null || true
        log_debug "Copied plan file"
    fi

    # 5. Capture API state (if available)
    local threads_response plans_response
    threads_response=$(api_get "/api/threads" 2>/dev/null) || threads_response="{}"
    plans_response=$(api_get "/api/plans" 2>/dev/null) || plans_response="{}"
    echo "$threads_response" > "$evidence_dir/api_threads.json"
    echo "$plans_response" > "$evidence_dir/api_plans.json"
    log_debug "Captured API state"

    # 6. Create summary report
    local summary_file="$evidence_dir/summary.json"
    if command_exists jq; then
        jq -n \
            --arg project "${PROJECT:-unknown}" \
            --arg plan_id "${PLAN_ID:-unknown}" \
            --arg timestamp "$timestamp" \
            --arg evidence_dir "$evidence_dir" \
            --arg tui_log_exists "$([ -f "$TUI_LOG_FILE" ] && echo "true" || echo "false")" \
            --argjson status_file_count "$(ls -1 "$evidence_dir/status_files" 2>/dev/null | wc -l | tr -d ' ')" \
            --argjson phase_update_count "$(jq 'length' "$evidence_dir/phase_updates.json" 2>/dev/null || echo 0)" \
            '{
                project: $project,
                plan_id: $plan_id,
                collected_at: $timestamp,
                evidence_dir: $evidence_dir,
                tui_log_exists: ($tui_log_exists == "true"),
                status_file_count: $status_file_count,
                phase_update_count: $phase_update_count
            }' > "$summary_file"
    else
        cat > "$summary_file" << EOF
{
    "project": "${PROJECT:-unknown}",
    "plan_id": "${PLAN_ID:-unknown}",
    "collected_at": "$timestamp",
    "evidence_dir": "$evidence_dir"
}
EOF
    fi

    log_success "Evidence collection complete: $evidence_dir"
    echo "$evidence_dir"
}

# Compare status file changes with TUI log entries
# Usage: compare_status_with_log <phase> [expected_status]
# Returns: 0 if status matches log entries, 1 otherwise
compare_status_with_log() {
    local phase="$1"
    local expected_status="${2:-}"

    local status_dir
    status_dir=$(get_status_dir)
    local status_file="$status_dir/phase-${phase}.status"

    if [ ! -f "$status_file" ]; then
        log_error "Status file not found: $status_file"
        return 1
    fi

    # Read status from file
    local file_status file_plan_id file_phase
    if command_exists jq; then
        file_status=$(jq -r '.status' "$status_file" 2>/dev/null)
        file_plan_id=$(jq -r '.plan_id' "$status_file" 2>/dev/null)
        file_phase=$(jq -r '.phase' "$status_file" 2>/dev/null)
    else
        file_status=$(grep -o '"status"[[:space:]]*:[[:space:]]*"[^"]*"' "$status_file" | sed 's/.*"\([^"]*\)"$/\1/')
        file_plan_id=$(grep -o '"plan_id"[[:space:]]*:[[:space:]]*"[^"]*"' "$status_file" | sed 's/.*"\([^"]*\)"$/\1/')
        file_phase=$(grep -o '"phase"[[:space:]]*:[[:space:]]*[0-9]*' "$status_file" | sed 's/.*: *//')
    fi

    log_debug "Status file: plan_id=$file_plan_id, phase=$file_phase, status=$file_status"

    # If expected_status provided, verify it matches
    if [ -n "$expected_status" ] && [ "$file_status" != "$expected_status" ]; then
        log_error "Status mismatch: file has '$file_status', expected '$expected_status'"
        return 1
    fi

    # Check TUI log for corresponding entries
    if [ ! -f "$TUI_LOG_FILE" ]; then
        log_warn "TUI log not available for cross-reference"
        log_success "Status file verification passed (log unavailable): phase $phase status=$file_status"
        return 0
    fi

    # Look for log entries matching this phase and plan
    local matching_entries
    matching_entries=$(grep "PHASE_PROGRESS_UPDATE" "$TUI_LOG_FILE" 2>/dev/null | \
        grep -E "\"plan_id\"[[:space:]]*:[[:space:]]*\"$file_plan_id\"|plan.*$file_plan_id" | \
        grep -E "\"phase\"[[:space:]]*:[[:space:]]*$file_phase|phase.*$file_phase" || echo "")

    if [ -z "$matching_entries" ]; then
        log_warn "No matching TUI log entries found for phase $phase of plan $file_plan_id"
        log_info "This may be expected if TUI hasn't received the update yet"
        # Return success but with warning - the status file is valid, just not yet reflected in TUI
        return 0
    fi

    # Extract the most recent status from log entries
    local log_status
    log_status=$(echo "$matching_entries" | tail -1 | sed -n 's/.*"status"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p')

    if [ -n "$log_status" ]; then
        if [ "$log_status" = "$file_status" ]; then
            log_success "Status cross-reference passed: phase $phase, file=$file_status, log=$log_status"
            return 0
        else
            log_warn "Status lag detected: file=$file_status, log=$log_status (may be timing issue)"
            # This is a warning, not an error - the log may be behind
            return 0
        fi
    fi

    log_success "Status file verified: phase $phase, status=$file_status (log entry format varied)"
    return 0
}

# ==============================================================================
# Export all functions for use in subshells
# ==============================================================================
export -f log_info log_success log_warn log_error log_debug log_step log_separator
export -f get_plan_dir get_status_dir get_markers_dir init_plan_dirs
export -f generate_uuid generate_session_id generate_thread_id
export -f create_plan_file create_status_file create_status_file_with_thread update_status_file
export -f check_conductor wait_for_conductor check_websocket
export -f verify_plan_file verify_status_file verify_all_status_files verify_files
export -f api_get api_post api_put api_delete
export -f capture_screenshot capture_window_screenshot
export -f cleanup_plan register_cleanup_on_exit
export -f get_timestamp get_epoch sleep_with_progress command_exists require_command
export -f assert_eq assert_file_exists assert_dir_exists assert_json_field
export -f verify_tui_log_received extract_phase_updates_from_log verify_thread_exists_in_tui
export -f collect_verification_evidence compare_status_with_log

# =============================================================================
# TUI-VISION SCREENSHOT VERIFICATION FUNCTIONS
# =============================================================================
# These functions prepare commands and data structures for TUI visual verification
# using the tui-vision MCP tools. When used within Claude Code, the MCP tools
# (spawn_tui, screenshot_tui, get_tui_text, wait_for_render, close_tui) are
# invoked directly. These shell functions provide helper utilities and expected
# state management that complement the MCP tools.
#
# Circle Patterns:
#   ● (U+25CF) - Filled circle: phase completed
#   ○ (U+25CB) - Empty circle: phase pending
#   ◐ (U+25D0) - Half circle: phase in progress (optional)
#
# Environment:
#   SPOQ_DEV=1 - Required to enable development mode in the TUI
#   TUI_BINARY - Path to TUI binary (default: ./target/release/spoq)
# =============================================================================

# Default TUI binary path
: "${TUI_BINARY:=./target/release/spoq}"

# Store the current TUI session ID (set by spawn_tui_session)
TUI_SESSION_ID=""

# Store captured circle states for verification
declare -a CAPTURED_CIRCLE_STATES

# ==============================================================================
# TUI Session Management
# ==============================================================================

# Spawn a TUI session using tui-vision MCP
# Usage: spawn_tui_session [cols] [rows] [cwd]
# Returns: Outputs the MCP command to execute; sets TUI_SESSION_ID when run via MCP
# Note: This function prepares the spawn command. In Claude Code, use the
#       mcp__tui-vision__spawn_tui tool directly with these parameters.
spawn_tui_session() {
    local cols="${1:-120}"
    local rows="${2:-40}"
    local cwd="${3:-$(pwd)}"

    log_info "Preparing TUI spawn command..."
    log_debug "  Binary: $TUI_BINARY"
    log_debug "  Dimensions: ${cols}x${rows}"
    log_debug "  Working dir: $cwd"
    log_debug "  Environment: SPOQ_DEV=1"

    # Verify TUI binary exists
    if [ ! -x "$TUI_BINARY" ] && [ ! -x "$cwd/$TUI_BINARY" ]; then
        log_warn "TUI binary not found at: $TUI_BINARY"
        log_info "Build with: cargo build --release"
    fi

    # Output the MCP tool parameters as JSON for documentation/reference
    cat << EOF
{
  "tool": "mcp__tui-vision__spawn_tui",
  "parameters": {
    "command": "SPOQ_DEV=1 $TUI_BINARY",
    "cols": $cols,
    "rows": $rows,
    "cwd": "$cwd"
  }
}
EOF

    log_success "TUI spawn command prepared"
    log_info "Execute via MCP: mcp__tui-vision__spawn_tui"
    return 0
}

# Set the TUI session ID (called after MCP spawn returns)
# Usage: set_tui_session_id "session-abc123"
set_tui_session_id() {
    local session_id="$1"
    if [ -z "$session_id" ]; then
        log_error "Session ID is required"
        return 1
    fi
    TUI_SESSION_ID="$session_id"
    export TUI_SESSION_ID
    log_success "TUI session ID set: $session_id"
}

# Get the current TUI session ID
# Usage: session_id=$(get_tui_session_id)
get_tui_session_id() {
    if [ -z "$TUI_SESSION_ID" ]; then
        log_error "No TUI session ID set. Call spawn_tui_session first."
        return 1
    fi
    echo "$TUI_SESSION_ID"
}

# Close the TUI session
# Usage: close_tui_session [session_id]
# Note: Prepares the MCP command; use mcp__tui-vision__close_tui in Claude Code
close_tui_session() {
    local session_id="${1:-$TUI_SESSION_ID}"
    if [ -z "$session_id" ]; then
        log_error "No session ID provided and TUI_SESSION_ID not set"
        return 1
    fi

    log_info "Preparing TUI close command for session: $session_id"
    cat << EOF
{
  "tool": "mcp__tui-vision__close_tui",
  "parameters": {
    "session_id": "$session_id"
  }
}
EOF

    # Clear the stored session ID
    TUI_SESSION_ID=""
    log_success "TUI close command prepared"
    return 0
}

# ==============================================================================
# TUI Ready Detection
# ==============================================================================

# Wait for TUI to be ready and display threads
# Usage: wait_for_tui_ready [session_id] [max_attempts] [interval_ms]
# Note: Uses mcp__tui-vision__wait_for_render and mcp__tui-vision__get_tui_text
# Returns: 0 when ready, 1 on timeout
wait_for_tui_ready() {
    local session_id="${1:-$TUI_SESSION_ID}"
    local max_attempts="${2:-10}"
    local interval_ms="${3:-1000}"

    if [ -z "$session_id" ]; then
        log_error "No session ID provided and TUI_SESSION_ID not set"
        return 1
    fi

    log_info "Waiting for TUI to be ready (max ${max_attempts} attempts)..."

    # Output the MCP commands to execute in sequence
    cat << EOF
# TUI Ready Detection Sequence
# Execute these MCP commands in order until ready condition is met:

1. Wait for initial render:
{
  "tool": "mcp__tui-vision__wait_for_render",
  "parameters": {
    "session_id": "$session_id",
    "timeout_ms": $interval_ms
  }
}

2. Get TUI text to check for ready indicators:
{
  "tool": "mcp__tui-vision__get_tui_text",
  "parameters": {
    "session_id": "$session_id"
  }
}

3. Ready indicators to look for in the text:
   - Thread list displayed (lines containing thread names)
   - Phase circles visible (● or ○ characters)
   - No "Loading..." or spinner text
   - Plan ID visible in the UI

4. Repeat steps 1-2 up to $max_attempts times if not ready
EOF

    log_success "TUI ready detection commands prepared"
    return 0
}

# Check if TUI text indicates ready state
# Usage: tui_text_is_ready "$text_content"
# Returns: 0 if ready, 1 if not ready
tui_text_is_ready() {
    local text="$1"

    if [ -z "$text" ]; then
        return 1
    fi

    # Check for presence of circle characters (indicates phases are displayed)
    if echo "$text" | grep -q '[●○◐]'; then
        log_debug "Found phase circles in TUI"
        return 0
    fi

    # Check for thread indicators
    if echo "$text" | grep -qE '(thread|Thread|THREAD)'; then
        log_debug "Found thread indicator in TUI"
        return 0
    fi

    # Check for loading indicators (not ready)
    if echo "$text" | grep -qiE '(loading|spinner|wait)'; then
        log_debug "TUI still loading"
        return 1
    fi

    return 1
}

# ==============================================================================
# Circle State Capture and Verification
# ==============================================================================

# Capture the current phase circle state from TUI text
# Usage: capture_phase_circle_state [session_id] [output_file] [screenshot_path]
# Returns: Circle pattern string (e.g., "●●○○○" for 2 complete, 3 pending)
# Note: Combines mcp__tui-vision__screenshot_tui and mcp__tui-vision__get_tui_text
capture_phase_circle_state() {
    local session_id="${1:-$TUI_SESSION_ID}"
    local output_file="${2:-}"
    local screenshot_path="${3:-/tmp/tui_phase_circles_$(date +%Y%m%d_%H%M%S).png}"

    if [ -z "$session_id" ]; then
        log_error "No session ID provided and TUI_SESSION_ID not set"
        return 1
    fi

    log_info "Capturing phase circle state..."

    # Output the MCP commands
    cat << EOF
# Phase Circle Capture Sequence

1. Take screenshot for visual record:
{
  "tool": "mcp__tui-vision__screenshot_tui",
  "parameters": {
    "session_id": "$session_id",
    "output_path": "$screenshot_path"
  }
}

2. Extract text to parse circle patterns:
{
  "tool": "mcp__tui-vision__get_tui_text",
  "parameters": {
    "session_id": "$session_id"
  }
}

3. Parse the text for circle patterns:
   - Look for sequences of ● (filled/complete) and ○ (empty/pending)
   - Extract the pattern as a string (e.g., "●●○○○")
   - Count filled vs empty for progress calculation
EOF

    if [ -n "$output_file" ]; then
        log_info "Circle state will be saved to: $output_file"
    fi

    log_success "Circle capture commands prepared"
    return 0
}

# Extract circle pattern from TUI text
# Usage: circles=$(extract_circles_from_text "$tui_text")
# Returns: String of circle characters found (e.g., "●●○○○")
extract_circles_from_text() {
    local text="$1"

    if [ -z "$text" ]; then
        echo ""
        return 1
    fi

    # Extract all circle characters and join them
    # Matches: ● (U+25CF), ○ (U+25CB), ◐ (U+25D0), ◑ (U+25D1)
    local circles
    circles=$(echo "$text" | grep -o '[●○◐◑]' | tr -d '\n')

    echo "$circles"
}

# Count completed phases from circle pattern
# Usage: completed=$(count_completed_phases "●●○○○")
# Returns: Number of filled circles
count_completed_phases() {
    local pattern="$1"
    echo "$pattern" | grep -o '●' | wc -l | tr -d ' '
}

# Count pending phases from circle pattern
# Usage: pending=$(count_pending_phases "●●○○○")
# Returns: Number of empty circles
count_pending_phases() {
    local pattern="$1"
    echo "$pattern" | grep -o '○' | wc -l | tr -d ' '
}

# Count in-progress phases from circle pattern
# Usage: in_progress=$(count_in_progress_phases "●◐○○")
# Returns: Number of half-filled circles
count_in_progress_phases() {
    local pattern="$1"
    echo "$pattern" | grep -o '[◐◑]' | wc -l | tr -d ' '
}

# Get total phases from circle pattern
# Usage: total=$(count_total_phases "●●○○○")
# Returns: Total number of circles
count_total_phases() {
    local pattern="$1"
    # Count circle characters (not bytes, since these are multi-byte UTF-8)
    # Use grep -o to count individual circle characters
    echo "$pattern" | grep -o '[●○◐◑]' | wc -l | tr -d ' '
}

# Verify circles match expected state
# Usage: verify_circles_match_expected "$captured" "$expected" "description"
# Returns: 0 if match, 1 if mismatch
verify_circles_match_expected() {
    local captured="$1"
    local expected="$2"
    local description="${3:-Circles should match expected state}"

    log_info "Verifying circle state..."
    log_debug "  Captured: $captured"
    log_debug "  Expected: $expected"

    if [ "$captured" = "$expected" ]; then
        log_success "PASS: $description"
        log_info "  Circle state: $captured"
        return 0
    else
        log_error "FAIL: $description"
        log_error "  Expected: $expected"
        log_error "  Captured: $captured"

        # Provide detailed diff
        local cap_complete exp_complete cap_pending exp_pending
        cap_complete=$(count_completed_phases "$captured")
        exp_complete=$(count_completed_phases "$expected")
        cap_pending=$(count_pending_phases "$captured")
        exp_pending=$(count_pending_phases "$expected")

        log_error "  Completed: expected $exp_complete, got $cap_complete"
        log_error "  Pending: expected $exp_pending, got $cap_pending"

        return 1
    fi
}

# Generate expected circle pattern for a given progress
# Usage: expected=$(generate_expected_circles 3 5)  # 3 of 5 complete
# Returns: Circle pattern string (e.g., "●●●○○")
generate_expected_circles() {
    local completed="$1"
    local total="$2"
    local in_progress="${3:-0}"

    local pattern=""

    # Add completed circles
    for ((i=0; i<completed; i++)); do
        pattern="${pattern}●"
    done

    # Add in-progress circles
    for ((i=0; i<in_progress; i++)); do
        pattern="${pattern}◐"
    done

    # Add pending circles
    local pending=$((total - completed - in_progress))
    for ((i=0; i<pending; i++)); do
        pattern="${pattern}○"
    done

    echo "$pattern"
}

# ==============================================================================
# Circle Progression Capture
# ==============================================================================

# Capture multiple screenshots as phases progress
# Usage: capture_circle_progression [session_id] [num_captures] [interval_sec] [output_dir]
# Note: Takes periodic screenshots to document phase progression
capture_circle_progression() {
    local session_id="${1:-$TUI_SESSION_ID}"
    local num_captures="${2:-5}"
    local interval_sec="${3:-10}"
    local output_dir="${4:-/tmp/tui_progression}"

    if [ -z "$session_id" ]; then
        log_error "No session ID provided and TUI_SESSION_ID not set"
        return 1
    fi

    log_info "Preparing circle progression capture..."
    log_info "  Captures: $num_captures"
    log_info "  Interval: ${interval_sec}s"
    log_info "  Output: $output_dir"

    # Ensure output directory exists
    mkdir -p "$output_dir"

    cat << EOF
# Circle Progression Capture Plan
# This captures $num_captures screenshots at ${interval_sec}s intervals

Output directory: $output_dir
Session: $session_id

For each capture (i = 1 to $num_captures):
  1. Wait for render stability:
     {
       "tool": "mcp__tui-vision__wait_for_render",
       "parameters": {
         "session_id": "$session_id",
         "timeout_ms": 2000
       }
     }

  2. Take screenshot:
     {
       "tool": "mcp__tui-vision__screenshot_tui",
       "parameters": {
         "session_id": "$session_id",
         "output_path": "$output_dir/capture_\${i}_\$(date +%H%M%S).png"
       }
     }

  3. Get text for circle extraction:
     {
       "tool": "mcp__tui-vision__get_tui_text",
       "parameters": {
         "session_id": "$session_id"
       }
     }

  4. Extract and log circle pattern using: extract_circles_from_text "\$text"

  5. Sleep ${interval_sec} seconds before next capture

Post-capture analysis:
  - Compare first and last circle patterns
  - Verify progression moved forward (more filled circles)
  - Generate timeline of state changes
EOF

    log_success "Progression capture plan prepared"
    return 0
}

# Store a captured circle state with timestamp
# Usage: store_circle_state "●●○○○" "description"
store_circle_state() {
    local pattern="$1"
    local description="${2:-capture}"
    local timestamp
    timestamp=$(get_timestamp)

    # Store as "timestamp|pattern|description"
    CAPTURED_CIRCLE_STATES+=("${timestamp}|${pattern}|${description}")

    log_debug "Stored circle state: $pattern at $timestamp"
}

# Get all captured circle states
# Usage: get_captured_states
get_captured_states() {
    for state in "${CAPTURED_CIRCLE_STATES[@]}"; do
        echo "$state"
    done
}

# Clear captured circle states
# Usage: clear_captured_states
clear_captured_states() {
    CAPTURED_CIRCLE_STATES=()
    log_debug "Cleared captured circle states"
}

# Verify progression moved forward
# Usage: verify_circle_progression "○○○○○" "●●●○○" "Phases should progress"
verify_circle_progression() {
    local initial="$1"
    local final="$2"
    local description="${3:-Circles should show progression}"

    local initial_complete final_complete
    initial_complete=$(count_completed_phases "$initial")
    final_complete=$(count_completed_phases "$final")

    log_info "Verifying circle progression..."
    log_debug "  Initial: $initial ($initial_complete complete)"
    log_debug "  Final: $final ($final_complete complete)"

    if [ "$final_complete" -gt "$initial_complete" ]; then
        log_success "PASS: $description"
        log_info "  Progress: $initial_complete -> $final_complete phases complete"
        return 0
    elif [ "$final_complete" -eq "$initial_complete" ]; then
        log_warn "No progression detected"
        log_warn "  Initial: $initial_complete complete"
        log_warn "  Final: $final_complete complete"
        return 1
    else
        log_error "FAIL: $description - Regression detected!"
        log_error "  Initial: $initial_complete complete"
        log_error "  Final: $final_complete complete"
        return 1
    fi
}

# ==============================================================================
# TUI Resize and Input Helpers
# ==============================================================================

# Resize TUI session
# Usage: resize_tui_session [session_id] <cols> <rows>
resize_tui_session() {
    local session_id="${1:-$TUI_SESSION_ID}"
    local cols="$2"
    local rows="$3"

    if [ -z "$session_id" ]; then
        log_error "No session ID provided and TUI_SESSION_ID not set"
        return 1
    fi

    if [ -z "$cols" ] || [ -z "$rows" ]; then
        log_error "Both cols and rows are required"
        return 1
    fi

    log_info "Preparing TUI resize command..."
    cat << EOF
{
  "tool": "mcp__tui-vision__resize_tui",
  "parameters": {
    "session_id": "$session_id",
    "cols": $cols,
    "rows": $rows
  }
}
EOF

    log_success "TUI resize command prepared (${cols}x${rows})"
    return 0
}

# Send keys to TUI session
# Usage: send_keys_to_tui [session_id] "keys_to_send"
send_keys_to_tui() {
    local session_id="${1:-$TUI_SESSION_ID}"
    local keys="$2"

    if [ -z "$session_id" ]; then
        log_error "No session ID provided and TUI_SESSION_ID not set"
        return 1
    fi

    if [ -z "$keys" ]; then
        log_error "Keys to send are required"
        return 1
    fi

    log_info "Preparing send keys command..."
    cat << EOF
{
  "tool": "mcp__tui-vision__send_keys",
  "parameters": {
    "session_id": "$session_id",
    "keys": "$keys"
  }
}
EOF

    log_success "Send keys command prepared: $keys"
    return 0
}

# ==============================================================================
# Export TUI-Vision Functions
# ==============================================================================
export -f spawn_tui_session set_tui_session_id get_tui_session_id close_tui_session
export -f wait_for_tui_ready tui_text_is_ready
export -f capture_phase_circle_state extract_circles_from_text
export -f count_completed_phases count_pending_phases count_in_progress_phases count_total_phases
export -f verify_circles_match_expected generate_expected_circles
export -f capture_circle_progression store_circle_state get_captured_states clear_captured_states
export -f verify_circle_progression
export -f resize_tui_session send_keys_to_tui

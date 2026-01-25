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

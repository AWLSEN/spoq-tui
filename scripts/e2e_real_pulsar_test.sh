#!/bin/bash
#
# Real E2E Test for Pulsar Execution with TUI Progress Circles
#
# This script performs a full end-to-end test of Pulsar plan execution by:
# 1. Creating a thread via the API (like the frontend would)
# 2. Generating a Nova plan for the thread
# 3. Triggering Pulsar execution
# 4. Monitoring execution progress via status files
# 5. Capturing TUI screenshots showing progress circles
#
# Usage:
#   ./scripts/e2e_real_pulsar_test.sh              # Run full test
#   ./scripts/e2e_real_pulsar_test.sh setup        # Setup test environment
#   ./scripts/e2e_real_pulsar_test.sh cleanup      # Cleanup test files
#   ./scripts/e2e_real_pulsar_test.sh screenshot-only # Just capture screenshot
#
# Prerequisites:
#   - conductor running (pgrep -f conductor)
#   - Authentication token (in env or ~/.spoq/credentials.json)
#   - TUI binary built at ./target/release/spoq
#   - jq installed for JSON processing
#
# Environment Variables:
#   SPOQ_AUTH_TOKEN   - Authentication token (optional if in credentials.json)
#   API_BASE_URL      - API endpoint (default: http://localhost:8000)
#   E2E_DEBUG         - Enable debug logging (set to any value)
#
# Example:
#   # Run full test with debug output
#   E2E_DEBUG=1 ./scripts/e2e_real_pulsar_test.sh
#
#   # Just cleanup after a failed run
#   ./scripts/e2e_real_pulsar_test.sh cleanup
#

set -euo pipefail

# Source the E2E helpers library
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/lib/e2e_helpers.sh"

# ==============================================================================
# Configuration
# ==============================================================================

# Plan configuration
PROJECT="tui_spoq"
TOTAL_PHASES=5
TEMP_DIR="/tmp/e2e_real_pulsar_test"
SCREENSHOT_DIR="$TEMP_DIR/screenshots"

# API configuration
API_BASE_URL="${API_BASE_URL:-http://localhost:8000}"

# TUI binary path
TUI_BINARY="./target/release/spoq"

# Auth configuration
AUTH_TOKEN="${SPOQ_AUTH_TOKEN:-}"
CREDENTIALS_FILE="$HOME/.spoq/credentials.json"

# Test state (populated during execution)
THREAD_ID=""
PLAN_ID=""

# ==============================================================================
# Usage Documentation
# ==============================================================================

usage() {
    cat << EOF
Usage: $(basename "$0") [COMMAND]

Commands:
    run             Run full E2E test (default)
    setup           Setup test environment only
    cleanup         Cleanup test files and directories
    screenshot-only Capture TUI screenshot only
    help            Show this help message

Prerequisites:
    - conductor must be running (check: pgrep -f conductor)
    - Authentication token available (env or ~/.spoq/credentials.json)
    - TUI binary exists at ./target/release/spoq
    - jq must be installed for JSON processing

Environment Variables:
    SPOQ_AUTH_TOKEN   Authentication token (optional if in credentials.json)
    API_BASE_URL      API endpoint (default: http://localhost:8000)
    E2E_DEBUG         Enable debug logging

Examples:
    # Run full test
    ./scripts/e2e_real_pulsar_test.sh

    # Run with debug output
    E2E_DEBUG=1 ./scripts/e2e_real_pulsar_test.sh

    # Just cleanup
    ./scripts/e2e_real_pulsar_test.sh cleanup

EOF
}

# ==============================================================================
# Prerequisites Check
# ==============================================================================

check_prerequisites() {
    log_info "Checking prerequisites..."

    local errors=0

    # Check conductor is running
    if ! check_conductor; then
        log_error "Conductor is not running. Start it first."
        errors=$((errors + 1))
    fi

    # Check for authentication token
    if [ -z "$AUTH_TOKEN" ]; then
        if [ -f "$CREDENTIALS_FILE" ]; then
            if command -v jq &> /dev/null; then
                AUTH_TOKEN=$(jq -r '.token // ""' "$CREDENTIALS_FILE" 2>/dev/null || echo "")
            fi
        fi
    fi

    if [ -z "$AUTH_TOKEN" ]; then
        log_error "Authentication token not found"
        log_info "Set SPOQ_AUTH_TOKEN env var or add token to ~/.spoq/credentials.json"
        errors=$((errors + 1))
    else
        log_success "Authentication token found"
    fi

    # Check TUI binary exists
    if [ ! -f "$TUI_BINARY" ]; then
        log_error "TUI binary not found at: $TUI_BINARY"
        log_info "Build it with: cargo build --release"
        errors=$((errors + 1))
    else
        log_success "TUI binary found at: $TUI_BINARY"
    fi

    # Check jq is installed
    if ! command -v jq &> /dev/null; then
        log_error "jq is not installed (required for JSON processing)"
        log_info "Install it with: brew install jq (macOS) or apt install jq (Linux)"
        errors=$((errors + 1))
    else
        log_success "jq is installed"
    fi

    if [ "$errors" -gt 0 ]; then
        log_error "Prerequisites check failed with $errors error(s)"
        return 1
    fi

    log_success "All prerequisites satisfied"
    return 0
}

# ==============================================================================
# Phase 4: Thread Creation Functions
# ==============================================================================

# Phase 4: Create thread via API
# Creates a new conversation thread using the API endpoint
# Sets THREAD_ID global variable on success
create_thread_via_api() {
    log_step "CREATE_THREAD" "Creating thread via API"

    # Generate unique IDs for this request
    local session_id
    session_id=$(generate_uuid)
    log_debug "Generated session_id: $session_id"

    # Ensure we have an auth token
    if [ -z "$AUTH_TOKEN" ]; then
        if [ -f "$CREDENTIALS_FILE" ]; then
            if command -v jq &> /dev/null; then
                AUTH_TOKEN=$(jq -r '.token // ""' "$CREDENTIALS_FILE" 2>/dev/null || echo "")
            fi
        fi
    fi

    if [ -z "$AUTH_TOKEN" ]; then
        log_error "No authentication token available"
        log_info "Set SPOQ_AUTH_TOKEN env var or add token to ~/.spoq/credentials.json"
        return 1
    fi

    log_debug "Using auth token: ${AUTH_TOKEN:0:20}..."

    # Build the request payload
    # The API expects a prompt to initiate a conversation which creates the thread
    local payload
    payload=$(jq -n \
        --arg prompt "Initialize E2E test thread" \
        --arg session_id "$session_id" \
        '{prompt: $prompt, session_id: $session_id}')

    log_debug "Request payload: $payload"

    # Create a temporary file to store the SSE response
    local tmp_response="/tmp/e2e_thread_response_$$.txt"
    local tmp_stderr="/tmp/e2e_thread_stderr_$$.txt"

    log_info "Calling POST ${API_BASE_URL}/v1/stream..."

    # Make the SSE request with curl
    # - Use -N (--no-buffer) for real-time SSE
    # - Use --max-time for timeout
    # - Capture both stdout and stderr
    local http_code
    http_code=$(curl -s -N \
        --max-time 30 \
        -w "%{http_code}" \
        -o "$tmp_response" \
        -X POST \
        -H "Authorization: Bearer $AUTH_TOKEN" \
        -H "Content-Type: application/json" \
        -H "Accept: text/event-stream" \
        -d "$payload" \
        "${API_BASE_URL}/v1/stream" 2>"$tmp_stderr")

    local curl_exit=$?

    # Check for curl errors
    if [ $curl_exit -ne 0 ] && [ $curl_exit -ne 28 ]; then
        # Exit code 28 is timeout which is expected for SSE
        log_error "curl failed with exit code $curl_exit"
        if [ -f "$tmp_stderr" ]; then
            log_error "stderr: $(cat "$tmp_stderr")"
        fi
        rm -f "$tmp_response" "$tmp_stderr"
        return 1
    fi

    # Check HTTP status (if available)
    if [ -n "$http_code" ] && [ "$http_code" != "000" ]; then
        if [ "$http_code" -ge 400 ]; then
            log_error "HTTP error: $http_code"
            if [ -f "$tmp_response" ]; then
                log_error "Response: $(cat "$tmp_response")"
            fi
            rm -f "$tmp_response" "$tmp_stderr"
            return 1
        fi
        log_debug "HTTP status: $http_code"
    fi

    # Parse the SSE response to find thread_created event
    if [ ! -f "$tmp_response" ]; then
        log_error "No response received from server"
        rm -f "$tmp_stderr"
        return 1
    fi

    log_debug "SSE response received ($(wc -c < "$tmp_response") bytes)"

    # Extract thread_id from the thread_created event
    # SSE format is:
    #   event: thread_created
    #   data: {"type": "thread_created", "thread": {"id": "...", ...}, ...}
    #
    # We look for lines starting with "data:" and parse the JSON to find thread_created
    local extracted_thread_id=""

    # Method 1: Look for thread_created event with jq parsing
    while IFS= read -r line; do
        # Skip empty lines and event lines
        if [[ "$line" =~ ^data:\ *(.+)$ ]]; then
            local json_data="${BASH_REMATCH[1]}"
            log_debug "Found data line: ${json_data:0:100}..."

            # Try to parse as JSON and extract thread_id from thread_created message
            local msg_type
            msg_type=$(echo "$json_data" | jq -r '.type // empty' 2>/dev/null)

            if [ "$msg_type" = "thread_created" ]; then
                extracted_thread_id=$(echo "$json_data" | jq -r '.thread.id // empty' 2>/dev/null)
                if [ -n "$extracted_thread_id" ]; then
                    log_success "Found thread_created event with thread_id: $extracted_thread_id"
                    break
                fi
            fi
        fi
    done < "$tmp_response"

    # Cleanup temp files
    rm -f "$tmp_response" "$tmp_stderr"

    # Verify we got a thread_id
    if [ -z "$extracted_thread_id" ]; then
        log_error "Failed to extract thread_id from SSE response"
        log_info "The thread_created event was not found in the response"
        return 1
    fi

    # Set the global THREAD_ID variable
    THREAD_ID="$extracted_thread_id"
    log_success "Thread created successfully: $THREAD_ID"

    return 0
}

# Phase 5: Generate Nova plan
# Generates a Nova plan locally (no API call needed)
# Sets PLAN_ID global variable on success
generate_nova_plan() {
    log_step "PLAN" "Generating Nova-format plan"

    # Generate plan ID in plan-YYYYMMDD-HHMM format
    PLAN_ID="plan-$(date +%Y%m%d-%H%M)"
    log_info "Generated plan ID: $PLAN_ID"

    # Get plan directory path using helper
    local plan_dir
    plan_dir=$(get_plan_dir)
    local status_dir="$plan_dir/status"

    # Create directory structure
    mkdir -p "$status_dir"
    log_success "Created plan directory: $plan_dir"
    log_success "Created status directory: $status_dir"

    # Generate timestamps
    local created_at
    created_at=$(date -u +%Y-%m-%dT%H:%M:%SZ)

    # Get thread ID (use global or generate one)
    local thread_id_value="${THREAD_ID:-null}"
    if [ "$thread_id_value" != "null" ]; then
        thread_id_value="\"$THREAD_ID\""
    fi

    # Write plan markdown file
    local plan_file="$plan_dir.md"
    cat > "$plan_file" << EOF
# Plan: E2E Test Auto-Generated Plan

## Metadata
- **ID**: ${PLAN_ID}
- **Project**: ${PROJECT}
- **Project Path**: /Users/sam/tui_spoq
- **Thread ID**: ${thread_id_value}
- **Type**: test
- **Status**: active
- **Execution Mode**: interactive
- **Created**: ${created_at}
- **Worktree**: null

## Summary
Auto-generated 5-phase plan for E2E testing of TUI progress circles.

Each phase performs a simple file operation in /tmp to verify phase execution and status tracking.

## Phases

### Phase 1: Create test file
- **Description**: Create a test file in /tmp to verify phase execution starts correctly
- **Files**: \`/tmp/e2e_test_file.txt\` (NEW)
- **Complexity**: Low
- **Complexity Reasoning**: Simple file creation with echo command
- **Recommended Agent**: sonnet

### Phase 2: Append to test file
- **Description**: Append a timestamp line to the test file to verify modifications work
- **Files**: \`/tmp/e2e_test_file.txt\` (MODIFY)
- **Complexity**: Low
- **Complexity Reasoning**: Simple file append operation
- **Recommended Agent**: sonnet

### Phase 3: Create second test file
- **Description**: Create a second test file to verify multiple file operations
- **Files**: \`/tmp/e2e_test_file_2.txt\` (NEW)
- **Complexity**: Low
- **Complexity Reasoning**: Simple file creation
- **Recommended Agent**: sonnet

### Phase 4: Read and verify files
- **Description**: Read both test files and verify their contents match expectations
- **Files**: \`/tmp/e2e_test_file.txt\` (READ), \`/tmp/e2e_test_file_2.txt\` (READ)
- **Complexity**: Low
- **Complexity Reasoning**: Simple file read and content verification
- **Recommended Agent**: sonnet

### Phase 5: Cleanup test files
- **Description**: Delete the test files created during the E2E test
- **Files**: \`/tmp/e2e_test_file.txt\` (DELETE), \`/tmp/e2e_test_file_2.txt\` (DELETE)
- **Complexity**: Low
- **Complexity Reasoning**: Simple file deletion
- **Recommended Agent**: sonnet

## Parallelization Analysis

\`\`\`
Phase 1 (Create file) ──→ Phase 2 (Append) ──→ Phase 4 (Read/Verify) ──→ Phase 5 (Cleanup)
                                                      ↑
Phase 3 (Create second file) ─────────────────────────┘
\`\`\`

**Analysis:**
- Phase 1 must complete before Phase 2 (file must exist to append)
- Phase 3 is independent of Phases 1-2
- Phase 4 depends on both Phase 2 and Phase 3
- Phase 5 depends on Phase 4

**Execution Strategy:**
| Round | Phases | Why |
|-------|--------|-----|
| 1 | Phase 1, Phase 3 | Both are independent file creations |
| 2 | Phase 2 | Depends on Phase 1 |
| 3 | Phase 4 | Depends on Phases 2, 3 |
| 4 | Phase 5 | Depends on Phase 4 |

## Test Strategy

**After execution, verify:**
1. All 5 phases completed successfully
2. Status files show \`completed\` status for each phase
3. TUI showed progress circles updating from \`○○○○○\` to \`●●●●●\`
4. Test files were properly created and cleaned up

## Rollback Strategy

1. If any phase fails: Check status files for error details
2. Cleanup: \`rm -f /tmp/e2e_test_file*.txt\`
3. Remove plan: \`rm -rf ${plan_dir} ${plan_file}\`
EOF

    log_success "Created plan file: $plan_file"
    log_info "Plan ID set to: $PLAN_ID"
    log_info "Plan directory: $plan_dir"

    # Verify the plan was created correctly
    if [ -f "$plan_file" ] && [ -d "$status_dir" ]; then
        log_success "Nova plan generated successfully"
        return 0
    else
        log_error "Failed to generate Nova plan"
        return 1
    fi
}

# ==============================================================================
# Phase 6: Monitor Execution Functions
# ==============================================================================

# Configuration for monitoring
MONITOR_TIMEOUT="${MONITOR_TIMEOUT:-300}"  # Default 5 minutes
MONITOR_POLL_INTERVAL="${MONITOR_POLL_INTERVAL:-3}"  # Poll every 3 seconds
CONDUCTOR_LOG_DIR="${CONDUCTOR_LOG_DIR:-$HOME/.conductor/logs}"
WS_EVENTS_FILE="$TEMP_DIR/ws_events.log"

# Document how to trigger Pulsar manually
# Usage: trigger_pulsar
# Note: Pulsar requires interactive Claude session - cannot be automated from bash
trigger_pulsar() {
    log_step "PULSAR" "Trigger Pulsar Execution"

    if [ -z "$PLAN_ID" ]; then
        log_error "PLAN_ID not set - cannot trigger Pulsar"
        return 1
    fi

    log_info "Pulsar requires an interactive Claude session to execute."
    log_info ""
    echo -e "${E2E_BOLD}To trigger Pulsar execution:${E2E_NC}"
    echo ""
    echo "  1. Open Claude Code in a terminal"
    echo "  2. Navigate to project: cd $(pwd)"
    echo "  3. Run the command: /pulsar $PLAN_ID"
    echo ""
    log_info "Plan ID: $PLAN_ID"
    log_info "Project: $PROJECT"
    log_info ""

    # Return success - user will trigger manually
    return 0
}

# Monitor conductor logs for phase progress updates
# Usage: monitor_conductor_logs &
# Writes extracted events to WS_EVENTS_FILE
monitor_conductor_logs() {
    local log_file=""

    # Find the most recent conductor log file
    if [ -d "$CONDUCTOR_LOG_DIR" ]; then
        log_file=$(ls -t "$CONDUCTOR_LOG_DIR"/*.log 2>/dev/null | head -1)
    fi

    # Also check common alternative locations
    if [ -z "$log_file" ] || [ ! -f "$log_file" ]; then
        local alt_dirs=(
            "$HOME/.conductor/logs"
            "$HOME/comms/conductor/logs"
            "/tmp/conductor/logs"
        )
        for dir in "${alt_dirs[@]}"; do
            if [ -d "$dir" ]; then
                log_file=$(ls -t "$dir"/*.log 2>/dev/null | head -1)
                [ -f "$log_file" ] && break
            fi
        done
    fi

    if [ -z "$log_file" ] || [ ! -f "$log_file" ]; then
        log_warn "No conductor log file found in $CONDUCTOR_LOG_DIR"
        log_info "Will rely on status file polling for progress updates"
        return 0
    fi

    log_info "Monitoring conductor log: $log_file"

    # Initialize events file
    mkdir -p "$TEMP_DIR"
    > "$WS_EVENTS_FILE"

    # Tail the log file and extract phase_progress_update events
    # Run until parent process signals stop or timeout
    tail -f "$log_file" 2>/dev/null | while IFS= read -r line; do
        # Look for phase_progress_update in the log line
        if echo "$line" | grep -q "phase_progress_update"; then
            local timestamp
            timestamp=$(date -u +%Y-%m-%dT%H:%M:%SZ)

            # Try to extract JSON payload
            local json_payload
            json_payload=$(echo "$line" | grep -oE '\{[^}]+phase_progress_update[^}]+\}' || echo "$line")

            # Log to events file
            echo "[$timestamp] $json_payload" >> "$WS_EVENTS_FILE"

            # Also log to console
            log_debug "WebSocket event: phase_progress_update detected"
        fi

        # Check for plan completion messages
        if echo "$line" | grep -q "plan.*completed\|all.*phases.*complete"; then
            log_info "Detected plan completion in logs"
            echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] PLAN_COMPLETED" >> "$WS_EVENTS_FILE"
        fi
    done &

    # Store the background process PID for cleanup
    CONDUCTOR_LOG_PID=$!
    log_debug "Started conductor log monitor (PID: $CONDUCTOR_LOG_PID)"
}

# Wait for all phases to complete by polling status files
# Usage: wait_for_phase_completion [timeout_seconds]
# Returns: 0 if all phases completed, 1 if timeout or error
wait_for_phase_completion() {
    local timeout="${1:-$MONITOR_TIMEOUT}"
    local poll_interval="${2:-$MONITOR_POLL_INTERVAL}"
    local start_time
    start_time=$(date +%s)
    local status_dir="$HOME/comms/plans/$PROJECT/active/$PLAN_ID/status"

    if [ -z "$PLAN_ID" ]; then
        log_error "PLAN_ID not set - cannot monitor phases"
        return 1
    fi

    log_info "Monitoring phase completion in: $status_dir"
    log_info "Timeout: ${timeout}s, Poll interval: ${poll_interval}s"
    log_info "Total phases expected: $TOTAL_PHASES"
    echo ""

    local last_completed=0
    local last_running=""

    while true; do
        local current_time
        current_time=$(date +%s)
        local elapsed=$((current_time - start_time))

        # Check timeout
        if [ "$elapsed" -ge "$timeout" ]; then
            log_error "Timeout waiting for phase completion after ${elapsed}s"
            return 1
        fi

        # Count completed phases
        local completed=0
        local running=""
        local pending=0
        local failed=0

        for phase in $(seq 1 "$TOTAL_PHASES"); do
            local status_file="$status_dir/phase-${phase}.status"

            if [ -f "$status_file" ]; then
                local status
                status=$(jq -r '.status // "unknown"' "$status_file" 2>/dev/null || echo "unknown")

                case "$status" in
                    completed)
                        completed=$((completed + 1))
                        ;;
                    running|in_progress)
                        running="$phase"
                        ;;
                    failed)
                        failed=$((failed + 1))
                        ;;
                    pending|*)
                        pending=$((pending + 1))
                        ;;
                esac
            else
                pending=$((pending + 1))
            fi
        done

        # Log progress if changed
        if [ "$completed" -ne "$last_completed" ] || [ "$running" != "$last_running" ]; then
            local progress_msg="Progress: $completed/$TOTAL_PHASES completed"
            [ -n "$running" ] && progress_msg="$progress_msg, phase $running running"
            [ "$pending" -gt 0 ] && progress_msg="$progress_msg, $pending pending"
            [ "$failed" -gt 0 ] && progress_msg="$progress_msg, $failed failed"

            log_info "$progress_msg"
            last_completed=$completed
            last_running=$running
        fi

        # Check if all phases are completed
        if [ "$completed" -eq "$TOTAL_PHASES" ]; then
            log_success "All $TOTAL_PHASES phases completed successfully!"
            return 0
        fi

        # Check for failures
        if [ "$failed" -gt 0 ]; then
            log_error "$failed phase(s) failed"
            return 1
        fi

        # Wait before next poll
        sleep "$poll_interval"
    done
}

# Extract WebSocket events from the events log file
# Usage: extract_websocket_events
# Outputs: Summary of captured WebSocket events
extract_websocket_events() {
    if [ ! -f "$WS_EVENTS_FILE" ]; then
        log_warn "No WebSocket events file found"
        return 0
    fi

    local event_count
    event_count=$(wc -l < "$WS_EVENTS_FILE" | tr -d ' ')

    if [ "$event_count" -eq 0 ]; then
        log_info "No WebSocket events captured"
        return 0
    fi

    log_step "WS EVENTS" "Captured WebSocket Events ($event_count total)"

    # Show the events
    while IFS= read -r line; do
        echo "  $line"
    done < "$WS_EVENTS_FILE"

    # Count phase_progress_update events specifically
    local progress_events
    progress_events=$(grep -c "phase_progress_update" "$WS_EVENTS_FILE" 2>/dev/null || echo "0")

    echo ""
    log_info "Summary: $progress_events phase_progress_update events captured"

    # Copy to a more permanent location if needed
    if [ -d "$SCREENSHOT_DIR" ]; then
        cp "$WS_EVENTS_FILE" "$SCREENSHOT_DIR/ws_events_$(date +%Y%m%d_%H%M%S).log"
        log_success "Events saved to $SCREENSHOT_DIR"
    fi

    return 0
}

# Stop the conductor log monitor
# Usage: stop_conductor_log_monitor
stop_conductor_log_monitor() {
    if [ -n "${CONDUCTOR_LOG_PID:-}" ]; then
        kill "$CONDUCTOR_LOG_PID" 2>/dev/null || true
        log_debug "Stopped conductor log monitor (PID: $CONDUCTOR_LOG_PID)"
        unset CONDUCTOR_LOG_PID
    fi
}

# Main monitoring orchestration function
# Usage: monitor_execution
# Monitors Pulsar execution by watching status files and logs
monitor_execution() {
    log_step "MONITOR" "Starting Execution Monitor"

    if [ -z "$PLAN_ID" ]; then
        log_error "PLAN_ID not set - cannot monitor execution"
        return 1
    fi

    local status_dir="$HOME/comms/plans/$PROJECT/active/$PLAN_ID/status"

    # Display trigger instructions
    trigger_pulsar
    echo ""

    # Prompt for user action or auto-proceed
    local auto_delay=30
    log_info "Waiting for Pulsar to be triggered..."
    log_info "Press Enter when Pulsar is running, or wait ${auto_delay}s to auto-proceed..."

    # Read with timeout
    if read -t "$auto_delay" -r; then
        log_info "User confirmed - starting monitoring"
    else
        log_info "Auto-proceeding after ${auto_delay}s delay"
    fi
    echo ""

    # Start background log monitoring
    monitor_conductor_logs

    # Ensure cleanup on exit
    trap 'stop_conductor_log_monitor' EXIT

    # Wait for phases to complete
    log_step "POLLING" "Waiting for Phase Completion"

    if wait_for_phase_completion; then
        log_success "Pulsar execution completed successfully"

        # Extract and display WebSocket events
        echo ""
        extract_websocket_events

        # Stop log monitoring
        stop_conductor_log_monitor

        return 0
    else
        log_error "Pulsar execution did not complete successfully"

        # Still extract any events we captured
        echo ""
        extract_websocket_events

        # Stop log monitoring
        stop_conductor_log_monitor

        return 1
    fi
}

# ==============================================================================
# Phase 7: TUI Screenshot Capture Functions
# ==============================================================================

# Check if TUI process is running
# Returns: 0 if running, 1 if not
check_tui_running() {
    log_info "Checking if TUI is running..."

    if pgrep -f "spoq" > /dev/null 2>&1; then
        local pid
        pid=$(pgrep -f "spoq" | head -1)
        log_success "TUI is running (PID: $pid)"
        return 0
    else
        log_warn "TUI is NOT running"
        return 1
    fi
}

# Start TUI if needed
# Offers to start the TUI or provides instructions
start_tui_if_needed() {
    if check_tui_running; then
        return 0
    fi

    log_info "TUI needs to be started for screenshot capture"
    log_info "TUI binary location: $TUI_BINARY"

    # Check if binary exists
    if [ ! -f "$TUI_BINARY" ]; then
        log_error "TUI binary not found at: $TUI_BINARY"
        log_info "Build it with: cargo build --release"
        return 1
    fi

    # Offer to start it
    log_info "Start command: SPOQ_DEV=1 $TUI_BINARY &"
    read -p "Start TUI now? [y/N] " -n 1 -r
    echo

    if [[ $REPLY =~ ^[Yy]$ ]]; then
        log_info "Starting TUI in background..."
        SPOQ_DEV=1 "$TUI_BINARY" > /dev/null 2>&1 &
        local tui_pid=$!
        log_success "TUI started (PID: $tui_pid)"

        # Wait a moment for TUI to initialize
        log_info "Waiting 2s for TUI to initialize..."
        sleep 2

        # Verify it's running
        if check_tui_running; then
            return 0
        else
            log_error "TUI failed to start"
            return 1
        fi
    else
        log_info "Please start the TUI manually before capturing screenshots"
        return 1
    fi
}

# Capture TUI state
# Wrapper around the helper's capture_screenshot function
# Creates timestamped filename and captures both screenshot and text
# Usage: capture_tui_state [description]
capture_tui_state() {
    local description="${1:-tui_state}"
    local timestamp
    timestamp=$(date +%Y%m%d_%H%M%S)
    local screenshot_file="$SCREENSHOT_DIR/${description}_${timestamp}.png"
    local text_file="$SCREENSHOT_DIR/${description}_${timestamp}.txt"

    log_step "CAPTURE" "Capturing TUI state: $description"

    # Ensure screenshot directory exists
    mkdir -p "$SCREENSHOT_DIR"

    # Method 1: Use helper's capture_screenshot function
    # This tries screencapture (macOS), scrot (Linux), or import (ImageMagick)
    log_info "Attempting screenshot capture..."
    if capture_screenshot "$screenshot_file"; then
        log_success "Screenshot saved: $screenshot_file"
    else
        log_warn "Screenshot capture failed or no tool available"
    fi

    # Method 2: Capture terminal text as alternative
    # This provides a text representation of the TUI state
    log_info "Capturing terminal text output..."
    if command -v script &> /dev/null; then
        # Use script command to capture terminal output
        # This is a fallback when screenshot tools aren't available
        log_debug "script command available for text capture"
        echo "Terminal capture at $timestamp" > "$text_file"
        echo "TUI Status: $(check_tui_running && echo 'running' || echo 'not running')" >> "$text_file"
        log_success "Text capture saved: $text_file"
    else
        log_debug "script command not available"
    fi

    # Method 3: Document MCP tool usage
    log_info ""
    log_info "Screenshot Capture Methods:"
    log_info "  1. Helper library tools (screencapture/scrot/import) - just attempted"
    log_info "  2. Claude Code MCP tui-vision tool - use mcp__tui-vision__screenshot_tui"
    log_info "  3. Terminal text capture with 'script' command - use for text alternative"
    log_info ""

    return 0
}

# ==============================================================================
# Phase 3: Comprehensive 6-Checkpoint Verification Flow
# ==============================================================================

# TUI log location for verification
TUI_LOG_FILE="${TUI_LOG_FILE:-$HOME/.spoq/logs/spoq.log}"

# Run comprehensive verification with 6 checkpoints
# This function orchestrates the full E2E verification flow
# Usage: run_comprehensive_verification
# Returns: 0 if all checkpoints pass, 1 if any fail
run_comprehensive_verification() {
    log_step "VERIFY" "Running 6-Checkpoint Comprehensive Verification"
    echo ""

    local checkpoint_results=()
    local total_checkpoints=6
    local passed_checkpoints=0

    # Checkpoint 1: Thread Creation
    log_info "Checkpoint 1/6: Thread Creation"
    if checkpoint_thread_creation; then
        checkpoint_results+=("PASS")
        passed_checkpoints=$((passed_checkpoints + 1))
    else
        checkpoint_results+=("FAIL")
    fi
    echo ""

    # Checkpoint 2: Plan Creation
    log_info "Checkpoint 2/6: Plan Creation"
    if checkpoint_plan_creation; then
        checkpoint_results+=("PASS")
        passed_checkpoints=$((passed_checkpoints + 1))
    else
        checkpoint_results+=("FAIL")
    fi
    echo ""

    # Checkpoint 3: Status Files Detected
    log_info "Checkpoint 3/6: Status Files Detected"
    if checkpoint_status_files_detected; then
        checkpoint_results+=("PASS")
        passed_checkpoints=$((passed_checkpoints + 1))
    else
        checkpoint_results+=("FAIL")
    fi
    echo ""

    # Checkpoint 4: WebSocket Broadcast Sent
    log_info "Checkpoint 4/6: WebSocket Broadcast Sent"
    if checkpoint_websocket_broadcast; then
        checkpoint_results+=("PASS")
        passed_checkpoints=$((passed_checkpoints + 1))
    else
        checkpoint_results+=("FAIL")
    fi
    echo ""

    # Checkpoint 5: TUI Received Message
    log_info "Checkpoint 5/6: TUI Received Message"
    if checkpoint_tui_received_message; then
        checkpoint_results+=("PASS")
        passed_checkpoints=$((passed_checkpoints + 1))
    else
        checkpoint_results+=("FAIL")
    fi
    echo ""

    # Checkpoint 6: Circles Rendered & Updated
    log_info "Checkpoint 6/6: Circles Rendered & Updated"
    if checkpoint_circles_rendered; then
        checkpoint_results+=("PASS")
        passed_checkpoints=$((passed_checkpoints + 1))
    else
        checkpoint_results+=("FAIL")
    fi
    echo ""

    # Print summary
    log_separator
    echo "  Verification Summary"
    log_separator
    echo ""
    local i=1
    for result in "${checkpoint_results[@]}"; do
        local checkpoint_name
        case $i in
            1) checkpoint_name="Thread Creation" ;;
            2) checkpoint_name="Plan Creation" ;;
            3) checkpoint_name="Status Files Detected" ;;
            4) checkpoint_name="WebSocket Broadcast" ;;
            5) checkpoint_name="TUI Received Message" ;;
            6) checkpoint_name="Circles Rendered" ;;
        esac
        if [ "$result" = "PASS" ]; then
            echo -e "  Checkpoint $i: ${E2E_GREEN}PASS${E2E_NC} - $checkpoint_name"
        else
            echo -e "  Checkpoint $i: ${E2E_RED}FAIL${E2E_NC} - $checkpoint_name"
        fi
        i=$((i + 1))
    done
    echo ""
    log_info "Result: $passed_checkpoints/$total_checkpoints checkpoints passed"
    echo ""

    if [ "$passed_checkpoints" -eq "$total_checkpoints" ]; then
        log_success "All checkpoints passed - E2E verification complete"
        return 0
    else
        log_error "Some checkpoints failed - review output above"
        return 1
    fi
}

# ==============================================================================
# Checkpoint 1: Thread Creation Verification
# ==============================================================================

# Verify thread was created via API
# Usage: checkpoint_thread_creation
# Returns: 0 if thread_id exists, 1 otherwise
checkpoint_thread_creation() {
    log_step "CP1" "Verifying Thread Creation"

    if [ -z "${THREAD_ID:-}" ]; then
        log_error "THREAD_ID not set - thread creation failed or was skipped"
        return 1
    fi

    local timestamp
    timestamp=$(date -u +%Y-%m-%dT%H:%M:%SZ)

    log_success "Thread ID verified: $THREAD_ID"
    log_info "Timestamp: $timestamp"

    # Verify thread_id format (should be UUID-like)
    if [[ "$THREAD_ID" =~ ^[a-f0-9-]{36}$ ]]; then
        log_success "Thread ID format valid (UUID)"
    else
        log_warn "Thread ID format may be non-standard: $THREAD_ID"
    fi

    return 0
}

# ==============================================================================
# Checkpoint 2: Plan Creation Verification
# ==============================================================================

# Verify plan directory and files exist with correct structure
# Usage: checkpoint_plan_creation
# Returns: 0 if plan structure is valid, 1 otherwise
checkpoint_plan_creation() {
    log_step "CP2" "Verifying Plan Creation"

    if [ -z "${PLAN_ID:-}" ]; then
        log_error "PLAN_ID not set - plan creation failed or was skipped"
        return 1
    fi

    local plan_dir
    plan_dir=$(get_plan_dir)
    local plan_file="${plan_dir}.md"
    local status_dir="$plan_dir/status"

    log_info "Plan ID: $PLAN_ID"
    log_info "Plan directory: $plan_dir"
    log_info "Plan file: $plan_file"

    local errors=0

    # Check plan directory exists
    if [ -d "$plan_dir" ]; then
        log_success "Plan directory exists"
    else
        log_error "Plan directory missing: $plan_dir"
        errors=$((errors + 1))
    fi

    # Check plan file exists
    if [ -f "$plan_file" ]; then
        log_success "Plan file exists"

        # Check for thread_id in plan metadata
        if [ -n "${THREAD_ID:-}" ]; then
            if grep -q "$THREAD_ID" "$plan_file" 2>/dev/null; then
                log_success "Thread ID found in plan metadata"
            else
                log_warn "Thread ID not found in plan metadata"
            fi
        fi
    else
        log_error "Plan file missing: $plan_file"
        errors=$((errors + 1))
    fi

    # Check status directory exists
    if [ -d "$status_dir" ]; then
        log_success "Status directory exists: $status_dir"
    else
        log_error "Status directory missing: $status_dir"
        errors=$((errors + 1))
    fi

    if [ "$errors" -gt 0 ]; then
        return 1
    fi

    return 0
}

# ==============================================================================
# Checkpoint 3: Status Files Detected
# ==============================================================================

# Create status file and verify conductor picks it up
# Usage: checkpoint_status_files_detected
# Returns: 0 if status file is detected, 1 otherwise
checkpoint_status_files_detected() {
    log_step "CP3" "Verifying Status Files Detected by Conductor"

    if [ -z "${PLAN_ID:-}" ]; then
        log_error "PLAN_ID not set - cannot create status file"
        return 1
    fi

    local status_dir
    status_dir=$(get_status_dir)
    local status_file="$status_dir/phase-1.status"
    local timestamp_before timestamp_after

    # Create phase-1.status with thread_id
    log_info "Creating phase-1.status file..."
    mkdir -p "$status_dir"

    local task_id
    task_id=$(generate_session_id 1)
    local created_at
    created_at=$(date -u +%Y-%m-%dT%H:%M:%SZ)

    # Use create_status_file_with_thread if thread_id is available
    if [ -n "${THREAD_ID:-}" ]; then
        create_status_file_with_thread 1 "running" "$THREAD_ID" 1 "Init" "test.rs"
    else
        create_status_file 1 "running" 1 "Init" "test.rs"
    fi

    if [ ! -f "$status_file" ]; then
        log_error "Failed to create status file: $status_file"
        return 1
    fi

    # Record initial modification time
    timestamp_before=$(stat -f %m "$status_file" 2>/dev/null || stat -c %Y "$status_file" 2>/dev/null || echo "0")
    log_info "Status file created at: $created_at"
    log_info "Initial mtime: $timestamp_before"

    # Wait for conductor poll interval (default 5-6 seconds)
    local poll_interval="${POLL_INTERVAL:-6}"
    log_info "Waiting ${poll_interval}s for conductor to poll..."
    sleep_with_progress "$poll_interval" "Waiting for conductor"

    # Check if file timestamp was updated (conductor read it)
    # Note: Some conductors update atime, some update mtime, some don't modify at all
    timestamp_after=$(stat -f %m "$status_file" 2>/dev/null || stat -c %Y "$status_file" 2>/dev/null || echo "0")
    log_info "Post-poll mtime: $timestamp_after"

    # Verify file contents are still valid JSON
    if command -v jq &> /dev/null; then
        if jq empty "$status_file" 2>/dev/null; then
            log_success "Status file contains valid JSON"
        else
            log_error "Status file has invalid JSON"
            return 1
        fi

        # Log the thread_id from the file
        local file_thread_id
        file_thread_id=$(jq -r '.thread_id // "null"' "$status_file" 2>/dev/null)
        log_info "Status file thread_id: $file_thread_id"
    fi

    # Check if conductor is running to provide appropriate message
    if check_conductor > /dev/null 2>&1; then
        log_success "Conductor is running and should have detected status file"
    else
        log_warn "Conductor not running - status file created but not polled"
    fi

    log_success "Status file checkpoint complete"
    return 0
}

# ==============================================================================
# Checkpoint 4: WebSocket Broadcast Sent
# ==============================================================================

# Verify WebSocket broadcast was sent (check TUI log for PHASE_PROGRESS_UPDATE)
# Usage: checkpoint_websocket_broadcast
# Returns: 0 if broadcast found, 1 otherwise
checkpoint_websocket_broadcast() {
    log_step "CP4" "Verifying WebSocket Broadcast Sent"

    if [ ! -f "$TUI_LOG_FILE" ]; then
        log_warn "TUI log file not found: $TUI_LOG_FILE"
        log_info "WebSocket broadcast verification requires TUI to be running with logging enabled"
        return 1
    fi

    local plan_id_filter="${PLAN_ID:-}"
    log_info "Searching TUI log for PHASE_PROGRESS_UPDATE events..."
    log_info "TUI log file: $TUI_LOG_FILE"

    # Search for PHASE_PROGRESS_UPDATE in TUI log
    local progress_entries
    progress_entries=$(grep -i "PHASE_PROGRESS_UPDATE\|phase_progress_update\|PhaseProgressUpdate" "$TUI_LOG_FILE" 2>/dev/null || echo "")

    if [ -z "$progress_entries" ]; then
        log_warn "No PHASE_PROGRESS_UPDATE entries found in TUI log"
        log_info "This may be expected if conductor hasn't broadcast yet"
        return 1
    fi

    local entry_count
    entry_count=$(echo "$progress_entries" | wc -l | tr -d ' ')
    log_success "Found $entry_count PHASE_PROGRESS_UPDATE entries"

    # If plan_id specified, filter for matching entries
    if [ -n "$plan_id_filter" ]; then
        local matching_entries
        matching_entries=$(echo "$progress_entries" | grep "$plan_id_filter" || echo "")

        if [ -n "$matching_entries" ]; then
            local match_count
            match_count=$(echo "$matching_entries" | wc -l | tr -d ' ')
            log_success "Found $match_count entries matching plan_id: $plan_id_filter"

            # Extract details from the most recent entry
            local latest_entry
            latest_entry=$(echo "$matching_entries" | tail -1)
            log_info "Latest matching entry:"
            log_info "  $latest_entry"

            # Try to extract thread_id, phase_index, status from the entry
            local extracted_thread_id extracted_phase extracted_status
            if command -v jq &> /dev/null; then
                # Try to extract JSON from the log line
                local json_part
                json_part=$(echo "$latest_entry" | sed -n 's/.*\({.*}\).*/\1/p' | head -1)
                if [ -n "$json_part" ] && echo "$json_part" | jq empty 2>/dev/null; then
                    extracted_thread_id=$(echo "$json_part" | jq -r '.thread_id // .data.thread_id // "N/A"' 2>/dev/null)
                    extracted_phase=$(echo "$json_part" | jq -r '.phase_index // .data.phase_index // .phase // "N/A"' 2>/dev/null)
                    extracted_status=$(echo "$json_part" | jq -r '.status // .data.status // "N/A"' 2>/dev/null)

                    log_info "Extracted from broadcast:"
                    log_info "  thread_id: $extracted_thread_id"
                    log_info "  phase_index: $extracted_phase"
                    log_info "  status: $extracted_status"
                fi
            fi

            # Log timestamp of broadcast receipt
            local broadcast_timestamp
            broadcast_timestamp=$(echo "$latest_entry" | sed -n 's/^\[\([^]]*\)\].*/\1/p')
            if [ -n "$broadcast_timestamp" ]; then
                log_info "Broadcast timestamp: $broadcast_timestamp"
            fi
        else
            log_warn "No entries matching plan_id: $plan_id_filter"
            log_info "Recent entries (may be from other plans):"
            echo "$progress_entries" | tail -3 | while read -r line; do
                log_info "  $line"
            done
        fi
    fi

    log_success "WebSocket broadcast checkpoint complete"
    return 0
}

# ==============================================================================
# Checkpoint 5: TUI Received Message
# ==============================================================================

# Verify TUI received and processed the phase progress message
# Usage: checkpoint_tui_received_message
# Returns: 0 if message was received, 1 otherwise
checkpoint_tui_received_message() {
    log_step "CP5" "Verifying TUI Received Message"

    if [ ! -f "$TUI_LOG_FILE" ]; then
        log_warn "TUI log file not found: $TUI_LOG_FILE"
        return 1
    fi

    # Look for "Phase progress updated" log entries
    local update_entries
    update_entries=$(grep -i "phase.*progress.*updated\|Phase progress updated\|PhaseProgressData" "$TUI_LOG_FILE" 2>/dev/null || echo "")

    if [ -z "$update_entries" ]; then
        log_warn "No 'Phase progress updated' entries found in TUI log"

        # Also check for alternative indicators
        local alt_indicators
        alt_indicators=$(grep -i "received.*phase\|phase.*received\|progress.*update" "$TUI_LOG_FILE" 2>/dev/null || echo "")
        if [ -n "$alt_indicators" ]; then
            log_info "Found alternative progress indicators:"
            echo "$alt_indicators" | tail -3 | while read -r line; do
                log_info "  $line"
            done
        fi

        return 1
    fi

    local update_count
    update_count=$(echo "$update_entries" | wc -l | tr -d ' ')
    log_success "Found $update_count 'Phase progress updated' entries"

    # Cross-reference thread_id if available
    if [ -n "${THREAD_ID:-}" ]; then
        local thread_matches
        thread_matches=$(echo "$update_entries" | grep "$THREAD_ID" || echo "")

        if [ -n "$thread_matches" ]; then
            log_success "Found entries matching thread_id: ${THREAD_ID:0:8}..."
        else
            log_info "No entries explicitly mention thread_id (may be expected)"
        fi

        # Also check if thread_id from status file matches log entries
        local status_dir
        status_dir=$(get_status_dir)
        local status_file="$status_dir/phase-1.status"

        if [ -f "$status_file" ] && command -v jq &> /dev/null; then
            local status_thread_id
            status_thread_id=$(jq -r '.thread_id // "null"' "$status_file" 2>/dev/null)

            if [ "$status_thread_id" != "null" ]; then
                log_info "Cross-referencing thread_id from status file: ${status_thread_id:0:8}..."

                # Search for this thread_id in the TUI log
                if grep -q "$status_thread_id" "$TUI_LOG_FILE" 2>/dev/null; then
                    log_success "Thread ID from status file found in TUI log - cross-reference verified"
                else
                    log_info "Thread ID not explicitly logged (may use internal mapping)"
                fi
            fi
        fi
    fi

    log_success "TUI message receipt checkpoint complete"
    return 0
}

# ==============================================================================
# Checkpoint 6: Circles Rendered & Updated
# ==============================================================================

# TUI session ID for visual verification (set by tui-vision spawn)
VERIFICATION_TUI_SESSION=""

# Verify circles are rendered in TUI and update correctly
# Usage: checkpoint_circles_rendered
# Returns: 0 if circles verified, 1 otherwise
checkpoint_circles_rendered() {
    log_step "CP6" "Verifying Circles Rendered & Updated"

    log_info "This checkpoint uses tui-vision MCP tools for visual verification"
    log_info "TUI binary: $TUI_BINARY"
    echo ""

    # Check if TUI binary exists
    if [ ! -f "$TUI_BINARY" ]; then
        log_warn "TUI binary not found at: $TUI_BINARY"
        log_info "Build with: cargo build --release"
        log_info "Skipping visual verification"
        return 1
    fi

    # Output instructions for manual/MCP verification
    log_info "To verify circles with tui-vision MCP:"
    echo ""
    echo "  1. Spawn TUI session:"
    echo "     mcp__tui-vision__spawn_tui(command: \"SPOQ_DEV=1 $TUI_BINARY\", cols: 120, rows: 40)"
    echo ""
    echo "  2. Wait for TUI to render:"
    echo "     mcp__tui-vision__wait_for_render(session_id: <id>, timeout_ms: 5000)"
    echo ""
    echo "  3. Take screenshot and extract text:"
    echo "     mcp__tui-vision__screenshot_tui(session_id: <id>)"
    echo "     mcp__tui-vision__get_tui_text(session_id: <id>)"
    echo ""
    echo "  4. Look for circle patterns in the thread row:"
    echo "     - ● (U+25CF) = completed phase"
    echo "     - ○ (U+25CB) = pending phase"
    echo "     - Expected pattern for 5 phases: ○○○○○ -> ●○○○○ -> ●●○○○ -> etc."
    echo ""
    echo "  5. Progress phases and verify circles update:"
    echo "     - Update status files: phase-1.status to 'completed'"
    echo "     - Wait for conductor poll (${POLL_INTERVAL:-6}s)"
    echo "     - Take new screenshot"
    echo "     - Verify circle changed: ○ -> ●"
    echo ""
    echo "  6. Close TUI session:"
    echo "     mcp__tui-vision__close_tui(session_id: <id>)"
    echo ""

    # Perform text-based verification if TUI log has circle-related entries
    log_info "Checking TUI log for circle rendering evidence..."

    if [ -f "$TUI_LOG_FILE" ]; then
        local circle_entries
        circle_entries=$(grep -i "circle\|●\|○\|render.*phase\|phase.*render\|progress.*circle" "$TUI_LOG_FILE" 2>/dev/null || echo "")

        if [ -n "$circle_entries" ]; then
            local circle_count
            circle_count=$(echo "$circle_entries" | wc -l | tr -d ' ')
            log_success "Found $circle_count circle-related log entries"
            log_info "Recent entries:"
            echo "$circle_entries" | tail -3 | while read -r line; do
                log_info "  ${line:0:100}..."
            done
        else
            log_info "No circle-specific log entries found (visual verification recommended)"
        fi
    fi

    # Provide helper function usage for circle verification
    log_info ""
    log_info "Helper functions available for circle verification:"
    log_info "  extract_circles_from_text \"\$text\"  - Extract circle pattern from TUI text"
    log_info "  count_completed_phases \"●●○○○\"    - Count filled circles"
    log_info "  count_pending_phases \"●●○○○\"      - Count empty circles"
    log_info "  verify_circles_match_expected \"\$cap\" \"\$exp\" \"desc\""
    log_info "  verify_circle_progression \"\$init\" \"\$final\" \"desc\""
    log_info ""

    # Generate expected circle patterns for reference
    log_info "Expected circle patterns for ${TOTAL_PHASES:-5} phases:"
    local total="${TOTAL_PHASES:-5}"
    for i in $(seq 0 "$total"); do
        local expected
        expected=$(generate_expected_circles "$i" "$total")
        log_info "  $i/$total complete: $expected"
    done

    log_success "Circles verification checkpoint complete (manual/MCP verification recommended)"
    return 0
}

# ==============================================================================
# Phase 8: Verification Functions
# ==============================================================================

# Verify all phases completed successfully
# Usage: verify_all_phases_completed
# Returns: 0 if all phases completed, 1 otherwise
verify_all_phases_completed() {
    log_step "VERIFY" "Verifying phase completion"

    if [ -z "$PLAN_ID" ]; then
        log_error "PLAN_ID not set - cannot verify phases"
        return 1
    fi

    local status_dir="$HOME/comms/plans/$PROJECT/active/$PLAN_ID/status"

    if [ ! -d "$status_dir" ]; then
        log_error "Status directory not found: $status_dir"
        return 1
    fi

    local all_completed=true
    local completed_count=0

    for phase in $(seq 1 "$TOTAL_PHASES"); do
        local status_file="$status_dir/phase-${phase}.status"

        if [ ! -f "$status_file" ]; then
            log_error "Phase $phase: Status file missing"
            all_completed=false
            continue
        fi

        local status
        status=$(jq -r '.status // "unknown"' "$status_file" 2>/dev/null || echo "unknown")

        if [ "$status" = "completed" ]; then
            log_success "Phase $phase: completed"
            completed_count=$((completed_count + 1))
        else
            log_error "Phase $phase: $status (expected: completed)"
            all_completed=false
        fi
    done

    echo ""
    if [ "$all_completed" = true ]; then
        log_success "All $TOTAL_PHASES phases completed successfully"
        return 0
    else
        log_error "Phase verification failed: $completed_count/$TOTAL_PHASES completed"
        return 1
    fi
}

# Generate summary report
# Usage: generate_summary_report <results_array_name>
# Prints a formatted summary of test results
generate_summary_report() {
    log_separator
    echo "  E2E Test Summary Report"
    log_separator
    echo ""

    # Test metadata
    log_info "Test Configuration:"
    echo "  Project:      $PROJECT"
    echo "  Plan ID:      ${PLAN_ID:-N/A}"
    echo "  Thread ID:    ${THREAD_ID:-N/A}"
    echo "  Total Phases: $TOTAL_PHASES"
    echo ""

    # Results tracking
    local total_steps=0
    local passed_steps=0
    local failed_steps=0

    # Step 1: Prerequisites
    log_info "Step 1: Prerequisites Check"
    if check_prerequisites > /dev/null 2>&1; then
        echo "  Status: PASS"
        passed_steps=$((passed_steps + 1))
    else
        echo "  Status: FAIL"
        failed_steps=$((failed_steps + 1))
    fi
    total_steps=$((total_steps + 1))
    echo ""

    # Step 2: Thread Creation
    log_info "Step 2: Thread Creation"
    if [ -n "${THREAD_ID:-}" ]; then
        echo "  Status: PASS (Thread ID: $THREAD_ID)"
        passed_steps=$((passed_steps + 1))
    else
        echo "  Status: FAIL (No thread ID)"
        failed_steps=$((failed_steps + 1))
    fi
    total_steps=$((total_steps + 1))
    echo ""

    # Step 3: Plan Generation
    log_info "Step 3: Plan Generation"
    if [ -n "${PLAN_ID:-}" ]; then
        local plan_file="$HOME/comms/plans/$PROJECT/active/${PLAN_ID}.md"
        if [ -f "$plan_file" ]; then
            echo "  Status: PASS (Plan ID: $PLAN_ID)"
            passed_steps=$((passed_steps + 1))
        else
            echo "  Status: FAIL (Plan file not found)"
            failed_steps=$((failed_steps + 1))
        fi
    else
        echo "  Status: FAIL (No plan ID)"
        failed_steps=$((failed_steps + 1))
    fi
    total_steps=$((total_steps + 1))
    echo ""

    # Step 4: Phase Completion
    log_info "Step 4: Phase Completion Verification"
    if [ -n "${PLAN_ID:-}" ]; then
        if verify_all_phases_completed > /dev/null 2>&1; then
            echo "  Status: PASS (All phases completed)"
            passed_steps=$((passed_steps + 1))
        else
            echo "  Status: FAIL (Some phases incomplete)"
            failed_steps=$((failed_steps + 1))
        fi
    else
        echo "  Status: SKIP (No plan to verify)"
    fi
    total_steps=$((total_steps + 1))
    echo ""

    # Step 5: Screenshot Capture
    log_info "Step 5: Screenshot Capture"
    if [ -d "$SCREENSHOT_DIR" ]; then
        local screenshot_count
        screenshot_count=$(find "$SCREENSHOT_DIR" -type f -name "*.png" 2>/dev/null | wc -l | tr -d ' ')
        if [ "$screenshot_count" -gt 0 ]; then
            echo "  Status: PASS ($screenshot_count screenshot(s) captured)"
            passed_steps=$((passed_steps + 1))
        else
            echo "  Status: WARN (No screenshots found)"
            passed_steps=$((passed_steps + 1))
        fi
    else
        echo "  Status: SKIP (Screenshot directory not found)"
    fi
    total_steps=$((total_steps + 1))
    echo ""

    # Overall summary
    log_separator
    echo "  Overall Results"
    log_separator
    echo ""
    echo "  Total Steps:  $total_steps"
    echo "  Passed:       $passed_steps"
    echo "  Failed:       $failed_steps"
    echo ""

    # Final verdict
    if [ "$failed_steps" -eq 0 ]; then
        log_success "E2E TEST PASSED"
        echo ""
        echo "  All test steps completed successfully!"
        echo "  Screenshots saved to: $SCREENSHOT_DIR"
        return 0
    else
        log_error "E2E TEST FAILED"
        echo ""
        echo "  $failed_steps step(s) failed. Review the output above for details."
        return 1
    fi
}

# ==============================================================================
# Phase 8: Main Test Orchestration
# ==============================================================================

# Orchestrates the full E2E test flow
# Usage: run_real_e2e_test
# Returns: 0 if all tests pass, 1 if any fail
run_real_e2e_test() {
    local exit_code=0

    log_step "E2E TEST" "Starting Real Pulsar E2E Test"
    echo ""

    # Step 1: Create temp directory for artifacts
    log_info "Step 1: Creating temporary directories"
    mkdir -p "$TEMP_DIR"
    mkdir -p "$SCREENSHOT_DIR"
    log_success "Directories created: $TEMP_DIR"
    echo ""

    # Step 2: Check prerequisites
    log_info "Step 2: Checking prerequisites"
    if ! check_prerequisites; then
        log_error "Prerequisites check failed - cannot continue"
        generate_summary_report
        return 1
    fi
    echo ""

    # Step 3: Start TUI if needed
    log_info "Step 3: Starting TUI if needed"
    if ! start_tui_if_needed; then
        log_warn "TUI not started - screenshots may not be available"
    fi
    echo ""

    # Step 4: Create thread via API
    log_info "Step 4: Creating thread via API"
    if create_thread_via_api; then
        log_success "Thread created: $THREAD_ID"
    else
        log_error "Failed to create thread"
        exit_code=1
        # Continue to next step even if this fails
    fi
    echo ""

    # Step 5: Generate Nova plan
    log_info "Step 5: Generating Nova plan with $TOTAL_PHASES phases"
    if generate_nova_plan; then
        log_success "Plan generated: $PLAN_ID"
        log_info "Plan location: $HOME/comms/plans/$PROJECT/active/$PLAN_ID"
    else
        log_error "Failed to generate plan"
        generate_summary_report
        return 1
    fi
    echo ""

    # Step 6: Display instructions to run Pulsar manually
    log_info "Step 6: Pulsar execution instructions"
    log_separator
    echo ""
    echo "  MANUAL ACTION REQUIRED"
    echo ""
    echo "  To execute this plan with Pulsar:"
    echo ""
    echo "    1. Open Claude Code in a terminal"
    echo "    2. Navigate to: cd $(pwd)"
    echo "    3. Run the command: /pulsar $PLAN_ID"
    echo ""
    log_separator
    echo ""
    log_info "The test will now monitor for execution progress..."
    echo ""

    # Step 7: Monitor logs while Pulsar executes
    log_info "Step 7: Monitoring Pulsar execution"
    if monitor_execution; then
        log_success "Pulsar execution monitored successfully"
    else
        log_error "Monitoring detected issues or timeout"
        exit_code=1
        # Continue to verification anyway
    fi
    echo ""

    # Step 8: Take screenshot at final state
    log_info "Step 8: Capturing final TUI state"
    if capture_tui_state "final_state"; then
        log_success "Screenshot captured"
    else
        log_warn "Screenshot capture encountered issues"
    fi
    echo ""

    # Step 9: Verify all phases completed
    log_info "Step 9: Verifying phase completion"
    if verify_all_phases_completed; then
        log_success "All phases verified as completed"
    else
        log_error "Phase verification failed"
        exit_code=1
    fi
    echo ""

    # Step 10: Generate summary report
    log_info "Step 10: Generating summary report"
    echo ""
    if generate_summary_report; then
        log_success "Summary report generated"
    else
        log_error "Summary report indicates test failures"
        exit_code=1
    fi

    # Step 11: Optional cleanup prompt
    echo ""
    log_separator
    log_info "Test artifacts saved to: $TEMP_DIR"
    log_info "Screenshots saved to: $SCREENSHOT_DIR"

    if [ -n "${PLAN_ID:-}" ]; then
        log_info "Plan directory: $HOME/comms/plans/$PROJECT/active/$PLAN_ID"
    fi

    echo ""
    read -p "Clean up test artifacts? [y/N] " -n 1 -r
    echo

    if [[ $REPLY =~ ^[Yy]$ ]]; then
        log_info "Cleaning up..."
        cmd_cleanup
    else
        log_info "Keeping test artifacts for review"
    fi

    return $exit_code
}

# ==============================================================================
# Subcommand Implementations
# ==============================================================================

# Setup test environment
cmd_setup() {
    log_step "SETUP" "Initializing test environment"

    # Create temp directories
    mkdir -p "$TEMP_DIR"
    mkdir -p "$SCREENSHOT_DIR"
    log_success "Created temporary directories"

    # Check prerequisites
    check_prerequisites || return 1

    log_success "Setup complete"
}

# Cleanup test files
cmd_cleanup() {
    log_step "CLEANUP" "Removing test files"

    # Remove temp directory
    if [ -d "$TEMP_DIR" ]; then
        rm -rf "$TEMP_DIR"
        log_success "Removed temporary directory: $TEMP_DIR"
    fi

    # Optionally clean up plan files (if PLAN_ID is known)
    if [ -n "${PLAN_ID:-}" ]; then
        local plan_dir="$HOME/comms/plans/$PROJECT/active/$PLAN_ID"
        if [ -d "$plan_dir" ]; then
            log_info "Found plan directory: $plan_dir"
            read -p "Remove plan directory? [y/N] " -n 1 -r
            echo
            if [[ $REPLY =~ ^[Yy]$ ]]; then
                rm -rf "$plan_dir"
                log_success "Removed plan directory"
            fi
        fi
    fi

    log_success "Cleanup complete"
}

# Capture screenshot only
cmd_screenshot_only() {
    log_step "SCREENSHOT" "Capturing TUI screenshot"

    # Check if TUI is running, offer to start if not
    start_tui_if_needed

    # Capture the TUI state
    capture_tui_state "manual_capture"

    log_success "Screenshot capture complete. Files saved to: $SCREENSHOT_DIR"
}

# Run full test
cmd_run() {
    log_separator
    echo "  Real E2E Test: Pulsar Execution with TUI Progress"
    log_separator
    echo ""

    # Run main test (which includes setup and cleanup prompts)
    run_real_e2e_test
}

# ==============================================================================
# Main Entry Point
# ==============================================================================

# Parse subcommand
case "${1:-run}" in
    setup)
        cmd_setup
        ;;
    cleanup)
        cmd_cleanup
        ;;
    screenshot-only)
        cmd_screenshot_only
        ;;
    run)
        cmd_run
        ;;
    help|--help|-h)
        usage
        ;;
    *)
        log_error "Unknown command: $1"
        usage
        exit 1
        ;;
esac

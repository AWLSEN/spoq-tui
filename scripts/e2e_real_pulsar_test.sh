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
API_CREATE_THREAD="/api/threads"
API_GENERATE_PLAN="/api/plans/generate"
API_TRIGGER_PULSAR="/api/plans/execute"

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
# Placeholder Functions (to be implemented in later phases)
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

# Phase 6: Monitor execution progress
# Monitors Pulsar execution by watching status files
# Logs progress updates as phases complete
monitor_execution() {
    log_warn "monitor_execution: Not yet implemented (Phase 6)"
    # TODO: Implement status file monitoring loop
    # Expected to watch: ~/comms/plans/$PROJECT/active/$PLAN_ID/status/
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

# Phase 8: Main test orchestration
# Orchestrates the full E2E test flow
run_real_e2e_test() {
    log_warn "run_real_e2e_test: Not yet implemented (Phase 8)"
    # TODO: Implement main test flow:
    # 1. Call create_thread_via_api
    # 2. Call generate_nova_plan
    # 3. Call monitor_execution (in background)
    # 4. Call capture_screenshot at key moments
    # 5. Verify results
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

    # Setup environment
    cmd_setup || return 1
    echo ""

    # Run main test
    run_real_e2e_test

    # Prompt for cleanup
    echo ""
    log_separator
    read -p "Press Enter to cleanup test files (or Ctrl+C to keep them)..."
    cmd_cleanup
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

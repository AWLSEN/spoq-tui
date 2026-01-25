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
    log_warn "create_thread_via_api: Not yet implemented (Phase 4)"
    # TODO: Implement API call to create thread
    # Expected to set: THREAD_ID
}

# Phase 5: Generate Nova plan
# Generates a Nova plan for the thread using the API
# Sets PLAN_ID global variable on success
generate_nova_plan() {
    log_warn "generate_nova_plan: Not yet implemented (Phase 5)"
    # TODO: Implement API call to generate plan
    # Expected to set: PLAN_ID
}

# Phase 6: Monitor execution progress
# Monitors Pulsar execution by watching status files
# Logs progress updates as phases complete
monitor_execution() {
    log_warn "monitor_execution: Not yet implemented (Phase 6)"
    # TODO: Implement status file monitoring loop
    # Expected to watch: ~/comms/plans/$PROJECT/active/$PLAN_ID/status/
}

# Phase 7: Capture screenshot - uses capture_screenshot() from e2e_helpers.sh
# The helper function captures screenshots of the TUI showing progress circles
# Usage: capture_screenshot [output_path] [session_id]
# Default output path: /tmp/e2e_screenshot_<timestamp>.png
# Output is saved to SCREENSHOT_DIR by cmd_screenshot_only()

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

    mkdir -p "$SCREENSHOT_DIR"
    local output_path="$SCREENSHOT_DIR/tui_$(date +%Y%m%d_%H%M%S).png"
    capture_screenshot "$output_path"
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

#!/bin/bash
#
# Unit tests for TUI-Vision Screenshot Verification Functions
# Tests the helper functions in e2e_helpers.sh
#

set -e

# Get the directory of this script
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LIB_DIR="$(dirname "$SCRIPT_DIR")/lib"

# Source the helpers library
# shellcheck source=../lib/e2e_helpers.sh
source "$LIB_DIR/e2e_helpers.sh"

# Test counters
TESTS_RUN=0
TESTS_PASSED=0
TESTS_FAILED=0

# ==============================================================================
# Test Helpers
# ==============================================================================

test_start() {
    local test_name="$1"
    echo -n "Testing: $test_name... "
    TESTS_RUN=$((TESTS_RUN + 1))
}

test_pass() {
    echo "PASS"
    TESTS_PASSED=$((TESTS_PASSED + 1))
}

test_fail() {
    local message="${1:-}"
    echo "FAIL"
    if [ -n "$message" ]; then
        echo "  Error: $message"
    fi
    TESTS_FAILED=$((TESTS_FAILED + 1))
}

# ==============================================================================
# Tests for Circle Pattern Functions
# ==============================================================================

test_extract_circles_from_text() {
    test_start "extract_circles_from_text"

    local text="Plan: test-plan ●●○○○ Status: running"
    local result
    result=$(extract_circles_from_text "$text")

    if [ "$result" = "●●○○○" ]; then
        test_pass
    else
        test_fail "Expected '●●○○○', got '$result'"
    fi
}

test_extract_circles_multiline() {
    test_start "extract_circles_from_text (multiline)"

    local text="Thread 1: ●○○
Thread 2: ●●○"
    local result
    result=$(extract_circles_from_text "$text")

    if [ "$result" = "●○○●●○" ]; then
        test_pass
    else
        test_fail "Expected '●○○●●○', got '$result'"
    fi
}

test_extract_circles_empty() {
    test_start "extract_circles_from_text (empty input)"

    local result
    # The function returns 1 for empty input, so we need to handle that
    result=$(extract_circles_from_text "" || true)

    if [ -z "$result" ]; then
        test_pass
    else
        test_fail "Expected empty, got '$result'"
    fi
}

test_count_completed_phases() {
    test_start "count_completed_phases"

    local result
    result=$(count_completed_phases "●●●○○")

    if [ "$result" = "3" ]; then
        test_pass
    else
        test_fail "Expected '3', got '$result'"
    fi
}

test_count_pending_phases() {
    test_start "count_pending_phases"

    local result
    result=$(count_pending_phases "●●○○○")

    if [ "$result" = "3" ]; then
        test_pass
    else
        test_fail "Expected '3', got '$result'"
    fi
}

test_count_in_progress_phases() {
    test_start "count_in_progress_phases"

    local result
    result=$(count_in_progress_phases "●◐○○")

    if [ "$result" = "1" ]; then
        test_pass
    else
        test_fail "Expected '1', got '$result'"
    fi
}

test_count_total_phases() {
    test_start "count_total_phases"

    local result
    result=$(count_total_phases "●●○○○")

    if [ "$result" = "5" ]; then
        test_pass
    else
        test_fail "Expected '5', got '$result'"
    fi
}

test_generate_expected_circles() {
    test_start "generate_expected_circles"

    local result
    result=$(generate_expected_circles 2 5)

    if [ "$result" = "●●○○○" ]; then
        test_pass
    else
        test_fail "Expected '●●○○○', got '$result'"
    fi
}

test_generate_expected_circles_with_in_progress() {
    test_start "generate_expected_circles (with in_progress)"

    local result
    result=$(generate_expected_circles 2 5 1)

    if [ "$result" = "●●◐○○" ]; then
        test_pass
    else
        test_fail "Expected '●●◐○○', got '$result'"
    fi
}

test_generate_expected_circles_all_complete() {
    test_start "generate_expected_circles (all complete)"

    local result
    result=$(generate_expected_circles 5 5)

    if [ "$result" = "●●●●●" ]; then
        test_pass
    else
        test_fail "Expected '●●●●●', got '$result'"
    fi
}

test_generate_expected_circles_all_pending() {
    test_start "generate_expected_circles (all pending)"

    local result
    result=$(generate_expected_circles 0 5)

    if [ "$result" = "○○○○○" ]; then
        test_pass
    else
        test_fail "Expected '○○○○○', got '$result'"
    fi
}

# ==============================================================================
# Tests for Circle Verification Functions
# ==============================================================================

test_verify_circles_match_expected_pass() {
    test_start "verify_circles_match_expected (match)"

    # Suppress log output for test
    if verify_circles_match_expected "●●○○○" "●●○○○" "test" > /dev/null 2>&1; then
        test_pass
    else
        test_fail "Expected verification to pass"
    fi
}

test_verify_circles_match_expected_fail() {
    test_start "verify_circles_match_expected (mismatch)"

    # Suppress log output for test
    if ! verify_circles_match_expected "●●○○○" "●●●○○" "test" > /dev/null 2>&1; then
        test_pass
    else
        test_fail "Expected verification to fail"
    fi
}

test_verify_circle_progression_pass() {
    test_start "verify_circle_progression (forward progress)"

    # Suppress log output for test
    if verify_circle_progression "○○○○○" "●●●○○" "test" > /dev/null 2>&1; then
        test_pass
    else
        test_fail "Expected progression verification to pass"
    fi
}

test_verify_circle_progression_no_change() {
    test_start "verify_circle_progression (no change)"

    # Suppress log output for test
    if ! verify_circle_progression "●●○○○" "●●○○○" "test" > /dev/null 2>&1; then
        test_pass
    else
        test_fail "Expected no-change to return failure"
    fi
}

test_verify_circle_progression_regression() {
    test_start "verify_circle_progression (regression)"

    # Suppress log output for test
    if ! verify_circle_progression "●●●○○" "●●○○○" "test" > /dev/null 2>&1; then
        test_pass
    else
        test_fail "Expected regression to return failure"
    fi
}

# ==============================================================================
# Tests for TUI Text Ready Detection
# ==============================================================================

test_tui_text_is_ready_with_circles() {
    test_start "tui_text_is_ready (with circles)"

    local text="Plan Status: ●●○○○ Active"
    if tui_text_is_ready "$text" > /dev/null 2>&1; then
        test_pass
    else
        test_fail "Expected text with circles to be ready"
    fi
}

test_tui_text_is_ready_with_thread() {
    test_start "tui_text_is_ready (with thread)"

    local text="Active Thread: my-thread-001"
    if tui_text_is_ready "$text" > /dev/null 2>&1; then
        test_pass
    else
        test_fail "Expected text with thread to be ready"
    fi
}

test_tui_text_is_ready_empty() {
    test_start "tui_text_is_ready (empty)"

    if ! tui_text_is_ready "" > /dev/null 2>&1; then
        test_pass
    else
        test_fail "Expected empty text to not be ready"
    fi
}

# ==============================================================================
# Tests for State Storage Functions
# ==============================================================================

test_store_and_get_circle_states() {
    test_start "store_circle_state and get_captured_states"

    # Clear any existing states
    clear_captured_states > /dev/null 2>&1

    # Store some states
    store_circle_state "○○○○○" "initial" > /dev/null 2>&1
    store_circle_state "●○○○○" "phase1_done" > /dev/null 2>&1
    store_circle_state "●●○○○" "phase2_done" > /dev/null 2>&1

    # Get states and count them
    local states
    states=$(get_captured_states)
    local count
    count=$(echo "$states" | wc -l | tr -d ' ')

    if [ "$count" = "3" ]; then
        # Verify last state contains expected pattern
        if echo "$states" | grep -q "●●○○○"; then
            test_pass
        else
            test_fail "States don't contain expected pattern"
        fi
    else
        test_fail "Expected 3 states, got $count"
    fi

    # Clean up
    clear_captured_states > /dev/null 2>&1
}

test_clear_captured_states() {
    test_start "clear_captured_states"

    # Store a state
    store_circle_state "●●●●●" "test" > /dev/null 2>&1

    # Clear states
    clear_captured_states > /dev/null 2>&1

    # Verify empty
    local states
    states=$(get_captured_states)

    if [ -z "$states" ]; then
        test_pass
    else
        test_fail "Expected empty states after clear"
    fi
}

# ==============================================================================
# Tests for Session Management Functions
# ==============================================================================

test_set_and_get_tui_session_id() {
    test_start "set_tui_session_id and get_tui_session_id"

    # Set session ID
    set_tui_session_id "test-session-123" > /dev/null 2>&1

    # Get session ID
    local result
    result=$(get_tui_session_id 2>/dev/null)

    if [ "$result" = "test-session-123" ]; then
        test_pass
    else
        test_fail "Expected 'test-session-123', got '$result'"
    fi

    # Clean up
    TUI_SESSION_ID=""
}

test_get_tui_session_id_not_set() {
    test_start "get_tui_session_id (not set)"

    # Ensure not set
    TUI_SESSION_ID=""

    # Should fail
    if ! get_tui_session_id > /dev/null 2>&1; then
        test_pass
    else
        test_fail "Expected failure when session ID not set"
    fi
}

# ==============================================================================
# Tests for MCP Command Preparation
# ==============================================================================

test_spawn_tui_session_output() {
    test_start "spawn_tui_session (output format)"

    local output
    output=$(spawn_tui_session 100 30 "/test/dir" 2>/dev/null)

    # Should contain the tool name
    if echo "$output" | grep -q "mcp__tui-vision__spawn_tui"; then
        # Should contain dimensions
        if echo "$output" | grep -q '"cols": 100' && echo "$output" | grep -q '"rows": 30'; then
            test_pass
        else
            test_fail "Missing or incorrect dimensions in output"
        fi
    else
        test_fail "Missing tool name in output"
    fi
}

test_close_tui_session_output() {
    test_start "close_tui_session (output format)"

    TUI_SESSION_ID="test-session-456"

    local output
    output=$(close_tui_session 2>/dev/null)

    # Should contain the tool name and session ID
    if echo "$output" | grep -q "mcp__tui-vision__close_tui" && \
       echo "$output" | grep -q "test-session-456"; then
        test_pass
    else
        test_fail "Missing tool name or session ID in output"
    fi
}

test_capture_phase_circle_state_output() {
    test_start "capture_phase_circle_state (output format)"

    TUI_SESSION_ID="test-session-789"

    local output
    output=$(capture_phase_circle_state 2>/dev/null)

    # Should contain screenshot and get_tui_text tools
    if echo "$output" | grep -q "mcp__tui-vision__screenshot_tui" && \
       echo "$output" | grep -q "mcp__tui-vision__get_tui_text"; then
        test_pass
    else
        test_fail "Missing expected MCP tools in output"
    fi
}

# ==============================================================================
# Run All Tests
# ==============================================================================

run_all_tests() {
    echo "=========================================="
    echo "TUI-Vision Helper Functions Tests"
    echo "=========================================="
    echo ""

    # Circle pattern functions
    test_extract_circles_from_text
    test_extract_circles_multiline
    test_extract_circles_empty
    test_count_completed_phases
    test_count_pending_phases
    test_count_in_progress_phases
    test_count_total_phases
    test_generate_expected_circles
    test_generate_expected_circles_with_in_progress
    test_generate_expected_circles_all_complete
    test_generate_expected_circles_all_pending

    # Circle verification functions
    test_verify_circles_match_expected_pass
    test_verify_circles_match_expected_fail
    test_verify_circle_progression_pass
    test_verify_circle_progression_no_change
    test_verify_circle_progression_regression

    # TUI ready detection
    test_tui_text_is_ready_with_circles
    test_tui_text_is_ready_with_thread
    test_tui_text_is_ready_empty

    # State storage functions
    test_store_and_get_circle_states
    test_clear_captured_states

    # Session management
    test_set_and_get_tui_session_id
    test_get_tui_session_id_not_set

    # MCP command preparation
    test_spawn_tui_session_output
    test_close_tui_session_output
    test_capture_phase_circle_state_output

    echo ""
    echo "=========================================="
    echo "Test Results"
    echo "=========================================="
    echo "Tests run:    $TESTS_RUN"
    echo "Tests passed: $TESTS_PASSED"
    echo "Tests failed: $TESTS_FAILED"
    echo "=========================================="

    if [ "$TESTS_FAILED" -gt 0 ]; then
        echo "SOME TESTS FAILED"
        exit 1
    else
        echo "ALL TESTS PASSED"
        exit 0
    fi
}

# Run tests if executed directly
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    run_all_tests
fi

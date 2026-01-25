#!/bin/bash
#
# Test Script for E2E Helpers Library
#
# Runs unit tests to verify all functions in e2e_helpers.sh work correctly.
#
# Usage:
#   ./scripts/lib/test_e2e_helpers.sh
#
# Exit codes:
#   0 - All tests passed
#   1 - One or more tests failed
#

set -e

# Get the directory of this script
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Source the library
source "$SCRIPT_DIR/e2e_helpers.sh"

# Test configuration
export PROJECT="test-project"
export PLAN_ID="plan-test-helpers"
export TOTAL_PHASES=3
export COMMS_BASE_DIR="/tmp/e2e_helpers_test_$$"

# Test counters
TESTS_RUN=0
TESTS_PASSED=0
TESTS_FAILED=0

# ==============================================================================
# Test Framework
# ==============================================================================

test_start() {
    echo ""
    echo "========================================"
    echo "  E2E Helpers Library - Test Suite"
    echo "========================================"
    echo ""
}

test_end() {
    echo ""
    echo "========================================"
    echo "  Test Results"
    echo "========================================"
    echo "  Tests run:    $TESTS_RUN"
    echo "  Tests passed: $TESTS_PASSED"
    echo "  Tests failed: $TESTS_FAILED"
    echo "========================================"

    if [ "$TESTS_FAILED" -gt 0 ]; then
        echo -e "\n${RED}FAILED${NC}"
        return 1
    else
        echo -e "\n${GREEN}PASSED${NC}"
        return 0
    fi
}

run_test() {
    local test_name="$1"
    local test_func="$2"

    TESTS_RUN=$((TESTS_RUN + 1))
    echo -e "${BLUE}TEST:${NC} $test_name"

    if $test_func; then
        TESTS_PASSED=$((TESTS_PASSED + 1))
        echo -e "  ${GREEN}PASS${NC}"
    else
        TESTS_FAILED=$((TESTS_FAILED + 1))
        echo -e "  ${RED}FAIL${NC}"
    fi
}

# ==============================================================================
# Test Cases
# ==============================================================================

test_logging_functions() {
    # Test that logging functions don't crash and produce output
    local output

    output=$(log_info "Test info message" 2>&1)
    [ -n "$output" ] || return 1

    output=$(log_success "Test success message" 2>&1)
    [ -n "$output" ] || return 1

    output=$(log_warn "Test warning message" 2>&1)
    [ -n "$output" ] || return 1

    output=$(log_error "Test error message" 2>&1)
    [ -n "$output" ] || return 1

    return 0
}

test_debug_logging() {
    # Test debug logging (only outputs when E2E_DEBUG is set)
    local output

    # Without E2E_DEBUG
    unset E2E_DEBUG
    output=$(log_debug "Should not appear" 2>&1)
    [ -z "$output" ] || return 1

    # With E2E_DEBUG
    export E2E_DEBUG=1
    output=$(log_debug "Should appear" 2>&1)
    [ -n "$output" ] || return 1

    unset E2E_DEBUG
    return 0
}

test_get_plan_dir() {
    local plan_dir
    plan_dir=$(get_plan_dir)

    # Should match expected pattern
    [[ "$plan_dir" == *"/plans/test-project/active/plan-test-helpers" ]] || return 1

    return 0
}

test_get_status_dir() {
    local status_dir
    status_dir=$(get_status_dir)

    # Should end with /status
    [[ "$status_dir" == *"/status" ]] || return 1

    return 0
}

test_init_plan_dirs() {
    init_plan_dirs

    local plan_dir status_dir markers_dir
    plan_dir=$(get_plan_dir)
    status_dir=$(get_status_dir)
    markers_dir=$(get_markers_dir)

    # Directories should exist
    [ -d "$status_dir" ] || return 1
    [ -d "$markers_dir" ] || return 1

    return 0
}

test_generate_uuid() {
    local uuid1 uuid2

    uuid1=$(generate_uuid)
    uuid2=$(generate_uuid)

    # Should not be empty
    [ -n "$uuid1" ] || return 1
    [ -n "$uuid2" ] || return 1

    # Should be different
    [ "$uuid1" != "$uuid2" ] || return 1

    # Should contain hyphens (UUID format)
    [[ "$uuid1" == *"-"* ]] || return 1

    return 0
}

test_generate_session_id() {
    local session_id

    session_id=$(generate_session_id 1)

    # Should start with phase-
    [[ "$session_id" == "phase-1-"* ]] || return 1

    # Should contain plan ID
    [[ "$session_id" == *"$PLAN_ID"* ]] || return 1

    return 0
}

test_generate_thread_id() {
    local thread_id

    thread_id=$(generate_thread_id)

    # Should be a UUID
    [ -n "$thread_id" ] || return 1
    [[ "$thread_id" == *"-"* ]] || return 1

    return 0
}

test_create_plan_file() {
    create_plan_file "Test Plan" "Phase 1" "Phase 2" "Phase 3"

    local plan_dir plan_file
    plan_dir=$(get_plan_dir)
    plan_file="$plan_dir/$PLAN_ID.md"

    # File should exist
    [ -f "$plan_file" ] || return 1

    # Should contain plan ID
    grep -q "$PLAN_ID" "$plan_file" || return 1

    # Should contain phases
    grep -q "Phase 1" "$plan_file" || return 1
    grep -q "Phase 2" "$plan_file" || return 1
    grep -q "Phase 3" "$plan_file" || return 1

    return 0
}

test_create_status_file() {
    create_status_file 1 "running"

    local status_dir status_file
    status_dir=$(get_status_dir)
    status_file="$status_dir/phase-1.status"

    # File should exist
    [ -f "$status_file" ] || return 1

    # Should be valid JSON
    jq empty "$status_file" 2>/dev/null || return 1

    # Should have correct status
    local status
    status=$(jq -r '.status' "$status_file")
    [ "$status" = "running" ] || return 1

    # Should have correct phase
    local phase
    phase=$(jq -r '.phase' "$status_file")
    [ "$phase" = "1" ] || return 1

    return 0
}

test_create_status_file_completed() {
    create_status_file 2 "completed"

    local status_dir status_file
    status_dir=$(get_status_dir)
    status_file="$status_dir/phase-2.status"

    # Should be valid JSON
    jq empty "$status_file" 2>/dev/null || return 1

    # Should have completed_at set (not null)
    local completed_at
    completed_at=$(jq -r '.completed_at' "$status_file")
    [ "$completed_at" != "null" ] || return 1

    return 0
}

test_create_status_file_with_thread() {
    local thread_id="test-thread-12345"
    create_status_file_with_thread 3 "running" "$thread_id"

    local status_dir status_file
    status_dir=$(get_status_dir)
    status_file="$status_dir/phase-3.status"

    # Should be valid JSON
    jq empty "$status_file" 2>/dev/null || return 1

    # Should have thread_id set
    local actual_thread
    actual_thread=$(jq -r '.thread_id' "$status_file")
    [ "$actual_thread" = "$thread_id" ] || return 1

    return 0
}

test_update_status_file() {
    # First create a file
    create_status_file 1 "pending"

    # Then update it
    update_status_file 1 "running"

    local status_dir status_file
    status_dir=$(get_status_dir)
    status_file="$status_dir/phase-1.status"

    # Should have updated status
    local status
    status=$(jq -r '.status' "$status_file")
    [ "$status" = "running" ] || return 1

    return 0
}

test_verify_plan_file() {
    # Create plan first
    create_plan_file "Verify Test"

    # Verify should pass
    verify_plan_file >/dev/null 2>&1 || return 1

    return 0
}

test_verify_status_file() {
    # Create status first
    create_status_file 1 "running"

    # Verify should pass
    verify_status_file 1 >/dev/null 2>&1 || return 1

    return 0
}

test_get_timestamp() {
    local ts
    ts=$(get_timestamp)

    # Should not be empty
    [ -n "$ts" ] || return 1

    # Should be in ISO 8601 format
    [[ "$ts" =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z$ ]] || return 1

    return 0
}

test_get_epoch() {
    local epoch
    epoch=$(get_epoch)

    # Should be a number
    [[ "$epoch" =~ ^[0-9]+$ ]] || return 1

    # Should be recent (after 2020)
    [ "$epoch" -gt 1577836800 ] || return 1

    return 0
}

test_command_exists() {
    # bash should exist
    command_exists bash || return 1

    # nonexistent command should not exist
    ! command_exists nonexistent_command_xyz_12345 || return 1

    return 0
}

test_assert_eq_pass() {
    # Should pass for equal values (suppress output)
    assert_eq "hello" "hello" "test" >/dev/null 2>&1 || return 1
    return 0
}

test_assert_eq_fail() {
    # Should fail for unequal values (suppress output)
    ! assert_eq "hello" "world" "test" >/dev/null 2>&1 || return 1
    return 0
}

test_assert_file_exists_pass() {
    local test_file="/tmp/e2e_test_file_$$"
    touch "$test_file"

    assert_file_exists "$test_file" "test" >/dev/null 2>&1 || return 1

    rm -f "$test_file"
    return 0
}

test_assert_file_exists_fail() {
    ! assert_file_exists "/nonexistent/file/xyz" "test" >/dev/null 2>&1 || return 1
    return 0
}

test_assert_dir_exists_pass() {
    assert_dir_exists "/tmp" "test" >/dev/null 2>&1 || return 1
    return 0
}

test_assert_dir_exists_fail() {
    ! assert_dir_exists "/nonexistent/dir/xyz" "test" >/dev/null 2>&1 || return 1
    return 0
}

test_assert_json_field() {
    local test_file="/tmp/e2e_test_json_$$"
    echo '{"status": "running", "phase": 1}' > "$test_file"

    assert_json_field "$test_file" ".status" "running" "test" >/dev/null 2>&1 || return 1
    assert_json_field "$test_file" ".phase" "1" "test" >/dev/null 2>&1 || return 1

    rm -f "$test_file"
    return 0
}

test_cleanup_plan() {
    # Create some files first
    init_plan_dirs
    create_plan_file "Cleanup Test"

    local plan_dir
    plan_dir=$(get_plan_dir)

    # Directory should exist
    [ -d "$plan_dir" ] || return 1

    # Cleanup
    cleanup_plan >/dev/null 2>&1

    # Directory should be gone
    [ ! -d "$plan_dir" ] || return 1

    return 0
}

# ==============================================================================
# TUI Log Verification Tests
# ==============================================================================

# Create a mock TUI log for testing
setup_mock_tui_log() {
    local mock_log_dir="/tmp/e2e_mock_tui_$$"
    mkdir -p "$mock_log_dir"
    export TUI_LOG_FILE="$mock_log_dir/spoq.log"

    # Create mock log entries
    cat > "$TUI_LOG_FILE" << 'EOF'
[2026-01-25T10:00:00Z] INFO Starting TUI
[2026-01-25T10:00:01Z] PHASE_PROGRESS_UPDATE {"plan_id": "plan-test-helpers", "phase": 1, "status": "running", "thread_id": "abc-123"}
[2026-01-25T10:00:02Z] DEBUG Some debug message
[2026-01-25T10:00:03Z] PHASE_PROGRESS_UPDATE {"plan_id": "plan-test-helpers", "phase": 1, "status": "completed", "thread_id": "abc-123"}
[2026-01-25T10:00:04Z] PHASE_PROGRESS_UPDATE {"plan_id": "plan-test-helpers", "phase": 2, "status": "running", "thread_id": "def-456"}
[2026-01-25T10:00:05Z] INFO Thread loaded: abc-123
[2026-01-25T10:00:06Z] PHASE_PROGRESS_UPDATE {"plan_id": "other-plan", "phase": 1, "status": "running", "thread_id": "xyz-789"}
EOF
}

cleanup_mock_tui_log() {
    local mock_log_dir="/tmp/e2e_mock_tui_$$"
    rm -rf "$mock_log_dir" 2>/dev/null || true
    unset TUI_LOG_FILE
}

test_verify_tui_log_received_success() {
    setup_mock_tui_log

    # Should find entries for our plan
    verify_tui_log_received "plan-test-helpers" 1 >/dev/null 2>&1 || { cleanup_mock_tui_log; return 1; }

    # Should find at least 3 entries for our plan
    verify_tui_log_received "plan-test-helpers" 3 >/dev/null 2>&1 || { cleanup_mock_tui_log; return 1; }

    cleanup_mock_tui_log
    return 0
}

test_verify_tui_log_received_failure() {
    setup_mock_tui_log

    # Should fail when expecting too many entries
    ! verify_tui_log_received "plan-test-helpers" 100 >/dev/null 2>&1 || { cleanup_mock_tui_log; return 1; }

    # Should fail for non-existent plan
    ! verify_tui_log_received "nonexistent-plan" 1 >/dev/null 2>&1 || { cleanup_mock_tui_log; return 1; }

    cleanup_mock_tui_log
    return 0
}

test_verify_tui_log_no_file() {
    # Set to non-existent file
    local old_log="${TUI_LOG_FILE:-}"
    export TUI_LOG_FILE="/nonexistent/path/spoq.log"

    # Should fail gracefully
    ! verify_tui_log_received "any-plan" 1 >/dev/null 2>&1 || { export TUI_LOG_FILE="$old_log"; return 1; }

    export TUI_LOG_FILE="$old_log"
    return 0
}

test_extract_phase_updates_from_log() {
    setup_mock_tui_log

    local output
    output=$(extract_phase_updates_from_log "plan-test-helpers" 2>/dev/null)

    # Should return valid JSON array
    echo "$output" | jq empty 2>/dev/null || { cleanup_mock_tui_log; return 1; }

    # Should have 3 entries for our plan
    local count
    count=$(echo "$output" | jq 'length')
    [ "$count" -eq 3 ] || { cleanup_mock_tui_log; return 1; }

    cleanup_mock_tui_log
    return 0
}

test_extract_phase_updates_empty_log() {
    local mock_log_dir="/tmp/e2e_mock_tui_$$"
    mkdir -p "$mock_log_dir"
    export TUI_LOG_FILE="$mock_log_dir/spoq.log"

    # Create empty log (no PHASE_PROGRESS_UPDATE entries)
    echo "[2026-01-25T10:00:00Z] INFO No phase updates" > "$TUI_LOG_FILE"

    local output
    output=$(extract_phase_updates_from_log 2>/dev/null)

    # Should return empty JSON array
    [ "$output" = "[]" ] || { cleanup_mock_tui_log; return 1; }

    cleanup_mock_tui_log
    return 0
}

test_verify_thread_exists_in_tui_via_log() {
    setup_mock_tui_log

    # Thread abc-123 is mentioned in the log
    verify_thread_exists_in_tui "abc-123" 1 >/dev/null 2>&1 || { cleanup_mock_tui_log; return 1; }

    cleanup_mock_tui_log
    return 0
}

test_verify_thread_not_found() {
    setup_mock_tui_log

    # Non-existent thread should not be found
    ! verify_thread_exists_in_tui "nonexistent-thread-xyz" 1 >/dev/null 2>&1 || { cleanup_mock_tui_log; return 1; }

    cleanup_mock_tui_log
    return 0
}

test_verify_thread_requires_id() {
    # Should fail when no thread ID provided
    ! verify_thread_exists_in_tui "" 1 >/dev/null 2>&1 || return 1
    return 0
}

test_collect_verification_evidence() {
    setup_mock_tui_log

    # Create some status files (suppress output)
    init_plan_dirs >/dev/null 2>&1
    create_status_file 1 "running" >/dev/null 2>&1
    create_status_file 2 "pending" >/dev/null 2>&1
    create_plan_file "Evidence Test" >/dev/null 2>&1

    local evidence_dir
    # The function outputs log messages to stdout, so capture only the last line (the path)
    evidence_dir=$(collect_verification_evidence "/tmp/e2e_evidence_test_$$" 2>/dev/null | tail -1)

    # Evidence directory should exist
    [ -d "$evidence_dir" ] || { cleanup_mock_tui_log; rm -rf "/tmp/e2e_evidence_test_$$"; return 1; }

    # Should contain TUI log tail
    [ -f "$evidence_dir/tui_log_tail.log" ] || { cleanup_mock_tui_log; rm -rf "/tmp/e2e_evidence_test_$$"; return 1; }

    # Should contain phase updates JSON
    [ -f "$evidence_dir/phase_updates.json" ] || { cleanup_mock_tui_log; rm -rf "/tmp/e2e_evidence_test_$$"; return 1; }

    # Should contain summary
    [ -f "$evidence_dir/summary.json" ] || { cleanup_mock_tui_log; rm -rf "/tmp/e2e_evidence_test_$$"; return 1; }

    # Summary should be valid JSON
    jq empty "$evidence_dir/summary.json" 2>/dev/null || { cleanup_mock_tui_log; rm -rf "/tmp/e2e_evidence_test_$$"; return 1; }

    # Should contain status files
    [ -d "$evidence_dir/status_files" ] || { cleanup_mock_tui_log; rm -rf "/tmp/e2e_evidence_test_$$"; return 1; }

    rm -rf "/tmp/e2e_evidence_test_$$"
    cleanup_mock_tui_log
    return 0
}

test_compare_status_with_log_matching() {
    setup_mock_tui_log

    # Create a status file that matches log entries
    init_plan_dirs
    create_status_file 1 "completed"

    # Should pass - status file exists and matches log
    compare_status_with_log 1 "completed" >/dev/null 2>&1 || { cleanup_mock_tui_log; return 1; }

    cleanup_mock_tui_log
    return 0
}

test_compare_status_with_log_status_mismatch() {
    setup_mock_tui_log

    # Create a status file
    init_plan_dirs
    create_status_file 1 "running"

    # Should fail - expected status doesn't match file
    ! compare_status_with_log 1 "completed" >/dev/null 2>&1 || { cleanup_mock_tui_log; return 1; }

    cleanup_mock_tui_log
    return 0
}

test_compare_status_with_log_no_file() {
    setup_mock_tui_log

    # Ensure no status file exists
    local status_dir
    status_dir=$(get_status_dir)
    rm -f "$status_dir/phase-99.status" 2>/dev/null

    # Should fail - status file doesn't exist
    ! compare_status_with_log 99 >/dev/null 2>&1 || { cleanup_mock_tui_log; return 1; }

    cleanup_mock_tui_log
    return 0
}

# ==============================================================================
# Test Runner
# ==============================================================================

cleanup_test_env() {
    rm -rf "$COMMS_BASE_DIR" 2>/dev/null || true
}

main() {
    test_start

    # Ensure clean state
    cleanup_test_env

    # Logging tests
    run_test "Logging functions produce output" test_logging_functions
    run_test "Debug logging respects E2E_DEBUG" test_debug_logging

    # Path functions
    run_test "get_plan_dir returns correct path" test_get_plan_dir
    run_test "get_status_dir returns correct path" test_get_status_dir
    run_test "init_plan_dirs creates directories" test_init_plan_dirs

    # UUID generation
    run_test "generate_uuid produces unique UUIDs" test_generate_uuid
    run_test "generate_session_id includes phase and plan" test_generate_session_id
    run_test "generate_thread_id returns UUID" test_generate_thread_id

    # File creation
    run_test "create_plan_file creates valid markdown" test_create_plan_file
    run_test "create_status_file creates valid JSON" test_create_status_file
    run_test "create_status_file sets completed_at for completed" test_create_status_file_completed
    run_test "create_status_file_with_thread includes thread_id" test_create_status_file_with_thread
    run_test "update_status_file modifies status" test_update_status_file

    # Verification
    run_test "verify_plan_file validates existing plan" test_verify_plan_file
    run_test "verify_status_file validates existing status" test_verify_status_file

    # Utilities
    run_test "get_timestamp returns ISO 8601 format" test_get_timestamp
    run_test "get_epoch returns Unix timestamp" test_get_epoch
    run_test "command_exists detects commands" test_command_exists

    # Assertions
    run_test "assert_eq passes for equal values" test_assert_eq_pass
    run_test "assert_eq fails for unequal values" test_assert_eq_fail
    run_test "assert_file_exists passes for existing file" test_assert_file_exists_pass
    run_test "assert_file_exists fails for missing file" test_assert_file_exists_fail
    run_test "assert_dir_exists passes for existing dir" test_assert_dir_exists_pass
    run_test "assert_dir_exists fails for missing dir" test_assert_dir_exists_fail
    run_test "assert_json_field validates JSON fields" test_assert_json_field

    # Cleanup
    run_test "cleanup_plan removes plan directory" test_cleanup_plan

    # TUI Log Verification tests
    run_test "verify_tui_log_received finds entries" test_verify_tui_log_received_success
    run_test "verify_tui_log_received fails for missing entries" test_verify_tui_log_received_failure
    run_test "verify_tui_log_received handles missing log file" test_verify_tui_log_no_file
    run_test "extract_phase_updates_from_log returns JSON array" test_extract_phase_updates_from_log
    run_test "extract_phase_updates_from_log handles empty log" test_extract_phase_updates_empty_log
    run_test "verify_thread_exists_in_tui finds thread in log" test_verify_thread_exists_in_tui_via_log
    run_test "verify_thread_exists_in_tui fails for missing thread" test_verify_thread_not_found
    run_test "verify_thread_exists_in_tui requires thread ID" test_verify_thread_requires_id
    run_test "collect_verification_evidence creates evidence dir" test_collect_verification_evidence
    run_test "compare_status_with_log validates matching status" test_compare_status_with_log_matching
    run_test "compare_status_with_log fails on status mismatch" test_compare_status_with_log_status_mismatch
    run_test "compare_status_with_log fails for missing file" test_compare_status_with_log_no_file

    # Final cleanup
    cleanup_test_env

    test_end
}

main "$@"

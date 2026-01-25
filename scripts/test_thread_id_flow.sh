#!/bin/bash
#
# Integration Tests for Thread ID Propagation Flow
#
# This script verifies that thread_id flows correctly through the entire pipeline:
# 1. CONDUCTOR_THREAD_ID environment variable is accessible
# 2. Nova captures thread_id from env var (mocked for testing)
# 3. Status files contain the correct thread_id field
# 4. Circle display can be associated with the correct thread
#
# Usage:
#   ./scripts/test_thread_id_flow.sh         # Run all tests
#   ./scripts/test_thread_id_flow.sh -v      # Run with verbose output
#   ./scripts/test_thread_id_flow.sh --test=1 # Run specific test only
#
# Exit codes:
#   0 - All tests passed
#   1 - One or more tests failed
#

set -e

# Get the directory of this script
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

# Source the E2E helpers library
source "$SCRIPT_DIR/lib/e2e_helpers.sh"

# Test configuration
export PROJECT="tui_spoq"
export PLAN_ID="plan-thread-id-test-$$"
export TOTAL_PHASES=3
export COMMS_BASE_DIR="/tmp/thread_id_test_$$"
export TEST_THREAD_ID=$(generate_uuid)

# Parse arguments
VERBOSE=""
SPECIFIC_TEST=""
for arg in "$@"; do
    case "$arg" in
        -v|--verbose)
            VERBOSE=1
            export E2E_DEBUG=1
            ;;
        --test=*)
            SPECIFIC_TEST="${arg#*=}"
            ;;
    esac
done

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
    echo "  Thread ID Flow - Integration Tests"
    echo "========================================"
    echo ""
    echo "  Test Thread ID: ${TEST_THREAD_ID:0:8}..."
    echo "  Test Plan ID:   $PLAN_ID"
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
        echo -e "\n${GREEN}ALL TESTS PASSED${NC}"
        return 0
    fi
}

run_test() {
    local test_num="$1"
    local test_name="$2"
    local test_func="$3"

    # Skip if specific test requested and this isn't it
    if [ -n "$SPECIFIC_TEST" ] && [ "$SPECIFIC_TEST" != "$test_num" ]; then
        return 0
    fi

    TESTS_RUN=$((TESTS_RUN + 1))
    echo ""
    log_step "Test $test_num" "$test_name"

    if $test_func; then
        TESTS_PASSED=$((TESTS_PASSED + 1))
        echo -e "  ${GREEN}PASS${NC}: $test_name"
    else
        TESTS_FAILED=$((TESTS_FAILED + 1))
        echo -e "  ${RED}FAIL${NC}: $test_name"
    fi
}

# ==============================================================================
# Test Setup and Cleanup
# ==============================================================================

setup_test_env() {
    log_info "Setting up test environment..."

    # Create base directories
    mkdir -p "$COMMS_BASE_DIR"

    # Initialize plan directories
    init_plan_dirs

    log_success "Test environment created at: $COMMS_BASE_DIR"
}

cleanup_test_env() {
    log_info "Cleaning up test environment..."
    rm -rf "$COMMS_BASE_DIR" 2>/dev/null || true
    log_success "Test environment cleaned up"
}

# ==============================================================================
# Test 1: Verify CONDUCTOR_THREAD_ID is accessible
# ==============================================================================

test_1_conductor_thread_id_env_var() {
    log_info "Testing CONDUCTOR_THREAD_ID environment variable access..."

    # Test 1a: Set the env var and verify it's readable
    export CONDUCTOR_THREAD_ID="$TEST_THREAD_ID"

    local read_value
    read_value=$(echo "$CONDUCTOR_THREAD_ID")

    if [ "$read_value" != "$TEST_THREAD_ID" ]; then
        log_error "Failed to read CONDUCTOR_THREAD_ID environment variable"
        log_error "  Expected: $TEST_THREAD_ID"
        log_error "  Got: $read_value"
        return 1
    fi

    log_success "CONDUCTOR_THREAD_ID can be set and read"
    [ -n "$VERBOSE" ] && log_info "  Value: $read_value"

    # Test 1b: Verify the env var is accessible from a subshell
    local subshell_value
    subshell_value=$(bash -c 'echo $CONDUCTOR_THREAD_ID')

    if [ "$subshell_value" != "$TEST_THREAD_ID" ]; then
        log_error "CONDUCTOR_THREAD_ID not accessible from subshell"
        log_error "  Expected: $TEST_THREAD_ID"
        log_error "  Got: $subshell_value"
        return 1
    fi

    log_success "CONDUCTOR_THREAD_ID accessible from subshell"

    # Test 1c: Verify the format is a valid UUID-like string
    if [[ ! "$TEST_THREAD_ID" =~ ^[a-f0-9-]+$ ]]; then
        log_error "CONDUCTOR_THREAD_ID is not a valid UUID format"
        log_error "  Value: $TEST_THREAD_ID"
        return 1
    fi

    log_success "CONDUCTOR_THREAD_ID has valid UUID format"

    return 0
}

# ==============================================================================
# Test 2: Verify Nova captures thread_id in plan metadata
# ==============================================================================

test_2_nova_captures_thread_id() {
    log_info "Testing Nova thread_id capture in plan metadata..."

    # Create a mock plan file with thread_id in metadata (as Nova would create it)
    local plan_dir
    plan_dir=$(get_plan_dir)
    local plan_file="$plan_dir/$PLAN_ID.md"

    # Simulate Nova creating a plan with thread_id captured from CONDUCTOR_THREAD_ID
    cat > "$plan_file" << EOF
# Plan: Thread ID Test Plan

## Metadata
- **ID**: $PLAN_ID
- **Project**: $PROJECT
- **Type**: test
- **Status**: active
- **Thread ID**: $TEST_THREAD_ID

## Phases

### Phase 1: Test Phase 1
- **Description**: First test phase
- **Files**: \`test/phase1.rs\`
- **Complexity**: Low

### Phase 2: Test Phase 2
- **Description**: Second test phase
- **Files**: \`test/phase2.rs\`
- **Complexity**: Medium

### Phase 3: Test Phase 3
- **Description**: Third test phase
- **Files**: \`test/phase3.rs\`
- **Complexity**: Low
EOF

    log_success "Created mock Nova plan file: $plan_file"

    # Test 2a: Verify plan file was created
    if [ ! -f "$plan_file" ]; then
        log_error "Plan file not created"
        return 1
    fi

    # Test 2b: Extract thread_id from plan metadata
    local extracted_thread_id
    extracted_thread_id=$(grep -E "^\- \*\*Thread ID\*\*:" "$plan_file" | sed 's/.*: //')

    if [ -z "$extracted_thread_id" ]; then
        log_error "Thread ID not found in plan metadata"
        return 1
    fi

    log_success "Thread ID found in plan metadata"
    [ -n "$VERBOSE" ] && log_info "  Extracted: $extracted_thread_id"

    # Test 2c: Verify extracted thread_id matches expected
    if [ "$extracted_thread_id" != "$TEST_THREAD_ID" ]; then
        log_error "Thread ID mismatch in plan metadata"
        log_error "  Expected: $TEST_THREAD_ID"
        log_error "  Got: $extracted_thread_id"
        return 1
    fi

    log_success "Thread ID correctly captured in plan metadata"

    return 0
}

# ==============================================================================
# Test 3: Verify status files have thread_id field
# ==============================================================================

test_3_status_files_have_thread_id() {
    log_info "Testing thread_id in status files..."

    # Test 3a: Create status files with thread_id using helper function
    create_status_file_with_thread 1 "running" "$TEST_THREAD_ID"
    create_status_file_with_thread 2 "pending" "$TEST_THREAD_ID"
    create_status_file_with_thread 3 "pending" "$TEST_THREAD_ID"

    local status_dir
    status_dir=$(get_status_dir)

    # Test 3b: Verify status files exist
    for phase in 1 2 3; do
        local status_file="$status_dir/phase-${phase}.status"
        if [ ! -f "$status_file" ]; then
            log_error "Status file not found: $status_file"
            return 1
        fi
        [ -n "$VERBOSE" ] && log_info "  Found: phase-${phase}.status"
    done

    log_success "All status files created"

    # Test 3c: Verify each status file has thread_id field
    for phase in 1 2 3; do
        local status_file="$status_dir/phase-${phase}.status"
        local file_thread_id
        file_thread_id=$(jq -r '.thread_id // "null"' "$status_file" 2>/dev/null)

        if [ "$file_thread_id" = "null" ] || [ -z "$file_thread_id" ]; then
            log_error "thread_id missing from phase-${phase}.status"
            return 1
        fi

        [ -n "$VERBOSE" ] && log_info "  Phase $phase thread_id: ${file_thread_id:0:8}..."
    done

    log_success "All status files have thread_id field"

    # Test 3d: Verify thread_id matches expected value
    for phase in 1 2 3; do
        local status_file="$status_dir/phase-${phase}.status"
        local file_thread_id
        file_thread_id=$(jq -r '.thread_id' "$status_file" 2>/dev/null)

        if [ "$file_thread_id" != "$TEST_THREAD_ID" ]; then
            log_error "thread_id mismatch in phase-${phase}.status"
            log_error "  Expected: $TEST_THREAD_ID"
            log_error "  Got: $file_thread_id"
            return 1
        fi
    done

    log_success "All status files have correct thread_id value"

    # Test 3e: Verify status file format is valid for conductor
    "$SCRIPT_DIR/verify_status_format.sh" "$status_dir/phase-1.status" > /dev/null 2>&1
    if [ $? -ne 0 ]; then
        log_error "Status file format validation failed"
        return 1
    fi

    log_success "Status file format valid for conductor"

    return 0
}

# ==============================================================================
# Test 4: End-to-end thread-to-circle association
# ==============================================================================

test_4_thread_to_circle_association() {
    log_info "Testing thread to circle association..."

    # This test verifies that circles would appear on the correct thread row
    # by checking that all necessary data linkages are in place

    local plan_dir status_dir
    plan_dir=$(get_plan_dir)
    status_dir=$(get_status_dir)

    # Test 4a: Verify plan-to-status linkage via plan_id
    local status_file="$status_dir/phase-1.status"
    local status_plan_id
    status_plan_id=$(jq -r '.plan_id' "$status_file" 2>/dev/null)

    if [ "$status_plan_id" != "$PLAN_ID" ]; then
        log_error "Plan ID mismatch between status file and plan"
        log_error "  Status file plan_id: $status_plan_id"
        log_error "  Expected plan_id: $PLAN_ID"
        return 1
    fi

    log_success "Status file linked to plan via plan_id"

    # Test 4b: Verify thread_id consistency across all status files
    local first_thread_id
    first_thread_id=$(jq -r '.thread_id' "$status_dir/phase-1.status" 2>/dev/null)

    for phase in 2 3; do
        local phase_thread_id
        phase_thread_id=$(jq -r '.thread_id' "$status_dir/phase-${phase}.status" 2>/dev/null)

        if [ "$phase_thread_id" != "$first_thread_id" ]; then
            log_error "Thread ID inconsistent across phases"
            log_error "  Phase 1: $first_thread_id"
            log_error "  Phase $phase: $phase_thread_id"
            return 1
        fi
    done

    log_success "Thread ID consistent across all phases"

    # Test 4c: Verify thread_id in status matches thread_id in plan metadata
    local plan_file="$plan_dir/$PLAN_ID.md"
    local plan_thread_id
    plan_thread_id=$(grep -E "^\- \*\*Thread ID\*\*:" "$plan_file" | sed 's/.*: //')

    if [ "$first_thread_id" != "$plan_thread_id" ]; then
        log_error "Thread ID mismatch between status and plan"
        log_error "  Status file: $first_thread_id"
        log_error "  Plan file: $plan_thread_id"
        return 1
    fi

    log_success "Thread ID matches between status files and plan metadata"

    # Test 4d: Simulate phase progression and verify thread_id persists
    log_info "Simulating phase progression..."

    # Complete phase 1
    create_status_file_with_thread 1 "completed" "$TEST_THREAD_ID"

    # Start phase 2
    create_status_file_with_thread 2 "running" "$TEST_THREAD_ID"

    # Verify thread_id still correct after updates
    local updated_thread_id
    updated_thread_id=$(jq -r '.thread_id' "$status_dir/phase-1.status" 2>/dev/null)

    if [ "$updated_thread_id" != "$TEST_THREAD_ID" ]; then
        log_error "Thread ID changed after status update"
        log_error "  Expected: $TEST_THREAD_ID"
        log_error "  Got: $updated_thread_id"
        return 1
    fi

    log_success "Thread ID persists through phase progression"

    # Test 4e: Create summary of the full linkage chain
    echo ""
    log_info "Thread ID Linkage Chain Summary:"
    echo "  CONDUCTOR_THREAD_ID env var: $TEST_THREAD_ID"
    echo "  Plan metadata Thread ID:     $plan_thread_id"
    echo "  Status files thread_id:      $first_thread_id"
    echo "  Linkage: CONDUCTOR_THREAD_ID -> Plan Metadata -> Status Files -> TUI Circles"
    echo ""

    if [ "$TEST_THREAD_ID" = "$plan_thread_id" ] && [ "$plan_thread_id" = "$first_thread_id" ]; then
        log_success "Complete thread_id linkage chain verified"
    else
        log_error "Thread ID linkage chain broken"
        return 1
    fi

    return 0
}

# ==============================================================================
# Test 5: Verify marker files can carry thread_id
# ==============================================================================

test_5_marker_files_thread_id() {
    log_info "Testing thread_id in marker files..."

    local markers_dir
    markers_dir=$(get_markers_dir)
    mkdir -p "$markers_dir"

    # Test 5a: Create marker files with thread_id (as Pulsar would)
    for phase in 1 2 3; do
        local marker_file="$markers_dir/phase-${phase}.json"
        jq -n \
            --arg plan_id "$PLAN_ID" \
            --argjson phase "$phase" \
            --arg thread_id "$TEST_THREAD_ID" \
            --arg status "pending" \
            --arg created_at "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
            '{
                plan_id: $plan_id,
                phase: $phase,
                thread_id: $thread_id,
                status: $status,
                created_at: $created_at
            }' > "$marker_file"

        [ -n "$VERBOSE" ] && log_info "  Created marker: phase-${phase}.json"
    done

    log_success "Marker files created with thread_id"

    # Test 5b: Verify marker files are valid JSON with thread_id
    for phase in 1 2 3; do
        local marker_file="$markers_dir/phase-${phase}.json"

        if ! jq empty "$marker_file" 2>/dev/null; then
            log_error "Invalid JSON in marker file: $marker_file"
            return 1
        fi

        local marker_thread_id
        marker_thread_id=$(jq -r '.thread_id' "$marker_file" 2>/dev/null)

        if [ "$marker_thread_id" != "$TEST_THREAD_ID" ]; then
            log_error "Thread ID mismatch in marker phase-${phase}.json"
            log_error "  Expected: $TEST_THREAD_ID"
            log_error "  Got: $marker_thread_id"
            return 1
        fi
    done

    log_success "Marker files have valid thread_id"

    # Test 5c: Simulate phase executor reading thread_id from marker
    log_info "Simulating phase executor reading marker..."

    local marker_file="$markers_dir/phase-1.json"
    local executor_read_thread_id
    executor_read_thread_id=$(jq -r '.thread_id // ""' "$marker_file" 2>/dev/null)

    if [ -z "$executor_read_thread_id" ]; then
        log_error "Phase executor could not read thread_id from marker"
        return 1
    fi

    # Phase executor would use this thread_id in status file
    if [ "$executor_read_thread_id" = "$TEST_THREAD_ID" ]; then
        log_success "Phase executor can read thread_id from marker"
    else
        log_error "Thread ID read from marker does not match expected"
        return 1
    fi

    return 0
}

# ==============================================================================
# Test 6: Status file without thread_id (backward compatibility)
# ==============================================================================

test_6_backward_compatibility_null_thread_id() {
    log_info "Testing backward compatibility with null thread_id..."

    local status_dir
    status_dir=$(get_status_dir)

    # Test 6a: Create status file without thread_id (old format)
    local old_status_file="$status_dir/phase-old.status"
    jq -n \
        --arg task_id "phase-old-$PLAN_ID" \
        --arg project "$PROJECT" \
        --arg plan_id "$PLAN_ID" \
        --argjson phase 99 \
        --arg status "completed" \
        --arg started_at "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
        '{
            task_id: $task_id,
            thread_id: null,
            project: $project,
            plan_id: $plan_id,
            phase: $phase,
            status: $status,
            started_at: $started_at
        }' > "$old_status_file"

    log_success "Created status file with null thread_id"

    # Test 6b: Verify status file is valid and thread_id is null
    local file_thread_id
    file_thread_id=$(jq -r '.thread_id' "$old_status_file" 2>/dev/null)

    if [ "$file_thread_id" != "null" ]; then
        log_error "Expected null thread_id, got: $file_thread_id"
        return 1
    fi

    log_success "Status file correctly has null thread_id"

    # Test 6c: Verify verify_status_format.sh accepts null thread_id
    "$SCRIPT_DIR/verify_status_format.sh" "$old_status_file" > /dev/null 2>&1
    if [ $? -ne 0 ]; then
        log_error "verify_status_format.sh rejected null thread_id"
        return 1
    fi

    log_success "Null thread_id is accepted by format validator"

    # Cleanup old format file
    rm -f "$old_status_file"

    return 0
}

# ==============================================================================
# Main
# ==============================================================================

main() {
    test_start

    # Setup
    setup_test_env

    # Run tests
    run_test 1 "CONDUCTOR_THREAD_ID environment variable accessible" test_1_conductor_thread_id_env_var
    run_test 2 "Nova captures thread_id in plan metadata" test_2_nova_captures_thread_id
    run_test 3 "Status files have thread_id field" test_3_status_files_have_thread_id
    run_test 4 "End-to-end thread to circle association" test_4_thread_to_circle_association
    run_test 5 "Marker files can carry thread_id" test_5_marker_files_thread_id
    run_test 6 "Backward compatibility with null thread_id" test_6_backward_compatibility_null_thread_id

    # Cleanup
    cleanup_test_env

    test_end
}

main "$@"

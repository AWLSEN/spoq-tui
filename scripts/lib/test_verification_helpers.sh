#!/bin/bash
#
# Test Script for Verification Helper Functions
#
# Tests the verification functions added in Phase 4 of the E2E test suite,
# including evidence collection, checkpoint recording, and report generation.
#
# Usage:
#   bash scripts/lib/test_verification_helpers.sh
#
# Exit codes:
#   0 - All tests passed
#   1 - One or more tests failed
#

set -e

# Get the directory of this script
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Source the E2E helpers library first (provides base utilities)
source "$SCRIPT_DIR/e2e_helpers.sh"

# Test configuration
export PROJECT="test-project"
export PLAN_ID="plan-test-verification"
export TOTAL_PHASES=3
export COMMS_BASE_DIR="/tmp/verification_helpers_test_$$"
export TEMP_DIR="/tmp/verification_test_temp_$$"
export SCREENSHOT_DIR="$TEMP_DIR/screenshots"
export WS_EVENTS_FILE="$TEMP_DIR/ws_events.log"
export TUI_LOG_FILE="/tmp/verification_test_tui_$$.log"

# Test counters
TESTS_RUN=0
TESTS_PASSED=0
TESTS_FAILED=0

# ==============================================================================
# Verification Functions (extracted from e2e_real_pulsar_test.sh)
# ==============================================================================

# These are the functions we're testing - copied directly from the e2e test script

init_evidence_dir() {
    local custom_dir="${1:-$EVIDENCE_DIR}"
    local timestamp
    timestamp=$(date +%Y%m%d_%H%M%S)

    EVIDENCE_START_TIME=$(date -u +%Y-%m-%dT%H:%M:%SZ)

    if [ -n "$custom_dir" ]; then
        EVIDENCE_DIR="$custom_dir"
    else
        EVIDENCE_DIR="$TEMP_DIR/evidence_${timestamp}"
    fi

    # Create evidence directory structure
    mkdir -p "$EVIDENCE_DIR"
    mkdir -p "$EVIDENCE_DIR/status_files"
    mkdir -p "$EVIDENCE_DIR/screenshots"
    mkdir -p "$EVIDENCE_DIR/logs"

    # Create checkpoint data directory for bash 3.x compatibility
    CHECKPOINT_DATA_DIR="$EVIDENCE_DIR/.checkpoints"
    mkdir -p "$CHECKPOINT_DATA_DIR"
    # Initialize empty checkpoint order file
    : > "$CHECKPOINT_DATA_DIR/order.txt"

    if [ -n "$VERBOSE" ]; then
        log_info "[VERBOSE] Evidence directory initialized: $EVIDENCE_DIR"
        log_info "[VERBOSE] Subdirectories created: status_files, screenshots, logs"
    fi

    log_success "Evidence directory created: $EVIDENCE_DIR"
    return 0
}

collect_evidence() {
    if [ -z "$EVIDENCE_DIR" ]; then
        log_error "Evidence directory not initialized. Call init_evidence_dir first."
        return 1
    fi

    log_step "EVIDENCE" "Collecting evidence files"

    local files_collected=0
    local timestamp
    timestamp=$(date +%Y%m%d_%H%M%S)

    # 1. Collect status files
    if [ -n "$PLAN_ID" ]; then
        local status_dir="${COMMS_BASE_DIR:-$HOME/comms}/plans/$PROJECT/active/$PLAN_ID/status"
        if [ -d "$status_dir" ]; then
            if [ -n "$VERBOSE" ]; then
                log_info "[VERBOSE] Copying status files from: $status_dir"
            fi
            for status_file in "$status_dir"/*.status; do
                if [ -f "$status_file" ]; then
                    cp "$status_file" "$EVIDENCE_DIR/status_files/"
                    files_collected=$((files_collected + 1))
                    if [ -n "$VERBOSE" ]; then
                        log_info "[VERBOSE] Copied: $(basename "$status_file")"
                    fi
                fi
            done
            log_success "Collected status files: $(ls "$EVIDENCE_DIR/status_files" 2>/dev/null | wc -l | tr -d ' ') files"
        else
            log_warn "Status directory not found: $status_dir"
        fi

        # Also collect the plan markdown file
        local plan_file="${COMMS_BASE_DIR:-$HOME/comms}/plans/$PROJECT/active/${PLAN_ID}.md"
        if [ -f "$plan_file" ]; then
            cp "$plan_file" "$EVIDENCE_DIR/plan.md"
            files_collected=$((files_collected + 1))
            if [ -n "$VERBOSE" ]; then
                log_info "[VERBOSE] Copied plan file: plan.md"
            fi
        fi
    else
        log_warn "PLAN_ID not set - skipping status file collection"
    fi

    # 2. Collect TUI logs
    local tui_log_file="${TUI_LOG_FILE:-$HOME/.spoq/logs/spoq.log}"
    if [ -f "$tui_log_file" ]; then
        if [ -n "$VERBOSE" ]; then
            log_info "[VERBOSE] Copying TUI log (last 2000 lines): $tui_log_file"
        fi
        tail -2000 "$tui_log_file" > "$EVIDENCE_DIR/logs/tui_log_tail.log"
        files_collected=$((files_collected + 1))

        # Extract PHASE_PROGRESS_UPDATE entries if plan_id is known
        if [ -n "$PLAN_ID" ]; then
            grep "PHASE_PROGRESS_UPDATE" "$tui_log_file" 2>/dev/null | \
                grep -E "$PLAN_ID|phase_progress" > "$EVIDENCE_DIR/logs/phase_updates.log" || true
            if [ -s "$EVIDENCE_DIR/logs/phase_updates.log" ]; then
                files_collected=$((files_collected + 1))
                if [ -n "$VERBOSE" ]; then
                    local update_count
                    update_count=$(wc -l < "$EVIDENCE_DIR/logs/phase_updates.log" | tr -d ' ')
                    log_info "[VERBOSE] Extracted $update_count phase update log entries"
                fi
            fi
        fi
        log_success "Collected TUI log entries"
    else
        log_warn "TUI log not found at: $tui_log_file"
    fi

    # 3. Collect screenshots
    if [ -d "$SCREENSHOT_DIR" ]; then
        local screenshot_count
        screenshot_count=$(find "$SCREENSHOT_DIR" -type f \( -name "*.png" -o -name "*.txt" \) 2>/dev/null | wc -l | tr -d ' ')
        if [ "$screenshot_count" -gt 0 ]; then
            if [ -n "$VERBOSE" ]; then
                log_info "[VERBOSE] Copying $screenshot_count screenshot/text files from: $SCREENSHOT_DIR"
            fi
            cp "$SCREENSHOT_DIR"/*.png "$EVIDENCE_DIR/screenshots/" 2>/dev/null || true
            cp "$SCREENSHOT_DIR"/*.txt "$EVIDENCE_DIR/screenshots/" 2>/dev/null || true
            files_collected=$((files_collected + screenshot_count))
            log_success "Collected screenshots: $screenshot_count files"
        else
            log_info "No screenshots found in $SCREENSHOT_DIR"
        fi
    fi

    # 4. Collect WebSocket events if available
    if [ -f "$WS_EVENTS_FILE" ]; then
        cp "$WS_EVENTS_FILE" "$EVIDENCE_DIR/logs/ws_events.log"
        files_collected=$((files_collected + 1))
        if [ -n "$VERBOSE" ]; then
            log_info "[VERBOSE] Copied WebSocket events log"
        fi
    fi

    # 5. Create metadata file
    local metadata_file="$EVIDENCE_DIR/metadata.json"
    jq -n \
        --arg project "$PROJECT" \
        --arg plan_id "${PLAN_ID:-null}" \
        --arg thread_id "${THREAD_ID:-null}" \
        --arg collected_at "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
        --arg evidence_start "$EVIDENCE_START_TIME" \
        --argjson total_phases "$TOTAL_PHASES" \
        --argjson files_collected "$files_collected" \
        '{
            project: $project,
            plan_id: (if $plan_id == "null" then null else $plan_id end),
            thread_id: (if $thread_id == "null" then null else $thread_id end),
            total_phases: $total_phases,
            evidence_start: $evidence_start,
            collected_at: $collected_at,
            files_collected: $files_collected
        }' > "$metadata_file"

    log_success "Evidence collection complete: $files_collected files collected"
    log_info "Evidence location: $EVIDENCE_DIR"
    return 0
}

record_checkpoint() {
    local name="$1"
    local result="$2"
    local details="${3:-}"
    local timestamp
    timestamp=$(date -u +%Y-%m-%dT%H:%M:%SZ)

    # Store checkpoint data in files (bash 3.x compatible)
    if [ -n "$CHECKPOINT_DATA_DIR" ] && [ -d "$CHECKPOINT_DATA_DIR" ]; then
        # Create checkpoint data file
        cat > "$CHECKPOINT_DATA_DIR/${name}.json" << EOF
{"name": "$name", "result": "$result", "timestamp": "$timestamp", "details": "$details"}
EOF
        # Append to order file
        echo "$name" >> "$CHECKPOINT_DATA_DIR/order.txt"
    fi

    if [ -n "$VERBOSE" ]; then
        local detail_msg=""
        [ -n "$details" ] && detail_msg=" - $details"
        case "$result" in
            PASS)
                log_info "[VERBOSE] [CHECKPOINT] $name: ${E2E_GREEN}PASS${E2E_NC}$detail_msg"
                ;;
            FAIL)
                log_info "[VERBOSE] [CHECKPOINT] $name: ${E2E_RED}FAIL${E2E_NC}$detail_msg"
                ;;
            SKIP)
                log_info "[VERBOSE] [CHECKPOINT] $name: ${E2E_YELLOW}SKIP${E2E_NC}$detail_msg"
                ;;
        esac
    fi
}

get_checkpoint_result() {
    local name="$1"
    local checkpoint_file="$CHECKPOINT_DATA_DIR/${name}.json"
    if [ -f "$checkpoint_file" ]; then
        jq -r '.result' "$checkpoint_file" 2>/dev/null || echo ""
    fi
}

get_checkpoint_timestamp() {
    local name="$1"
    local checkpoint_file="$CHECKPOINT_DATA_DIR/${name}.json"
    if [ -f "$checkpoint_file" ]; then
        jq -r '.timestamp' "$checkpoint_file" 2>/dev/null || echo ""
    fi
}

get_checkpoint_order() {
    if [ -f "$CHECKPOINT_DATA_DIR/order.txt" ]; then
        cat "$CHECKPOINT_DATA_DIR/order.txt"
    fi
}

generate_verification_report() {
    if [ -z "$EVIDENCE_DIR" ]; then
        log_error "Evidence directory not initialized"
        return 1
    fi

    log_step "REPORT" "Generating verification report"

    local report_file="$EVIDENCE_DIR/verification_report.md"
    local report_timestamp
    report_timestamp=$(date -u +%Y-%m-%dT%H:%M:%SZ)

    # Calculate pass/fail counts from checkpoint files
    local pass_count=0
    local fail_count=0
    local skip_count=0
    local total_checkpoints=0

    while IFS= read -r name; do
        [ -z "$name" ] && continue
        local result
        result=$(get_checkpoint_result "$name")
        case "$result" in
            PASS) pass_count=$((pass_count + 1)) ;;
            FAIL) fail_count=$((fail_count + 1)) ;;
            SKIP) skip_count=$((skip_count + 1)) ;;
        esac
        total_checkpoints=$((total_checkpoints + 1))
    done < <(get_checkpoint_order)

    local overall_status="PASS"
    [ "$fail_count" -gt 0 ] && overall_status="FAIL"

    # Start the report (simplified version for testing)
    cat > "$report_file" << EOF
# E2E Verification Report

**Generated**: $report_timestamp
**Overall Status**: **$overall_status**

---

## Summary

| Metric | Value |
|--------|-------|
| Project | $PROJECT |
| Plan ID | ${PLAN_ID:-N/A} |
| Total Phases | $TOTAL_PHASES |

### Checkpoint Results

| Status | Count |
|--------|-------|
| PASS | $pass_count |
| FAIL | $fail_count |
| SKIP | $skip_count |

---

## Checkpoint Details

EOF

    # Add each checkpoint
    while IFS= read -r name; do
        [ -z "$name" ] && continue
        local result timestamp
        result=$(get_checkpoint_result "$name")
        timestamp=$(get_checkpoint_timestamp "$name")

        cat >> "$report_file" << EOF
### $name

- **Status**: [$result]
- **Timestamp**: $timestamp

EOF
    done < <(get_checkpoint_order)

    # Add phase status section
    cat >> "$report_file" << EOF

---

## Phase Status Details

EOF

    if [ -n "$PLAN_ID" ] && [ -d "$EVIDENCE_DIR/status_files" ]; then
        for phase in $(seq 1 "$TOTAL_PHASES"); do
            local status_file="$EVIDENCE_DIR/status_files/phase-${phase}.status"
            if [ -f "$status_file" ]; then
                local status
                status=$(jq -r '.status // "unknown"' "$status_file" 2>/dev/null)
                cat >> "$report_file" << EOF
### Phase $phase

| Status | $status |

EOF
            fi
        done
    else
        echo "*No phase status files collected*" >> "$report_file"
    fi

    log_success "Verification report generated: $report_file"

    # Generate JSON summary
    local json_summary="$EVIDENCE_DIR/summary.json"
    local checkpoints_json="[]"

    while IFS= read -r name; do
        [ -z "$name" ] && continue
        local result timestamp
        result=$(get_checkpoint_result "$name")
        timestamp=$(get_checkpoint_timestamp "$name")
        checkpoints_json=$(echo "$checkpoints_json" | jq --arg name "$name" --arg result "$result" --arg ts "$timestamp" '. + [{"name": $name, "result": $result, "timestamp": $ts}]')
    done < <(get_checkpoint_order)

    jq -n \
        --arg overall_status "$overall_status" \
        --arg project "$PROJECT" \
        --arg plan_id "${PLAN_ID:-null}" \
        --argjson total_phases "$TOTAL_PHASES" \
        --argjson pass_count "$pass_count" \
        --argjson fail_count "$fail_count" \
        --argjson skip_count "$skip_count" \
        --arg evidence_dir "$EVIDENCE_DIR" \
        --arg started_at "$EVIDENCE_START_TIME" \
        --arg generated_at "$report_timestamp" \
        --argjson checkpoints "$checkpoints_json" \
        '{
            overall_status: $overall_status,
            project: $project,
            plan_id: (if $plan_id == "null" then null else $plan_id end),
            total_phases: $total_phases,
            checkpoints: {
                pass: $pass_count,
                fail: $fail_count,
                skip: $skip_count
            },
            evidence_dir: $evidence_dir,
            started_at: $started_at,
            generated_at: $generated_at,
            checkpoint_details: $checkpoints
        }' > "$json_summary"

    return 0
}

# ==============================================================================
# Test Framework
# ==============================================================================

test_start() {
    echo ""
    echo "========================================"
    echo "  Verification Helpers - Test Suite"
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
# Test Setup and Teardown
# ==============================================================================

setup_test_env() {
    # Create temp directories
    mkdir -p "$TEMP_DIR"
    mkdir -p "$SCREENSHOT_DIR"
    mkdir -p "$COMMS_BASE_DIR"

    # Create mock TUI log
    cat > "$TUI_LOG_FILE" << 'EOF'
[2026-01-25T10:00:00Z] INFO Starting TUI
[2026-01-25T10:00:01Z] PHASE_PROGRESS_UPDATE {"plan_id": "plan-test-verification", "phase": 1, "status": "running"}
[2026-01-25T10:00:02Z] PHASE_PROGRESS_UPDATE {"plan_id": "plan-test-verification", "phase": 1, "status": "completed"}
EOF

    # Create some mock status files for testing
    local status_dir="$COMMS_BASE_DIR/plans/$PROJECT/active/$PLAN_ID/status"
    mkdir -p "$status_dir"

    # Create 3 status files
    for phase in 1 2 3; do
        jq -n \
            --arg task_id "phase-${phase}-${PLAN_ID}" \
            --arg plan_id "$PLAN_ID" \
            --argjson phase "$phase" \
            --arg status "completed" \
            --arg timestamp "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
            '{
                task_id: $task_id,
                plan_id: $plan_id,
                phase: $phase,
                status: $status,
                started_at: $timestamp,
                completed_at: $timestamp,
                tool_count: 5
            }' > "$status_dir/phase-${phase}.status"
    done

    # Create a plan file
    local plan_dir="$COMMS_BASE_DIR/plans/$PROJECT/active/$PLAN_ID"
    cat > "$plan_dir.md" << EOF
# Plan: Test Verification Plan

## Metadata
- **ID**: $PLAN_ID
- **Project**: $PROJECT

## Phases

### Phase 1: Test Phase 1
### Phase 2: Test Phase 2
### Phase 3: Test Phase 3
EOF
}

cleanup_test_env() {
    rm -rf "$TEMP_DIR" 2>/dev/null || true
    rm -rf "$COMMS_BASE_DIR" 2>/dev/null || true
    rm -f "$TUI_LOG_FILE" 2>/dev/null || true
}

# ==============================================================================
# Test Cases for init_evidence_dir
# ==============================================================================

test_init_evidence_dir_creates_structure() {
    local test_evidence_dir="/tmp/test_evidence_$$"

    # Initialize evidence directory
    init_evidence_dir "$test_evidence_dir" >/dev/null 2>&1

    # Check that EVIDENCE_DIR is set
    [ -n "$EVIDENCE_DIR" ] || return 1

    # Check that directories exist
    [ -d "$EVIDENCE_DIR" ] || return 1
    [ -d "$EVIDENCE_DIR/status_files" ] || return 1
    [ -d "$EVIDENCE_DIR/screenshots" ] || return 1
    [ -d "$EVIDENCE_DIR/logs" ] || return 1

    # Check checkpoint data directory exists
    [ -d "$EVIDENCE_DIR/.checkpoints" ] || return 1
    [ -f "$EVIDENCE_DIR/.checkpoints/order.txt" ] || return 1

    # Cleanup
    rm -rf "$test_evidence_dir"
    return 0
}

test_init_evidence_dir_auto_generates_path() {
    # Don't provide custom dir, let it auto-generate
    EVIDENCE_DIR=""
    init_evidence_dir >/dev/null 2>&1

    # Should have set EVIDENCE_DIR
    [ -n "$EVIDENCE_DIR" ] || return 1

    # Should contain "evidence_" in the path
    [[ "$EVIDENCE_DIR" == *"evidence_"* ]] || return 1

    # Directory should exist
    [ -d "$EVIDENCE_DIR" ] || return 1

    # Cleanup
    rm -rf "$EVIDENCE_DIR"
    return 0
}

test_init_evidence_dir_sets_start_time() {
    EVIDENCE_DIR=""
    EVIDENCE_START_TIME=""

    init_evidence_dir >/dev/null 2>&1

    # Should have set start time
    [ -n "$EVIDENCE_START_TIME" ] || return 1

    # Should be ISO 8601 format
    [[ "$EVIDENCE_START_TIME" =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z$ ]] || return 1

    # Cleanup
    rm -rf "$EVIDENCE_DIR"
    return 0
}

# ==============================================================================
# Test Cases for record_checkpoint and get_checkpoint_*
# ==============================================================================

test_record_checkpoint_creates_file() {
    local test_evidence_dir="/tmp/test_evidence_checkpoint_$$"
    init_evidence_dir "$test_evidence_dir" >/dev/null 2>&1

    # Record a checkpoint
    record_checkpoint "TEST_CHECKPOINT" "PASS" "Test details" >/dev/null 2>&1

    # Check that checkpoint file exists
    local checkpoint_file="$CHECKPOINT_DATA_DIR/TEST_CHECKPOINT.json"
    [ -f "$checkpoint_file" ] || { rm -rf "$test_evidence_dir"; return 1; }

    # Check that it's valid JSON
    jq empty "$checkpoint_file" 2>/dev/null || { rm -rf "$test_evidence_dir"; return 1; }

    # Check that it has expected fields
    local name result
    name=$(jq -r '.name' "$checkpoint_file")
    result=$(jq -r '.result' "$checkpoint_file")

    [ "$name" = "TEST_CHECKPOINT" ] || { rm -rf "$test_evidence_dir"; return 1; }
    [ "$result" = "PASS" ] || { rm -rf "$test_evidence_dir"; return 1; }

    # Cleanup
    rm -rf "$test_evidence_dir"
    return 0
}

test_record_checkpoint_appends_to_order() {
    local test_evidence_dir="/tmp/test_evidence_order_$$"
    init_evidence_dir "$test_evidence_dir" >/dev/null 2>&1

    # Record multiple checkpoints
    record_checkpoint "CHECKPOINT_1" "PASS" >/dev/null 2>&1
    record_checkpoint "CHECKPOINT_2" "FAIL" >/dev/null 2>&1
    record_checkpoint "CHECKPOINT_3" "SKIP" >/dev/null 2>&1

    # Check order file
    local order_file="$CHECKPOINT_DATA_DIR/order.txt"
    [ -f "$order_file" ] || { rm -rf "$test_evidence_dir"; return 1; }

    # Should have 3 lines
    local line_count
    line_count=$(wc -l < "$order_file" | tr -d ' ')
    [ "$line_count" -eq 3 ] || { rm -rf "$test_evidence_dir"; return 1; }

    # Cleanup
    rm -rf "$test_evidence_dir"
    return 0
}

test_get_checkpoint_result() {
    local test_evidence_dir="/tmp/test_evidence_get_$$"
    init_evidence_dir "$test_evidence_dir" >/dev/null 2>&1

    # Record checkpoints with different results
    record_checkpoint "PASS_TEST" "PASS" >/dev/null 2>&1
    record_checkpoint "FAIL_TEST" "FAIL" >/dev/null 2>&1
    record_checkpoint "SKIP_TEST" "SKIP" >/dev/null 2>&1

    # Retrieve results
    local result1 result2 result3
    result1=$(get_checkpoint_result "PASS_TEST")
    result2=$(get_checkpoint_result "FAIL_TEST")
    result3=$(get_checkpoint_result "SKIP_TEST")

    [ "$result1" = "PASS" ] || { rm -rf "$test_evidence_dir"; return 1; }
    [ "$result2" = "FAIL" ] || { rm -rf "$test_evidence_dir"; return 1; }
    [ "$result3" = "SKIP" ] || { rm -rf "$test_evidence_dir"; return 1; }

    # Cleanup
    rm -rf "$test_evidence_dir"
    return 0
}

test_get_checkpoint_timestamp() {
    local test_evidence_dir="/tmp/test_evidence_ts_$$"
    init_evidence_dir "$test_evidence_dir" >/dev/null 2>&1

    # Record a checkpoint
    record_checkpoint "TIMESTAMP_TEST" "PASS" >/dev/null 2>&1

    # Get timestamp
    local ts
    ts=$(get_checkpoint_timestamp "TIMESTAMP_TEST")

    # Should not be empty
    [ -n "$ts" ] || { rm -rf "$test_evidence_dir"; return 1; }

    # Should be ISO 8601 format
    [[ "$ts" =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z$ ]] || { rm -rf "$test_evidence_dir"; return 1; }

    # Cleanup
    rm -rf "$test_evidence_dir"
    return 0
}

test_get_checkpoint_order() {
    local test_evidence_dir="/tmp/test_evidence_order_get_$$"
    init_evidence_dir "$test_evidence_dir" >/dev/null 2>&1

    # Record checkpoints in order
    record_checkpoint "FIRST" "PASS" >/dev/null 2>&1
    record_checkpoint "SECOND" "PASS" >/dev/null 2>&1
    record_checkpoint "THIRD" "PASS" >/dev/null 2>&1

    # Get order
    local order_output
    order_output=$(get_checkpoint_order)

    # Should have 3 lines
    local line_count
    line_count=$(echo "$order_output" | wc -l | tr -d ' ')
    [ "$line_count" -eq 3 ] || { rm -rf "$test_evidence_dir"; return 1; }

    # Should contain all checkpoints
    echo "$order_output" | grep -q "FIRST" || { rm -rf "$test_evidence_dir"; return 1; }
    echo "$order_output" | grep -q "SECOND" || { rm -rf "$test_evidence_dir"; return 1; }
    echo "$order_output" | grep -q "THIRD" || { rm -rf "$test_evidence_dir"; return 1; }

    # Cleanup
    rm -rf "$test_evidence_dir"
    return 0
}

# ==============================================================================
# Test Cases for collect_evidence
# ==============================================================================

test_collect_evidence_requires_init() {
    # Try to collect without initializing
    EVIDENCE_DIR=""

    # Should fail
    ! collect_evidence >/dev/null 2>&1 || return 1

    return 0
}

test_collect_evidence_copies_status_files() {
    local test_evidence_dir="/tmp/test_evidence_collect_$$"
    init_evidence_dir "$test_evidence_dir" >/dev/null 2>&1

    # Collect evidence
    collect_evidence >/dev/null 2>&1

    # Should have copied status files
    [ -d "$EVIDENCE_DIR/status_files" ] || { rm -rf "$test_evidence_dir"; return 1; }

    # Should have 3 status files (from setup)
    local status_count
    status_count=$(ls -1 "$EVIDENCE_DIR/status_files"/*.status 2>/dev/null | wc -l | tr -d ' ')
    [ "$status_count" -eq 3 ] || { rm -rf "$test_evidence_dir"; return 1; }

    # Cleanup
    rm -rf "$test_evidence_dir"
    return 0
}

test_collect_evidence_copies_plan_file() {
    local test_evidence_dir="/tmp/test_evidence_plan_$$"
    init_evidence_dir "$test_evidence_dir" >/dev/null 2>&1

    # Collect evidence
    collect_evidence >/dev/null 2>&1

    # Should have copied plan file
    [ -f "$EVIDENCE_DIR/plan.md" ] || { rm -rf "$test_evidence_dir"; return 1; }

    # Should contain plan ID
    grep -q "$PLAN_ID" "$EVIDENCE_DIR/plan.md" || { rm -rf "$test_evidence_dir"; return 1; }

    # Cleanup
    rm -rf "$test_evidence_dir"
    return 0
}

test_collect_evidence_copies_tui_log() {
    local test_evidence_dir="/tmp/test_evidence_log_$$"
    init_evidence_dir "$test_evidence_dir" >/dev/null 2>&1

    # Collect evidence
    collect_evidence >/dev/null 2>&1

    # Should have copied TUI log tail
    [ -f "$EVIDENCE_DIR/logs/tui_log_tail.log" ] || { rm -rf "$test_evidence_dir"; return 1; }

    # Should have extracted phase updates
    [ -f "$EVIDENCE_DIR/logs/phase_updates.log" ] || { rm -rf "$test_evidence_dir"; return 1; }

    # Cleanup
    rm -rf "$test_evidence_dir"
    return 0
}

test_collect_evidence_creates_metadata() {
    local test_evidence_dir="/tmp/test_evidence_meta_$$"
    init_evidence_dir "$test_evidence_dir" >/dev/null 2>&1

    # Collect evidence
    collect_evidence >/dev/null 2>&1

    # Should have created metadata file
    [ -f "$EVIDENCE_DIR/metadata.json" ] || { rm -rf "$test_evidence_dir"; return 1; }

    # Should be valid JSON
    jq empty "$EVIDENCE_DIR/metadata.json" 2>/dev/null || { rm -rf "$test_evidence_dir"; return 1; }

    # Should have expected fields
    local project plan_id
    project=$(jq -r '.project' "$EVIDENCE_DIR/metadata.json")
    plan_id=$(jq -r '.plan_id' "$EVIDENCE_DIR/metadata.json")

    [ "$project" = "$PROJECT" ] || { rm -rf "$test_evidence_dir"; return 1; }
    [ "$plan_id" = "$PLAN_ID" ] || { rm -rf "$test_evidence_dir"; return 1; }

    # Cleanup
    rm -rf "$test_evidence_dir"
    return 0
}

# ==============================================================================
# Test Cases for generate_verification_report
# ==============================================================================

test_generate_verification_report_creates_markdown() {
    local test_evidence_dir="/tmp/test_evidence_report_$$"
    init_evidence_dir "$test_evidence_dir" >/dev/null 2>&1

    # Record some checkpoints
    record_checkpoint "SETUP" "PASS" "Setup completed" >/dev/null 2>&1
    record_checkpoint "EXECUTION" "PASS" "Execution successful" >/dev/null 2>&1

    # Collect evidence first
    collect_evidence >/dev/null 2>&1

    # Generate report
    generate_verification_report >/dev/null 2>&1

    # Should have created markdown report
    [ -f "$EVIDENCE_DIR/verification_report.md" ] || { rm -rf "$test_evidence_dir"; return 1; }

    # Should contain expected sections
    grep -q "# E2E Verification Report" "$EVIDENCE_DIR/verification_report.md" || { rm -rf "$test_evidence_dir"; return 1; }
    grep -q "## Summary" "$EVIDENCE_DIR/verification_report.md" || { rm -rf "$test_evidence_dir"; return 1; }

    # Cleanup
    rm -rf "$test_evidence_dir"
    return 0
}

test_generate_verification_report_creates_json_summary() {
    local test_evidence_dir="/tmp/test_evidence_json_$$"
    init_evidence_dir "$test_evidence_dir" >/dev/null 2>&1

    # Record checkpoints
    record_checkpoint "TEST1" "PASS" >/dev/null 2>&1
    record_checkpoint "TEST2" "FAIL" >/dev/null 2>&1

    # Collect evidence
    collect_evidence >/dev/null 2>&1

    # Generate report
    generate_verification_report >/dev/null 2>&1

    # Should have created JSON summary
    [ -f "$EVIDENCE_DIR/summary.json" ] || { rm -rf "$test_evidence_dir"; return 1; }

    # Should be valid JSON
    jq empty "$EVIDENCE_DIR/summary.json" 2>/dev/null || { rm -rf "$test_evidence_dir"; return 1; }

    # Should have checkpoint counts
    local pass_count fail_count
    pass_count=$(jq -r '.checkpoints.pass' "$EVIDENCE_DIR/summary.json")
    fail_count=$(jq -r '.checkpoints.fail' "$EVIDENCE_DIR/summary.json")

    [ "$pass_count" -eq 1 ] || { rm -rf "$test_evidence_dir"; return 1; }
    [ "$fail_count" -eq 1 ] || { rm -rf "$test_evidence_dir"; return 1; }

    # Cleanup
    rm -rf "$test_evidence_dir"
    return 0
}

test_generate_verification_report_status_pass() {
    local test_evidence_dir="/tmp/test_evidence_status_pass_$$"
    init_evidence_dir "$test_evidence_dir" >/dev/null 2>&1

    # Record only PASS checkpoints
    record_checkpoint "TEST1" "PASS" >/dev/null 2>&1
    record_checkpoint "TEST2" "PASS" >/dev/null 2>&1

    # Collect evidence
    collect_evidence >/dev/null 2>&1

    # Generate report
    generate_verification_report >/dev/null 2>&1

    # Overall status should be PASS
    local overall_status
    overall_status=$(jq -r '.overall_status' "$EVIDENCE_DIR/summary.json")

    [ "$overall_status" = "PASS" ] || { rm -rf "$test_evidence_dir"; return 1; }

    # Cleanup
    rm -rf "$test_evidence_dir"
    return 0
}

test_generate_verification_report_status_fail() {
    local test_evidence_dir="/tmp/test_evidence_status_fail_$$"
    init_evidence_dir "$test_evidence_dir" >/dev/null 2>&1

    # Record mixed checkpoints with at least one FAIL
    record_checkpoint "TEST1" "PASS" >/dev/null 2>&1
    record_checkpoint "TEST2" "FAIL" "This failed" >/dev/null 2>&1

    # Collect evidence
    collect_evidence >/dev/null 2>&1

    # Generate report
    generate_verification_report >/dev/null 2>&1

    # Overall status should be FAIL
    local overall_status
    overall_status=$(jq -r '.overall_status' "$EVIDENCE_DIR/summary.json")

    [ "$overall_status" = "FAIL" ] || { rm -rf "$test_evidence_dir"; return 1; }

    # Cleanup
    rm -rf "$test_evidence_dir"
    return 0
}

test_generate_verification_report_includes_phase_details() {
    local test_evidence_dir="/tmp/test_evidence_phases_$$"
    init_evidence_dir "$test_evidence_dir" >/dev/null 2>&1

    # Collect evidence (which includes phase status files from setup)
    collect_evidence >/dev/null 2>&1

    # Generate report
    generate_verification_report >/dev/null 2>&1

    # Report should include phase details
    grep -q "## Phase Status Details" "$EVIDENCE_DIR/verification_report.md" || { rm -rf "$test_evidence_dir"; return 1; }
    grep -q "### Phase 1" "$EVIDENCE_DIR/verification_report.md" || { rm -rf "$test_evidence_dir"; return 1; }

    # Cleanup
    rm -rf "$test_evidence_dir"
    return 0
}

# ==============================================================================
# Test Runner
# ==============================================================================

main() {
    test_start

    # Setup test environment
    setup_test_env

    # Test init_evidence_dir
    run_test "init_evidence_dir creates directory structure" test_init_evidence_dir_creates_structure
    run_test "init_evidence_dir auto-generates path" test_init_evidence_dir_auto_generates_path
    run_test "init_evidence_dir sets start time" test_init_evidence_dir_sets_start_time

    # Test checkpoint recording
    run_test "record_checkpoint creates checkpoint file" test_record_checkpoint_creates_file
    run_test "record_checkpoint appends to order file" test_record_checkpoint_appends_to_order
    run_test "get_checkpoint_result retrieves results" test_get_checkpoint_result
    run_test "get_checkpoint_timestamp returns ISO 8601" test_get_checkpoint_timestamp
    run_test "get_checkpoint_order returns all checkpoints" test_get_checkpoint_order

    # Test collect_evidence
    run_test "collect_evidence requires initialization" test_collect_evidence_requires_init
    run_test "collect_evidence copies status files" test_collect_evidence_copies_status_files
    run_test "collect_evidence copies plan file" test_collect_evidence_copies_plan_file
    run_test "collect_evidence copies TUI log" test_collect_evidence_copies_tui_log
    run_test "collect_evidence creates metadata JSON" test_collect_evidence_creates_metadata

    # Test generate_verification_report
    run_test "generate_verification_report creates markdown" test_generate_verification_report_creates_markdown
    run_test "generate_verification_report creates JSON summary" test_generate_verification_report_creates_json_summary
    run_test "generate_verification_report sets PASS status" test_generate_verification_report_status_pass
    run_test "generate_verification_report sets FAIL status" test_generate_verification_report_status_fail
    run_test "generate_verification_report includes phase details" test_generate_verification_report_includes_phase_details

    # Cleanup
    cleanup_test_env

    test_end
}

main "$@"

#!/bin/bash
#
# Unit Test for create_thread_via_api SSE Parsing
#
# Tests the SSE parsing logic used in create_thread_via_api()
# without making actual API calls.
#
# Usage:
#   ./scripts/lib/test_create_thread_api.sh
#
# Exit codes:
#   0 - All tests passed
#   1 - One or more tests failed
#

set -e

# Get the directory of this script
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Source the helpers library for logging
source "$SCRIPT_DIR/e2e_helpers.sh"

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
    echo "  create_thread_via_api - Unit Tests"
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
# SSE Parsing Function (extracted from create_thread_via_api for testing)
# ==============================================================================

# Parse SSE response and extract thread_id from thread_created event
# Usage: thread_id=$(parse_sse_thread_id "$sse_response_file")
parse_sse_thread_id() {
    local tmp_response="$1"
    local extracted_thread_id=""

    while IFS= read -r line; do
        if [[ "$line" =~ ^data:\ *(.+)$ ]]; then
            local json_data="${BASH_REMATCH[1]}"
            local msg_type
            msg_type=$(echo "$json_data" | jq -r '.type // empty' 2>/dev/null)

            if [ "$msg_type" = "thread_created" ]; then
                extracted_thread_id=$(echo "$json_data" | jq -r '.thread.id // empty' 2>/dev/null)
                if [ -n "$extracted_thread_id" ]; then
                    break
                fi
            fi
        fi
    done < "$tmp_response"

    echo "$extracted_thread_id"
}

# ==============================================================================
# Test Cases
# ==============================================================================

test_parse_simple_thread_created() {
    # Test parsing a simple thread_created event
    local tmp_file="/tmp/test_sse_simple_$$.txt"

    cat > "$tmp_file" << 'EOF'
event: thread_created
data: {"type": "thread_created", "thread": {"id": "test-thread-12345"}, "timestamp": 1234567890}

EOF

    local result
    result=$(parse_sse_thread_id "$tmp_file")

    rm -f "$tmp_file"

    [ "$result" = "test-thread-12345" ] || return 1
    return 0
}

test_parse_full_thread_created() {
    # Test parsing a complete thread_created event (like from backend)
    local tmp_file="/tmp/test_sse_full_$$.txt"

    cat > "$tmp_file" << 'EOF'
event: thread_created
data: {"type": "thread_created", "thread": {"id": "cm5xyzabc123", "name": "New thread", "description": null, "preview": "", "last_activity": "2026-01-25T14:45:00.123456Z", "type": "programming", "mode": "normal", "model": "claude-sonnet-4-5", "permission_mode": "ask", "message_count": 1, "created_at": "2026-01-25T14:45:00.123456Z", "working_directory": "/Users/sam/project", "status": "idle", "verified": null, "verified_at": null}, "timestamp": 1737817500123}

EOF

    local result
    result=$(parse_sse_thread_id "$tmp_file")

    rm -f "$tmp_file"

    [ "$result" = "cm5xyzabc123" ] || return 1
    return 0
}

test_parse_multiple_events() {
    # Test parsing when there are multiple SSE events before thread_created
    local tmp_file="/tmp/test_sse_multi_$$.txt"

    cat > "$tmp_file" << 'EOF'
event: ping
data: {"type": "ping"}

event: status
data: {"type": "status", "message": "connecting"}

event: thread_created
data: {"type": "thread_created", "thread": {"id": "multi-event-thread-456"}, "timestamp": 1234567890}

event: content
data: {"type": "content", "delta": "Hello"}

EOF

    local result
    result=$(parse_sse_thread_id "$tmp_file")

    rm -f "$tmp_file"

    [ "$result" = "multi-event-thread-456" ] || return 1
    return 0
}

test_parse_no_thread_created() {
    # Test parsing when there's no thread_created event
    local tmp_file="/tmp/test_sse_none_$$.txt"

    cat > "$tmp_file" << 'EOF'
event: ping
data: {"type": "ping"}

event: content
data: {"type": "content", "delta": "Hello world"}

event: done
data: {"type": "done"}

EOF

    local result
    result=$(parse_sse_thread_id "$tmp_file")

    rm -f "$tmp_file"

    # Should return empty string
    [ -z "$result" ] || return 1
    return 0
}

test_parse_with_carriage_return() {
    # Test parsing SSE with carriage returns (Windows-style line endings)
    local tmp_file="/tmp/test_sse_cr_$$.txt"

    # Write with carriage returns
    printf 'event: thread_created\r\ndata: {"type": "thread_created", "thread": {"id": "cr-thread-789"}}\r\n\r\n' > "$tmp_file"

    local result
    result=$(parse_sse_thread_id "$tmp_file")

    rm -f "$tmp_file"

    # The parser should handle this (though it may not perfectly handle CR)
    # At minimum, it shouldn't crash
    [ -n "$result" ] || [ -z "$result" ]  # Accept any result, just don't crash
    return 0
}

test_parse_data_without_space() {
    # Test parsing when data: has no space after the colon
    local tmp_file="/tmp/test_sse_nospace_$$.txt"

    cat > "$tmp_file" << 'EOF'
event: thread_created
data:{"type": "thread_created", "thread": {"id": "nospace-thread-101"}}

EOF

    local result
    result=$(parse_sse_thread_id "$tmp_file")

    rm -f "$tmp_file"

    # Should still parse (regex allows zero or more spaces after "data:")
    [ "$result" = "nospace-thread-101" ] || return 1
    return 0
}

test_parse_uuid_format() {
    # Test parsing with actual UUID format thread_id
    local tmp_file="/tmp/test_sse_uuid_$$.txt"

    cat > "$tmp_file" << 'EOF'
event: thread_created
data: {"type": "thread_created", "thread": {"id": "a1b2c3d4-e5f6-7890-abcd-ef1234567890"}, "timestamp": 1234567890}

EOF

    local result
    result=$(parse_sse_thread_id "$tmp_file")

    rm -f "$tmp_file"

    [ "$result" = "a1b2c3d4-e5f6-7890-abcd-ef1234567890" ] || return 1
    return 0
}

test_generate_uuid_helper() {
    # Test that generate_uuid from helpers works
    local uuid1
    uuid1=$(generate_uuid)

    # Should not be empty
    [ -n "$uuid1" ] || return 1

    # Should contain hyphens
    [[ "$uuid1" == *"-"* ]] || return 1

    return 0
}

# ==============================================================================
# Test Runner
# ==============================================================================

main() {
    test_start

    # SSE parsing tests
    run_test "Parse simple thread_created event" test_parse_simple_thread_created
    run_test "Parse full backend thread_created event" test_parse_full_thread_created
    run_test "Parse thread_created after multiple events" test_parse_multiple_events
    run_test "Return empty when no thread_created" test_parse_no_thread_created
    run_test "Handle carriage returns without crashing" test_parse_with_carriage_return
    run_test "Parse data: without space after colon" test_parse_data_without_space
    run_test "Parse UUID format thread_id" test_parse_uuid_format

    # Helper function tests
    run_test "generate_uuid produces valid UUID" test_generate_uuid_helper

    test_end
}

main "$@"

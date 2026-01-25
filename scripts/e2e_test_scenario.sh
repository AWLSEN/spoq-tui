#!/bin/bash
#
# E2E Test Scenario for TUI Progress Circles
#
# This script simulates the full flow from plan creation to TUI circle rendering.
# It creates files exactly like Nova/Pulsar would create them, allowing manual
# verification of the conductor -> WebSocket -> TUI pipeline.
#
# Usage:
#   ./scripts/e2e_test_scenario.sh         # Run full test
#   ./scripts/e2e_test_scenario.sh cleanup # Just cleanup
#   ./scripts/e2e_test_scenario.sh setup   # Just setup (no cleanup)
#
# Prerequisites:
#   - conductor running (check: pgrep -f conductor)
#   - TUI can be started with: SPOQ_DEV=1 ./target/release/spoq
#   - Optional: websocat for WebSocket verification
#

set -e

# Source the E2E helpers library
source "$(dirname "$0")/lib/e2e_helpers.sh"

# Configuration
PLAN_ID="plan-test-e2e"
PROJECT="tui_spoq"
PLAN_DIR="$HOME/comms/plans/$PROJECT/active/$PLAN_ID"
STATUS_DIR="$PLAN_DIR/status"
TOTAL_PHASES=3
POLL_INTERVAL=6  # seconds (conductor polls every 5s)

# Step 0: Cleanup function
cleanup() {
    log_info "Cleaning up test files..."
    rm -rf "$PLAN_DIR"
    log_success "Cleanup complete"
}

# Step 1: Create plan directory and plan file
create_plan() {
    log_info "Creating plan directory structure..."
    mkdir -p "$STATUS_DIR"

    cat > "$PLAN_DIR/$PLAN_ID.md" << 'EOF'
# Plan: E2E Test Plan

## Metadata
- **ID**: plan-test-e2e
- **Project**: tui_spoq
- **Type**: test
- **Status**: active

## Phases

### Phase 1: Setup Test Environment
- **Description**: Create test fixtures
- **Files**: `test/fixture.rs`
- **Complexity**: Low

### Phase 2: Run Integration Tests
- **Description**: Execute integration suite
- **Files**: `tests/integration.rs`
- **Complexity**: Medium

### Phase 3: Verify Results
- **Description**: Check test output
- **Files**: `test/verify.rs`
- **Complexity**: Low
EOF

    log_success "Created plan file: $PLAN_DIR/$PLAN_ID.md"
}


# Main test flow
run_full_test() {
    echo "========================================"
    echo "  E2E Test: TUI Progress Circles"
    echo "========================================"
    echo ""

    # Check prerequisites
    check_conductor || true

    # Step 1: Create plan
    log_info "STEP 1: Creating plan structure..."
    create_plan
    echo ""

    # Step 2: Create phase 1 as running
    log_info "STEP 2: Setting phase 1 to 'running'..."
    create_status_file 1 "running"
    echo ""

    # Expected TUI display: [1] E2E Test Plan  ●○○ 1/3
    log_info "Expected TUI display: ●○○ 1/3"
    echo ""

    # Step 3: Wait and verify
    log_info "STEP 3: Waiting for conductor..."
    wait_for_conductor
    echo ""

    # Step 4: Progress to phase 2
    log_info "STEP 4: Completing phase 1, starting phase 2..."
    create_status_file 1 "completed"
    create_status_file 2 "running"
    echo ""

    # Expected TUI display: [1] E2E Test Plan  ●●○ 2/3
    log_info "Expected TUI display: ●●○ 2/3"
    echo ""

    # Step 5: Wait and verify
    log_info "STEP 5: Waiting for conductor..."
    wait_for_conductor
    echo ""

    # Step 6: Complete all phases
    log_info "STEP 6: Completing all phases..."
    create_status_file 2 "completed"
    create_status_file 3 "running"
    sleep 2
    create_status_file 3 "completed"
    echo ""

    # Expected TUI display: ●●● 3/3 (or cleared)
    log_info "Expected TUI display: ●●● 3/3"
    echo ""

    # Step 7: Final verification
    log_info "STEP 7: Final verification..."
    verify_files
    echo ""

    # Step 8: Instructions for manual TUI verification
    echo "========================================"
    echo "  Manual TUI Verification Steps"
    echo "========================================"
    echo ""
    echo "1. Start the TUI:"
    echo "   cd /Users/sam/tui_spoq && SPOQ_DEV=1 ./target/release/spoq"
    echo ""
    echo "2. Navigate to the dashboard (press 'd' or similar)"
    echo ""
    echo "3. Look for a thread showing '$PLAN_ID' with progress circles"
    echo ""
    echo "4. Expected formats at each step:"
    echo "   - After phase 1 running:   ●○○ 1/3"
    echo "   - After phase 2 running:   ●●○ 2/3"
    echo "   - After phase 3 complete:  ●●● 3/3"
    echo ""
    echo "5. Check TUI logs for any parse errors:"
    echo "   grep -i 'parse\|error' ~/.spoq/logs/spoq.log"
    echo ""

    # Cleanup prompt
    echo "========================================"
    read -p "Press Enter to cleanup test files (or Ctrl+C to keep them)..."
    cleanup
}

# Parse arguments
case "${1:-}" in
    cleanup)
        cleanup
        ;;
    setup)
        check_conductor || true
        create_plan
        create_status_file 1 "running"
        verify_files
        log_info "Setup complete. Run with 'cleanup' argument to remove files."
        ;;
    *)
        run_full_test
        ;;
esac

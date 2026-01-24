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

# Configuration
PLAN_ID="plan-test-e2e"
PROJECT="tui_spoq"
PLAN_DIR="$HOME/comms/plans/$PROJECT/active/$PLAN_ID"
STATUS_DIR="$PLAN_DIR/status"
TOTAL_PHASES=3
POLL_INTERVAL=6  # seconds (conductor polls every 5s)

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

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

# Step 2: Create status file for a phase
create_status_file() {
    local phase=$1
    local status=$2
    local timestamp=$(date -u +%Y-%m-%dT%H:%M:%SZ)

    local completed_at="null"
    if [ "$status" = "completed" ]; then
        completed_at="\"$timestamp\""
    fi

    cat > "$STATUS_DIR/phase-$phase.status" << EOF
{
  "task_id": "phase-$phase-$PLAN_ID",
  "thread_id": null,
  "project": "$PROJECT",
  "plan_id": "$PLAN_ID",
  "phase": $phase,
  "status": "$status",
  "tool_count": 5,
  "last_tool": "Edit",
  "last_file": "src/test.rs",
  "started_at": "$timestamp",
  "updated_at": "$timestamp",
  "completed_at": $completed_at
}
EOF

    log_success "Created status file: phase-$phase.status (status: $status)"
}

# Step 3: Check if conductor is running
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

# Step 4: Wait for conductor to pick up changes
wait_for_conductor() {
    log_info "Waiting ${POLL_INTERVAL}s for conductor to pick up status file..."
    sleep $POLL_INTERVAL
}

# Step 5: Check WebSocket (if websocat is available)
check_websocket() {
    if command -v websocat &> /dev/null; then
        log_info "Listening for WebSocket messages (5s timeout)..."
        timeout 5 websocat -t ws://localhost:8000/ws 2>/dev/null | head -5 || log_warn "No WebSocket messages received"
    else
        log_warn "websocat not installed. Install with: brew install websocat"
        log_info "Manual verification: Connect to ws://localhost:8000/ws and look for phase_progress_update messages"
    fi
}

# Step 6: Verify file structure
verify_files() {
    log_info "Verifying file structure..."

    if [ -f "$PLAN_DIR/$PLAN_ID.md" ]; then
        log_success "Plan file exists"
    else
        log_error "Plan file missing!"
        return 1
    fi

    for phase in $(seq 1 $TOTAL_PHASES); do
        if [ -f "$STATUS_DIR/phase-$phase.status" ]; then
            local status=$(jq -r '.status' "$STATUS_DIR/phase-$phase.status")
            log_success "Phase $phase status file exists (status: $status)"
        fi
    done
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

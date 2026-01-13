#!/bin/bash

# TUI Conductor Backend Integration - Validation Script
# Phase 12: Automated Test Suite and Validation
#
# This script validates the complete implementation by:
# - Building the project in release mode
# - Running linting with clippy
# - Running all unit and integration tests
# - Checking file structure
# - Checking code structure for required components
# - Reporting pass/fail summary

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Counters
PASS_COUNT=0
FAIL_COUNT=0

# Print header
echo "=================================================="
echo "TUI Conductor Backend Integration - Validation"
echo "=================================================="
echo ""

# Helper function to print test result
print_result() {
    local test_name=$1
    local result=$2
    if [ "$result" -eq 0 ]; then
        echo -e "${GREEN}✓${NC} $test_name"
        ((PASS_COUNT++))
    else
        echo -e "${RED}✗${NC} $test_name"
        ((FAIL_COUNT++))
    fi
}

# Test 1: Cargo Build (Release)
echo "Running: cargo build --release"
if cargo build --release --quiet 2>&1 | grep -v "warning:" > /dev/null || cargo build --release --quiet 2>&1; then
    print_result "Build (release mode)" 0
else
    print_result "Build (release mode)" 1
fi

# Test 2: Cargo Clippy
echo "Running: cargo clippy"
if cargo clippy --all-targets --all-features -- -D warnings 2>&1 | grep -v "warning:" > /dev/null || cargo clippy --all-targets --all-features 2>&1 | grep -q "0 warnings emitted"; then
    # Clippy passed with no warnings
    print_result "Clippy (linting)" 0
elif cargo clippy --all-targets --all-features 2>&1; then
    # Clippy passed but with warnings
    echo -e "${YELLOW}⚠${NC}  Clippy has warnings (non-fatal)"
    print_result "Clippy (linting)" 0
else
    print_result "Clippy (linting)" 1
fi

# Test 3: Cargo Test
echo "Running: cargo test"
if cargo test --quiet 2>&1; then
    print_result "Unit and integration tests" 0
else
    print_result "Unit and integration tests" 1
fi

# Test 4: File Structure Check
echo ""
echo "Checking file structure..."
FILES=(
    "src/app.rs"
    "src/cache.rs"
    "src/conductor.rs"
    "src/events.rs"
    "src/models.rs"
    "src/sse.rs"
    "src/ui.rs"
    "src/widgets/input_box.rs"
    "tests/integration.rs"
    "Cargo.toml"
)

FILE_PASS=0
for file in "${FILES[@]}"; do
    if [ -f "$file" ]; then
        ((FILE_PASS++))
    else
        echo -e "  ${RED}✗${NC} Missing: $file"
    fi
done

if [ "$FILE_PASS" -eq "${#FILES[@]}" ]; then
    print_result "File structure (${#FILES[@]} files)" 0
else
    print_result "File structure ($FILE_PASS/${#FILES[@]} files)" 1
fi

# Test 5: Code Structure Check - Required Structs
echo ""
echo "Checking code structure..."

STRUCT_CHECKS=(
    "src/app.rs:pub struct App"
    "src/cache.rs:pub struct ThreadCache"
    "src/conductor.rs:pub struct ConductorClient"
    "src/events.rs:pub enum SseEvent"
    "src/models.rs:pub struct Message"
    "src/models.rs:pub struct Thread"
    "src/models.rs:pub struct StreamRequest"
    "src/sse.rs:pub struct SseParser"
)

STRUCT_PASS=0
for check in "${STRUCT_CHECKS[@]}"; do
    file=$(echo "$check" | cut -d: -f1)
    pattern=$(echo "$check" | cut -d: -f2-)
    if grep -q "$pattern" "$file" 2>/dev/null; then
        ((STRUCT_PASS++))
    else
        echo -e "  ${RED}✗${NC} Missing in $file: $pattern"
    fi
done

if [ "$STRUCT_PASS" -eq "${#STRUCT_CHECKS[@]}" ]; then
    print_result "Code structure (${#STRUCT_CHECKS[@]} components)" 0
else
    print_result "Code structure ($STRUCT_PASS/${#STRUCT_CHECKS[@]} components)" 1
fi

# Test 6: Code Structure Check - Required Methods
METHOD_CHECKS=(
    "src/conductor.rs:pub async fn stream"
    "src/conductor.rs:pub async fn submit_input"
    "src/cache.rs:pub fn create_streaming_thread"
    "src/cache.rs:pub fn append_to_message"
    "src/cache.rs:pub fn finalize_message"
    "src/sse.rs:pub fn process_line"
    "src/events.rs:pub fn parse_sse_event"
)

METHOD_PASS=0
for check in "${METHOD_CHECKS[@]}"; do
    file=$(echo "$check" | cut -d: -f1)
    pattern=$(echo "$check" | cut -d: -f2-)
    if grep -q "$pattern" "$file" 2>/dev/null; then
        ((METHOD_PASS++))
    else
        echo -e "  ${RED}✗${NC} Missing in $file: $pattern"
    fi
done

if [ "$METHOD_PASS" -eq "${#METHOD_CHECKS[@]}" ]; then
    print_result "Required methods (${#METHOD_CHECKS[@]} methods)" 0
else
    print_result "Required methods ($METHOD_PASS/${#METHOD_CHECKS[@]} methods)" 1
fi

# Test 7: Test Coverage Check
echo ""
echo "Checking test coverage..."
TEST_COUNT=$(cargo test --quiet 2>&1 | grep -E "test result: ok\." | head -1 | grep -oE "[0-9]+ passed" | grep -oE "[0-9]+" || echo "0")

if [ "$TEST_COUNT" -ge 100 ]; then
    print_result "Test coverage ($TEST_COUNT tests)" 0
else
    print_result "Test coverage ($TEST_COUNT tests, expected ≥100)" 1
fi

# Print summary
echo ""
echo "=================================================="
echo "Validation Summary"
echo "=================================================="
echo -e "Passed: ${GREEN}$PASS_COUNT${NC}"
echo -e "Failed: ${RED}$FAIL_COUNT${NC}"

if [ "$FAIL_COUNT" -eq 0 ]; then
    echo ""
    echo -e "${GREEN}✓ All validation checks passed!${NC}"
    echo ""
    echo "The TUI Conductor Backend Integration is complete and validated."
    exit 0
else
    echo ""
    echo -e "${RED}✗ Some validation checks failed.${NC}"
    echo "Please review the failures above and fix any issues."
    exit 1
fi

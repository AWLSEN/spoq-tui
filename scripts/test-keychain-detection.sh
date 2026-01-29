#!/bin/bash
# Test script for keychain change detection
#
# This script:
# 1. Uses a TEST keychain item (not real Claude Code credentials)
# 2. Logs keychain poller activity to /tmp/spoq_keychain_test.log
# 3. Allows you to modify the test keychain to verify detection
#
# Usage:
#   Terminal 1: ./scripts/test-keychain-detection.sh
#   Terminal 2: ./scripts/modify-test-keychain.sh
#   Then check: tail -f /tmp/spoq_keychain_test.log

set -e

LOG_FILE="/tmp/spoq_keychain_test.log"
TEST_SERVICE="spoq-test-credentials"

echo "=== Keychain Detection Test ===" | tee "$LOG_FILE"
echo "Log file: $LOG_FILE" | tee -a "$LOG_FILE"
echo "Test keychain service: $TEST_SERVICE" | tee -a "$LOG_FILE"
echo "" | tee -a "$LOG_FILE"

# Ensure test keychain exists
if ! security find-generic-password -s "$TEST_SERVICE" -a "$USER" &>/dev/null; then
    echo "Creating test keychain item..." | tee -a "$LOG_FILE"
    security add-generic-password -s "$TEST_SERVICE" -a "$USER" -w "initial-test-token"
fi

echo "Current test keychain value:" | tee -a "$LOG_FILE"
security find-generic-password -s "$TEST_SERVICE" -a "$USER" -w 2>/dev/null | tee -a "$LOG_FILE"
echo "" | tee -a "$LOG_FILE"

echo "Starting app with test keychain..." | tee -a "$LOG_FILE"
echo "Keychain poller will log to: $LOG_FILE" | tee -a "$LOG_FILE"
echo "" | tee -a "$LOG_FILE"
echo "To modify test keychain (in another terminal):" | tee -a "$LOG_FILE"
echo "  security add-generic-password -U -s '$TEST_SERVICE' -a \"\$USER\" -w 'new-value-\$(date +%s)'" | tee -a "$LOG_FILE"
echo "" | tee -a "$LOG_FILE"
echo "Press Ctrl+C to stop" | tee -a "$LOG_FILE"
echo "===============================" | tee -a "$LOG_FILE"

# Run with test keychain and debug logging
SPOQ_TEST_KEYCHAIN_SERVICE="$TEST_SERVICE" \
RUST_LOG="spoq::credential_watcher=debug,spoq::conductor=debug" \
cargo run 2>&1 | tee -a "$LOG_FILE"

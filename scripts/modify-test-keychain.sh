#!/bin/bash
# Modify the test keychain item to trigger change detection
#
# Usage: ./scripts/modify-test-keychain.sh [value]
# If no value provided, uses a timestamp-based value

TEST_SERVICE="spoq-test-credentials"
VALUE="${1:-test-token-$(date +%s)}"

echo "Modifying test keychain..."
echo "Service: $TEST_SERVICE"
echo "New value: $VALUE"

security add-generic-password -U -s "$TEST_SERVICE" -a "$USER" -w "$VALUE"

echo ""
echo "Done! Check /tmp/spoq_keychain_test.log for detection."
echo "Note: Poller runs every 30 seconds, so wait up to 30s to see the change."

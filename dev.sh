#!/bin/bash
# Local development script for SPOQ CLI
# Connects to conductor at localhost:8000
# Run: ./dev.sh

cd "$(dirname "$0")"

echo "========================================="
echo "SPOQ CLI (dev)"
echo "========================================="
echo "Connecting to conductor at localhost:8000"
echo ""

export SPOQ_DEV=1
export RUST_LOG="${RUST_LOG:-info}"

cargo run

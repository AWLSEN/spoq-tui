#!/bin/bash
#
# Verify Status File Format
#
# This script validates that status files match the expected format
# for conductor's PulsarMonitor and TUI's phase progress display.
#
# Usage:
#   ./scripts/verify_status_format.sh                    # Check all active status files
#   ./scripts/verify_status_format.sh path/to/file.status # Check specific file
#

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_success() { echo -e "${GREEN}[OK]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }

# Required fields for a valid status file
REQUIRED_FIELDS=(
    "task_id"
    "project"
    "plan_id"
    "phase"
    "status"
)

# Optional fields
OPTIONAL_FIELDS=(
    "thread_id"
    "tool_count"
    "last_tool"
    "last_file"
    "started_at"
    "updated_at"
    "completed_at"
)

validate_file() {
    local file="$1"
    local errors=0

    echo "Validating: $file"

    # Check if file exists
    if [ ! -f "$file" ]; then
        log_error "File does not exist"
        return 1
    fi

    # Check if valid JSON
    if ! jq empty "$file" 2>/dev/null; then
        log_error "Invalid JSON"
        return 1
    fi

    # Check required fields
    for field in "${REQUIRED_FIELDS[@]}"; do
        value=$(jq -r ".$field // \"__MISSING__\"" "$file")
        if [ "$value" = "__MISSING__" ] || [ "$value" = "null" ]; then
            log_error "Missing required field: $field"
            ((errors++))
        else
            log_success "$field: $value"
        fi
    done

    # Check optional fields
    for field in "${OPTIONAL_FIELDS[@]}"; do
        value=$(jq -r ".$field // \"__MISSING__\"" "$file")
        if [ "$value" != "__MISSING__" ]; then
            log_success "$field: $value"
        fi
    done

    # Validate status value
    status=$(jq -r '.status' "$file")
    case "$status" in
        running|completed|pending|failed)
            log_success "Status value is valid: $status"
            ;;
        *)
            log_error "Invalid status value: $status (expected: running, completed, pending, or failed)"
            ((errors++))
            ;;
    esac

    # Validate phase is a number
    phase=$(jq -r '.phase' "$file")
    if ! [[ "$phase" =~ ^[0-9]+$ ]]; then
        log_error "Phase must be a number: $phase"
        ((errors++))
    fi

    # Validate timestamps are ISO 8601 format (if present)
    for ts_field in started_at updated_at completed_at; do
        ts=$(jq -r ".$ts_field // \"null\"" "$file")
        if [ "$ts" != "null" ] && [ -n "$ts" ]; then
            # Basic ISO 8601 check (YYYY-MM-DDTHH:MM:SSZ)
            if [[ "$ts" =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z$ ]]; then
                log_success "Timestamp $ts_field is valid ISO 8601"
            else
                log_warn "Timestamp $ts_field may not be ISO 8601: $ts"
            fi
        fi
    done

    echo ""
    if [ $errors -eq 0 ]; then
        log_success "File is valid!"
        return 0
    else
        log_error "Found $errors error(s)"
        return 1
    fi
}

# Main
if [ -n "$1" ]; then
    # Validate specific file
    validate_file "$1"
else
    # Find all status files in active plans
    echo "Scanning for status files in ~/comms/plans/*/active/*/status/*.status"
    echo ""

    found=0
    for file in ~/comms/plans/*/active/*/status/*.status; do
        if [ -f "$file" ]; then
            validate_file "$file"
            echo "----------------------------------------"
            ((found++))
        fi
    done

    if [ $found -eq 0 ]; then
        log_warn "No status files found"
    else
        echo "Validated $found status file(s)"
    fi
fi

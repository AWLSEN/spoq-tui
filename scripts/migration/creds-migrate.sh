#!/usr/bin/env bash
#
# creds-migrate.sh - Extract and restore CLI tool credentials for VPS migration
#
# Supported tools:
#   - GitHub CLI (gh)
#   - Claude Code
#   - Codex (OpenAI)
#
# Usage:
#   ./creds-migrate.sh export [output_file]   - Export credentials to archive
#   ./creds-migrate.sh import <archive_file>  - Import credentials from archive
#   ./creds-migrate.sh list                   - List detected credentials
#
# Works on: macOS, Linux

set -euo pipefail

VERSION="1.0.0"
SCRIPT_NAME="$(basename "$0")"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Files/dirs to include in export (will be populated dynamically)
EXPORT_ITEMS=()

# Temporary directory for staging
STAGING_DIR=""

# Flag for Claude Code keychain availability (macOS only)
CLAUDE_KEYCHAIN_AVAILABLE=false

#------------------------------------------------------------------------------
# Utility Functions
#------------------------------------------------------------------------------

log_info() {
    echo -e "${BLUE}[INFO]${NC} $*"
}

log_success() {
    echo -e "${GREEN}[OK]${NC} $*"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $*"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $*" >&2
}

die() {
    log_error "$@"
    exit 1
}

detect_os() {
    case "$(uname -s)" in
        Darwin) echo "macos" ;;
        Linux)  echo "linux" ;;
        *)      echo "unknown" ;;
    esac
}

get_home_dir() {
    echo "${HOME:-$(eval echo ~)}"
}

cleanup() {
    if [[ -n "${STAGING_DIR:-}" && -d "${STAGING_DIR}" ]]; then
        rm -rf "${STAGING_DIR}"
    fi
}

trap cleanup EXIT

#------------------------------------------------------------------------------
# Credential Detection
#------------------------------------------------------------------------------

check_gh_credentials() {
    local home_dir="$1"
    local hosts_file="${home_dir}/.config/gh/hosts.yml"

    if [[ -f "$hosts_file" ]]; then
        log_success "GitHub CLI: Found credentials at ~/.config/gh/"
        EXPORT_ITEMS+=(".config/gh")
        return 0
    else
        log_warn "GitHub CLI: No credentials found"
        return 1
    fi
}

check_claude_credentials() {
    local home_dir="$1"
    local claude_json="${home_dir}/.claude.json"
    local claude_dir="${home_dir}/.claude"
    local found=false
    local os_type
    os_type="$(detect_os)"

    # On macOS, check Keychain for actual OAuth tokens
    if [[ "$os_type" == "macos" ]]; then
        if security find-generic-password -s "Claude Code-credentials" &>/dev/null; then
            log_success "Claude Code: Found OAuth tokens in macOS Keychain"
            # Mark that we need to extract from keychain (handled in export)
            CLAUDE_KEYCHAIN_AVAILABLE=true
            found=true
        fi
    fi

    if [[ -f "$claude_json" ]]; then
        # Check if it contains OAuth account info
        if grep -q "oauthAccount" "$claude_json" 2>/dev/null; then
            log_success "Claude Code: Found account metadata in ~/.claude.json"
            EXPORT_ITEMS+=(".claude.json")
            found=true
        fi
    fi

    if [[ -d "$claude_dir" ]]; then
        # Export essential Claude config files (not history/cache)
        local has_settings=false
        if [[ -f "${home_dir}/.claude/settings.json" ]]; then
            has_settings=true
        fi
        if [[ -f "${home_dir}/.claude/settings.local.json" ]]; then
            has_settings=true
        fi

        if $has_settings; then
            log_success "Claude Code: Found config directory at ~/.claude/"
            # Add .claude.json only if not already added
            if ! $found; then
                EXPORT_ITEMS+=(".claude.json")
            fi
            EXPORT_ITEMS+=(".claude/settings.json")
            EXPORT_ITEMS+=(".claude/settings.local.json")
            found=true
        fi
    fi

    if ! $found; then
        log_warn "Claude Code: No credentials found"
        return 1
    fi
    return 0
}

check_codex_credentials() {
    local home_dir="$1"
    local codex_auth="${home_dir}/.codex/auth.json"

    if [[ -f "$codex_auth" ]]; then
        log_success "Codex: Found credentials at ~/.codex/"
        EXPORT_ITEMS+=(".codex")
        return 0
    else
        log_warn "Codex: No credentials found"
        return 1
    fi
}

#------------------------------------------------------------------------------
# Export Function
#------------------------------------------------------------------------------

do_export() {
    local output_file="${1:-}"
    local home_dir
    home_dir="$(get_home_dir)"
    local os_type
    os_type="$(detect_os)"
    local timestamp
    timestamp="$(date +%Y%m%d_%H%M%S)"

    log_info "Detecting credentials on ${os_type}..."
    echo ""

    # Detect all credentials
    local found_any=false
    check_gh_credentials "$home_dir" && found_any=true
    check_claude_credentials "$home_dir" && found_any=true
    check_codex_credentials "$home_dir" && found_any=true

    echo ""

    if ! $found_any; then
        die "No credentials found to export"
    fi

    # Set default output file if not provided
    if [[ -z "$output_file" ]]; then
        output_file="spoq-creds-${timestamp}.tar.gz"
    fi

    # Create staging directory
    STAGING_DIR="$(mktemp -d)"
    local manifest_file="${STAGING_DIR}/manifest.json"

    log_info "Staging credentials for export..."

    # Copy files to staging
    for item in "${EXPORT_ITEMS[@]}"; do
        local src="${home_dir}/${item}"
        local dest="${STAGING_DIR}/${item}"

        if [[ -e "$src" ]]; then
            mkdir -p "$(dirname "$dest")"
            if [[ -d "$src" ]]; then
                # For directories, copy without cache/history files
                rsync -a --exclude='*.log' \
                         --exclude='history.jsonl' \
                         --exclude='cache/' \
                         --exclude='session-env/' \
                         --exclude='shell-snapshots/' \
                         --exclude='telemetry/' \
                         --exclude='debug/' \
                         --exclude='todos/' \
                         --exclude='paste-cache/' \
                         --exclude='file-history/' \
                         --exclude='projects/' \
                         --exclude='statsig/' \
                         --exclude='sessions/' \
                         --exclude='log/' \
                         "$src/" "$dest/" 2>/dev/null || cp -r "$src" "$dest"
            else
                cp "$src" "$dest"
            fi
            log_success "  Staged: ${item}"
        fi
    done

    # Extract Claude Code OAuth tokens from macOS Keychain
    if [[ "${CLAUDE_KEYCHAIN_AVAILABLE}" == "true" ]]; then
        log_info "Extracting Claude Code OAuth tokens from Keychain..."
        local keychain_creds
        keychain_creds="$(security find-generic-password -s "Claude Code-credentials" -w 2>/dev/null || true)"

        if [[ -n "$keychain_creds" ]]; then
            mkdir -p "${STAGING_DIR}/.claude"
            echo "$keychain_creds" > "${STAGING_DIR}/.claude/credentials.json"
            chmod 600 "${STAGING_DIR}/.claude/credentials.json"
            log_success "  Staged: .claude/credentials.json (from Keychain)"

            # Add to items list for manifest
            EXPORT_ITEMS+=(".claude/credentials.json")
        else
            log_warn "  Could not extract Keychain credentials (may need password)"
        fi
    fi

    # Create manifest
    cat > "$manifest_file" << EOF
{
    "version": "${VERSION}",
    "created_at": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
    "source_os": "${os_type}",
    "source_hostname": "$(hostname)",
    "items": $(printf '%s\n' "${EXPORT_ITEMS[@]}" | jq -R . | jq -s .)
}
EOF

    # Create archive
    log_info "Creating archive: ${output_file}"
    tar -czf "$output_file" -C "$STAGING_DIR" .

    echo ""
    log_success "Export complete!"
    log_info "Archive: ${output_file}"
    log_info "Size: $(du -h "$output_file" | cut -f1)"
    echo ""
    log_warn "SECURITY: This archive contains sensitive credentials."
    log_warn "          Transfer securely and delete after import."
}

#------------------------------------------------------------------------------
# Import Function
#------------------------------------------------------------------------------

do_import() {
    local archive_file="${1:-}"
    local home_dir
    home_dir="$(get_home_dir)"

    if [[ -z "$archive_file" ]]; then
        die "Usage: ${SCRIPT_NAME} import <archive_file>"
    fi

    if [[ ! -f "$archive_file" ]]; then
        die "Archive not found: ${archive_file}"
    fi

    # Create staging directory
    STAGING_DIR="$(mktemp -d)"

    log_info "Extracting archive..."
    tar -xzf "$archive_file" -C "$STAGING_DIR"

    # Read manifest
    local manifest_file="${STAGING_DIR}/manifest.json"
    if [[ ! -f "$manifest_file" ]]; then
        die "Invalid archive: manifest.json not found"
    fi

    local source_os
    source_os="$(jq -r '.source_os' "$manifest_file")"
    local created_at
    created_at="$(jq -r '.created_at' "$manifest_file")"

    log_info "Archive created: ${created_at}"
    log_info "Source OS: ${source_os}"
    echo ""

    # Import each item
    local items
    items="$(jq -r '.items[]' "$manifest_file")"

    for item in $items; do
        local src="${STAGING_DIR}/${item}"
        local dest="${home_dir}/${item}"

        if [[ -e "$src" ]]; then
            # Backup existing if present
            if [[ -e "$dest" ]]; then
                local backup="${dest}.backup.$(date +%Y%m%d_%H%M%S)"
                log_warn "Backing up existing: ${item} -> $(basename "$backup")"
                mv "$dest" "$backup"
            fi

            # Create parent directory if needed
            mkdir -p "$(dirname "$dest")"

            # Copy to destination
            if [[ -d "$src" ]]; then
                cp -r "$src" "$dest"
            else
                cp "$src" "$dest"
            fi

            # Set secure permissions for sensitive files
            if [[ "$item" == *"hosts.yml"* ]] || [[ "$item" == *"auth.json"* ]] || [[ "$item" == *"credentials.json"* ]]; then
                chmod 600 "$dest"
            fi

            log_success "Imported: ${item}"
        fi
    done

    # Handle Claude Code credentials on Linux (no Keychain)
    local creds_file="${home_dir}/.claude/credentials.json"
    if [[ -f "$creds_file" ]] && [[ "$(detect_os)" == "linux" ]]; then
        log_info "Claude Code credentials imported to ~/.claude/credentials.json"
        log_info "Claude Code on Linux should read from this file automatically."
    fi

    echo ""
    log_success "Import complete!"
    log_info "You may need to restart your terminal or CLI tools."
}

#------------------------------------------------------------------------------
# List Function
#------------------------------------------------------------------------------

do_list() {
    local home_dir
    home_dir="$(get_home_dir)"
    local os_type
    os_type="$(detect_os)"

    echo ""
    echo "=== Credential Status (${os_type}) ==="
    echo ""

    check_gh_credentials "$home_dir" || true
    check_claude_credentials "$home_dir" || true
    check_codex_credentials "$home_dir" || true

    echo ""
}

#------------------------------------------------------------------------------
# Main
#------------------------------------------------------------------------------

show_help() {
    cat << EOF
${SCRIPT_NAME} v${VERSION} - CLI Credentials Migration Tool

USAGE:
    ${SCRIPT_NAME} export [output_file]   Export credentials to archive
    ${SCRIPT_NAME} import <archive_file>  Import credentials from archive
    ${SCRIPT_NAME} list                   List detected credentials
    ${SCRIPT_NAME} help                   Show this help

SUPPORTED TOOLS:
    - GitHub CLI (gh)     ~/.config/gh/
    - Claude Code         macOS Keychain + ~/.claude.json, ~/.claude/
    - Codex (OpenAI)      ~/.codex/

NOTES:
    On macOS, Claude Code OAuth tokens are stored in Keychain.
    This script extracts them and stores in .claude/credentials.json
    for import on Linux systems.

EXAMPLES:
    # Export credentials
    ${SCRIPT_NAME} export
    ${SCRIPT_NAME} export my-creds.tar.gz

    # Import on target machine
    ${SCRIPT_NAME} import spoq-creds-20250119.tar.gz

    # Check what credentials exist
    ${SCRIPT_NAME} list

EOF
}

main() {
    local command="${1:-help}"
    shift || true

    case "$command" in
        export)
            do_export "$@"
            ;;
        import)
            do_import "$@"
            ;;
        list)
            do_list
            ;;
        help|--help|-h)
            show_help
            ;;
        *)
            die "Unknown command: ${command}. Use '${SCRIPT_NAME} help' for usage."
            ;;
    esac
}

main "$@"

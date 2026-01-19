#!/usr/bin/env bash
#
# repos-manage.sh - List and manage GitHub repositories for VPS workflow
#
# Usage:
#   ./repos-manage.sh list                    - List all accessible repos
#   ./repos-manage.sh list --local            - Show only locally cloned repos
#   ./repos-manage.sh clone <repo>            - Clone a repo (or pull if exists)
#   ./repos-manage.sh clone-all               - Clone all repos (use with caution)
#   ./repos-manage.sh sync <repo>             - Pull latest for a repo
#   ./repos-manage.sh sync-all                - Pull latest for all local repos
#   ./repos-manage.sh search <query>          - Search repos by name
#
# Environment:
#   REPOS_DIR - Base directory for repos (default: ~/repos)
#
# Requires: gh (GitHub CLI), git

set -euo pipefail

VERSION="1.0.0"
SCRIPT_NAME="$(basename "$0")"

# Configuration
REPOS_DIR="${REPOS_DIR:-${HOME}/repos}"
CACHE_FILE="${HOME}/.cache/spoq/repos-cache.json"
CACHE_TTL=300  # 5 minutes

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
DIM='\033[2m'
NC='\033[0m'

#------------------------------------------------------------------------------
# Utility Functions
#------------------------------------------------------------------------------

log_info() { echo -e "${BLUE}[INFO]${NC} $*"; }
log_success() { echo -e "${GREEN}[OK]${NC} $*"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $*"; }
log_error() { echo -e "${RED}[ERROR]${NC} $*" >&2; }
die() { log_error "$@"; exit 1; }

check_dependencies() {
    if ! command -v gh &>/dev/null; then
        die "GitHub CLI (gh) is required. Install: https://cli.github.com/"
    fi
    if ! gh auth status &>/dev/null; then
        die "Not logged into GitHub CLI. Run: gh auth login"
    fi
    if ! command -v jq &>/dev/null; then
        die "jq is required. Install: brew install jq (macOS) or apt install jq (Linux)"
    fi
}

ensure_repos_dir() {
    if [[ ! -d "$REPOS_DIR" ]]; then
        mkdir -p "$REPOS_DIR"
        log_info "Created repos directory: $REPOS_DIR"
    fi
}

ensure_cache_dir() {
    local cache_dir
    cache_dir="$(dirname "$CACHE_FILE")"
    [[ -d "$cache_dir" ]] || mkdir -p "$cache_dir"
}

#------------------------------------------------------------------------------
# Repository Fetching (with caching)
#------------------------------------------------------------------------------

is_cache_valid() {
    if [[ ! -f "$CACHE_FILE" ]]; then
        return 1
    fi
    local cache_age
    cache_age=$(( $(date +%s) - $(stat -f %m "$CACHE_FILE" 2>/dev/null || stat -c %Y "$CACHE_FILE" 2>/dev/null) ))
    [[ $cache_age -lt $CACHE_TTL ]]
}

fetch_repos() {
    local force="${1:-false}"

    ensure_cache_dir

    if [[ "$force" != "true" ]] && is_cache_valid; then
        cat "$CACHE_FILE"
        return
    fi

    log_info "Fetching repositories from GitHub..." >&2

    local all_repos="[]"

    # 1. Fetch user's own repos (owner + collaborator)
    log_info "  Fetching personal repositories..." >&2
    local user_repos
    user_repos=$(gh repo list --limit 1000 --json nameWithOwner,description,isPrivate,pushedAt,primaryLanguage,isFork,url 2>/dev/null || echo "[]")
    all_repos=$(echo "$all_repos" "$user_repos" | jq -s 'add')

    # 2. Fetch all organizations the user belongs to
    log_info "  Fetching organization memberships..." >&2
    local orgs
    orgs=$(gh api user/orgs --jq '.[].login' 2>/dev/null || echo "")

    # 3. Fetch repos from each organization
    if [[ -n "$orgs" ]]; then
        while IFS= read -r org; do
            [[ -z "$org" ]] && continue
            log_info "  Fetching repos from org: ${org}..." >&2
            local org_repos
            org_repos=$(gh repo list "$org" --limit 1000 --json nameWithOwner,description,isPrivate,pushedAt,primaryLanguage,isFork,url 2>/dev/null || echo "[]")
            all_repos=$(echo "$all_repos" "$org_repos" | jq -s 'add')
        done <<< "$orgs"
    fi

    # 4. Fetch repos where user is a collaborator (outside of own/org repos)
    log_info "  Fetching collaborated repositories..." >&2
    local collab_repos
    collab_repos=$(gh api user/repos --paginate --jq '.[] | {nameWithOwner: .full_name, description: .description, isPrivate: .private, pushedAt: .pushed_at, primaryLanguage: {name: .language}, isFork: .fork, url: .html_url}' 2>/dev/null | jq -s '.' || echo "[]")
    all_repos=$(echo "$all_repos" "$collab_repos" | jq -s 'add')

    # Deduplicate and sort by most recently pushed
    all_repos=$(echo "$all_repos" | jq 'unique_by(.nameWithOwner) | sort_by(.pushedAt) | reverse')

    local count
    count=$(echo "$all_repos" | jq 'length')
    log_info "  Found ${count} repositories total." >&2

    # Cache the result
    echo "$all_repos" > "$CACHE_FILE"
    echo "$all_repos"
}

#------------------------------------------------------------------------------
# Local Repo Detection
#------------------------------------------------------------------------------

get_local_repos() {
    if [[ ! -d "$REPOS_DIR" ]]; then
        echo "[]"
        return
    fi

    local repos="[]"

    # Find all git repos in REPOS_DIR (2 levels deep: owner/repo)
    while IFS= read -r git_dir; do
        local repo_path
        repo_path="$(dirname "$git_dir")"
        local repo_name
        repo_name="$(basename "$(dirname "$repo_path")")/$(basename "$repo_path")"

        # Get remote URL to determine full name
        local remote_url
        remote_url=$(git -C "$repo_path" remote get-url origin 2>/dev/null || echo "")

        if [[ -n "$remote_url" ]]; then
            # Extract owner/repo from URL
            local full_name
            full_name=$(echo "$remote_url" | sed -E 's#.*github\.com[:/]([^/]+/[^/]+)(\.git)?$#\1#')
            repos=$(echo "$repos" | jq --arg name "$full_name" --arg path "$repo_path" '. + [{"name": $name, "path": $path}]')
        fi
    done < <(find "$REPOS_DIR" -maxdepth 3 -name ".git" -type d 2>/dev/null)

    echo "$repos"
}

is_repo_cloned() {
    local repo_name="$1"
    local owner="${repo_name%%/*}"
    local name="${repo_name##*/}"
    local repo_path="${REPOS_DIR}/${owner}/${name}"

    [[ -d "${repo_path}/.git" ]]
}

get_repo_path() {
    local repo_name="$1"
    local owner="${repo_name%%/*}"
    local name="${repo_name##*/}"
    echo "${REPOS_DIR}/${owner}/${name}"
}

#------------------------------------------------------------------------------
# Commands
#------------------------------------------------------------------------------

cmd_list() {
    local show_local_only=false
    local refresh=false
    local json_output=false

    while [[ $# -gt 0 ]]; do
        case "$1" in
            --local|-l) show_local_only=true; shift ;;
            --refresh|-r) refresh=true; shift ;;
            --json|-j) json_output=true; shift ;;
            *) shift ;;
        esac
    done

    check_dependencies
    ensure_repos_dir

    if $show_local_only; then
        local local_repos
        local_repos=$(get_local_repos)

        if $json_output; then
            echo "$local_repos"
            return
        fi

        echo ""
        echo -e "${CYAN}=== Local Repositories (${REPOS_DIR}) ===${NC}"
        echo ""

        local count
        count=$(echo "$local_repos" | jq 'length')

        if [[ "$count" -eq 0 ]]; then
            echo "  No repositories cloned yet."
            echo ""
            echo "  Use: $SCRIPT_NAME clone <owner/repo>"
            return
        fi

        echo "$local_repos" | jq -r '.[] | "  \(.name) → \(.path)"'
        echo ""
        echo -e "${DIM}Total: ${count} repos${NC}"
        return
    fi

    local repos
    if $refresh; then
        repos=$(fetch_repos true)
    else
        repos=$(fetch_repos)
    fi

    # For JSON output, add cloned status and output raw JSON
    if $json_output; then
        local local_repos
        local_repos=$(get_local_repos)

        # Add 'cloned' and 'localPath' fields to each repo
        echo "$repos" | jq --argjson local "$local_repos" '
            map(. + {
                cloned: (any($local[]; .name == .nameWithOwner)),
                localPath: (($local | map(select(.name == .nameWithOwner)) | first // null) | .path // null)
            })
        '
        return
    fi

    local local_repos
    local_repos=$(get_local_repos)

    echo ""
    echo -e "${CYAN}=== GitHub Repositories ===${NC}"
    echo ""

    # Print header
    printf "  ${DIM}%-40s %-10s %-8s %s${NC}\n" "REPOSITORY" "LANGUAGE" "STATUS" "DESCRIPTION"
    printf "  ${DIM}%-40s %-10s %-8s %s${NC}\n" "──────────" "────────" "──────" "───────────"

    echo "$repos" | jq -r '.[] | "\(.nameWithOwner)|\(.primaryLanguage.name // "-")|\(.isPrivate)|\(.description // "")"' | while IFS='|' read -r name lang private desc; do
        local status=""
        local status_color=""

        if is_repo_cloned "$name"; then
            status="cloned"
            status_color="${GREEN}"
        else
            status="remote"
            status_color="${DIM}"
        fi

        local visibility=""
        if [[ "$private" == "true" ]]; then
            visibility=" 🔒"
        fi

        # Truncate description
        if [[ ${#desc} -gt 40 ]]; then
            desc="${desc:0:37}..."
        fi

        printf "  %-40s %-10s ${status_color}%-8s${NC} %s%s\n" "$name" "${lang:--}" "$status" "$desc" "$visibility"
    done

    echo ""
    local total
    total=$(echo "$repos" | jq 'length')
    local cloned
    cloned=$(echo "$local_repos" | jq 'length')
    echo -e "${DIM}Total: ${total} repos (${cloned} cloned locally)${NC}"
    echo ""
}

cmd_clone() {
    local repo_name="${1:-}"

    if [[ -z "$repo_name" ]]; then
        die "Usage: $SCRIPT_NAME clone <owner/repo>"
    fi

    # Handle short names (without owner) - assume current user
    if [[ ! "$repo_name" =~ / ]]; then
        local current_user
        current_user=$(gh api user --jq '.login')
        repo_name="${current_user}/${repo_name}"
    fi

    check_dependencies
    ensure_repos_dir

    local repo_path
    repo_path=$(get_repo_path "$repo_name")
    local owner="${repo_name%%/*}"

    # Ensure owner directory exists
    mkdir -p "${REPOS_DIR}/${owner}"

    if [[ -d "${repo_path}/.git" ]]; then
        log_info "Repository already cloned, pulling latest..."
        git -C "$repo_path" pull --ff-only
        log_success "Updated: $repo_path"
    else
        log_info "Cloning ${repo_name}..."
        gh repo clone "$repo_name" "$repo_path"
        log_success "Cloned: $repo_path"
    fi

    echo ""
    echo -e "  ${CYAN}cd ${repo_path}${NC}"
}

cmd_clone_all() {
    log_warn "This will clone ALL your repositories. This may take a while and use significant disk space."
    echo ""
    read -p "Are you sure? (y/N) " -n 1 -r
    echo ""

    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        log_info "Aborted."
        return
    fi

    check_dependencies
    ensure_repos_dir

    local repos
    repos=$(fetch_repos true)

    echo "$repos" | jq -r '.[].nameWithOwner' | while read -r repo_name; do
        echo ""
        log_info "Processing: $repo_name"
        cmd_clone "$repo_name" || true
    done

    echo ""
    log_success "All repositories processed."
}

cmd_sync() {
    local repo_name="${1:-}"

    if [[ -z "$repo_name" ]]; then
        die "Usage: $SCRIPT_NAME sync <owner/repo>"
    fi

    # Handle short names
    if [[ ! "$repo_name" =~ / ]]; then
        local current_user
        current_user=$(gh api user --jq '.login')
        repo_name="${current_user}/${repo_name}"
    fi

    local repo_path
    repo_path=$(get_repo_path "$repo_name")

    if [[ ! -d "${repo_path}/.git" ]]; then
        die "Repository not cloned: $repo_name. Use: $SCRIPT_NAME clone $repo_name"
    fi

    log_info "Syncing ${repo_name}..."

    # Fetch all remotes
    git -C "$repo_path" fetch --all --prune

    # Pull current branch
    local current_branch
    current_branch=$(git -C "$repo_path" branch --show-current)

    if [[ -n "$current_branch" ]]; then
        git -C "$repo_path" pull --ff-only origin "$current_branch" 2>/dev/null || \
            log_warn "Could not fast-forward. You may have local changes."
    fi

    log_success "Synced: $repo_path"
}

cmd_sync_all() {
    check_dependencies

    local local_repos
    local_repos=$(get_local_repos)
    local count
    count=$(echo "$local_repos" | jq 'length')

    if [[ "$count" -eq 0 ]]; then
        log_info "No local repositories to sync."
        return
    fi

    log_info "Syncing ${count} repositories..."
    echo ""

    echo "$local_repos" | jq -r '.[].name' | while read -r repo_name; do
        cmd_sync "$repo_name" || true
    done

    echo ""
    log_success "All repositories synced."
}

cmd_search() {
    local query="${1:-}"

    if [[ -z "$query" ]]; then
        die "Usage: $SCRIPT_NAME search <query>"
    fi

    check_dependencies

    local repos
    repos=$(fetch_repos)

    echo ""
    echo -e "${CYAN}=== Search Results: \"${query}\" ===${NC}"
    echo ""

    local results
    results=$(echo "$repos" | jq --arg q "$query" '[.[] | select(.nameWithOwner | ascii_downcase | contains($q | ascii_downcase))]')
    local count
    count=$(echo "$results" | jq 'length')

    if [[ "$count" -eq 0 ]]; then
        echo "  No repositories found matching \"$query\""
        return
    fi

    echo "$results" | jq -r '.[].nameWithOwner' | while read -r name; do
        local status=""
        if is_repo_cloned "$name"; then
            status="${GREEN}[cloned]${NC}"
        fi
        echo -e "  $name $status"
    done

    echo ""
    echo -e "${DIM}Found: ${count} repos${NC}"
}

cmd_open() {
    local repo_name="${1:-}"

    if [[ -z "$repo_name" ]]; then
        die "Usage: $SCRIPT_NAME open <owner/repo>"
    fi

    # Handle short names
    if [[ ! "$repo_name" =~ / ]]; then
        local current_user
        current_user=$(gh api user --jq '.login')
        repo_name="${current_user}/${repo_name}"
    fi

    local repo_path
    repo_path=$(get_repo_path "$repo_name")

    # Clone if not exists
    if [[ ! -d "${repo_path}/.git" ]]; then
        cmd_clone "$repo_name"
    else
        # Pull latest
        log_info "Pulling latest changes..."
        git -C "$repo_path" pull --ff-only 2>/dev/null || true
    fi

    echo ""
    echo -e "${GREEN}Ready to work:${NC}"
    echo -e "  ${CYAN}cd ${repo_path}${NC}"
}

#------------------------------------------------------------------------------
# Help
#------------------------------------------------------------------------------

show_help() {
    cat << EOF
${SCRIPT_NAME} v${VERSION} - GitHub Repository Manager

USAGE:
    ${SCRIPT_NAME} list [--local] [--refresh] [--json]  List repositories
    ${SCRIPT_NAME} clone <owner/repo>                   Clone repo (or pull if exists)
    ${SCRIPT_NAME} clone-all                            Clone all repos (caution!)
    ${SCRIPT_NAME} sync <owner/repo>                    Pull latest for a repo
    ${SCRIPT_NAME} sync-all                             Pull latest for all local repos
    ${SCRIPT_NAME} search <query>                       Search repos by name
    ${SCRIPT_NAME} open <owner/repo>                    Clone + cd ready (for integration)

OPTIONS:
    --local, -l     Show only locally cloned repos
    --refresh, -r   Force refresh from GitHub API
    --json, -j      Output in JSON format (for programmatic use)

ENVIRONMENT:
    REPOS_DIR       Base directory for repos (default: ~/repos)

EXAMPLES:
    # List all your repos
    ${SCRIPT_NAME} list

    # Output as JSON (for spoq-tui integration)
    ${SCRIPT_NAME} list --json

    # See what's cloned locally
    ${SCRIPT_NAME} list --local

    # Clone a repo
    ${SCRIPT_NAME} clone myorg/myrepo

    # Update all local repos
    ${SCRIPT_NAME} sync-all

    # Search for a repo
    ${SCRIPT_NAME} search api

EOF
}

#------------------------------------------------------------------------------
# Main
#------------------------------------------------------------------------------

main() {
    local command="${1:-help}"
    shift || true

    case "$command" in
        list|ls)        cmd_list "$@" ;;
        clone|cl)       cmd_clone "$@" ;;
        clone-all)      cmd_clone_all "$@" ;;
        sync|pull)      cmd_sync "$@" ;;
        sync-all)       cmd_sync_all "$@" ;;
        search|find)    cmd_search "$@" ;;
        open|work)      cmd_open "$@" ;;
        help|--help|-h) show_help ;;
        *)              die "Unknown command: ${command}. Use '${SCRIPT_NAME} help' for usage." ;;
    esac
}

main "$@"

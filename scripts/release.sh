#!/bin/bash
#
# Spoq CLI Release Script
#
# Automates the entire release process:
# 1. Bump version in Cargo.toml
# 2. Build all platforms
# 3. Upload to Railway
# 4. Commit and push
#
# Usage: ./scripts/release.sh <version>
# Example: ./scripts/release.sh 0.2.0

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

log_info() { echo -e "${CYAN}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[OK]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1" >&2; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_step() { echo -e "\n${CYAN}${BOLD}==> $1${NC}"; }

# Check arguments
if [ $# -ne 1 ]; then
    log_error "Usage: $0 <version>"
    log_error "Example: $0 0.2.0"
    exit 1
fi

NEW_VERSION="$1"

# Validate version format (semantic versioning)
if ! [[ "$NEW_VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    log_error "Invalid version format. Expected: X.Y.Z (e.g., 0.2.0)"
    exit 1
fi

# Railway deploy key
DEPLOY_KEY="96fe8f6b83d23f716669d24c2757b38e77c445547f30fcd5dee511aa1ff613f8"
DEPLOY_URL="https://download.spoq.dev/cli/release"
VERSION_URL="https://download.spoq.dev/cli/version"

log_step "Starting release process for v$NEW_VERSION"

# Check we're in the right directory
if [ ! -f "Cargo.toml" ]; then
    log_error "Cargo.toml not found. Run this script from the project root."
    exit 1
fi

# Check git status
if [ -n "$(git status --porcelain)" ]; then
    log_warn "Working directory has uncommitted changes"
    read -p "Continue anyway? (y/N) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        exit 1
    fi
fi

# Check required tools
log_step "Checking prerequisites"
for cmd in cargo rustup jq curl; do
    if ! command -v $cmd &> /dev/null; then
        log_error "$cmd is not installed"
        exit 1
    fi
done

# Check cargo-zigbuild
if ! command -v cargo-zigbuild &> /dev/null; then
    log_warn "cargo-zigbuild not found. Install it? (recommended for cross-compilation)"
    read -p "Install cargo-zigbuild? (y/N) " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        cargo install cargo-zigbuild
    else
        log_error "cargo-zigbuild is required for Linux builds"
        exit 1
    fi
fi

# Check zig
if ! command -v zig &> /dev/null; then
    log_warn "zig not found. Install it? (required for cross-compilation)"
    log_info "Run: brew install zig (macOS) or visit https://ziglang.org/download/"
    exit 1
fi

log_success "All prerequisites found"

# Bump version
log_step "Bumping version to $NEW_VERSION"
CURRENT_VERSION=$(grep '^version = ' Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')
log_info "Current version: $CURRENT_VERSION"
log_info "New version: $NEW_VERSION"

if [ "$CURRENT_VERSION" == "$NEW_VERSION" ]; then
    log_error "Version is already $NEW_VERSION"
    exit 1
fi

# Update Cargo.toml
sed -i '' 's/^version = ".*"/version = "'$NEW_VERSION'"/' Cargo.toml
log_success "Updated Cargo.toml"

# Build all platforms
log_step "Building all platforms"

log_info "Building darwin-x86_64 (native)..."
cargo build --release --quiet
log_success "Built darwin-x86_64"

log_info "Building darwin-aarch64..."
cargo build --release --target aarch64-apple-darwin --quiet
log_success "Built darwin-aarch64"

log_info "Building linux-x86_64 (with zig)..."
cargo zigbuild --release --target x86_64-unknown-linux-gnu --quiet
log_success "Built linux-x86_64"

log_info "Building linux-aarch64 (with zig)..."
cargo zigbuild --release --target aarch64-unknown-linux-gnu --quiet
log_success "Built linux-aarch64"

# Verify all binaries exist
log_step "Verifying binaries"
BINARIES=(
    "target/release/spoq:darwin-x86_64"
    "target/aarch64-apple-darwin/release/spoq:darwin-aarch64"
    "target/x86_64-unknown-linux-gnu/release/spoq:linux-x86_64"
    "target/aarch64-unknown-linux-gnu/release/spoq:linux-aarch64"
)

for entry in "${BINARIES[@]}"; do
    IFS=':' read -r path platform <<< "$entry"
    if [ ! -f "$path" ]; then
        log_error "Binary not found: $path"
        exit 1
    fi
    size=$(du -h "$path" | cut -f1)
    log_info "$platform: $size"
done

log_success "All binaries built successfully"

# Upload to Railway
log_step "Uploading to Railway"

for entry in "${BINARIES[@]}"; do
    IFS=':' read -r path platform <<< "$entry"

    log_info "Uploading $platform..."

    response=$(curl -s -X POST "$DEPLOY_URL" \
        -H "Authorization: Bearer $DEPLOY_KEY" \
        -F "version=$NEW_VERSION" \
        -F "platform=$platform" \
        -F "binary=@$path")

    if echo "$response" | jq -e '.success' > /dev/null 2>&1; then
        checksum=$(echo "$response" | jq -r '.checksum')
        log_success "Uploaded $platform (checksum: ${checksum:0:16}...)"
    else
        log_error "Failed to upload $platform"
        echo "$response" | jq '.'
        exit 1
    fi
done

log_success "All platforms uploaded"

# Verify deployment
log_step "Verifying deployment"
version_info=$(curl -s "$VERSION_URL")
deployed_version=$(echo "$version_info" | jq -r '.version')
platforms=$(echo "$version_info" | jq -r '.platforms | join(", ")')

if [ "$deployed_version" == "$NEW_VERSION" ]; then
    log_success "Version endpoint shows: v$deployed_version"
    log_info "Platforms: $platforms"
else
    log_error "Version mismatch! Expected: $NEW_VERSION, Got: $deployed_version"
    exit 1
fi

# Commit and push
log_step "Committing changes"

git add Cargo.toml Cargo.lock
git commit -m "Release v$NEW_VERSION"
git tag "v$NEW_VERSION"

log_success "Created commit and tag v$NEW_VERSION"

read -p "Push to remote? (y/N) " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    git push origin main --tags
    log_success "Pushed to remote"
else
    log_warn "Skipped push. Run manually: git push origin main --tags"
fi

# Summary
log_step "Release Complete!"
echo ""
log_success "Version: v$NEW_VERSION"
log_success "Platforms: darwin-x86_64, darwin-aarch64, linux-x86_64, linux-aarch64"
log_success "Download URL: https://download.spoq.dev/cli/download/{platform}"
echo ""
log_info "Test the release:"
echo "  curl -fsSL https://download.spoq.dev/cli/download/darwin-x86_64 -o /tmp/spoq"
echo "  chmod +x /tmp/spoq"
echo "  /tmp/spoq --version"
echo ""
log_info "Users can update with:"
echo "  spoq --update"

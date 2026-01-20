#!/bin/sh
# Install script for spoq - POSIX-compliant
# Usage: curl -fsSL https://download.spoq.dev/cli | sh

set -e

DOWNLOAD_URL="https://download.spoq.dev"
BINARY_NAME="spoq"

# Colors (if terminal supports them)
if [ -t 1 ]; then
    RED='\033[0;31m'
    GREEN='\033[0;32m'
    YELLOW='\033[0;33m'
    BLUE='\033[0;34m'
    CYAN='\033[0;36m'
    BOLD='\033[1m'
    NC='\033[0m'
else
    RED=''
    GREEN=''
    YELLOW=''
    BLUE=''
    CYAN=''
    BOLD=''
    NC=''
fi

info() {
    printf "%b==>%b %s\n" "$BLUE" "$NC" "$1"
}

success() {
    printf "%b==>%b %s\n" "$GREEN" "$NC" "$1"
}

warn() {
    printf "%bWarning:%b %s\n" "$YELLOW" "$NC" "$1"
}

error() {
    printf "%bError:%b %s\n" "$RED" "$NC" "$1" >&2
    exit 1
}

# Detect OS
detect_os() {
    os=$(uname -s)
    case "$os" in
        Linux)
            echo "linux"
            ;;
        Darwin)
            echo "darwin"
            ;;
        *)
            error "Unsupported operating system: $os"
            ;;
    esac
}

# Detect architecture
detect_arch() {
    arch=$(uname -m)
    case "$arch" in
        x86_64|amd64)
            echo "x86_64"
            ;;
        aarch64|arm64)
            echo "aarch64"
            ;;
        *)
            error "Unsupported architecture: $arch"
            ;;
    esac
}

# Check for curl or wget
get_downloader() {
    if command -v curl >/dev/null 2>&1; then
        echo "curl"
    elif command -v wget >/dev/null 2>&1; then
        echo "wget"
    else
        error "Neither curl nor wget found. Please install one of them."
    fi
}

# Download file
download() {
    url="$1"
    output="$2"
    downloader=$(get_downloader)

    if [ "$downloader" = "curl" ]; then
        curl -fsSL "$url" -o "$output"
    else
        wget -q "$url" -O "$output"
    fi
}

# Main installation
main() {
    info "Installing $BINARY_NAME..."

    # Detect platform
    os=$(detect_os)
    arch=$(detect_arch)
    platform="${os}-${arch}"
    info "Detected platform: $platform"

    # Determine install directory
    if [ -n "$INSTALL_DIR" ]; then
        install_dir="$INSTALL_DIR"
    elif [ -w "/usr/local/bin" ]; then
        install_dir="/usr/local/bin"
    else
        install_dir="$HOME/.local/bin"
    fi

    info "Install directory: $install_dir"

    # Create install directory if needed
    if [ ! -d "$install_dir" ]; then
        info "Creating directory: $install_dir"
        mkdir -p "$install_dir"
    fi

    # Construct download URL
    url="${DOWNLOAD_URL}/cli/download/${platform}"

    info "Downloading from: $url"

    # Create temp directory
    tmp_dir=$(mktemp -d)
    trap 'rm -rf "$tmp_dir"' EXIT

    # Download binary directly (raw binary, no tarball)
    download "$url" "$tmp_dir/$BINARY_NAME" || error "Failed to download binary"

    # Make executable and install
    chmod +x "$tmp_dir/$BINARY_NAME"
    mv "$tmp_dir/$BINARY_NAME" "$install_dir/$BINARY_NAME"

    # Verify installation
    if "$install_dir/$BINARY_NAME" --version >/dev/null 2>&1; then
        version=$("$install_dir/$BINARY_NAME" --version 2>&1 | head -n1)
        success "Successfully installed: $version"
    else
        warn "Binary installed but verification failed"
    fi

    # Check if install_dir is in PATH
    case ":$PATH:" in
        *":$install_dir:"*)
            success "Installation complete! Run '$BINARY_NAME' to get started."
            ;;
        *)
            echo ""
            warn "$install_dir is not in your PATH"
            echo ""
            echo "Add it to your PATH by adding this line to your shell config:"
            echo ""
            echo "  export PATH=\"\$PATH:$install_dir\""
            echo ""
            echo "Then restart your shell or run:"
            echo ""
            echo "  source ~/.bashrc  # or ~/.zshrc"
            echo ""
            ;;
    esac
}

main

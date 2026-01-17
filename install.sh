#!/bin/sh
# Install script for spoq - POSIX-compliant
# Usage: curl -fsSL https://raw.githubusercontent.com/AWLSEN/spoq-tui/main/install.sh | sh

set -e

REPO="AWLSEN/spoq-tui"
BINARY_NAME="spoq"

# Colors (if terminal supports them)
if [ -t 1 ]; then
    RED='\033[0;31m'
    GREEN='\033[0;32m'
    YELLOW='\033[0;33m'
    BLUE='\033[0;34m'
    NC='\033[0m' # No Color
else
    RED=''
    GREEN=''
    YELLOW=''
    BLUE=''
    NC=''
fi

info() {
    printf "${BLUE}==>${NC} %s\n" "$1"
}

success() {
    printf "${GREEN}==>${NC} %s\n" "$1"
}

warn() {
    printf "${YELLOW}Warning:${NC} %s\n" "$1"
}

error() {
    printf "${RED}Error:${NC} %s\n" "$1" >&2
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
            echo "x64"
            ;;
        aarch64|arm64)
            echo "arm64"
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
    info "Detected platform: $os-$arch"

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
    tarball="${BINARY_NAME}-${os}-${arch}.tar.gz"
    url="https://github.com/${REPO}/releases/latest/download/${tarball}"

    info "Downloading from: $url"

    # Create temp directory
    tmp_dir=$(mktemp -d)
    trap 'rm -rf "$tmp_dir"' EXIT

    # Download tarball
    download "$url" "$tmp_dir/$tarball" || error "Failed to download $tarball"

    # Extract binary
    info "Extracting..."
    tar -xzf "$tmp_dir/$tarball" -C "$tmp_dir" || error "Failed to extract $tarball"

    # Install binary
    if [ -f "$tmp_dir/$BINARY_NAME" ]; then
        mv "$tmp_dir/$BINARY_NAME" "$install_dir/$BINARY_NAME"
        chmod +x "$install_dir/$BINARY_NAME"
    else
        error "Binary not found in archive"
    fi

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

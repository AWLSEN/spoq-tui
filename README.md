# spoq-tui

A TUI client for spoq.

## Installation

### Shell script (recommended)

Quick installation via curl:

```bash
curl -fsSL https://download.spoq.dev/cli | sh
```

This script automatically detects your platform and architecture, downloads the appropriate binary, and installs it to `~/.local/bin/spoq`.

### Cargo (Rust users)

If you have Rust installed:

```bash
cargo install spoq
```

### Manual download

Download pre-built binaries directly:

```bash
# macOS Apple Silicon
curl -fsSL https://download.spoq.dev/cli/download/darwin-aarch64 -o spoq

# macOS Intel
curl -fsSL https://download.spoq.dev/cli/download/darwin-x86_64 -o spoq

# Linux x86_64
curl -fsSL https://download.spoq.dev/cli/download/linux-x86_64 -o spoq

# Linux ARM64
curl -fsSL https://download.spoq.dev/cli/download/linux-aarch64 -o spoq
```

After downloading, make it executable and move it to your PATH:

```bash
chmod +x spoq
sudo mv spoq /usr/local/bin/
```

## Building from Source

Prerequisites:
- Rust 1.70 or later
- Cargo

Clone the repository and build:

```bash
git clone https://github.com/AWLSEN/spoq-tui.git
cd spoq-tui
cargo build --release
```

The compiled binary will be available at `target/release/spoq`.

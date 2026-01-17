# spoq-tui

A TUI client for spoq.

## Installation

### npm (recommended for Node.js users)

```bash
npm install -g @oaftobark/spoq
```

After installation, run `spoq` from your terminal.

### Shell script

Quick installation via curl:

```bash
curl -sSL https://raw.githubusercontent.com/AWLSEN/spoq-tui/main/install.sh | sh
```

This script automatically detects your platform and architecture, downloads the appropriate binary, and installs it to `~/.local/bin/spoq`.

### Cargo (Rust users)

If you have Rust installed:

```bash
cargo install spoq
```

### Manual download

Download pre-built binaries from the [GitHub Releases](https://github.com/AWLSEN/spoq-tui/releases) page.

Available platforms:
- macOS (Intel and Apple Silicon)
- Linux (x86_64, ARM64, ARMv7)
- Windows (x86_64)

After downloading, extract the binary and place it in your PATH.

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

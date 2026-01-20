# spoq-tui

A TUI client for spoq.

## Installation

Install via the official installer script:

```bash
curl -fsSL https://download.spoq.dev/cli | sh
```

This script automatically detects your platform and architecture, downloads the appropriate binary, and installs it to `~/.local/bin/spoq`.

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

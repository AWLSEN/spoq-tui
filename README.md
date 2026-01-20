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

## Development

### Auto-Update System

The CLI includes an automatic update system:
- Checks for updates on every launch (max once per 24 hours)
- Downloads updates silently in the background
- Installs on next restart
- Manual update: `spoq --update`

### Making a Release

**Quick release (automated):**
```bash
./scripts/release.sh 0.2.0
```

This script handles:
- Version bump
- Cross-platform builds (macOS Intel/ARM, Linux x64/ARM64)
- Upload to Railway
- Git commit and tag

**Manual release:**
See [docs/RELEASE.md](docs/RELEASE.md) for detailed instructions.

**Quick reference:**
See [.claude/release-quick-ref.md](.claude/release-quick-ref.md) for one-shot commands.

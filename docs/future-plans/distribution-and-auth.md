# Distribution & Authentication Architecture

> Status: Planning
> Created: 2025-01-17

## Overview

Transform spoq-tui from a locally-built development tool into a distributable, authenticated CLI application similar to Claude Code.

## Goals

1. One-line installation: `curl -fsSL https://spoq.dev/install.sh | bash`
2. Secure authentication with own OAuth provider
3. Protected binary distribution
4. Token-based API authorization

---

## Architecture

### 1. Authentication Flow

```
spoq                           Browser                    Conductor
  │                               │                           │
  │  1. Check ~/.spoq/credentials.json                        │
  │                               │                           │
  │  [No credentials or expired]  │                           │
  │                               │                           │
  │  2. Start local server :9876  │                           │
  │                               │                           │
  │  3. Open browser ────────────→│                           │
  │     https://conductor/oauth/authorize?                    │
  │       redirect_uri=http://localhost:9876/callback         │
  │       client_id=spoq-cli                                  │
  │                               │                           │
  │                               │  4. User authenticates    │
  │                               │─────────────────────────→ │
  │                               │                           │
  │                               │  5. Redirect with code    │
  │                               │←─────────────────────────│
  │                               │                           │
  │  6. Receive code ←────────────│                           │
  │     localhost:9876/callback?code=xxx                      │
  │                               │                           │
  │  7. Exchange code for tokens                              │
  │     POST /oauth/token { code, client_id }                 │
  │─────────────────────────────────────────────────────────→ │
  │                               │                           │
  │  8. Receive tokens                                        │
  │     { access_token, refresh_token, expires_in }           │
  │←─────────────────────────────────────────────────────────│
  │                               │                           │
  │  9. Save to ~/.spoq/credentials.json (mode 600)           │
  │                               │                           │
  │  10. All API calls: Authorization: Bearer <token>         │
  │─────────────────────────────────────────────────────────→ │
```

### 2. Token Refresh Flow

```
spoq                                              Conductor
  │                                                   │
  │  Token expired (401 or local expiry check)        │
  │                                                   │
  │  POST /oauth/refresh                              │
  │  { refresh_token }                                │
  │─────────────────────────────────────────────────→ │
  │                                                   │
  │  { access_token, refresh_token, expires_in }      │
  │←─────────────────────────────────────────────────│
  │                                                   │
  │  Update credentials.json                          │
  │  Retry original request                           │
```

### 3. Credential Storage

```
~/.spoq/
├── credentials.json    # mode 600 (owner read/write only)
├── config.json         # mode 644 (optional user settings)
└── bin/
    └── spoq            # installed binary
```

**credentials.json structure:**
```json
{
  "access_token": "eyJhbG...",
  "refresh_token": "dGhpcy...",
  "expires_at": 1705520400,
  "user_id": "user_xxx",
  "email": "user@example.com"
}
```

---

## Binary Security

### Compilation & Distribution

| Layer | Implementation |
|-------|----------------|
| Compiled binary | Rust → native machine code (no source shipped) |
| Symbol stripping | `strip target/release/spoq` removes debug info |
| Release optimizations | LTO, codegen-units=1 for smaller/faster binary |
| Code signing | Apple notarization (macOS), Authenticode (Windows) |
| Checksum verification | SHA256 published with each release |

### Cargo.toml Release Profile

```toml
[profile.release]
opt-level = 3
lto = true
codegen-units = 1
panic = "abort"
strip = true
```

### Transport Security

- HTTPS only (reject HTTP)
- TLS 1.2+ minimum
- Optional: Certificate pinning for conductor domain

---

## Installation System

### Install Script (install.sh)

```bash
#!/bin/bash
set -euo pipefail

VERSION="${SPOQ_VERSION:-latest}"
BASE_URL="https://releases.spoq.dev"

# Detect platform
OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
ARCH="$(uname -m)"

case "$ARCH" in
    x86_64)  ARCH="x86_64" ;;
    aarch64|arm64) ARCH="aarch64" ;;
    *) echo "Unsupported architecture: $ARCH"; exit 1 ;;
esac

case "$OS" in
    darwin) PLATFORM="apple-darwin" ;;
    linux)  PLATFORM="unknown-linux-gnu" ;;
    *) echo "Unsupported OS: $OS"; exit 1 ;;
esac

TARGET="${ARCH}-${PLATFORM}"
DOWNLOAD_URL="${BASE_URL}/${VERSION}/spoq-${TARGET}.tar.gz"
CHECKSUM_URL="${BASE_URL}/${VERSION}/spoq-${TARGET}.tar.gz.sha256"

INSTALL_DIR="${HOME}/.spoq/bin"
mkdir -p "$INSTALL_DIR"

echo "Downloading spoq for ${TARGET}..."
curl -fsSL "$DOWNLOAD_URL" -o /tmp/spoq.tar.gz
curl -fsSL "$CHECKSUM_URL" -o /tmp/spoq.sha256

# Verify checksum
echo "Verifying checksum..."
cd /tmp && sha256sum -c spoq.sha256

# Extract and install
tar -xzf /tmp/spoq.tar.gz -C "$INSTALL_DIR"
chmod +x "$INSTALL_DIR/spoq"

# Cleanup
rm -f /tmp/spoq.tar.gz /tmp/spoq.sha256

# Add to PATH
add_to_path() {
    local rc_file="$1"
    local path_line='export PATH="$HOME/.spoq/bin:$PATH"'
    if [ -f "$rc_file" ] && ! grep -q ".spoq/bin" "$rc_file"; then
        echo "$path_line" >> "$rc_file"
        echo "Added to $rc_file"
    fi
}

add_to_path "$HOME/.bashrc"
add_to_path "$HOME/.zshrc"

echo ""
echo "spoq installed successfully!"
echo "Run 'spoq' to get started (restart shell or run: source ~/.zshrc)"
```

### GitHub Actions Release Workflow

```yaml
name: Release

on:
  push:
    tags: ['v*']

env:
  CARGO_TERM_COLOR: always

jobs:
  build:
    strategy:
      matrix:
        include:
          - os: macos-latest
            target: x86_64-apple-darwin
          - os: macos-latest
            target: aarch64-apple-darwin
          - os: ubuntu-latest
            target: x86_64-unknown-linux-gnu
          - os: ubuntu-latest
            target: aarch64-unknown-linux-gnu

    runs-on: ${{ matrix.os }}

    steps:
      - uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}

      - name: Install cross-compilation tools
        if: matrix.target == 'aarch64-unknown-linux-gnu'
        run: |
          sudo apt-get update
          sudo apt-get install -y gcc-aarch64-linux-gnu

      - name: Build release binary
        run: cargo build --release --target ${{ matrix.target }}

      - name: Strip binary
        run: |
          if [[ "${{ matrix.os }}" == "macos-latest" ]]; then
            strip target/${{ matrix.target }}/release/spoq
          else
            strip target/${{ matrix.target }}/release/spoq || true
          fi

      - name: Package binary
        run: |
          cd target/${{ matrix.target }}/release
          tar -czvf spoq-${{ matrix.target }}.tar.gz spoq
          shasum -a 256 spoq-${{ matrix.target }}.tar.gz > spoq-${{ matrix.target }}.tar.gz.sha256

      - name: Upload artifacts
        uses: actions/upload-artifact@v4
        with:
          name: spoq-${{ matrix.target }}
          path: |
            target/${{ matrix.target }}/release/spoq-${{ matrix.target }}.tar.gz
            target/${{ matrix.target }}/release/spoq-${{ matrix.target }}.tar.gz.sha256

  release:
    needs: build
    runs-on: ubuntu-latest
    steps:
      - uses: actions/download-artifact@v4

      - name: Create GitHub Release
        uses: softprops/action-gh-release@v1
        with:
          files: |
            spoq-*/spoq-*.tar.gz
            spoq-*/spoq-*.tar.gz.sha256
          generate_release_notes: true
```

---

## New File Structure

```
src/
├── auth.rs              # NEW: OAuth flow, token management
├── config.rs            # NEW: Paths, settings, API URL
├── credentials.rs       # NEW: Read/write credentials.json
├── conductor.rs         # MODIFY: Add auth headers, handle 401
├── main.rs              # MODIFY: Auth check before app start
└── ...

scripts/
└── install.sh           # NEW: Distribution install script

.github/workflows/
└── release.yml          # NEW: Build & publish workflow

Cargo.toml               # MODIFY: Add release profile, metadata
```

---

## Implementation Phases

### Phase 1: Credential Management
- [ ] Create `src/config.rs` - paths, directories
- [ ] Create `src/credentials.rs` - read/write credentials.json
- [ ] Set file permissions (600) on credential file
- [ ] Add `dirs` crate for cross-platform home directory

### Phase 2: OAuth Flow
- [ ] Create `src/auth.rs` - OAuth orchestration
- [ ] Implement local callback server (port 9876)
- [ ] Browser opening with `open` crate (already have)
- [ ] Token exchange with conductor
- [ ] Token refresh logic

### Phase 3: Conductor Integration
- [ ] Modify `ConductorClient` to require auth token
- [ ] Add `Authorization: Bearer` header to all requests
- [ ] Handle 401 responses → trigger refresh or re-auth
- [ ] Update base URL to HTTPS production endpoint

### Phase 4: Startup Flow
- [ ] Modify `main.rs` to check credentials on startup
- [ ] Show auth status in UI
- [ ] Handle auth errors gracefully
- [ ] Add `spoq logout` command

### Phase 5: Distribution
- [ ] Create `scripts/install.sh`
- [ ] Add release profile to `Cargo.toml`
- [ ] Create `.github/workflows/release.yml`
- [ ] Setup releases CDN/hosting
- [ ] macOS code signing (optional)

### Phase 6: Backend (Conductor)
- [ ] Implement `/oauth/authorize` endpoint
- [ ] Implement `/oauth/token` endpoint
- [ ] Implement `/oauth/refresh` endpoint
- [ ] Add token validation middleware
- [ ] User management/storage

---

## Conductor API Endpoints (Required)

| Method | Endpoint | Purpose |
|--------|----------|---------|
| GET | `/oauth/authorize` | Initiate OAuth, show login page |
| POST | `/oauth/token` | Exchange code for tokens |
| POST | `/oauth/refresh` | Refresh expired access token |
| GET | `/v1/auth/me` | Validate token, get user info |
| POST | `/v1/auth/logout` | Revoke tokens |

---

## Security Checklist

- [ ] Credentials file has 600 permissions
- [ ] HTTPS only (no HTTP fallback)
- [ ] Access tokens short-lived (15-60 min)
- [ ] Refresh tokens rotated on use
- [ ] Binary stripped of debug symbols
- [ ] Release builds use LTO
- [ ] Checksums published with releases
- [ ] macOS binary notarized (optional)

---

## Dependencies to Add

```toml
[dependencies]
dirs = "5"                    # Cross-platform directories
axum = "0.7"                  # Already have - for callback server

[profile.release]
opt-level = 3
lto = true
codegen-units = 1
panic = "abort"
strip = true
```

---

## Decisions

| Item | Decision |
|------|----------|
| Conductor hosting | Railway |
| Rate limiting | None |
| Refresh tokens | Yes |

## Open Questions

1. **Domain**: Custom domain for Railway? (api.spoq.dev → Railway)
2. **User registration**: Self-service signup or invite-only?
3. **Token lifetimes**: Access token (15min? 1hr?), Refresh token (7d? 30d?)
4. **Multiple devices**: Same account on multiple machines?

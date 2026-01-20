# Release Guide for Spoq CLI

This guide explains how to create and deploy a new release of the Spoq CLI to Railway.

## Prerequisites

- Rust toolchain (1.70+)
- `cargo-zigbuild` for cross-compilation: `cargo install cargo-zigbuild`
- Zig compiler: `brew install zig` (macOS) or [zig.cc](https://ziglang.org/download/)
- Railway deploy key (stored in 1Password or secure location)

## Quick Release (Automated)

Use the automated release script:

```bash
./scripts/release.sh 0.2.0
```

This will:
1. Bump version in `Cargo.toml`
2. Build all platforms (darwin-x86_64, darwin-aarch64, linux-x86_64, linux-aarch64)
3. Upload all binaries to Railway
4. Commit and push changes

## Manual Release Process

### Step 1: Bump Version

Edit `Cargo.toml`:
```toml
[package]
version = "0.2.0"  # Change this
```

### Step 2: Build All Platforms

```bash
# macOS Intel (native)
cargo build --release

# macOS Apple Silicon
cargo build --release --target aarch64-apple-darwin

# Linux x86_64 (with zig)
cargo zigbuild --release --target x86_64-unknown-linux-gnu

# Linux ARM64 (with zig)
cargo zigbuild --release --target aarch64-unknown-linux-gnu
```

**Binaries will be at:**
- `target/release/spoq` (darwin-x86_64)
- `target/aarch64-apple-darwin/release/spoq` (darwin-aarch64)
- `target/x86_64-unknown-linux-gnu/release/spoq` (linux-x86_64)
- `target/aarch64-unknown-linux-gnu/release/spoq` (linux-aarch64)

### Step 3: Upload to Railway

**Railway API Endpoint:**
```
POST https://download.spoq.dev/cli/release
```

**Deploy Key (Authorization header):**
```
96fe8f6b83d23f716669d24c2757b38e77c445547f30fcd5dee511aa1ff613f8
```

**Upload each platform:**

```bash
VERSION="0.2.0"
DEPLOY_KEY="96fe8f6b83d23f716669d24c2757b38e77c445547f30fcd5dee511aa1ff613f8"

# darwin-x86_64
curl -X POST https://download.spoq.dev/cli/release \
  -H "Authorization: Bearer $DEPLOY_KEY" \
  -F "version=$VERSION" \
  -F "platform=darwin-x86_64" \
  -F "binary=@target/release/spoq"

# darwin-aarch64
curl -X POST https://download.spoq.dev/cli/release \
  -H "Authorization: Bearer $DEPLOY_KEY" \
  -F "version=$VERSION" \
  -F "platform=darwin-aarch64" \
  -F "binary=@target/aarch64-apple-darwin/release/spoq"

# linux-x86_64
curl -X POST https://download.spoq.dev/cli/release \
  -H "Authorization: Bearer $DEPLOY_KEY" \
  -F "version=$VERSION" \
  -F "platform=linux-x86_64" \
  -F "binary=@target/x86_64-unknown-linux-gnu/release/spoq"

# linux-aarch64
curl -X POST https://download.spoq.dev/cli/release \
  -H "Authorization: Bearer $DEPLOY_KEY" \
  -F "version=$VERSION" \
  -F "platform=linux-aarch64" \
  -F "binary=@target/aarch64-unknown-linux-gnu/release/spoq"
```

### Step 4: Verify Deployment

```bash
curl -s https://download.spoq.dev/cli/version | jq
```

Should show:
```json
{
  "version": "0.2.0",
  "platforms": [
    "linux-x86_64",
    "darwin-aarch64",
    "darwin-x86_64",
    "linux-aarch64"
  ]
}
```

### Step 5: Commit and Push

```bash
git add Cargo.toml Cargo.lock
git commit -m "Release v$VERSION"
git tag "v$VERSION"
git push origin main --tags
```

## Platform Details

### Supported Platforms

| Platform | Target Triple | Binary Size (approx) |
|----------|--------------|---------------------|
| macOS Intel | darwin-x86_64 | 9.0 MB |
| macOS Apple Silicon | darwin-aarch64 | 8.4 MB |
| Linux x86_64 | linux-x86_64 | 10.0 MB |
| Linux ARM64 | linux-aarch64 | 8.9 MB |

### Platform Detection in Code

The CLI auto-detects platform using:
- macOS: `std::env::consts::OS = "macos"`
- Linux: `std::env::consts::OS = "linux"`
- x86_64: `std::env::consts::ARCH = "x86_64"`
- ARM64: `std::env::consts::ARCH = "aarch64"`

Maps to platform strings:
- `darwin-x86_64` (macOS Intel)
- `darwin-aarch64` (macOS Apple Silicon)
- `linux-x86_64` (Linux x64)
- `linux-aarch64` (Linux ARM64)

## Railway Infrastructure

### Services

- **conductor-version**: Serves binaries and version metadata
  - URL: `https://download.spoq.dev`
  - Volume: `/app/releases` (stores binaries)
  - Deploy key required for uploads

### Endpoints

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/cli/version` | GET | Get latest version info |
| `/cli/download/{platform}` | GET | Download binary for platform |
| `/cli/release` | POST | Upload new binary (requires auth) |

## Auto-Update System

The CLI includes an auto-update system that:
1. Checks for updates on every launch (max once per 24 hours)
2. Downloads updates in the background
3. Installs on next restart
4. Keeps backup of previous version

Users can manually trigger updates:
```bash
spoq --update
```

## Troubleshooting

### Build Errors

**"unable to find framework"** (zig cross-compilation)
- Use native `cargo build` instead of `cargo zigbuild` for macOS targets
- zig has issues with macOS SDK frameworks when cross-compiling to macOS

**"File lock on build directory"**
- Builds are sequential due to cargo's file locking
- Wait for previous build to complete

### Upload Errors

**"Invalid platform"**
- Ensure platform string matches exactly: `darwin-x86_64`, `darwin-aarch64`, `linux-x86_64`, `linux-aarch64`
- NOT: `darwin-x64`, `linux-x64`, etc.

**"Unauthorized"**
- Check deploy key is correct
- Ensure `Authorization: Bearer` header is present

### Version Check Failures

**Users can't update**
- Verify version endpoint returns correct version: `curl https://download.spoq.dev/cli/version`
- Check all platforms are uploaded
- Ensure version in `Cargo.toml` matches uploaded version

## Security Notes

- **Deploy Key**: Keep the Railway deploy key secure. Store in 1Password or equivalent.
- **Binary Verification**: Railway stores SHA256 checksums of all uploads
- **No Code Signing**: Currently not implementing macOS/Windows code signing (can be added later)

## One-Shot Command for Claude

For a new Claude session or quick release, copy this entire command:

```bash
# Set version
VERSION="0.2.0"

# Update Cargo.toml
sed -i '' 's/version = "[^"]*"/version = "'$VERSION'"/' Cargo.toml

# Build all platforms
cargo build --release && \
cargo build --release --target aarch64-apple-darwin && \
cargo zigbuild --release --target x86_64-unknown-linux-gnu && \
cargo zigbuild --release --target aarch64-unknown-linux-gnu

# Upload all platforms
DEPLOY_KEY="96fe8f6b83d23f716669d24c2757b38e77c445547f30fcd5dee511aa1ff613f8"
for platform in "darwin-x86_64:target/release/spoq" \
                "darwin-aarch64:target/aarch64-apple-darwin/release/spoq" \
                "linux-x86_64:target/x86_64-unknown-linux-gnu/release/spoq" \
                "linux-aarch64:target/aarch64-unknown-linux-gnu/release/spoq"; do
  IFS=':' read -r plat path <<< "$platform"
  curl -X POST https://download.spoq.dev/cli/release \
    -H "Authorization: Bearer $DEPLOY_KEY" \
    -F "version=$VERSION" \
    -F "platform=$plat" \
    -F "binary=@$path"
done

# Verify
curl -s https://download.spoq.dev/cli/version | jq

# Commit and push
git add Cargo.toml Cargo.lock && \
git commit -m "Release v$VERSION" && \
git tag "v$VERSION" && \
git push origin main --tags
```

## Testing a Release

After deploying:

1. **Install the new version:**
   ```bash
   curl -fsSL https://download.spoq.dev/cli/download/darwin-x86_64 -o /tmp/spoq
   chmod +x /tmp/spoq
   sudo mv /tmp/spoq /usr/local/bin/spoq
   ```

2. **Verify version:**
   ```bash
   spoq --version  # Should show new version
   ```

3. **Test auto-update (from previous version):**
   - Install previous version
   - Run `spoq --update`
   - Restart and verify new version installed

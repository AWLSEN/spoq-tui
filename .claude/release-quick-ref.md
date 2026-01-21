# Quick Reference: Spoq CLI Release

## One-Command Release

```bash
./scripts/release.sh 0.2.0
```

This handles everything: version bump, builds, uploads, commit, and push.

---

## Manual Release (If Script Fails)

### 1. Update Version
```bash
# Edit Cargo.toml, change version to 0.2.0
sed -i '' 's/version = "[^"]*"/version = "0.2.0"/' Cargo.toml
```

### 2. Build All Platforms
```bash
cargo build --release  # darwin-x86_64
cargo build --release --target aarch64-apple-darwin  # darwin-aarch64
cargo zigbuild --release --target x86_64-unknown-linux-gnu  # linux-x86_64
cargo zigbuild --release --target aarch64-unknown-linux-gnu  # linux-aarch64
```

### 3. Upload to Railway
```bash
DEPLOY_KEY="96fe8f6b83d23f716669d24c2757b38e77c445547f30fcd5dee511aa1ff613f8"
VERSION="0.2.0"

# Upload all platforms
curl -X POST https://download.spoq.dev/cli/release \
  -H "Authorization: Bearer $DEPLOY_KEY" \
  -F "version=$VERSION" \
  -F "platform=darwin-x86_64" \
  -F "binary=@target/release/spoq"

curl -X POST https://download.spoq.dev/cli/release \
  -H "Authorization: Bearer $DEPLOY_KEY" \
  -F "version=$VERSION" \
  -F "platform=darwin-aarch64" \
  -F "binary=@target/aarch64-apple-darwin/release/spoq"

curl -X POST https://download.spoq.dev/cli/release \
  -H "Authorization: Bearer $DEPLOY_KEY" \
  -F "version=$VERSION" \
  -F "platform=linux-x86_64" \
  -F "binary=@target/x86_64-unknown-linux-gnu/release/spoq"

curl -X POST https://download.spoq.dev/cli/release \
  -H "Authorization: Bearer $DEPLOY_KEY" \
  -F "version=$VERSION" \
  -F "platform=linux-aarch64" \
  -F "binary=@target/aarch64-unknown-linux-gnu/release/spoq"
```

### 4. Verify & Commit
```bash
# Check version endpoint
curl -s https://download.spoq.dev/cli/version | jq

# Commit
git add Cargo.toml Cargo.lock
git commit -m "Release v$VERSION"
git tag "v$VERSION"
git push origin main --tags
```

---

## Important Info

**Railway Deploy Key:**
```
96fe8f6b83d23f716669d24c2757b38e77c445547f30fcd5dee511aa1ff613f8
```

**Railway Upload Endpoint:**
```
POST https://download.spoq.dev/cli/release
```

**Platform Strings (must match exactly):**
- `darwin-x86_64` (macOS Intel)
- `darwin-aarch64` (macOS Apple Silicon)
- `linux-x86_64` (Linux x64)
- `linux-aarch64` (Linux ARM64)

**Binary Locations:**
- `target/release/spoq` → darwin-x86_64
- `target/aarch64-apple-darwin/release/spoq` → darwin-aarch64
- `target/x86_64-unknown-linux-gnu/release/spoq` → linux-x86_64
- `target/aarch64-unknown-linux-gnu/release/spoq` → linux-aarch64

---

## Troubleshooting

**Build fails on macOS ARM cross-compile with zig:**
- Use native cargo instead: `cargo build --release --target aarch64-apple-darwin`
- zig has issues with macOS frameworks

**"Invalid platform" error:**
- Check platform string matches exactly (darwin-x86_64, NOT darwin-x64)
- Verify in `src/update/downloader.rs` Platform::as_str()

**Builds are slow:**
- Builds run sequentially due to cargo file locking
- Average time: ~15-20 minutes for all platforms

**Upload fails with 400:**
- Check deploy key is correct
- Ensure Authorization header has "Bearer" prefix
- Verify platform string format

---

## Testing Release

```bash
# Download and test
curl -fsSL https://download.spoq.dev/cli/download/darwin-x86_64 -o /tmp/spoq
chmod +x /tmp/spoq
/tmp/spoq --version  # Should show new version

# Test auto-update (from old version)
spoq --update  # Downloads new version
# Restart spoq
spoq --version  # Should show new version
```

---

## Full Documentation

See [docs/RELEASE.md](../docs/RELEASE.md) for complete guide.

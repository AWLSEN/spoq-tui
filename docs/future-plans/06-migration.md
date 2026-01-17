# First-Time Setup and Migration

## Overview

When a user sets up Spoq Cloud for the first time, we offer to migrate their development environment from their local machine. This creates a seamless transition where their Conductor instance feels like "their computer in the cloud."

## What We Migrate

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  MIGRATION SCOPE                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  From Local Machine → To Conductor                                          │
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │  Git Repositories                                          Required │    │
│  │  └── All repos in ~/projects, ~/code, ~/dev, etc.                   │    │
│  │  └── Preserves full git history                                     │    │
│  │  └── Preserves remotes configuration                                │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │  GitHub Authentication                                     Optional │    │
│  │  └── SSH keys (~/.ssh/id_* + config)                               │    │
│  │  └── GitHub CLI token (~/.config/gh/hosts.yml)                     │    │
│  │  └── Git credentials (~/.git-credentials)                          │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │  Git Configuration                                         Optional │    │
│  │  └── ~/.gitconfig (name, email, aliases)                           │    │
│  │  └── ~/.gitignore_global                                           │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │  Shell Configuration                                       Optional │    │
│  │  └── ~/.bashrc, ~/.zshrc                                           │    │
│  │  └── ~/.aliases                                                     │    │
│  │  └── Environment variables (filtered for secrets)                  │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

## GitHub Auth Token Locations

### 1. GitHub CLI (`gh`)

```yaml
# ~/.config/gh/hosts.yml
github.com:
    user: alice
    oauth_token: gho_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
    git_protocol: ssh
```

**Migration**: Copy entire `~/.config/gh/` directory.

### 2. Git Credential Storage

```bash
# Check which credential helper is configured
git config --global credential.helper

# Common helpers:
# - osxkeychain (macOS Keychain)
# - manager (Windows Credential Manager)
# - store (plaintext ~/.git-credentials)
# - cache (temporary in-memory)
```

**~/.git-credentials format:**
```
https://alice:ghp_xxxxxxxxxxxxxxxxxxxx@github.com
https://alice:glpat-xxxxxxxxxxxxxxxxxxxx@gitlab.com
```

**Migration**:
- If using `store`: Copy `~/.git-credentials`
- If using `osxkeychain`: Export via `security` command or prompt user to re-auth
- If using `cache`: Not persistent, skip

### 3. SSH Keys

```
~/.ssh/
├── id_ed25519           # Private key
├── id_ed25519.pub       # Public key
├── id_rsa               # Legacy RSA private key
├── id_rsa.pub           # Legacy RSA public key
├── config               # SSH config (hosts, identities)
└── known_hosts          # Verified host fingerprints
```

**Migration**: Copy entire `~/.ssh/` with proper permissions (700 for dir, 600 for private keys).

### 4. Git Global Config

```ini
# ~/.gitconfig
[user]
    name = Alice Smith
    email = alice@example.com
[core]
    editor = vim
    excludesfile = ~/.gitignore_global
[alias]
    co = checkout
    br = branch
    st = status
[pull]
    rebase = true
```

**Migration**: Copy `~/.gitconfig` and `~/.gitignore_global`.

## Migration Flow

### User Experience

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  WEB APP - MIGRATION (After initial setup)                                   │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │                                                                      │    │
│  │   Bring your code to Spoq                              Step 1 of 3  │    │
│  │                                                                      │    │
│  │   We can copy your projects and settings from your computer.        │    │
│  │   Everything transfers securely and stays private.                  │    │
│  │                                                                      │    │
│  │   To start, run this command on your Mac:                          │    │
│  │                                                                      │    │
│  │   ┌────────────────────────────────────────────────────────────┐   │    │
│  │   │ curl -fsSL https://spoq.dev/migrate | bash                 │   │    │
│  │   │                                                        [Copy]│   │    │
│  │   └────────────────────────────────────────────────────────────┘   │    │
│  │                                                                      │    │
│  │   This will scan your computer and show what can be migrated.      │    │
│  │   Nothing is uploaded until you approve.                           │    │
│  │                                                                      │    │
│  │                                                                      │    │
│  │   ┌────────────────────┐                                           │    │
│  │   │  I've run the command  │                                       │    │
│  │   └────────────────────┘                                           │    │
│  │                                                                      │    │
│  │   Skip migration →                                                  │    │
│  │                                                                      │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Migration Script (Local Machine)

```bash
#!/bin/bash
# spoq-migrate.sh - Downloaded and run on user's local machine

set -e

echo "═══════════════════════════════════════════════════════════════"
echo "                    Spoq Migration Tool                         "
echo "═══════════════════════════════════════════════════════════════"
echo ""

# Get migration token from user (shown in web app)
read -p "Enter your migration code from spoq.dev: " MIGRATION_TOKEN

# Validate token with API
MIGRATION_INFO=$(curl -s "https://api.spoq.dev/migrate/validate" \
    -H "Authorization: Bearer $MIGRATION_TOKEN")

if [ "$(echo $MIGRATION_INFO | jq -r '.valid')" != "true" ]; then
    echo "Invalid migration code. Please check and try again."
    exit 1
fi

SUBDOMAIN=$(echo $MIGRATION_INFO | jq -r '.subdomain')
UPLOAD_URL=$(echo $MIGRATION_INFO | jq -r '.upload_url')

echo ""
echo "Migrating to: $SUBDOMAIN.spoq.dev"
echo ""

# ═══════════════════════════════════════════════════════════════════
# PHASE 1: DISCOVERY
# ═══════════════════════════════════════════════════════════════════

echo "Scanning your computer..."
echo ""

# Find git repositories
GIT_REPOS=()
SEARCH_DIRS=("$HOME/projects" "$HOME/code" "$HOME/dev" "$HOME/workspace" "$HOME/src" "$HOME/repos" "$HOME/Documents")

for dir in "${SEARCH_DIRS[@]}"; do
    if [ -d "$dir" ]; then
        while IFS= read -r repo; do
            if [ -n "$repo" ]; then
                GIT_REPOS+=("$repo")
            fi
        done < <(find "$dir" -maxdepth 3 -name ".git" -type d 2>/dev/null | xargs -I {} dirname {})
    fi
done

# Calculate sizes
TOTAL_REPO_SIZE=0
declare -A REPO_SIZES
for repo in "${GIT_REPOS[@]}"; do
    size=$(du -sm "$repo" 2>/dev/null | cut -f1)
    REPO_SIZES["$repo"]=$size
    TOTAL_REPO_SIZE=$((TOTAL_REPO_SIZE + size))
done

# Check for auth tokens
HAS_GH_CLI=false
HAS_SSH_KEYS=false
HAS_GIT_CREDENTIALS=false
HAS_GITCONFIG=false

[ -f "$HOME/.config/gh/hosts.yml" ] && HAS_GH_CLI=true
[ -f "$HOME/.ssh/id_ed25519" ] || [ -f "$HOME/.ssh/id_rsa" ] && HAS_SSH_KEYS=true
[ -f "$HOME/.git-credentials" ] && HAS_GIT_CREDENTIALS=true
[ -f "$HOME/.gitconfig" ] && HAS_GITCONFIG=true

# ═══════════════════════════════════════════════════════════════════
# PHASE 2: SHOW SUMMARY
# ═══════════════════════════════════════════════════════════════════

echo "═══════════════════════════════════════════════════════════════"
echo "                    Migration Summary                           "
echo "═══════════════════════════════════════════════════════════════"
echo ""
echo "Git Repositories (${#GIT_REPOS[@]} found, ${TOTAL_REPO_SIZE}MB total):"
echo "──────────────────────────────────────────────────────────────"
for repo in "${GIT_REPOS[@]}"; do
    name=$(basename "$repo")
    size=${REPO_SIZES["$repo"]}
    echo "  ✓ $name (${size}MB)"
done
echo ""

echo "Authentication & Config:"
echo "──────────────────────────────────────────────────────────────"
$HAS_GH_CLI && echo "  ✓ GitHub CLI (gh) - logged in"
$HAS_SSH_KEYS && echo "  ✓ SSH Keys for Git"
$HAS_GIT_CREDENTIALS && echo "  ✓ Git Credentials (stored tokens)"
$HAS_GITCONFIG && echo "  ✓ Git Config (name, email, aliases)"
echo ""

echo "═══════════════════════════════════════════════════════════════"
echo ""

# ═══════════════════════════════════════════════════════════════════
# PHASE 3: CONFIRMATION
# ═══════════════════════════════════════════════════════════════════

echo "What would you like to migrate?"
echo ""
echo "  [1] Everything (recommended)"
echo "  [2] Repositories only"
echo "  [3] Let me choose"
echo "  [q] Cancel"
echo ""
read -p "Choose option: " CHOICE

case $CHOICE in
    1)
        MIGRATE_REPOS=true
        MIGRATE_AUTH=true
        MIGRATE_CONFIG=true
        ;;
    2)
        MIGRATE_REPOS=true
        MIGRATE_AUTH=false
        MIGRATE_CONFIG=false
        ;;
    3)
        # Interactive selection
        read -p "Migrate repositories? [Y/n] " -n 1 -r
        echo
        [[ $REPLY =~ ^[Yy]$ ]] || [ -z "$REPLY" ] && MIGRATE_REPOS=true || MIGRATE_REPOS=false

        read -p "Migrate GitHub auth (SSH keys, tokens)? [Y/n] " -n 1 -r
        echo
        [[ $REPLY =~ ^[Yy]$ ]] || [ -z "$REPLY" ] && MIGRATE_AUTH=true || MIGRATE_AUTH=false

        read -p "Migrate git config? [Y/n] " -n 1 -r
        echo
        [[ $REPLY =~ ^[Yy]$ ]] || [ -z "$REPLY" ] && MIGRATE_CONFIG=true || MIGRATE_CONFIG=false
        ;;
    q|Q)
        echo "Migration cancelled."
        exit 0
        ;;
esac

# ═══════════════════════════════════════════════════════════════════
# PHASE 4: PACKAGE AND UPLOAD
# ═══════════════════════════════════════════════════════════════════

echo ""
echo "Preparing migration package..."

TEMP_DIR=$(mktemp -d)
PACKAGE_DIR="$TEMP_DIR/spoq-migration"
mkdir -p "$PACKAGE_DIR"

# Package repositories
if [ "$MIGRATE_REPOS" = true ]; then
    echo "  Packaging repositories..."
    mkdir -p "$PACKAGE_DIR/repos"
    for repo in "${GIT_REPOS[@]}"; do
        name=$(basename "$repo")
        echo "    - $name"
        # Use git bundle for efficient transfer with full history
        (cd "$repo" && git bundle create "$PACKAGE_DIR/repos/$name.bundle" --all 2>/dev/null) || \
        # Fallback to tarball if bundle fails
        tar -czf "$PACKAGE_DIR/repos/$name.tar.gz" -C "$(dirname "$repo")" "$name"
    done
fi

# Package auth
if [ "$MIGRATE_AUTH" = true ]; then
    echo "  Packaging authentication..."
    mkdir -p "$PACKAGE_DIR/auth"

    # GitHub CLI
    if [ -d "$HOME/.config/gh" ]; then
        cp -r "$HOME/.config/gh" "$PACKAGE_DIR/auth/gh-cli"
    fi

    # SSH keys
    if [ -d "$HOME/.ssh" ]; then
        mkdir -p "$PACKAGE_DIR/auth/ssh"
        # Copy keys and config, but not known_hosts (will be regenerated)
        cp "$HOME/.ssh/id_"* "$PACKAGE_DIR/auth/ssh/" 2>/dev/null || true
        cp "$HOME/.ssh/config" "$PACKAGE_DIR/auth/ssh/" 2>/dev/null || true
    fi

    # Git credentials
    if [ -f "$HOME/.git-credentials" ]; then
        cp "$HOME/.git-credentials" "$PACKAGE_DIR/auth/"
    fi
fi

# Package config
if [ "$MIGRATE_CONFIG" = true ]; then
    echo "  Packaging configuration..."
    mkdir -p "$PACKAGE_DIR/config"

    [ -f "$HOME/.gitconfig" ] && cp "$HOME/.gitconfig" "$PACKAGE_DIR/config/"
    [ -f "$HOME/.gitignore_global" ] && cp "$HOME/.gitignore_global" "$PACKAGE_DIR/config/"
fi

# Create manifest
cat > "$PACKAGE_DIR/manifest.json" << EOF
{
    "version": "1.0",
    "created_at": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
    "source_hostname": "$(hostname)",
    "repos": $(printf '%s\n' "${GIT_REPOS[@]}" | jq -R . | jq -s .),
    "has_auth": $MIGRATE_AUTH,
    "has_config": $MIGRATE_CONFIG
}
EOF

# Create encrypted archive
echo "  Creating secure package..."
ARCHIVE="$TEMP_DIR/migration.tar.gz.enc"

# Generate one-time encryption key
ENC_KEY=$(openssl rand -hex 32)

tar -czf - -C "$TEMP_DIR" spoq-migration | \
    openssl enc -aes-256-cbc -salt -pbkdf2 -pass "pass:$ENC_KEY" -out "$ARCHIVE"

ARCHIVE_SIZE=$(du -h "$ARCHIVE" | cut -f1)
echo ""
echo "Package ready: $ARCHIVE_SIZE"

# ═══════════════════════════════════════════════════════════════════
# PHASE 5: UPLOAD
# ═══════════════════════════════════════════════════════════════════

echo ""
echo "Uploading to your Spoq..."

# Upload with progress
curl -X POST "$UPLOAD_URL" \
    -H "Authorization: Bearer $MIGRATION_TOKEN" \
    -H "X-Encryption-Key: $ENC_KEY" \
    -F "file=@$ARCHIVE" \
    --progress-bar | cat

echo ""
echo "═══════════════════════════════════════════════════════════════"
echo "                    Migration Complete!                         "
echo "═══════════════════════════════════════════════════════════════"
echo ""
echo "Your code and settings have been copied to $SUBDOMAIN.spoq.dev"
echo ""
echo "Connect now:"
echo "  ssh $USER@$SUBDOMAIN.spoq.dev"
echo ""

# Cleanup
rm -rf "$TEMP_DIR"
```

### Server-Side Processing

```rust
// migration_processor.rs

use std::path::Path;
use flate2::read::GzDecoder;
use tar::Archive;

pub struct MigrationProcessor {
    user_home: PathBuf,
}

impl MigrationProcessor {
    pub async fn process_migration(
        &self,
        encrypted_archive: &Path,
        encryption_key: &str,
    ) -> Result<MigrationResult> {
        // Decrypt archive
        let decrypted = self.decrypt_archive(encrypted_archive, encryption_key)?;

        // Extract
        let temp_dir = tempfile::tempdir()?;
        let mut archive = Archive::new(GzDecoder::new(File::open(&decrypted)?));
        archive.unpack(&temp_dir)?;

        let migration_dir = temp_dir.path().join("spoq-migration");
        let manifest: Manifest = serde_json::from_reader(
            File::open(migration_dir.join("manifest.json"))?
        )?;

        let mut result = MigrationResult::default();

        // Process repositories
        if migration_dir.join("repos").exists() {
            result.repos = self.process_repos(&migration_dir.join("repos")).await?;
        }

        // Process auth
        if migration_dir.join("auth").exists() {
            result.auth = self.process_auth(&migration_dir.join("auth")).await?;
        }

        // Process config
        if migration_dir.join("config").exists() {
            result.config = self.process_config(&migration_dir.join("config")).await?;
        }

        Ok(result)
    }

    async fn process_repos(&self, repos_dir: &Path) -> Result<Vec<String>> {
        let workspace = self.user_home.join("workspace");
        fs::create_dir_all(&workspace)?;

        let mut imported = Vec::new();

        for entry in fs::read_dir(repos_dir)? {
            let entry = entry?;
            let path = entry.path();
            let name = path.file_stem().unwrap().to_str().unwrap();

            if path.extension().map(|e| e == "bundle").unwrap_or(false) {
                // Git bundle - clone from it
                let repo_path = workspace.join(name);
                Command::new("git")
                    .args(["clone", path.to_str().unwrap(), repo_path.to_str().unwrap()])
                    .status()?;
                imported.push(name.to_string());
            } else if path.extension().map(|e| e == "gz").unwrap_or(false) {
                // Tarball - extract
                let mut archive = Archive::new(GzDecoder::new(File::open(&path)?));
                archive.unpack(&workspace)?;
                imported.push(name.to_string());
            }
        }

        Ok(imported)
    }

    async fn process_auth(&self, auth_dir: &Path) -> Result<AuthResult> {
        let mut result = AuthResult::default();

        // GitHub CLI
        let gh_src = auth_dir.join("gh-cli");
        if gh_src.exists() {
            let gh_dest = self.user_home.join(".config/gh");
            fs::create_dir_all(gh_dest.parent().unwrap())?;
            copy_dir_all(&gh_src, &gh_dest)?;
            result.gh_cli = true;
        }

        // SSH keys
        let ssh_src = auth_dir.join("ssh");
        if ssh_src.exists() {
            let ssh_dest = self.user_home.join(".ssh");
            fs::create_dir_all(&ssh_dest)?;

            for entry in fs::read_dir(&ssh_src)? {
                let entry = entry?;
                let dest_path = ssh_dest.join(entry.file_name());
                fs::copy(entry.path(), &dest_path)?;

                // Set correct permissions
                let perms = if entry.file_name().to_str().unwrap().ends_with(".pub") {
                    0o644
                } else {
                    0o600
                };
                fs::set_permissions(&dest_path, fs::Permissions::from_mode(perms))?;
            }

            // Set .ssh directory permissions
            fs::set_permissions(&ssh_dest, fs::Permissions::from_mode(0o700))?;

            result.ssh_keys = true;
        }

        // Git credentials
        let creds_src = auth_dir.join(".git-credentials");
        if creds_src.exists() {
            let creds_dest = self.user_home.join(".git-credentials");
            fs::copy(&creds_src, &creds_dest)?;
            fs::set_permissions(&creds_dest, fs::Permissions::from_mode(0o600))?;
            result.git_credentials = true;
        }

        Ok(result)
    }

    async fn process_config(&self, config_dir: &Path) -> Result<ConfigResult> {
        let mut result = ConfigResult::default();

        // Git config
        let gitconfig_src = config_dir.join(".gitconfig");
        if gitconfig_src.exists() {
            fs::copy(&gitconfig_src, self.user_home.join(".gitconfig"))?;
            result.gitconfig = true;
        }

        // Global gitignore
        let gitignore_src = config_dir.join(".gitignore_global");
        if gitignore_src.exists() {
            fs::copy(&gitignore_src, self.user_home.join(".gitignore_global"))?;
            result.gitignore = true;
        }

        Ok(result)
    }
}
```

## Security Considerations

### Encryption

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  MIGRATION SECURITY                                                          │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  1. One-time encryption key generated locally                               │
│     └── Never stored on our servers                                         │
│     └── Sent via separate header, not in archive                           │
│                                                                              │
│  2. Archive encrypted with AES-256-CBC                                      │
│     └── Salt + PBKDF2 key derivation                                        │
│     └── Industry standard encryption                                        │
│                                                                              │
│  3. TLS for upload                                                          │
│     └── Additional layer of transport encryption                            │
│                                                                              │
│  4. Immediate processing                                                    │
│     └── Archive deleted after extraction                                    │
│     └── Encryption key discarded                                            │
│                                                                              │
│  5. No plaintext secrets in logs                                            │
│     └── Tokens are masked in any output                                     │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Token Handling

```rust
// Mask tokens in logs
fn mask_token(token: &str) -> String {
    if token.len() > 8 {
        format!("{}...{}", &token[..4], &token[token.len()-4..])
    } else {
        "****".to_string()
    }
}

// Validate tokens still work after migration
async fn verify_github_auth(home: &Path) -> Result<AuthStatus> {
    // Test SSH
    let ssh_result = Command::new("ssh")
        .args(["-T", "git@github.com"])
        .env("HOME", home)
        .output()?;

    let ssh_ok = String::from_utf8_lossy(&ssh_result.stderr)
        .contains("successfully authenticated");

    // Test gh CLI
    let gh_result = Command::new("gh")
        .args(["auth", "status"])
        .env("HOME", home)
        .output()?;

    let gh_ok = gh_result.status.success();

    Ok(AuthStatus { ssh_ok, gh_ok })
}
```

## Post-Migration Verification

### TUI First Connection After Migration

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                                                              │
│                               SPOQ                                           │
│                                                                              │
│                    Welcome back, Alice!                                     │
│                                                                              │
│   ─────────────────────────────────────────────────────────────────────     │
│                                                                              │
│   Migration complete! Here's what we brought over:                         │
│                                                                              │
│   Repositories (12):                                                        │
│     ~/workspace/myapp                                                       │
│     ~/workspace/api-server                                                  │
│     ~/workspace/dotfiles                                                    │
│     ... and 9 more                                                          │
│                                                                              │
│   Authentication:                                                           │
│     ✓ GitHub SSH key working                                               │
│     ✓ GitHub CLI authenticated                                             │
│                                                                              │
│   Configuration:                                                            │
│     ✓ Git config (alice@example.com)                                       │
│     ✓ Git aliases imported                                                 │
│                                                                              │
│   ─────────────────────────────────────────────────────────────────────     │
│                                                                              │
│   Try: cd ~/workspace/myapp && git status                                  │
│                                                                              │
│   ┌─────────────────────────────────────────────────────────────────────┐   │
│   │ >                                                                    │   │
│   └─────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Verification Commands

```bash
# Test GitHub SSH
$ ssh -T git@github.com
Hi alice! You've successfully authenticated...

# Test GitHub CLI
$ gh auth status
github.com
  ✓ Logged in to github.com as alice
  ✓ Git operations for github.com configured to use ssh protocol.

# Test git config
$ git config --global user.email
alice@example.com

# List migrated repos
$ ls ~/workspace
api-server/  myapp/  dotfiles/  ...
```

## Edge Cases

### Keychain-Stored Credentials (macOS)

```bash
# macOS stores git credentials in Keychain
# These cannot be directly exported

# Detection
if git config --global credential.helper | grep -q "osxkeychain"; then
    echo "Your Git passwords are stored in macOS Keychain."
    echo "You'll need to re-authenticate on first push."
    echo ""
    echo "Options:"
    echo "  1. Use SSH keys instead (recommended)"
    echo "  2. Create a Personal Access Token on GitHub"
    echo "     and enter it when prompted"
fi
```

### Large Repositories

```bash
# For repos > 1GB, offer options
if [ $size -gt 1024 ]; then
    echo "  ⚠ $name is ${size}MB (large)"
    echo "    Options:"
    echo "    [f] Full migration (may take a while)"
    echo "    [s] Shallow clone (recent history only)"
    echo "    [r] Remote only (just add git remote, clone on demand)"
fi
```

### Private Repos with Different Auth

```bash
# Some repos might use different credentials
# Parse .git/config for each repo to detect

for repo in "${GIT_REPOS[@]}"; do
    remote_url=$(cd "$repo" && git remote get-url origin 2>/dev/null)

    case "$remote_url" in
        *github.com*)
            # Uses GitHub auth
            ;;
        *gitlab.com*)
            echo "Note: $repo uses GitLab - make sure GitLab auth is configured"
            ;;
        *bitbucket.org*)
            echo "Note: $repo uses Bitbucket - make sure Bitbucket auth is configured"
            ;;
    esac
done
```

## Alternative: Manual Token Setup

For users who prefer not to migrate tokens automatically:

```
┌─ GitHub Setup ──────────────────────────────────────────────┐
│                                                              │
│  Your repositories were migrated, but GitHub authentication │
│  needs to be set up.                                        │
│                                                              │
│  Option 1: Generate new SSH key (recommended)               │
│  ─────────────────────────────────────────────              │
│  Run: ssh-keygen -t ed25519 -C "alice@spoq.dev"            │
│  Then add the public key to GitHub:                         │
│  https://github.com/settings/ssh/new                        │
│                                                              │
│  Option 2: Use GitHub CLI                                   │
│  ─────────────────────────────────────────────              │
│  Run: gh auth login                                         │
│  Follow the prompts to authenticate.                        │
│                                                              │
│  Option 3: Personal Access Token                            │
│  ─────────────────────────────────────────────              │
│  Create token: https://github.com/settings/tokens           │
│  Run: git config --global credential.helper store          │
│  Then push to any repo and enter the token when prompted.  │
│                                                              │
│  [Enter] I've set up authentication                         │
│  [s] Skip for now                                           │
│                                                              │
└──────────────────────────────────────────────────────────────┘
```

## Implementation Checklist

### Migration Script (Client-Side)
- [ ] Repository discovery (multiple common directories)
- [ ] Git bundle creation for repos
- [ ] SSH key collection
- [ ] GitHub CLI token collection
- [ ] Git credentials collection
- [ ] Git config collection
- [ ] Interactive selection UI
- [ ] Encryption with one-time key
- [ ] Upload with progress bar
- [ ] Cleanup temp files

### Server-Side Processing
- [ ] Migration token generation/validation
- [ ] Secure upload endpoint
- [ ] Archive decryption
- [ ] Repository extraction (bundle and tarball)
- [ ] Auth file placement with correct permissions
- [ ] Config file placement
- [ ] Post-migration verification
- [ ] Cleanup

### Web App
- [ ] Migration initiation page
- [ ] Migration code display
- [ ] Status polling/updates
- [ ] Completion confirmation

### TUI
- [ ] Post-migration welcome screen
- [ ] Auth verification display
- [ ] Repo listing
- [ ] Manual auth setup guide

### Security
- [ ] One-time encryption key handling
- [ ] No token logging
- [ ] Immediate archive deletion
- [ ] Permission verification on sensitive files

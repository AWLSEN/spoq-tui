# Token Verification Before and After VPS Provisioning

**Date:** 2026-01-22
**Status:** Ready for Planning

## What We're Building

A two-phase token verification system that:

1. **Pre-Provisioning**: Verifies required tokens (Claude Code, GitHub CLI) exist locally before allowing VPS provisioning
2. **Post-Provisioning**: Tests that migrated tokens actually work on the VPS by running commands non-interactively

This ensures tokens are available before provisioning starts and validates they work after migration completes.

## Current State

**Token Migration Flow:**
- Migration runs AFTER VPS is provisioned and ready
- Creates archive at `~/.spoq-migration/archive.tar.gz`
- Saves `token_archive_path` to credentials
- No verification that tokens work on VPS
- No pre-check that required tokens exist locally

**Problem:**
- Users can start provisioning without required tokens
- Migration may succeed (archive created) but tokens don't work on VPS
- No feedback loop to know if tokens are usable
- User discovers issues much later when trying to use Claude/GitHub on VPS

## Why This Approach

### Approach: Separate Verification Module

Create new module `src/auth/token_verification.rs` with:
- `verify_local_tokens()` - Check local token availability before provisioning
- `verify_vps_tokens()` - SSH to VPS and test tokens work after migration

**Benefits:**
1. **Clear separation of concerns** - Detection vs Provisioning vs Verification
2. **Testable** - Can unit test verification logic independently
3. **Reusable** - Can use for `/sync` command to re-verify later
4. **Maintainable** - Verification logic in one place, easy to update

**Rejected Alternatives:**
- **Inline verification**: Makes provisioning flow too long and complex
- **Script-based**: Harder to provide good error messages, less type safety

## Key Decisions

### 1. Required vs Optional Tokens

**Required (block provisioning):**
- Claude Code token - core functionality
- GitHub CLI token - essential for development workflow

**Optional (continue with warning):**
- Codex token - nice to have
- Other future tokens

### 2. Pre-Provisioning Gate

- Run `verify_local_tokens()` at START of provisioning flow
- Block before VPS type selection (Managed vs BYOVPS)
- Show clear error: "Missing required tokens. Please login to Claude Code and GitHub CLI"
- Provide instructions on how to authenticate

### 3. Post-Provisioning Verification

**When**: Immediately after VPS provisioning completes and migration runs

**What to test:**

**Claude Code:**
```bash
echo "testing 123" | claude --non-interactive
```
- Expects: Successful response (any response indicates token works)
- Verifies: Claude CLI is installed, authenticated, and functional

**GitHub CLI:**
```bash
gh auth status
```
- Expects: Exit code 0 or "Logged in to github.com" message
- Verifies: GitHub CLI is installed and authenticated

**On Success:**
- Display: ✅ "Claude Code and GitHub CLI verified on VPS"

**On Failure (graceful):**
- Display: ⚠️ "Warning: Could not verify [Claude Code/GitHub CLI] on VPS. You may need to manually SSH and log in."
- Still mark VPS as ready
- User can continue with manual setup

### 4. No State Persistence

**Decision**: Don't persist verification results to credentials.json

**Rationale:**
- Verification is a point-in-time check during provisioning
- If migration succeeds but verification fails, user will manually fix
- No value in showing stale verification status on future startups
- Keeps credentials.json focused on auth/VPS metadata
- User can always SSH and test manually if unsure

### 5. SSH Connection for Verification

**Method**: Use existing SSH credentials from provisioning
- Managed VPS: Use `ssh_password` from provisioning
- BYOVPS: Use SSH credentials user provided
- Connection info already in `credentials.vps_ip`, `credentials.vps_hostname`

**Timeout**: 30 seconds per command (Claude/GitHub)
- Long enough for SSH handshake + command execution
- Short enough to not hang indefinitely

## Implementation Outline

### New Module: `src/auth/token_verification.rs`

```rust
/// Result of local token verification
pub struct LocalTokenVerification {
    pub claude_code_present: bool,
    pub github_cli_present: bool,
    pub codex_present: bool,
    pub all_required_present: bool,
}

/// Result of VPS token verification
pub struct VpsTokenVerification {
    pub claude_code_works: bool,
    pub github_cli_works: bool,
    pub ssh_error: Option<String>,
}

/// Verify required tokens exist locally before provisioning
pub fn verify_local_tokens() -> Result<LocalTokenVerification, TokenVerificationError>

/// SSH to VPS and verify tokens work
pub fn verify_vps_tokens(
    vps_ip: &str,
    ssh_username: &str,
    ssh_password: &str,
) -> Result<VpsTokenVerification, TokenVerificationError>
```

### Integration Points

**1. Provisioning Flow Start** (`src/auth/provisioning_flow.rs:357`)
```rust
pub fn run_provisioning_flow(...) -> Result<(), CentralApiError> {
    // NEW: Verify local tokens before proceeding
    let local_tokens = verify_local_tokens()?;
    if !local_tokens.all_required_present {
        return Err(...); // Block provisioning
    }

    // Existing flow continues...
}
```

**2. After Managed VPS Ready** (`src/auth/provisioning_flow.rs:704`)
```rust
// Run token migration after VPS is ready
let migration_result = run_token_migration();
save_credentials(credentials);

// NEW: Verify tokens work on VPS
let vps_verification = verify_vps_tokens(
    credentials.vps_ip.as_ref().unwrap(),
    "spoq", // or from credentials
    &ssh_password,
)?;
display_verification_results(&vps_verification);
```

**3. After BYOVPS Ready** (Similar pattern in BYOVPS flows)

### Error Handling

**Local Verification Failure:**
- Show error message
- List which required tokens are missing
- Provide login instructions
- Exit with error code

**VPS Verification Failure:**
- Log warning (don't error)
- Display which tokens failed verification
- Show manual SSH login instructions
- Continue to mark VPS as ready

## Open Questions

### 1. SSH Library Choice
- **Option A**: Use existing SSH library (if any in dependencies)
- **Option B**: Add new dependency like `ssh2` crate
- **Option C**: Shell out to `ssh` command via `std::process::Command`

**Recommendation**: Start with Option C (shell out) for simplicity. Can refactor to SSH library later if needed.

### 2. Non-Interactive Claude Mode
- What exact command should we run?
- Does `claude --non-interactive` flag exist?
- Alternative: `echo "test" | claude --stdin`?
- Need to verify command format on VPS

**TODO**: Test actual Claude CLI command format on VPS

### 3. GitHub CLI Verification Command
- `gh auth status` - best option?
- Alternative: `gh api user` (tests API access)?
- What exit code indicates success?

**TODO**: Verify `gh auth status` behavior

### 4. Timeout Values
- 30 seconds per command reasonable?
- Should SSH connection be separate timeout from command execution?
- What if VPS is slow to respond?

**TODO**: Test with real VPS to determine appropriate timeouts

### 5. Multiple SSH Connections
- Open one SSH connection and run both commands?
- Or separate SSH connection per command?
- Trade-off: Speed vs simplicity

**Recommendation**: Separate connections for simplicity. Can optimize later if performance is an issue.

## Success Criteria

**Must Have:**
1. Block provisioning if Claude Code token missing locally
2. Block provisioning if GitHub CLI token missing locally
3. Run verification commands on VPS after migration
4. Display clear success/warning messages
5. Don't fail provisioning if VPS verification fails (graceful)

**Nice to Have:**
1. Detailed error messages when verification fails
2. Instructions on how to manually fix token issues
3. Verification for optional tokens (Codex)

## Future Enhancements

1. **`/sync` command** - Re-run verification manually anytime
2. **Verification status persistence** - Track verification history
3. **Automatic retry** - If verification fails, retry migration
4. **VPS health check** - Periodic token verification in background
5. **Token refresh flow** - Detect when VPS tokens expire and re-migrate

## Related Files

- `src/auth/token_migration.rs` - Token detection and export logic
- `src/auth/provisioning_flow.rs` - Where verification will be called
- `src/auth/credentials.rs` - Credentials structure (no changes needed)
- `scripts/migration/creds-migrate.sh` - Migration shell script

## Next Steps

1. Create `src/auth/token_verification.rs` module
2. Implement local token verification using existing `detect_tokens()`
3. Implement VPS token verification with SSH commands
4. Integrate into provisioning flow at two checkpoints
5. Add tests for verification logic
6. Manual testing with real VPS

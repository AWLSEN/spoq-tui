---
title: Startup Health Check Sequence
date: 2026-01-22
status: refined
---

# Startup Health Check Sequence

## User Vision

When SPOQ starts and finds an existing VPS connection, run a health check sequence before loading the TUI:

### Startup Sequence Flow

```
Starting SPOQ...
✓ Authentication verified

Running VPS health checks...

[1/2] Checking conductor health...
  → Connecting to conductor at https://conductor.example.com
  ⠋ Pinging /health endpoint...
  ✓ Conductor is responding (200 OK)

[2/2] Verifying VPS tokens...
  ⠋ Checking Claude Code on VPS...
  ✓ Claude Code verified
  ⠋ Checking GitHub CLI on VPS...
  ✓ GitHub CLI verified

✓ All systems ready!

[Loading TUI...]
```

## Health Check Steps

### Step 1: Conductor Health Check
- **Endpoint**: `/health` on conductor URL
- **Method**: GET request
- **Expected**: 200 OK status
- **Timeout**: 10 seconds
- **On failure**: Warn user but continue

### Step 2: VPS Token Verification
- **Check 1**: SSH to VPS → `claude -p "test verification"`
- **Check 2**: SSH to VPS → `gh auth status`
- **Run**: In parallel (faster)
- **Timeout**: 30 seconds per check
- **On failure**: Warn user but continue

## Visual Design

### Success Flow
```
Running VPS health checks...

[1/2] Checking conductor health...
  ⠋ Pinging /health endpoint...
  ✓ Conductor responding (123ms)

[2/2] Verifying VPS tokens...
  ⠋ Checking tokens in parallel...
  ✓ Claude Code verified
  ✓ GitHub CLI verified

✓ All systems ready! Starting TUI...
```

### Partial Failure Flow
```
Running VPS health checks...

[1/2] Checking conductor health...
  ⠋ Pinging /health endpoint...
  ⚠️  Conductor not responding (timeout)

[2/2] Verifying VPS tokens...
  ⠋ Checking tokens in parallel...
  ✓ Claude Code verified
  ⚠️  GitHub CLI verification failed

⚠️  Some checks failed. You may experience issues.
Starting TUI anyway...
```

### Complete Failure Flow
```
Running VPS health checks...

[1/2] Checking conductor health...
  ⠋ Pinging /health endpoint...
  ✗ Conductor unreachable (network error)

[2/2] Verifying VPS tokens...
  ⠋ Checking tokens in parallel...
  ✗ Could not connect to VPS (SSH timeout)

⚠️  Health checks failed. VPS may be offline.
What would you like to do?
  [C] Continue anyway
  [R] Retry checks
  [Q] Quit
```

## Implementation Details

### Where to Add

In `main.rs`, after VPS status check, before TUI initialization:

```rust
// After VPS check...
if credentials.has_vps() {
    println!("\nRunning VPS health checks...\n");

    // Step 1: Check conductor health
    println!("[1/2] Checking conductor health...");
    let conductor_healthy = check_conductor_health(&credentials);

    // Step 2: Verify tokens on VPS
    println!("\n[2/2] Verifying VPS tokens...");
    let tokens_verified = verify_vps_tokens_parallel(&credentials);

    // Display results
    display_health_check_results(conductor_healthy, tokens_verified);

    // Optional: Ask user what to do if critical failures
    if should_prompt_user(conductor_healthy, tokens_verified) {
        match prompt_health_check_action() {
            HealthCheckAction::Continue => { /* proceed */ }
            HealthCheckAction::Retry => { /* retry checks */ }
            HealthCheckAction::Quit => std::process::exit(0),
        }
    }
}
```

### New Functions Needed

1. **`check_conductor_health(credentials) -> HealthStatus`**
   - GET request to `{conductor_url}/health`
   - Return: Healthy, Unhealthy, Unreachable

2. **`verify_vps_tokens_parallel(credentials) -> TokenVerificationResult`**
   - Run Claude Code and GitHub CLI checks in parallel
   - Use tokio tasks or threads
   - Return: Both, ClaudeOnly, GitHubOnly, Neither

3. **`display_health_check_results(...)`**
   - Show summary of checks
   - Colorized output (green ✓, yellow ⚠️, red ✗)

4. **`prompt_health_check_action() -> HealthCheckAction`**
   - Interactive prompt if critical issues
   - Options: Continue, Retry, Quit

### Loading Indicators

Use simple text-based spinners:
```rust
// Spinner characters: ⠋ ⠙ ⠹ ⠸ ⠼ ⠴ ⠦ ⠧ ⠇ ⠏
let spinner = Spinner::new("Checking...");
// ... do work ...
spinner.success("✓ Done");
```

### Performance

- **Total time (success)**: ~3-5 seconds
  - Conductor health: ~100-500ms
  - Token verification (parallel): ~2-4 seconds

- **Total time (failure)**: ~10-30 seconds
  - Conductor timeout: 10s
  - SSH timeout: 30s (but parallel, so only 30s total)

### Non-Blocking Behavior

- All checks are non-blocking
- Warnings shown but TUI starts anyway
- Only prompt user on **critical failures** (optional)

## Conductor Health Endpoint

Need to determine:
- What is the conductor URL? (stored in credentials?)
- Does `/health` endpoint exist?
- What does it return? (200 OK with JSON? Plain text?)

## BLOCKER: SSH Password Not Stored

**Problem discovered**: SSH passwords are NOT stored in credentials for security reasons. We cannot SSH to VPS on every startup without prompting for password.

**Options to resolve**:

### Option 1: Store Encrypted SSH Password (Recommended)
- Add `ssh_password_encrypted` field to credentials
- Use system keychain or encryption key
- Pros: Seamless startup experience
- Cons: Password stored on disk (encrypted)

### Option 2: Skip SSH Verification, Only Check Conductor
- Only ping `/v1/health` endpoint on startup
- Skip token verification completely
- Pros: Simple, no password needed
- Cons: Doesn't verify tokens actually work

### Option 3: Make SSH Verification Optional
- Add `--verify-tokens` flag to opt-in to SSH verification
- Prompt for SSH password only when flag used
- Pros: User choice, no stored password
- Cons: Most users won't use it

### Option 4: Use SSH Keys Instead (Future)
- Set up SSH keys during provisioning
- Use keys for password-less SSH
- Pros: More secure, better UX
- Cons: Requires provisioning changes

**Recommended approach**: Start with Option 2 (conductor health only) for MVP, then add Option 4 (SSH keys) later for full token verification.

## Open Questions

1. ~~What is the conductor URL?~~ ✅ Found: `credentials.vps_url`
2. ~~Does conductor have /health endpoint?~~ ✅ Found: `/v1/health`
3. **How to handle SSH credentials?** See blocker above - need user decision
4. **Should we make it skippable?** Add `--skip-health-check` flag?

## Success Criteria

- ✓ Health checks run on every startup (when VPS exists)
- ✓ Checks complete in <5 seconds on healthy VPS
- ✓ Clear visual progress indicators
- ✓ Graceful handling of failures
- ✓ Non-blocking - TUI starts even if checks fail
- ✓ Helpful error messages for troubleshooting

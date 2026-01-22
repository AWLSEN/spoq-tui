---
title: Add VPS Token Verification on Every Startup
date: 2026-01-22
status: brainstorming
---

# Add VPS Token Verification on Every Startup

## User Request

On every startup, before loading the TUI:
- SSH to the VPS
- Run `claude -p "testing verification"` to verify Claude Code works
- Run `gh auth status` to verify GitHub CLI works
- Show a loading indicator while checking
- Display results before starting TUI

## Design Goals

1. **Fast and Non-Blocking**: Don't make startup feel slow
2. **Visual Feedback**: Show progress while checking
3. **Graceful Degradation**: Continue even if verification fails (warn user)
4. **Clear Output**: Show what's being checked and the results

## UX Options

### Option 1: Spinner with Status Updates (Recommended)

```
Starting SPOQ...
✓ Authentication verified
✓ VPS found (192.168.1.100)

Verifying VPS tokens...
⠋ Checking Claude Code on VPS...
✓ Claude Code verified on VPS
⠋ Checking GitHub CLI on VPS...
✓ GitHub CLI verified on VPS

Your VPS is ready!
[Starting TUI...]
```

**Pros:**
- Clean, professional output
- Real-time feedback on what's happening
- Feels responsive

**Cons:**
- Requires spinner implementation

### Option 2: Progress Bar

```
Starting SPOQ...
Verifying VPS tokens: [████████░░] 80% (GitHub CLI)
✓ Claude Code verified
⠋ GitHub CLI checking...
```

**Pros:**
- Shows overall progress
- Familiar UX pattern

**Cons:**
- More complex to implement
- Might feel slower

### Option 3: Simple Dots Animation

```
Starting SPOQ...
Verifying tokens on VPS...
✓ Claude Code verified
✓ GitHub CLI verified
```

**Pros:**
- Simplest implementation
- Clean output

**Cons:**
- Less informative

## Implementation Approach

### Where to Add

In `main.rs`, after VPS status check but before TUI starts:

```rust
// After VPS check passes...
if credentials.has_vps() {
    println!("\nVerifying VPS tokens...");

    // Show spinner while checking
    let spinner = start_spinner("Checking Claude Code on VPS...");

    match verify_vps_tokens(...) {
        Ok(verification) => {
            stop_spinner(spinner);
            display_verification_results(&verification);
        }
        Err(e) => {
            stop_spinner(spinner);
            eprintln!("⚠️  Warning: Could not verify VPS tokens: {}", e);
            eprintln!("Continuing anyway...\n");
        }
    }
}
```

### Spinner Implementation

Options:
1. **indicatif crate** - Popular spinner/progress bar library
2. **spinners crate** - Lightweight spinner library
3. **Custom implementation** - Simple manual spinner with threads

### Performance Considerations

- **SSH timeout**: Already set to 30s in `run_ssh_command`
- **Total time**: ~2-5 seconds for both checks (if VPS responds quickly)
- **Parallel execution**: Could run both checks in parallel to save time

### Failure Handling

If verification fails:
- ✓ **Continue to TUI** (non-blocking)
- ✗ **Don't exit**
- ⚠️ **Show warning** with manual fix instructions
- 📝 **Log to debug** for troubleshooting

## Questions

1. **Should we cache results?** Skip check if verified recently (e.g., last 5 minutes)?
   - Pro: Faster restart cycles during development
   - Con: Might miss issues

2. **Parallel verification?** Run Claude Code and GitHub CLI checks simultaneously?
   - Pro: Faster (cut time in half)
   - Con: More complex error handling

3. **Timeout customization?** Allow user to skip with Ctrl+C or set timeout?
   - Pro: User control
   - Con: More complexity

## Recommended Implementation

**Phase 1: Basic Implementation**
- Add verification call in main.rs after VPS check
- Simple "Checking..." message without spinner
- Display results using existing `display_vps_verification_results()`
- Non-blocking: continue even if checks fail

**Phase 2: Polish (Optional)**
- Add spinner using `indicatif` crate
- Run checks in parallel
- Add result caching (5-minute TTL)

## Code Changes Needed

1. **main.rs** - Add verification call after VPS check
2. **token_verification.rs** - Possibly add parallel check function
3. **Cargo.toml** - Add `indicatif` dependency (if using spinners)

## Success Criteria

- ✓ Verification runs on every startup
- ✓ Takes < 5 seconds on responsive VPS
- ✓ Shows clear feedback to user
- ✓ Doesn't block if verification fails
- ✓ Graceful handling of SSH errors

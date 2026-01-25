# Phase 2: Nova Thread ID Capture Modification

## Summary
Modified the Nova skill prompt to capture thread ID from the `CONDUCTOR_THREAD_ID` environment variable as the primary method.

## File Modified
- **Path**: `/Users/sam/.claude/plugins/cache/awlsen-plugins/starry-night/3.6.1/commands/nova.md`
- **Section**: Step 6: Save Plan (lines 265-283)

## Changes Made

### Before
The Nova skill only checked for thread ID in the system prompt prefix:
1. Look for `[Thread: xxx]` prefix at the very start of your system prompt
2. If found: Extract the thread_id value
3. If not found: Use "null" as the value

### After
Added environment variable check as the primary method:
1. **Method 1 (Primary)**: Run `echo $CONDUCTOR_THREAD_ID` to check environment variable
   - If non-empty: Use the value (e.g., "01JHHXYZ...")
2. **Method 2 (Fallback)**: Look for `[Thread: xxx]` prefix at the very start of your system prompt
   - If found: Extract the thread_id value (e.g., "[Thread: abc123]" → "abc123")
3. **Method 3 (Default)**: If both methods fail, use "null" as the value

## Impact
- Nova agents will now check `$CONDUCTOR_THREAD_ID` first when creating plans
- This enables Conductor to set the thread ID via environment variable injection
- Maintains backward compatibility with the prompt prefix method
- Thread IDs will be properly captured in plan metadata for tracking and status updates

## Testing
To verify this works:
1. Set `CONDUCTOR_THREAD_ID=test-thread-123` in environment
2. Run `/nova` command to create a plan
3. Check generated plan metadata for `Thread ID: test-thread-123`

## Date
2026-01-25

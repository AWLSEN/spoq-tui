# OS Notifications for Plan Events

## Idea

Send OS-level notifications when plan states change, so users know when plans are queued or ready for review - even if the TUI is closed (Conductor keeps running on VPS).

## Notification Events

### 1. Plan Queued

- **Trigger**: Nova saves a plan to queue
- **Title**: `📋 Plan Queued`
- **Body**: Plan summary preview
- **Context**: Project name, phase count

### 2. Plan Ready for Review

- **Trigger**: Pulsar finishes execution, moves plan to `review/`
- **Title**: `✅ Ready for Review`
- **Body**: Plan summary preview
- **Context**: Project name, completion status

## Approach

Use `osascript` on macOS (inherits terminal's notification permissions - no extra consent needed). Fallback to `notify-send` on Linux.

## Future Extensions

- Plan execution started
- Plan failed
- Configurable per-project enable/disable

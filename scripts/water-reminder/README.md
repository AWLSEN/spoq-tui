# Water Reminder Script

A smart macOS notification system that reminds you to stay hydrated based on your computer activity.

## Features

- 💧 **Activity-based reminders**: Adjusts notification frequency based on computer usage
- 🎯 **Smart intervals**:
  - Active usage: Every 30 minutes
  - Idle time: Every 1.5 hours
- 🔔 **Native macOS notifications** with sound
- 🎲 **Random messages** to keep reminders fresh
- 🚀 **Runs in background** as a launchd service

## Quick Install

Install with a single command:

```bash
curl -fsSL https://raw.githubusercontent.com/AWLSEN/spoq-tui/main/scripts/water-reminder/install-water-reminder.sh | bash
```

## Manual Installation

1. **Download the script:**
   ```bash
   mkdir -p ~/.claude/scripts ~/.claude/logs
   curl -fsSL https://raw.githubusercontent.com/AWLSEN/spoq-tui/main/scripts/water-reminder/water-reminder.sh -o ~/.claude/scripts/water-reminder.sh
   chmod +x ~/.claude/scripts/water-reminder.sh
   ```

2. **Download the service file:**
   ```bash
   curl -fsSL https://raw.githubusercontent.com/AWLSEN/spoq-tui/main/scripts/water-reminder/com.claude.waterreminder.plist -o ~/Library/LaunchAgents/com.claude.waterreminder.plist
   sed -i '' "s|/Users/nidhishgajjar|$HOME|g" ~/Library/LaunchAgents/com.claude.waterreminder.plist
   ```

3. **Start the service:**
   ```bash
   launchctl load ~/Library/LaunchAgents/com.claude.waterreminder.plist
   ```

## Usage

### Start/Stop

```bash
# Start the reminder service
launchctl load ~/Library/LaunchAgents/com.claude.waterreminder.plist

# Stop the reminder service
launchctl unload ~/Library/LaunchAgents/com.claude.waterreminder.plist

# Check if running
launchctl list | grep waterreminder
```

### View Logs

```bash
# Watch live logs
tail -f ~/.claude/logs/water-reminder.log

# View error logs
tail -f ~/.claude/logs/water-reminder.error.log
```

### Test Manually

Run the script directly to test notifications:

```bash
~/.claude/scripts/water-reminder.sh
```

Press `Ctrl+C` to stop.

## How It Works

The script monitors your system idle time (time since last keyboard/mouse input) and adjusts reminder frequency:

- **Active** (idle < 5 minutes): You're actively using your computer, so reminders come every 30 minutes
- **Idle** (idle > 5 minutes): You're away or inactive, so reminders are spaced out to 1.5 hours

This ensures you get timely reminders when you're working, but won't be disturbed with frequent notifications when you're away.

## Customization

Edit `~/.claude/scripts/water-reminder.sh` to customize:

- **Intervals**: Change `ACTIVE_INTERVAL` and `IDLE_INTERVAL` (in seconds)
- **Idle threshold**: Adjust `IDLE_THRESHOLD` to change when the script considers you "idle"
- **Messages**: Add or modify messages in the `MESSAGES` array
- **Notification sound**: Change the sound name in the `osascript` command

## Uninstall

```bash
# Stop and remove the service
launchctl unload ~/Library/LaunchAgents/com.claude.waterreminder.plist
rm ~/Library/LaunchAgents/com.claude.waterreminder.plist

# Remove the script and logs (optional)
rm -rf ~/.claude/scripts/water-reminder.sh
rm -rf ~/.claude/logs/water-reminder*
```

## Requirements

- macOS (tested on macOS 10.14+)
- No additional dependencies required

## License

MIT License - Feel free to use and modify!

## Contributing

Found a bug or want to add a feature? Pull requests are welcome!

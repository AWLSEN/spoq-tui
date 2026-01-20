#!/bin/bash

# Water Reminder Installer
# Quick install: curl -fsSL https://raw.githubusercontent.com/AWLSEN/spoq-tui/main/scripts/water-reminder/install-water-reminder.sh | bash

set -e

echo "🌊 Water Reminder Installer"
echo "============================"
echo ""

# Detect OS
if [[ "$OSTYPE" != "darwin"* ]]; then
    echo "❌ Error: This script currently only supports macOS."
    echo "   Linux support coming soon!"
    exit 1
fi

# Create directories
echo "📁 Creating directories..."
mkdir -p ~/.claude/scripts
mkdir -p ~/.claude/logs
mkdir -p ~/Library/LaunchAgents

# Download the water reminder script
echo "⬇️  Downloading water reminder script..."
SCRIPT_URL="https://raw.githubusercontent.com/AWLSEN/spoq-tui/main/scripts/water-reminder/water-reminder.sh"
curl -fsSL "$SCRIPT_URL" -o ~/.claude/scripts/water-reminder.sh
chmod +x ~/.claude/scripts/water-reminder.sh

# Download the launchd plist
echo "⬇️  Downloading service configuration..."
PLIST_URL="https://raw.githubusercontent.com/AWLSEN/spoq-tui/main/scripts/water-reminder/com.claude.waterreminder.plist"
curl -fsSL "$PLIST_URL" -o ~/Library/LaunchAgents/com.claude.waterreminder.plist

# Update the plist with the current user's home directory
sed -i '' "s|/Users/nidhishgajjar|$HOME|g" ~/Library/LaunchAgents/com.claude.waterreminder.plist

# Unload if already running
launchctl unload ~/Library/LaunchAgents/com.claude.waterreminder.plist 2>/dev/null || true

# Load the service
echo "🚀 Starting water reminder service..."
launchctl load ~/Library/LaunchAgents/com.claude.waterreminder.plist

echo ""
echo "✅ Installation complete!"
echo ""
echo "The water reminder is now running in the background."
echo ""
echo "📋 Useful commands:"
echo "  Stop:     launchctl unload ~/Library/LaunchAgents/com.claude.waterreminder.plist"
echo "  Start:    launchctl load ~/Library/LaunchAgents/com.claude.waterreminder.plist"
echo "  Logs:     tail -f ~/.claude/logs/water-reminder.log"
echo "  Test:     ~/.claude/scripts/water-reminder.sh"
echo ""
echo "💧 Stay hydrated!"

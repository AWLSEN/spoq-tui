#!/bin/bash

# Water Reminder Script
# Sends notifications based on computer activity level

# Configuration
ACTIVE_INTERVAL=1800      # 30 minutes in seconds
IDLE_INTERVAL=5400        # 1.5 hours in seconds
IDLE_THRESHOLD=300        # 5 minutes in seconds - threshold to consider "idle"

# Notification messages
MESSAGES=(
    "💧 Time to drink some water!"
    "🚰 Hydration check! Grab some water."
    "💦 Don't forget to stay hydrated!"
    "🌊 Water break time!"
    "💧 Your body needs water. Take a sip!"
)

# Function to get system idle time in seconds
get_idle_time() {
    # Get idle time from ioreg (macOS)
    idle_ns=$(ioreg -c IOHIDSystem | awk '/HIDIdleTime/ {print $NF}')
    # Convert from nanoseconds to seconds
    echo $((idle_ns / 1000000000))
}

# Function to send notification
send_notification() {
    local message="${MESSAGES[$RANDOM % ${#MESSAGES[@]}]}"
    osascript -e "display notification \"$message\" with title \"Water Reminder\" sound name \"Glass\"" 2>/dev/null
}

# Main loop
echo "Water reminder started. Activity-based intervals:"
echo "  - Active (idle < 5 min): every 30 minutes"
echo "  - Idle (idle > 5 min): every 1.5 hours"
echo "Press Ctrl+C to stop."

last_reminder=0

while true; do
    current_time=$(date +%s)
    idle_time=$(get_idle_time)

    # Determine interval based on activity
    if [ "$idle_time" -lt "$IDLE_THRESHOLD" ]; then
        interval=$ACTIVE_INTERVAL
        status="active"
    else
        interval=$IDLE_INTERVAL
        status="idle"
    fi

    # Check if it's time to send a reminder
    time_since_last=$((current_time - last_reminder))

    if [ "$time_since_last" -ge "$interval" ] || [ "$last_reminder" -eq 0 ]; then
        send_notification
        last_reminder=$current_time
        echo "[$(date '+%Y-%m-%d %H:%M:%S')] Reminder sent (status: $status, idle: ${idle_time}s)"
    fi

    # Sleep for 1 minute before checking again
    sleep 60
done

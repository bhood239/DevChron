#!/bin/bash
# Example Waybar integration script for DevChron
# Place this in ~/.config/waybar/scripts/devchron.sh

STATUS_FILE="$HOME/.cache/devchron/status.json"

if [ ! -f "$STATUS_FILE" ]; then
    echo '{"text": "⏸", "tooltip": "DevChron not running", "class": "inactive"}'
    exit 0
fi

# Read status with jq
PHASE=$(jq -r '.phase' "$STATUS_FILE" 2>/dev/null)
TIME=$(jq -r '.time_remaining' "$STATUS_FILE" 2>/dev/null)
RUNNING=$(jq -r '.is_running' "$STATUS_FILE" 2>/dev/null)
SESSION=$(jq -r '.session' "$STATUS_FILE" 2>/dev/null)

# Determine emoji based on phase
case "$PHASE" in
    "focus")
        EMOJI="🍅"
        CLASS="focus"
        ;;
    "short_break")
        EMOJI="☕"
        CLASS="break"
        ;;
    "long_break")
        EMOJI="🌴"
        CLASS="long-break"
        ;;
    *)
        EMOJI="⏸"
        CLASS="inactive"
        ;;
esac

# Add pause indicator if not running
if [ "$RUNNING" = "false" ]; then
    EMOJI="⏸"
    CLASS="paused"
fi

# Output Waybar JSON format
echo "{\"text\": \"$EMOJI $TIME\", \"tooltip\": \"Session $SESSION\", \"class\": \"$CLASS\"}"

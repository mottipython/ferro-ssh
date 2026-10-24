#!/usr/bin/env bash
set -e

rm -f /root/.ssh/known_hosts 2>/dev/null || true
mkdir -p /root/.ssh

Xvfb :99 -screen 0 1150x750x24 &
export DISPLAY=:99
export LIBGL_ALWAYS_SOFTWARE=1
sleep 1
openbox &
sleep 1

/app/target/release/ferro-ssh --connect "Docker Test Server" &
PID=$!
sleep 3.5

# Step 1: Click "Accept Once (Session Only)" at x=600, y=500
xdotool mousemove 600 500 click 1
sleep 2.5

# Step 2: Capture Password Modal
import -window root /app/docs/screenshots/06-password-modal.png
echo "Captured 06-password-modal.png"

# Step 3: Type password and press Enter
# Auto-focus is active in password modal, but let's also click center
xdotool mousemove 575 375 click 1
sleep 0.5
xdotool type "password123"
sleep 0.5
xdotool key Return
sleep 3.5

# Step 4: Send commands into the terminal
xdotool type "uname -a"
xdotool key Return
sleep 1
xdotool type "uptime"
xdotool key Return
sleep 1
xdotool type "free -h"
xdotool key Return
sleep 2

# Step 5: Capture connected terminal session
import -window root /app/docs/screenshots/07-terminal-connected.png
echo "Captured 07-terminal-connected.png"

kill $PID || true
sleep 1

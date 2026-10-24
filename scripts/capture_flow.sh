#!/usr/bin/env bash
set -e

mkdir -p /app/docs/screenshots
rm -f /root/.ssh/known_hosts 2>/dev/null || true
mkdir -p /root/.ssh

Xvfb :99 -screen 0 1150x750x24 &
export DISPLAY=:99
export LIBGL_ALWAYS_SOFTWARE=1
sleep 1

# Launch FerroSSH
/app/target/release/ferro-ssh &
APP_PID=$!
sleep 2

# 1. Main Dashboard
import -window root /app/docs/screenshots/01-dashboard.png
echo "[1/6] Captured 01-dashboard.png"

# 2. Port Tunnels View
xdotool mousemove 365 14 click 1
sleep 1
import -window root /app/docs/screenshots/02-tunnels.png
echo "[2/6] Captured 02-tunnels.png"

# 3. Settings View
xdotool mousemove 475 14 click 1
sleep 1
import -window root /app/docs/screenshots/03-settings.png
echo "[3/6] Captured 03-settings.png"

# 4. Back to Terminal & trigger connection to Docker Test Server
xdotool mousemove 160 14 click 1
sleep 1

# Click the play button on "Docker Test Server" (x=200, y=125)
xdotool mousemove 200 125 click 1
sleep 2

# Capture Host Key Verification Modal (TOFU)
import -window root /app/docs/screenshots/04-fingerprint-dialog.png
echo "[4/6] Captured 04-fingerprint-dialog.png"

# Click "Accept Once" or "Accept & Save"
# Modal is centered around [575, 375]. "Accept & Save" is on the left, "Accept Once" in middle, "Reject" on right.
# Let's find window or click around y=440, x=575
xdotool mousemove 575 440 click 1 || true
sleep 1.5

# Capture Password Modal
import -window root /app/docs/screenshots/05-password-modal.png
echo "[5/6] Captured 05-password-modal.png"

# Type password into modal
xdotool type "password123"
sleep 0.5
xdotool key Return
sleep 2.5

# Run a sample command in the terminal to show live activity
xdotool type "df -h"
xdotool key Return
sleep 1

# Capture Active Terminal & Command Toolbox
import -window root /app/docs/screenshots/06-terminal-toolbox.png
echo "[6/6] Captured 06-terminal-toolbox.png"

kill $APP_PID || true
sleep 1
echo "All screenshots captured successfully!"

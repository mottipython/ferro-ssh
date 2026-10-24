#!/usr/bin/env bash
set -e

mkdir -p /app/docs/screenshots
rm -f /root/.ssh/known_hosts 2>/dev/null || true
mkdir -p /root/.ssh

Xvfb :99 -screen 0 1150x750x24 &
export DISPLAY=:99
export LIBGL_ALWAYS_SOFTWARE=1
sleep 1

# 1. Main Dashboard
/app/target/release/ferro-ssh &
PID=$!
sleep 2
import -window root /app/docs/screenshots/01-dashboard.png
kill $PID || true
sleep 1
echo "Captured 01-dashboard.png"

# 2. Tunnels View
/app/target/release/ferro-ssh --view tunnels &
PID=$!
sleep 2
import -window root /app/docs/screenshots/02-tunnels.png
kill $PID || true
sleep 1
echo "Captured 02-tunnels.png"

# 3. Settings View
/app/target/release/ferro-ssh --view settings &
PID=$!
sleep 2
import -window root /app/docs/screenshots/03-settings.png
kill $PID || true
sleep 1
echo "Captured 03-settings.png"

# 4. SFTP Explorer View
/app/target/release/ferro-ssh --view sftp &
PID=$!
sleep 2
import -window root /app/docs/screenshots/04-sftp-browser.png
kill $PID || true
sleep 1
echo "Captured 04-sftp-browser.png"

# 5. Connect to Docker Test Server -> Triggers Fingerprint TOFU modal!
/app/target/release/ferro-ssh --connect "Docker Test Server" &
PID=$!
sleep 2.5
import -window root /app/docs/screenshots/05-fingerprint-dialog.png
kill $PID || true
sleep 1
echo "Captured 05-fingerprint-dialog.png"

echo "All views captured successfully!"

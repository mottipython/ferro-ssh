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
echo "Captured 01-dashboard.png"

# 2. Port Tunnels View (Click at top tab: Port Tunnels)
# In 1150px wide window:
# "⚡ FerroSSH" is ~120px
# "🖥 Terminal" is ~130px to ~230px
# "📂 SFTP Files" is ~240px to ~340px
# "🔀 Port Tunnels" is ~350px to ~460px
# "⚙ Settings" is ~470px to ~560px
xdotool mousemove 400 18 click 1
sleep 1
import -window root /app/docs/screenshots/02-tunnels.png
echo "Captured 02-tunnels.png"

# 3. Settings View
xdotool mousemove 510 18 click 1
sleep 1
import -window root /app/docs/screenshots/03-settings.png
echo "Captured 03-settings.png"

# 4. Back to Terminal View
xdotool mousemove 180 18 click 1
sleep 1

kill $APP_PID || true
sleep 1

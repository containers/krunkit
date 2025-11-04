#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
#
# Setup script for WSLg-like functionality in the Linux guest
# This script should be run inside the Linux VM to configure
# Wayland, Weston, and PulseAudio for GUI application support.

set -e

# Configuration - can be overridden via environment variables
WESTON_WIDTH="${WESTON_WIDTH:-1920}"
WESTON_HEIGHT="${WESTON_HEIGHT:-1080}"

echo "Setting up WSLg-like environment..."
echo "Display resolution: ${WESTON_WIDTH}x${WESTON_HEIGHT}"
echo "Note: Set WESTON_WIDTH and WESTON_HEIGHT environment variables to change resolution"

# Detect package manager
if command -v apt-get &> /dev/null; then
    PKG_MANAGER="apt-get"
    PKG_INSTALL="apt-get install -y"
    PKG_UPDATE="apt-get update"
elif command -v dnf &> /dev/null; then
    PKG_MANAGER="dnf"
    PKG_INSTALL="dnf install -y"
    PKG_UPDATE="dnf check-update || true"
elif command -v pacman &> /dev/null; then
    PKG_MANAGER="pacman"
    PKG_INSTALL="pacman -S --noconfirm"
    PKG_UPDATE="pacman -Sy"
else
    echo "Error: No supported package manager found (apt-get, dnf, or pacman)"
    exit 1
fi

echo "Detected package manager: $PKG_MANAGER"

# Update package lists
echo "Updating package lists..."
sudo $PKG_UPDATE

# Install required packages
echo "Installing Wayland, Weston, and PulseAudio..."
case $PKG_MANAGER in
    apt-get)
        sudo $PKG_INSTALL \
            weston \
            wayland-protocols \
            libwayland-client0 \
            libwayland-server0 \
            pulseaudio \
            pulseaudio-utils \
            dbus-x11 \
            xwayland \
            mesa-utils \
            libgl1-mesa-dri \
            libglx-mesa0
        ;;
    dnf)
        sudo $PKG_INSTALL \
            weston \
            wayland-protocols-devel \
            wayland \
            pulseaudio \
            pulseaudio-utils \
            dbus-x11 \
            xorg-x11-server-Xwayland \
            mesa-dri-drivers \
            mesa-libGL
        ;;
    pacman)
        sudo $PKG_INSTALL \
            weston \
            wayland-protocols \
            wayland \
            pulseaudio \
            pulseaudio-alsa \
            dbus \
            xorg-xwayland \
            mesa \
            mesa-utils
        ;;
esac

# Get current user's UID for proper directory permissions
USER_UID=$(id -u)
RUNTIME_DIR="/run/user/${USER_UID}"

# Create runtime directory
mkdir -p "${RUNTIME_DIR}"
chmod 700 "${RUNTIME_DIR}"

# Create PulseAudio directory
mkdir -p "${RUNTIME_DIR}/pulse"
chmod 700 "${RUNTIME_DIR}/pulse"

# Create Weston configuration
mkdir -p ~/.config
cat > ~/.config/weston.ini <<EOF
[core]
backend=headless-backend.so
renderer=pixman

[shell]
panel-position=none

[output]
name=headless
mode=${WESTON_WIDTH}x${WESTON_HEIGHT}

[screen-share]
command=/usr/bin/weston --backend=rdp-backend.so --no-clients-resize
EOF

# Create systemd user directory
mkdir -p ~/.config/systemd/user

# Create Weston systemd service with dynamic runtime directory
cat > ~/.config/systemd/user/weston.service <<EOF
[Unit]
Description=Weston Wayland Compositor
After=dbus.service

[Service]
Type=notify
Environment=XDG_RUNTIME_DIR=${RUNTIME_DIR}
Environment=WAYLAND_DISPLAY=wayland-0
ExecStart=/usr/bin/weston --logger-scopes=log,protocol
Restart=on-failure
RestartSec=5

[Install]
WantedBy=default.target
EOF

# Create PulseAudio systemd service with dynamic runtime directory
cat > ~/.config/systemd/user/pulseaudio.service <<EOF
[Unit]
Description=PulseAudio Sound Server
After=dbus.service

[Service]
Type=notify
Environment=XDG_RUNTIME_DIR=${RUNTIME_DIR}
ExecStart=/usr/bin/pulseaudio --daemonize=no --log-target=stderr
Restart=on-failure
RestartSec=5

[Install]
WantedBy=default.target
EOF

# Set up environment variables in profile
cat >> ~/.bashrc <<EOF

# WSLg-like environment variables
export XDG_RUNTIME_DIR=${RUNTIME_DIR}
export WAYLAND_DISPLAY=wayland-0
export XDG_SESSION_TYPE=wayland
export PULSE_SERVER=unix:${RUNTIME_DIR}/pulse/native
export GDK_BACKEND=wayland
export QT_QPA_PLATFORM=wayland
export SDL_VIDEODRIVER=wayland
export CLUTTER_BACKEND=wayland
EOF

echo ""
echo "WSLg-like environment setup complete!"
echo ""
echo "To start the services, run:"
echo "  systemctl --user daemon-reload"
echo "  systemctl --user enable weston pulseaudio"
echo "  systemctl --user start weston pulseaudio"
echo ""
echo "Or simply source your .bashrc and start services manually:"
echo "  source ~/.bashrc"
echo "  weston &"
echo "  pulseaudio --start"
echo ""
echo "You can now install and run GUI applications:"
echo "  sudo $PKG_INSTALL gedit firefox gimp"
echo "  gedit &"
echo ""

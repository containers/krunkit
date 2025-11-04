#!/bin/bash
# SPDX-License-Identifier: Apache-2.0
#
# Example: Running krunkit with WSLg-like GUI support
#
# This example shows how to start a VM with GUI application support
# using the WSLg-like features.

# Prerequisites:
# 1. A Linux disk image (e.g., Ubuntu, Fedora)
# 2. libkrun-efi installed on macOS
# 3. Network connectivity setup (gvproxy or similar)

# Paths - adjust these to your environment
DISK_IMAGE="$HOME/vms/linux-gui.img"
NETWORK_SOCKET="/tmp/krunkit-network.sock"
SHARED_DIR="/tmp/krunkit-shared"

# Create shared directory for socket communication
mkdir -p "$SHARED_DIR"

# Launch krunkit with WSLg-like GUI support
krunkit \
  --cpus 4 \
  --memory 4096 \
  --wslg-gui \
  --wslg-audio \
  --wslg-gpu-width 1920 \
  --wslg-gpu-height 1080 \
  --device virtio-blk,path="$DISK_IMAGE",format=raw \
  --device virtio-net,type=unixgram,path="$NETWORK_SOCKET",mac=52:54:00:12:34:56 \
  --device virtio-fs,sharedDir="$SHARED_DIR",mountTag=shared \
  --device virtio-vsock,port=6000,socketURL=/tmp/wayland.sock,listen \
  --device virtio-vsock,port=4713,socketURL=/tmp/pulseaudio.sock,listen \
  --restful-uri tcp://localhost:8081 \
  --log-file /tmp/krunkit.log

# Note: Inside the VM, you'll need to:
# 1. Run the setup script: bash /path/to/setup-wslg-guest.sh
# 2. Install GUI applications: sudo apt install gedit firefox gimp
# 3. Launch applications: gedit &

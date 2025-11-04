# Quick Start: WSLg-like Features on macOS

This guide will help you get started with running Linux GUI applications on macOS using krunkit's WSLg-like features.

## Prerequisites

1. **macOS** with Apple Silicon (M1/M2/M3) or Intel
2. **krunkit** installed via Homebrew:
   ```bash
   brew tap slp/krunkit
   brew install krunkit
   ```
3. **A Linux disk image** (Ubuntu, Fedora, or your preferred distribution)
4. **Network backend** (gvproxy or vmnet-helper)

## Step 1: Prepare Your Linux VM

First, create or download a Linux disk image. For this example, we'll assume you have an Ubuntu image at `~/vms/ubuntu.img`.

If you need to create one, you can use tools like:
- `virt-install` with KVM/QEMU
- Download a cloud image from Ubuntu/Fedora
- Build a custom image using Packer

## Step 2: Boot the VM (Initial Setup)

Start the VM without GUI features first to set up the necessary components:

```bash
krunkit \
  --cpus 4 \
  --memory 4096 \
  --device virtio-blk,path=~/vms/ubuntu.img,format=raw \
  --device virtio-net,type=unixgram,path=/tmp/network.sock,mac=52:54:00:12:34:56 \
  --restful-uri tcp://localhost:8081
```

## Step 3: Setup WSLg Components in Guest

Once the VM is running, log in and run the setup script:

```bash
# Inside the VM
# First, get the setup script into the VM
# (via network copy, or mount a shared directory)

# Make it executable
chmod +x setup-wslg-guest.sh

# Run the setup
./setup-wslg-guest.sh

# Source the new environment
source ~/.bashrc

# Enable and start services
systemctl --user daemon-reload
systemctl --user enable weston pulseaudio
systemctl --user start weston pulseaudio
```

Alternatively, manually install the required packages:

```bash
# Ubuntu/Debian
sudo apt update
sudo apt install weston wayland-protocols pulseaudio pulseaudio-utils \
                 xwayland mesa-utils dbus-x11 gedit firefox

# Fedora
sudo dnf install weston wayland-protocols pulseaudio pulseaudio-utils \
                 xorg-x11-server-Xwayland mesa-dri-drivers gedit firefox

# Arch Linux
sudo pacman -S weston wayland-protocols pulseaudio xorg-xwayland \
               mesa gedit firefox
```

## Step 4: Shutdown and Reboot with GUI Support

Shutdown the VM cleanly:

```bash
# From another terminal
curl -X POST http://localhost:8081/vm/state -d '{"state": "Stop"}'
```

Now restart with GUI support enabled:

```bash
krunkit \
  --cpus 4 \
  --memory 4096 \
  --wslg-gui \
  --wslg-audio \
  --wslg-gpu-width 1920 \
  --wslg-gpu-height 1080 \
  --device virtio-blk,path=~/vms/ubuntu.img,format=raw \
  --device virtio-net,type=unixgram,path=/tmp/network.sock,mac=52:54:00:12:34:56 \
  --restful-uri tcp://localhost:8081 \
  --log-file ~/krunkit.log
```

## Step 5: Launch GUI Applications

Once the VM boots, you can launch GUI applications from within the VM:

```bash
# Inside the VM
gedit &
firefox &
```

The applications will render using the virtio-gpu device with GPU acceleration.

## Troubleshooting

### GUI Applications Don't Start

Check that Weston is running:
```bash
ps aux | grep weston
echo $WAYLAND_DISPLAY
```

If not running, start it:
```bash
systemctl --user start weston
# or manually
weston &
```

### No Audio

Check PulseAudio:
```bash
ps aux | grep pulseaudio
echo $PULSE_SERVER
```

Test audio:
```bash
paplay /usr/share/sounds/alsa/Front_Center.wav
```

### Poor Performance

1. Increase RAM allocation: `--memory 8192`
2. Add more CPUs: `--cpus 8`
3. Check GPU is enabled in guest:
   ```bash
   lspci | grep VGA
   glxinfo | grep renderer
   ```

### Display Issues

Check virtio-gpu resolution:
```bash
# Inside VM
xrandr  # if using XWayland
weston-info  # for Wayland info
```

## Example: Complete Workflow

Here's a complete example script:

```bash
#!/bin/bash
# complete-wslg-example.sh

VM_IMAGE="$HOME/vms/ubuntu-gui.img"
NETWORK_SOCK="/tmp/krunkit-net.sock"

# Start gvproxy for networking (in another terminal)
# gvproxy -listen unix://$NETWORK_SOCK ...

# Start krunkit with WSLg features
krunkit \
  --cpus 6 \
  --memory 8192 \
  --wslg-gui \
  --wslg-audio \
  --wslg-gpu-width 2560 \
  --wslg-gpu-height 1440 \
  --device virtio-blk,path="$VM_IMAGE",format=raw \
  --device virtio-net,type=unixgram,path="$NETWORK_SOCK",mac=52:54:00:12:34:56,offloading=on,vfkitMagic=on \
  --device virtio-fs,sharedDir="$HOME/shared",mountTag=shared \
  --restful-uri tcp://localhost:8081 \
  --log-file "$HOME/krunkit-wslg.log"
```

## Next Steps

- **Install your favorite apps**: `sudo apt install gimp inkscape blender`
- **Configure multi-monitor**: Adjust `--wslg-gpu-width` and `--wslg-gpu-height`
- **Share files**: Use virtio-fs with `--device virtio-fs,sharedDir=/path,mountTag=tag`
- **Customize Weston**: Edit `~/.config/weston.ini` in the guest

## Advanced Usage

### Custom Resolution

```bash
krunkit ... --wslg-gpu-width 3840 --wslg-gpu-height 2160  # 4K
krunkit ... --wslg-gpu-width 2560 --wslg-gpu-height 1440  # 2K
```

### Audio Only (No GUI)

```bash
krunkit ... --wslg-audio  # without --wslg-gui
```

### Debug Mode

```bash
krunkit ... --krun-log-level 5 --log-file /tmp/debug.log
```

## Resources

- [Full Documentation](./wslg-like-features.md)
- [Usage Guide](./usage.md)
- [Contributing](../CONTRIBUTING.md)
- [libkrun Documentation](https://github.com/containers/libkrun)

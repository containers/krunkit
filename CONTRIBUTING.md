# Contributing to krunkit

Thank you for your interest in contributing to krunkit!

## Building from Source

### Prerequisites

- macOS (krunkit uses the macOS Hypervisor.framework)
- Rust toolchain (latest stable)
- libkrun-efi installed via Homebrew

### Build Instructions

```bash
# Install dependencies
brew tap slp/krunkit
brew install libkrun-efi

# Clone the repository
git clone https://github.com/ericcurtin/krunkit.git
cd krunkit

# Build
make

# Install (optional)
sudo make install
```

## WSLg-like System Distro

The WSLg-like features in krunkit rely on a properly configured Linux guest system with Wayland, Weston, and PulseAudio.

### Creating a System Distro

You can create a custom system distro for WSLg-like functionality by:

1. **Start with a minimal Linux distribution**
   - Ubuntu Server, Fedora Server, or Alpine Linux
   - Minimum 2GB disk space
   - Recommend 4GB+ RAM allocation

2. **Install required components**
   
   Use the provided setup script:
   ```bash
   # Copy script to VM
   scp scripts/setup-wslg-guest.sh user@vm:/tmp/
   
   # Inside the VM
   bash /tmp/setup-wslg-guest.sh
   ```

   Or manually install:
   ```bash
   # Ubuntu/Debian
   sudo apt update
   sudo apt install weston wayland-protocols pulseaudio pulseaudio-utils \
                    xwayland mesa-utils dbus-x11

   # Fedora
   sudo dnf install weston wayland-protocols pulseaudio pulseaudio-utils \
                    xorg-x11-server-Xwayland mesa-dri-drivers dbus-x11

   # Arch Linux
   sudo pacman -S weston wayland-protocols pulseaudio pulseaudio-alsa \
                  xorg-xwayland mesa dbus
   ```

3. **Configure services**

   The setup script creates systemd user services for Weston and PulseAudio. You can also configure them manually:

   ```ini
   # ~/.config/systemd/user/weston.service
   [Unit]
   Description=Weston Wayland Compositor
   After=dbus.service

   [Service]
   Type=notify
   Environment=XDG_RUNTIME_DIR=/run/user/1000
   Environment=WAYLAND_DISPLAY=wayland-0
   ExecStart=/usr/bin/weston --logger-scopes=log,protocol
   Restart=on-failure

   [Install]
   WantedBy=default.target
   ```

4. **Set environment variables**

   Add to `~/.bashrc`:
   ```bash
   export XDG_RUNTIME_DIR=/run/user/1000
   export WAYLAND_DISPLAY=wayland-0
   export XDG_SESSION_TYPE=wayland
   export PULSE_SERVER=unix:/run/user/1000/pulse/native
   export GDK_BACKEND=wayland
   export QT_QPA_PLATFORM=wayland
   ```

5. **Test the setup**
   ```bash
   # Start services
   systemctl --user start weston pulseaudio
   
   # Install test applications
   sudo apt install gedit weston-terminal
   
   # Launch test application
   weston-terminal &
   ```

### Building a Custom Disk Image

For advanced users who want to create a pre-configured disk image:

```bash
# Create a raw disk image
qemu-img create -f raw linux-wslg.img 10G

# Install Linux using virt-install or similar
virt-install --name linux-wslg \
             --ram 4096 \
             --disk path=linux-wslg.img,format=raw \
             --cdrom ubuntu-22.04.iso \
             --os-variant ubuntu22.04

# After installation, boot and run setup script
# Then shutdown and use the image with krunkit
```

## Development Guidelines

### Code Style

- Follow Rust standard formatting: `cargo fmt`
- Run clippy for linting: `cargo clippy`
- Add tests for new functionality

### Testing

```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run specific test
cargo test test_name
```

### Pull Request Process

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests if applicable
5. Run `cargo fmt` and `cargo clippy`
6. Submit a pull request with a clear description

## Architecture

### Component Overview

```
krunkit (Rust binary)
├── cmdline.rs      - Command line argument parsing
├── context.rs      - VM context and execution
├── status.rs       - RESTful service for VM management
├── virtio.rs       - virtio device configuration
└── wslg.rs         - WSLg-like feature management

Guest VM
├── Weston          - Wayland compositor
├── PulseAudio      - Audio server
├── XWayland        - X11 compatibility layer
└── Linux Apps      - GUI applications
```

### Key Concepts

- **virtio devices**: Paravirtualized devices for efficient I/O
- **virtio-gpu**: Graphics acceleration and display
- **virtio-vsock**: Socket communication between host and guest
- **virtio-fs**: File system sharing
- **libkrun-efi**: Hypervisor and VM management library

## License

krunkit is licensed under Apache-2.0. See [LICENSE](LICENSE) for details.

## Questions?

- Open an issue for bugs or feature requests
- Check existing issues and pull requests first
- Be respectful and constructive in discussions

# WSLg vs krunkit: Feature Comparison

This document compares Microsoft's WSLg (Windows Subsystem for Linux GUI) with krunkit's WSLg-like implementation for macOS.

## Overview

| Aspect | WSLg (Windows) | krunkit (macOS) |
|--------|----------------|-----------------|
| **Host OS** | Windows 10/11 | macOS (Intel/Apple Silicon) |
| **Hypervisor** | Hyper-V | Hypervisor.framework + libkrun |
| **Display Protocol** | RDP (RAIL/VAIL) | Wayland/Weston + virtio-gpu |
| **Compositor** | Weston (RDP backend) | Weston (headless/virtio backend) |
| **Audio** | PulseAudio over RDP | PulseAudio over virtio-vsock |
| **GPU Acceleration** | WDDM virtualization | virtio-gpu with Metal/Venus |
| **Integration** | Windows Start Menu | macOS dock/launcher (planned) |

## Architecture Differences

### Display and Graphics

**WSLg:**
- Uses RDP protocol with VAIL (Virtualized Application Integrated Locally)
- Windows RDP client (mstsc) receives and composites windows
- Shared memory via virtio-fs for zero-copy rendering
- Native Windows window chrome

**krunkit:**
- Uses Wayland protocol with virtio-gpu
- Weston compositor runs in guest
- Graphics rendered via virtio-gpu to host
- macOS composites final output
- Future: Native macOS window integration

### Audio Architecture

**WSLg:**
- PulseAudio server in system distro
- Audio streams over RDP channel
- Integrated with Windows audio stack
- Bidirectional: microphone and speakers

**krunkit:**
- PulseAudio server in guest
- Audio forwarded via virtio-vsock sockets
- Integrates with macOS CoreAudio (planned)
- Bidirectional support

### System Distro

**WSLg:**
- Based on CBL-Mariner Linux
- Read-only mount
- Automatically paired with each user distro
- Contains Weston, PulseAudio, FreeRDP

**krunkit:**
- User-configurable distro (Ubuntu, Fedora, etc.)
- Standard read-write mount
- Single VM per instance
- User installs Weston, PulseAudio
- Host-side compositor for graphics display

## Feature Parity

### ✅ Supported Features

| Feature | WSLg | krunkit | Notes |
|---------|------|---------|-------|
| GUI Applications | ✅ | ✅ | Both support X11 and Wayland apps |
| GPU Acceleration | ✅ | ✅ | Both use virtualized GPU |
| Audio Output | ✅ | ✅ | Speaker/headphone support |
| Audio Input | ✅ | ✅ | Microphone support |
| File Sharing | ✅ | ✅ | WSLg uses virtio-fs, krunkit supports virtio-fs |
| Network | ✅ | ✅ | Both support networking |

### 🚧 Partial Support

| Feature | WSLg | krunkit | Status |
|---------|------|---------|--------|
| Clipboard | ✅ | 🚧 | krunkit: planned enhancement |
| Multi-monitor | ✅ | 🚧 | krunkit: single display currently |
| Window Integration | ✅ | 🚧 | krunkit: macOS integration planned |
| App Discovery | ✅ | 🚧 | krunkit: basic support implemented |

### ❌ WSLg-Specific Features

| Feature | Notes |
|---------|-------|
| Start Menu Integration | Windows-specific; macOS dock integration planned |
| Windows Notifications | Not applicable to macOS |
| WSL-specific commands | krunkit uses standard VM commands |

## Technical Implementation

### Rendering Pipeline

**WSLg:**
```
Linux App → Weston (RDP backend) → RDP Protocol → mstsc → Windows DWM → Display
```

**krunkit:**
```
Linux App → Weston (headless) → virtio-gpu → macOS WindowServer → Display
```

### Memory Sharing

**WSLg:**
- Uses virtio-fs for shared memory
- Zero-copy rendering with VAIL
- Efficient for high frame rates

**krunkit:**
- Uses virtio-gpu for graphics
- Memory-mapped buffers
- Efficient for GPU-accelerated content

### Process Model

**WSLg:**
```
Windows Host
├── WSL VM
│   ├── System Distro (Weston, PulseAudio)
│   └── User Distro (User apps)
└── mstsc (RDP client)
```

**krunkit:**
```
macOS Host
└── krunkit VM
    └── Linux Distro (Weston, PulseAudio, User apps)
```

## Configuration Comparison

### WSLg Configuration

```powershell
# WSLg is automatically enabled with WSL 2
# Disable via .wslconfig:
[wsl2]
guiApplications=false
```

### krunkit Configuration

```bash
# Enable GUI support
krunkit --wslg-gui --wslg-audio \
  --wslg-gpu-width 1920 --wslg-gpu-height 1080 \
  --cpus 4 --memory 4096 \
  --device virtio-blk,path=disk.img,format=raw
```

## Performance Characteristics

### Rendering Performance

**WSLg:**
- Optimized RDP with VAIL
- Near-native performance for 2D
- Good 3D with dGPU
- Some overhead with discrete GPUs at high frame rates

**krunkit:**
- virtio-gpu performance
- Depends on Metal translation
- Good for most GUI applications
- GPU-accelerated 3D rendering

### Resource Usage

**WSLg:**
- Minimal overhead (shared kernel)
- Efficient memory usage
- Integrated with Windows scheduler

**krunkit:**
- Full VM overhead
- Independent memory allocation
- macOS Hypervisor.framework scheduler

## Use Cases

### Best for WSLg
- Windows developers needing Linux tools
- Enterprise Windows environments
- DirectX/WDDM-based workflows
- Tight Windows integration required

### Best for krunkit
- macOS developers needing Linux environments
- Testing Linux applications on Mac
- Development workflows on Apple Silicon
- Standalone Linux VMs on macOS

## Migration Guide

### From WSLg to krunkit

1. **Export your WSL distro:**
   ```powershell
   wsl --export Ubuntu ubuntu.tar
   ```

2. **Convert to disk image:**
   ```bash
   # Create a raw disk image
   qemu-img create -f raw ubuntu.img 10G
   
   # Mount and extract
   # (requires additional steps to make bootable)
   ```

3. **Configure krunkit:**
   ```bash
   krunkit --wslg-gui --wslg-audio \
     --device virtio-blk,path=ubuntu.img,format=raw \
     --cpus 4 --memory 4096
   ```

## Future Roadmap

### krunkit Planned Features

- ✅ Basic WSLg-like functionality
- 🚧 macOS dock integration
- 🚧 Clipboard sharing
- 🚧 Multi-monitor support
- 🚧 Automatic app discovery
- 🚧 Native window chrome
- 🚧 Drag-and-drop between macOS and Linux

### Community Contributions

We welcome contributions to improve feature parity with WSLg! See [CONTRIBUTING.md](../CONTRIBUTING.md) for details.

## Conclusion

Both WSLg and krunkit provide excellent GUI application support for Linux on their respective host platforms. WSLg is more mature and deeply integrated with Windows, while krunkit brings similar functionality to macOS users with its own architectural approach optimized for macOS's technologies.

Choose based on your host platform:
- **Windows users**: Use WSLg (native, mature, well-integrated)
- **macOS users**: Use krunkit (native, growing, macOS-optimized)

## References

- [WSLg GitHub Repository](https://github.com/microsoft/wslg)
- [WSLg Architecture Overview](https://github.com/microsoft/wslg/blob/main/README.md)
- [krunkit Documentation](../README.md)
- [libkrun Project](https://github.com/containers/libkrun)
- [Wayland Protocol](https://wayland.freedesktop.org/)
- [Weston Compositor](https://gitlab.freedesktop.org/wayland/weston)

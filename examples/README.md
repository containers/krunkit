# krunkit Examples

This directory contains example configurations and scripts for running krunkit with various features.

## WSLg-like Features

### wslg-gui-example.sh

A complete example showing how to launch krunkit with WSLg-like GUI support for running Linux GUI applications on macOS.

**Features demonstrated:**
- GUI application support via `--wslg-gui`
- Audio support via `--wslg-audio`
- Custom GPU resolution
- Network connectivity
- Shared filesystem
- RESTful management API

**Usage:**
```bash
# Edit the script to set your VM image path
vim wslg-gui-example.sh

# Make it executable (if not already)
chmod +x wslg-gui-example.sh

# Run it
./wslg-gui-example.sh
```

**Prerequisites:**
1. A Linux disk image with GUI applications installed
2. Network backend running (gvproxy, vmnet-helper, etc.)
3. libkrun-efi installed on macOS
4. Guest VM configured with Weston and PulseAudio (use scripts/setup-wslg-guest.sh)

## Creating Your Own Examples

When creating custom krunkit configurations, consider these key aspects:

### 1. Resource Allocation
```bash
--cpus 4          # Number of vCPUs
--memory 4096     # RAM in MiB
```

### 2. Storage
```bash
--device virtio-blk,path=/path/to/disk.img,format=raw
--device virtio-blk,path=/path/to/data.qcow2,format=qcow2
```

### 3. Networking
```bash
# Unix datagram (gvproxy)
--device virtio-net,type=unixgram,path=/tmp/net.sock,mac=52:54:00:12:34:56

# Unix stream (passt)
--device virtio-net,type=unixstream,path=/tmp/net.sock,mac=52:54:00:12:34:56
```

### 4. GUI Support
```bash
--wslg-gui                    # Enable GUI
--wslg-audio                  # Enable audio
--wslg-gpu-width 1920         # GPU width
--wslg-gpu-height 1080        # GPU height
```

### 5. File Sharing
```bash
--device virtio-fs,sharedDir=/path/on/host,mountTag=sharename
```

### 6. Management
```bash
--restful-uri tcp://localhost:8081    # HTTP API
--pidfile /tmp/krunkit.pid            # PID file
--log-file /tmp/krunkit.log           # Log file
```

## Example Configurations

### Minimal VM
```bash
krunkit \
  --cpus 2 \
  --memory 2048 \
  --device virtio-blk,path=minimal.img,format=raw
```

### Development VM
```bash
krunkit \
  --cpus 8 \
  --memory 16384 \
  --device virtio-blk,path=dev.img,format=raw \
  --device virtio-net,type=unixgram,path=/tmp/net.sock,mac=52:54:00:12:34:56 \
  --device virtio-fs,sharedDir=$HOME/projects,mountTag=projects \
  --restful-uri tcp://localhost:8081
```

### GUI Workstation
```bash
krunkit \
  --cpus 8 \
  --memory 16384 \
  --wslg-gui \
  --wslg-audio \
  --wslg-gpu-width 2560 \
  --wslg-gpu-height 1440 \
  --device virtio-blk,path=workstation.img,format=raw \
  --device virtio-net,type=unixgram,path=/tmp/net.sock,mac=52:54:00:12:34:56 \
  --device virtio-fs,sharedDir=$HOME/shared,mountTag=shared \
  --restful-uri tcp://localhost:8081 \
  --log-file $HOME/krunkit-workstation.log
```

## Testing Your Configuration

1. **Start the VM:**
   ```bash
   ./your-example.sh
   ```

2. **Check status via RESTful API:**
   ```bash
   curl http://localhost:8081/vm/state
   ```

3. **Stop the VM:**
   ```bash
   curl -X POST http://localhost:8081/vm/state -d '{"state": "Stop"}'
   ```

## Troubleshooting

### VM won't start
- Check that all paths exist (disk images, network sockets, shared directories)
- Verify libkrun-efi is installed
- Check logs: `tail -f /tmp/krunkit.log`

### GUI applications don't appear
- Ensure `--wslg-gui` flag is set
- Verify Weston is running in guest: `ps aux | grep weston`
- Check environment variables in guest: `echo $WAYLAND_DISPLAY`

### No network connectivity
- Ensure network backend (gvproxy, vmnet-helper) is running
- Verify socket path is correct
- Check MAC address is unique

### Audio not working
- Ensure `--wslg-audio` flag is set
- Verify PulseAudio is running in guest: `ps aux | grep pulseaudio`
- Test with: `paplay /usr/share/sounds/alsa/Front_Center.wav`

## Additional Resources

- [Quick Start Guide](../docs/quickstart-wslg.md)
- [WSLg Features Documentation](../docs/wslg-like-features.md)
- [Complete Usage Guide](../docs/usage.md)
- [WSLg Comparison](../docs/wslg-comparison.md)

## Contributing Examples

Have a useful configuration? Submit a PR with:
1. The example script
2. A description of what it demonstrates
3. Any prerequisites or special setup needed
4. Expected behavior

See [CONTRIBUTING.md](../CONTRIBUTING.md) for guidelines.

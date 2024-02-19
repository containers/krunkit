# krunkit Command Line

`krunkit` can be used to create virtual machines (VM) using macOS virtualization framework and the `libkrun`
virtual machine monitor (VMM) library. The `libkrun` virtual machine configuration can be specified through
commadn line options.

Specifying VM bootloader configuration, vCPU, and RAM allocation is required. Not all device configurations are
required, but libkrun will at least expect a virtio-blk device to be used as the VM's root disk.

## Generic Options

- `--restful-URI`

The URI (address) of the RESTful service. The default is `tcp://localhost:8081`. `tcp` is the only valid scheme.

### Virtual Machine Resources

These options specify the number of vCPUs and amount of RAM made available to the VM.

- `--cpus`

Number of virtual CPUs (vCPU) available in the VM.

- `--memory`

Amount of memory available to the VM. The values is in MiB (mibibytes, 1024^3 bytes).

#### Example

This configures the krun VM to use two vCPUs and 2048 MiB of RAM:

```
--cpus 2 --memory 2048
```

## Device Configuration

Various virtio devices can be added to the libkrun VMs. They are all paravirtualized devices that can be specified
using the `--device` flag.

### Disk

#### Description

The `virtio-blk` option adds a disk to the VM. This disk is backed by an image file on the host machine. This file
is a raw image file.

#### Arguments

- `path`: The absolute path to the disk image file.

#### Example

This adds a virtio-blk device to the VM which will be backed by the raw image at `/Users/user/virtio-blk.img`:

```
--device virtio-blk,path=/Users/user/virtio-blk.img
```

### Networking

#### Description

The `virtio-net` option adds a network interface to the VM.

#### Arguments

- `unixSocketPath`: Path to a UNIX socket to attach to the guest network interface.
- `mac`: MAC address of the VM.

#### Example

This adds a virtio-net device to the VM and redirects all guest network traffic to the corresponding socket at
`/Users/user/virtio-net.sock` with a MAC address of `ff:ff:ff:ff:ff:ff`:

```
--device virtio-net,unixSocketPath=/Users/user/virtio-net.sock,mac=ff:ff:ff:ff:ff:ff
```

### Serial Port

#### Description

The `virtio-serial` option adds a serial device to the VM. This is useful to redirect text output from the virtual
machine to a log file.

#### Arguments

- `logFilePath`: Path to a file in which the VM serial port output should be written.

#### Example

This adds a virtio-serial device to the VM, and will log everything written to the device to
`/Users/user/virtio-serial.log`:

```
--device virtio-serial,logFilePath=/Users/user/virtio-serial.log
```

### Random Number Generator

#### Description

The `virtio-rng` option adds a random number generator device to the VM. The device will feed entropy from the
host to the VM.

#### Example

This adds a virtio-rnog device to the VM:

```
--device virtio-rng
```

### vsock

#### Description

The `virtio-vsock` option adds a vsock communication channel between the host and guest. macOS does not have host
support for `AF_VSOCK` sockets, so the vsock port will be exposed as a UNIX socket on the host.

Multiple instances of a `virtio-vsock` device can be specified, yet port numbers for these sockets must be unique.

#### Arguments

- `port`: `AF_VSOCK` port to connect to on the guest.
- `socketURL`: Path to the UNIX socket on the host.

#### Example

This adds a virtio-vsock device to the VM, and will forward all guest socket communication to
`/Users/user/virtio-vsock.sock` (the VM can connect to the vsock on port `1024`):

```
--device virtio-vsock,port=1024,socketURL=/Users/user/virtio-vsock.sock
```

### File Sharing

#### Description

The `virtio-fs` option allows a guest to share a file system directory with a host. The share can be mounted in
the guest with `mount -t virtiofs MOUNT_TAG /mnt`, with `MOUNT_TAG` corresponding to the mount tag specified in
the arguments.

#### Arguments

- `sharedDir`: Absolute path to the host directory to share with the guest.
- `mountTag`: Tag which will be used to mount the shared directory in the guest.

#### Example

This will share `/Users/user/virtio-fs` with the guest:

```
--device virtio-fs,sharedDir=/Users/user/virtio-fs,mountTag=MOUNT_TAG
```

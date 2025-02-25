# krunkit

`krunkit` is a tool to launch configurable virtual machines using the [libkrun](https://github.com/containers/libkrun) platform.

## Installation

`krunkit` relies on the `efi` flavor of `libkrun`. At present, `libkrun-efi` is only available on macOS. We provide a Homebrew repository to install `krunkit` and all of its dependencies, installable with:

```
$ brew tap slp/krunkit
$ brew install krunkit
```

## Building from source

As noted above, `krunkit` relies on the `efi` flavor of `libkrun`. Ensure that is installed on your system.

```
# If libkrun-efi.dylib is not located at /opt/homebrew/opt/libkrun-efi/lib/
# provide the path at which it's located using the LIBKRUN_EFI variable. Otherwise,
# the Makefile will default to using the /opt/homebrew/... path.
$ make LIBKRUN_EFI=<path to libkrun-efi.dylib>

$ sudo make install
```

## Usage

See [`docs/usage.md`](./docs/usage.md).

License: Apache-2.0

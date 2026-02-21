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

Build and install using default `PREFIX` (`/usr/local`):

```
make
sudo make install
```

To build with `libkrun-efi` from *Homebrew* or *MacPorts* use the appropriate `PREFIX`:

```
make PREFIX=/opt/homebrew
sudo make install PREFIX=/opt/homebrew
```

## Usage

See [`docs/usage.md`](./docs/usage.md).

License: Apache-2.0

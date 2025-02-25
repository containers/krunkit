OS = $(shell uname -s)
KRUNKIT_RELEASE = target/release/krunkit
KRUNKIT_DEBUG = target/debug/krunkit
KRUNKIT_HOMEBREW = /opt/homebrew/opt/libkrun-efi/lib/libkrun-efi.dylib

ifeq ($(PREFIX),)
    PREFIX := /usr/local
endif

.PHONY: install clean

all: $(KRUNKIT_RELEASE)

debug: $(KRUNKIT_DEBUG)

$(KRUNKIT_RELEASE):
	cargo build --release
ifeq ($(OS),Darwin)
ifneq ($(LIBKRUN_EFI),)
	install_name_tool -change $(KRUNKIT_HOMEBREW) $(LIBKRUN_EFI) $@
endif
	codesign --entitlements krunkit.entitlements --force -s - $@
endif

$(KRUNKIT_DEBUG):
	cargo build --debug

install: $(KRUNKIT_RELEASE)
	install -d $(DESTDIR)$(PREFIX)/bin
	install -m 755 $(KRUNKIT_RELEASE) $(DESTDIR)$(PREFIX)/bin

clean:
	cargo clean

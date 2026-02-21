OS = $(shell uname -s)
KRUNKIT_RELEASE = target/release/krunkit
KRUNKIT_DEBUG = target/debug/krunkit

PREFIX ?= /usr/local
export PREFIX

.PHONY: install clean $(KRUNKIT_RELEASE) $(KRUNKIT_DEBUG)

all: $(KRUNKIT_RELEASE)

debug: $(KRUNKIT_DEBUG)

$(KRUNKIT_RELEASE):
	cargo build --release
ifeq ($(OS),Darwin)
	codesign --entitlements krunkit.entitlements --force -s - $@
endif

$(KRUNKIT_DEBUG):
	cargo build --debug

install: $(KRUNKIT_RELEASE)
	install -d $(DESTDIR)$(PREFIX)/bin
	install -m 755 $(KRUNKIT_RELEASE) $(DESTDIR)$(PREFIX)/bin

clean:
	cargo clean

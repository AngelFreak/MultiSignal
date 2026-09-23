# Installs Signal Profiles for the current user (no root needed).
PREFIX ?= $(HOME)/.local
APP_ID = io.github.multisignal.MultiSignal

.PHONY: install uninstall

install:
	cargo build --release
	install -Dm755 target/release/multisignal $(PREFIX)/bin/multisignal
	install -Dm644 data/$(APP_ID).desktop $(PREFIX)/share/applications/$(APP_ID).desktop
	-update-desktop-database $(PREFIX)/share/applications 2>/dev/null

uninstall:
	rm -f $(PREFIX)/bin/multisignal $(PREFIX)/share/applications/$(APP_ID).desktop
	-update-desktop-database $(PREFIX)/share/applications 2>/dev/null

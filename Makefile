SHELL := /bin/sh
CARGO ?= $(if $(wildcard $(HOME)/.cargo/bin/cargo),$(HOME)/.cargo/bin/cargo,cargo)
PREFIX ?= /usr/local
DESTDIR ?=
VERSION := $(shell sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
ARCH := $(shell dpkg --print-architecture)
PACKAGE := dist/thugsrf_$(VERSION)_$(ARCH).deb

.PHONY: all build debug run demo check test fmt lint clean install uninstall deb deps toolchain addons help
all: build
build:
	$(CARGO) build --release --locked
debug:
	$(CARGO) build --locked
run:
	$(CARGO) run --locked --
demo:
	$(CARGO) run --locked -- tui --demo
check:
	$(CARGO) fmt --all -- --check
	$(CARGO) clippy --locked --all-targets -- -D warnings
	$(CARGO) test --locked
test: build
	$(CARGO) test --locked
	python3 scripts/smoke.py
	python3 scripts/tui-smoke.py
	python3 scripts/receiver-smoke.py
	python3 scripts/ai-smoke.py
fmt:
	$(CARGO) fmt --all
lint:
	$(CARGO) clippy --locked --all-targets -- -D warnings
install: build
	install -Dm755 target/release/thugsrf $(DESTDIR)$(PREFIX)/bin/thugsrf
	install -Dm644 docs/thugsrf.1 $(DESTDIR)$(PREFIX)/share/man/man1/thugsrf.1
	install -d $(DESTDIR)$(PREFIX)/share/thugsrf
	cp -R addons $(DESTDIR)$(PREFIX)/share/thugsrf/
	install -Dm644 assets/logo.txt $(DESTDIR)$(PREFIX)/share/thugsrf/logo.txt
uninstall:
	rm -f $(DESTDIR)$(PREFIX)/bin/thugsrf $(DESTDIR)$(PREFIX)/share/man/man1/thugsrf.1
# Stages a normal Debian binary package. Native architecture follows dpkg.
deb: build
	python3 scripts/package.py --version $(VERSION) --arch $(ARCH)
toolchain:
	sh scripts/toolchain.sh
deps:
	sudo apt-get update
	sudo apt-get install -y build-essential pkg-config curl ca-certificates dpkg-dev hackrf rtl-sdr rtl-433 alsa-utils python3
addons:
	python3 scripts/install-addons.py
clean:
	$(CARGO) clean
help:
	@echo 'build debug run demo check test fmt lint install deb deps toolchain addons clean'
	@echo 'Overrides: CARGO=... PREFIX=/usr DESTDIR=/tmp/stage'

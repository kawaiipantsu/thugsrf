# Installation

## Debian package

Download the amd64 `.deb` from [Releases](https://github.com/kawaiipantsu/thugsrf/releases), then install the downloaded file:

```sh
sudo apt install ./thugsrf_1.0.1_amd64.deb
thugsrf --version
thugsrf doctor
thugsrf addon install
thugsrf tui
```

Package installation resolves declared dependencies. Addon installation copies missing bundled modules into your own configuration without overwriting existing files. Existing modified addons are preserved during upgrades; installing the package alone does not replace those user copies.

## Build from source

```sh
git clone https://github.com/kawaiipantsu/thugsrf.git
cd thugsrf
make deps
make toolchain
make build
make check
make test
make deb
sudo apt install ./dist/thugsrf_1.0.1_amd64.deb
```

The pinned toolchain and Cargo.lock define reproducible dependency versions. `make install` uses `/usr/local` by default; `PREFIX=/usr` and `DESTDIR=/tmp/stage` support staging. Avoid leaving an older `/usr/local/bin/thugsrf` ahead of a packaged `/usr/bin/thugsrf`; check `type -a thugsrf`.

`make protocol-deps` installs optional larger decoder backends. Availability of a backend does not establish that a particular protocol can be decoded reliably; consult [[Protocol-Catalog]].

## Hardware

Connect HackRF over USB, attach an appropriate antenna, and run `thugsrf doctor`. Only one process should own the radio at a time. Use the distribution's device-access rules for an ordinary user; routine operation should not require running the TUI as root.

HackRF supports signed 8-bit IQ at 8–20 MS/s in this application. Nooelec NESDR uses the `rtl` device, unsigned IQ, and a default 2.4 MS/s rate. ALSA audio uses `audio` at 48 kHz by default. Hardware-free exploration uses `thugsrf tui --demo`.

See [[Troubleshooting]] for USB timeouts and source conflicts.

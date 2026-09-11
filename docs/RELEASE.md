THUGS(red) RF 0.1.0 introduces a native Linux radio investigation workbench by Kawaiipantsu from THUGS(red).

- Rust 1.98.1 / edition 2024; full Makefile build and Debian package.
- Responsive RGB TUI with braille spectrum, block waterfall, detection, recording, addon and settings panels.
- HackRF reception and finite replay; RTL-SDR reception; ALSA audio capture/playback.
- IQ/WAV analysis, OOK pulse extraction, FSK discriminator, basic AM/FM demodulation and OOK/AFSK generation.
- Executable JSON addons, including an offline rtl_433 adapter and frequency-context identifier.
- SQLite investigation history and JSON export.
- Explicit OpenAI, Anthropic and local-model requests for measured features and spectrum images.

Verified with the connected HackRF One: full-size passive capture, live TUI reception and persisted analysis. Audio capture, synthetic protocol/DSP tests, local AI HTTP contract, terminal layouts, and Debian packaging passed. See docs/VERIFICATION.md for limits. NESDR hardware, physical RF transmission and cloud inference were not exercised.

Install the Debian package with `sudo apt install ./thugsrf_0.1.0_amd64.deb`, then run `thugsrf doctor` and `thugsrf`. Press Space to start reception. Bundled addon examples are installed under `/usr/share/thugsrf/addons`; copy the desired directories to `~/.config/thugsrf/` and enable them in the Addons panel.

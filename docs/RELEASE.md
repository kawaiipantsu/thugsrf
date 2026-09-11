THUGS(red) RF 0.2.1 adds keyboard tuning and readable frequency input.

- Spectrum Left/Right tunes by 500 kHz by default; Up/Down and Page Up/Page Down tune by 10 MHz.
- Settings exposes fine_tune_hz and coarse_tune_hz. Both accept human-readable units, as does frequency: 443mhz, 145.252MHz, 12.5kHz, or integer Hz.
- The global --frequency CLI option accepts the same notation.
- Live tuning releases the previous receiver before restarting, clears stale plots, and validates hardware bounds. Stopped reception stays stopped. Keyboard changes affect the session; Settings saves startup defaults.
- GitHub Wiki provides the manual, guides, worked examples, screenshots, configuration reference and command help.

Validation: exact unit parsing and atomic config validation tests, terminal layout checks, strict Clippy, and a real PTY with a fake HackRF verifying every tuning key, custom steps, stopped/live behavior and exclusive device access. No RF transmission performed.

Install the Debian package and restart the TUI. Existing configurations gain default tuning steps automatically.

THUGS(red) RF 1.0.0 lets Spectrum and live audio run together for the first time, and marks the project's first stable release.

- Spectrum and Listen no longer fight over the radio. Press `a` on the Spectrum tab (or Space on Listen) to start hearing the tuned frequency while the waterfall keeps updating — there is exactly one receiver process underneath, feeding the display, live decoders and demodulated audio at once.
- Tuning while listening is no longer blocked. Arrow/PgUp/PgDn keys, typed frequency entry, click-to-tune and channel/preset selection all retune live; audio follows the new frequency automatically, with only the same brief gap the spectrum display already shows while the receiver restarts.
- Stopping audio leaves Spectrum running untouched, and stopping the receiver now cleanly stops audio with it instead of leaving it silently stalled. Mode/bandwidth changes (`m`/`b`) reconfigure the live audio path in place without disturbing reception.
- Headless `thugsrf listen`/`thugsrf talk` are unchanged — they still open their own receiver exactly as before.

This is a breaking change to internal `Stream`/`Audio` APIs, not to the CLI or saved configuration: no settings migration is needed, and existing addons, channels and presets keep working as-is. The version moves to 1.0.0 to mark this as a stable baseline going forward.

Validation: Rust unit/layout tests and strict Clippy; a PTY tuning test extended with a simulated audio sink proves a single receiver process handles Spectrum, decoders and audio together, that audio survives a live retune, and that toggling it off/on never restarts or duplicates the receiver; existing CLI/audio/protocol/decoder regressions. These changes were not validated with over-the-air reception; no RF transmission was performed.

Install the Debian package and restart the TUI. Existing settings, channels and addon activation are preserved; no configuration changes are required to use concurrent Spectrum + Listen.

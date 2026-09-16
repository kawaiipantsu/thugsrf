THUGS(red) RF 1.0.2 fixes an RX front-end gap found while chasing a "noise instead of music" WFM report, and makes gain settings fully controllable.

- The receive path (Spectrum, Listen, `record`, `scan`, `listen`/`talk`) never explicitly commanded the HackRF's RF amplifier state, leaving it at whatever a previous tool or session left it in. It's now explicit every time, and controllable: a new `amp_enable` setting (off by default, editable in Settings or via `thugsrf config set amp_enable true`) replaces the implicit, undefined prior state. HackRF's own LNA/VGA gains already provide RX gain; the amp mainly matters for weak/distant signals and otherwise increases overload risk.
- `lna_gain`'s default moves from 16 to 32 dB for new installations, matching common HackRF reception guidance (existing saved configurations are untouched).
- Fixed a live-audio correctness bug: a read from the IQ pipe could occasionally return an odd number of bytes; the trailing byte was silently dropped instead of carried into the next read, permanently desyncing every I/Q pair for the rest of the session.
- Removed three more leftover "spectrum held"/"tuning blocked while listening" descriptions in `docs/RADIO.md` that 1.0.0 missed.

If you're chasing noisy or garbled audio on a real station: try `thugsrf config set amp_enable false` explicitly (now the default) and `thugsrf config set lna_gain 32`, then retune. If a signal is very weak, `amp_enable true` adds roughly 14 dB at the cost of dynamic range on strong nearby signals.

Validation: `cargo test`, strict Clippy, and the full smoke suite all pass. The amp/gain changes were checked against a real HackRF with real over-the-air FM broadcast reception (captured IQ, confirmed a genuine strong signal via spectrum analysis, compared output statistics). No RF transmission was performed.

Install the Debian package and restart the TUI. No configuration changes are required; `amp_enable` appears with its default value on existing configurations automatically.

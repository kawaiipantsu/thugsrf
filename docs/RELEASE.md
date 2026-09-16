THUGS(red) RF 1.0.1 is a bugfix follow-up to 1.0.0's concurrent Spectrum + Listen.

- The live audio worker's shutdown could raise an unhandled `subprocess.TimeoutExpired` if a child (the ALSA sink) took longer than 10 seconds to exit, crashing the whole session with a raw Python traceback ("exit 1") instead of stopping cleanly. It now force-kills a slow-to-exit child and continues instead of crashing.
- Audio failures now also persist a timestamped copy of the full diagnostics to `~/.config/thugsrf/logs/audio-<timestamp>.log` (kept across future runs; the newest 20 are retained), in addition to the existing in-session Workbench display. Earlier, a later successful or failed run silently overwrote the one working diagnostics file, so a transient failure could become impossible to find afterward.
- Removed two leftover "spectrum held while listening" status strings in the TUI (Spectrum tab status line and sidebar) that were never updated when 1.0.0 stopped holding the spectrum display during listening.
- If you see `RuntimeError: hackrf_transfer exited 1` with "Couldn't transfer any bytes for one second" in the Workbench diagnostics after pressing `a`, that specific message means the audio worker opened its own receiver instead of tapping the already-running one — a symptom of running a build older than 1.0.0's concurrent-listening change. Confirm with `thugsrf --version` and `type -a thugsrf` (an older `/usr/local/bin/thugsrf` ahead of a packaged `/usr/bin/thugsrf` on PATH is the usual cause).

Validation: `cargo test`, strict Clippy, and the full smoke suite (including the PTY concurrent-listening scenario from 1.0.0) all pass unchanged. These changes were not validated with over-the-air reception; no RF transmission was performed.

Install the Debian package and restart the TUI. No configuration changes are required.

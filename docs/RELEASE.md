THUGS(red) RF 0.3.0 adds live multi-decoder output and interactive Spectrum inspection.

- Enable compatible decoders in Addons and press `d` for their shared rolling console. Up to three run concurrently; results include decoder, capture time and tuned frequency. `D` pauses/resumes, `s` toggles timestamped log saving, and `x` clears the display.
- RDS decoding works on saved IQ/MPX, as a live Spectrum addon, and continuously during WFM listening. Station names, PI, RadioText, programme type and traffic status use the optional redsea backend.
- Spectrum supports Enter-to-tune with readable units, mouse peak selection, `t` to tune a selected peak, brackets/mouse-wheel zoom, and `0` for full span.
- `m` selects NFM/FM/WFM/AM presets; `b` edits receive bandwidth using units. `a` starts/stops listening. The graph marks receive filter edges.
- `c` cycles waterfall palettes; `+`/`-` adjust its display threshold. `f` cycles FFT resolution through 2048, 8192, 32768 and 65536 bins. New configurations default to 8192; narrow peaks survive display reduction.
- Spectrum `s` exports separate ASCII signal-graph and waterfall files at one column per visible FFT bin. Decoder Console `s` instead writes decoder-output-<timestamp>.log. Files go under the thugsrf config directory and are never overwritten.
- Wideband Survey gains frequency labels and vertical dividers that adapt to terminal width. The Wiki documents all controls, decoder behavior, logs, exports and installation.

Live addons use bounded contiguous snapshots, not lossless persistent protocol sessions: slow decoders can skip windows and boundary-spanning messages can be missed. Audio listening pauses the Spectrum addon feed, while continuous WFM RDS remains available. Larger FFTs use more CPU and memory. Redsea is optional and must be installed separately; WFM audio still works without it.

Validation: Rust unit/layout tests and strict Clippy; actual PTY tests with simulated receivers for tuning, mouse interaction, FFT changes, native-bin ASCII exports, multi-decoder output, retuning, log start/stop, cancellation and child cleanup; CLI/audio/protocol regressions; actual redsea decoding of a known MPX fixture and generated signed/unsigned FM IQ. These changes were not validated with over-the-air reception; no RF transmission was performed.

Install the Debian package, run `thugsrf addon install` to add the new RDS addon without overwriting existing user files, and restart the TUI. Existing settings and addon activation are preserved. Press `f` to increase FFT resolution in an existing configuration, or save `fft_size = 8192` in Settings.

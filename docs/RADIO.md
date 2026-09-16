# Survey, listening and channels

**7 Survey** uses `hackrf_sweep` to assemble a sequential panorama, with latest measurements and peak hold. Space starts/stops; **s** saves the panorama to SQLite findings, including the pass in which each bin was last measured. Settings controls `sweep_start_mhz`, `sweep_end_mhz`, and `sweep_bin_hz`; defaults are 1–6000 MHz and 1 MHz bins. Coverage describes the current pass. Unvisited bins retain earlier values; a panorama is not simultaneous full-span reception or a wideband IQ recording. HackRF has at most about 20 MHz instantaneous bandwidth. Narrow/short-lived signals can be missed by coarse or sequential sweeps. Sweep levels are relative, not calibrated dBm. Choose a frequency and return to Spectrum for detailed recording/decoding.

**8 Listen** supplies broadcast FM, upper MW and SW tuning presets. Up/down selects, Enter tunes, Space starts/stops audio. Change `frequency`, `listen_mode`, `listen_bandwidth`, `squelch_dbfs` and `audio_device` through Settings. Presets are starting frequencies, not claims that stations are currently on air. Broadcast FM is mono with European 50 µs de-emphasis; no stereo decoding. RDS station name, PI, programme type, RadioText and traffic announcements appear live when `redsea` is installed. RDS is decoded before audio filtering and de-emphasis. Narrow FM and AM use stateful channel/audio filtering and automatic audio gain. The audio worker uses bounded NumPy/SciPy blocks behind the Rust CLI. Diagnostic output is in `~/.local/share/thugsrf/audio.log` and errors appear in Workbench.

```sh
thugsrf listen --frequency 100000000 --mode wfm --bandwidth 200000
thugsrf listen --frequency 6070000 --mode am --bandwidth 10000
thugsrf listen --frequency 145500000 --mode fm --bandwidth 12500
thugsrf --device rtl listen --frequency 100000000 --mode wfm --bandwidth 200000
```

Listening sessions are bounded to one hour; `--seconds` shortens this. Space/Ctrl-C stops early. ALSA output uses the configured device. HackRF supports upper MW from roughly 1 MHz; lower MW needs an upconverter. NESDR requires an HF upconverter for MW/SW; this release has no automatic converter-offset setting. Tune to the translated frequency manually. SSB, synchronous AM and automatic repeater scanning are not implemented.

**9 VHF/UHF** lists local channels/repeaters with RX, TX, CTCSS, mode and source. Enter tunes RX; Space listens. `a` prepares an editable add command; `e` prepares a field edit. `t` prepares a ten-second microphone TX command for the selected entry; append `--confirm-tx` and press Enter to transmit. It is finite half-duplex voice, not duplex or hold-to-talk. Receiver, sweep and audio workers release the radio before another operation starts.

```sh
thugsrf channels
thugsrf channels add 'My repeater' --rx 145600000 --tx 145000000 --ctcss 77 --source 'My verified local directory'
thugsrf channels set 3 ctcss_hz 77
thugsrf channels remove 3
# Deliberate finite transmission on your selected channel:
thugsrf talk --frequency 145000000 --mode fm --ctcss 77 --seconds 10 --confirm-tx
```

The example repeater above is a configuration example, not a verified Danish station. The default directory contains two clearly labelled simplex presets. Edit `~/.config/thugsrf/repeaters.toml` to change existing entries; add/set/remove commands also work from the TUI command bar. `tx_hz` is optional. CTCSS generates a TX sub-audible tone; RX currently uses power squelch, not CTCSS decoding/gating. Digital voice transmission and DCS are not provided. Microphone AM/NFM TX is limited to 60 seconds, RF amplifier off and TX gain 0. One ALSA device setting supplies both input and output. Physical RF TX has not been tested; generated IQ and process handling are tested with fake devices.

Danish repeater references: [OZ1LN maps and lists](https://www.oz1ln.dk/kort_og_lister/). [RepeaterBook's API](https://www.repeaterbook.com/wiki/doku.php?id=api) requires approved application access and user credentials; no bulk directory has been scraped or bundled.

**1 Spectrum:** Left/Right tunes down/up by `fine_tune_hz` (default 500 kHz). Up/Down and Page Up/Page Down tune up/down by `coarse_tune_hz` (default 10 MHz). Enter opens direct frequency input. Both steps are editable in Settings, accepting units such as `12.5kHz` or `10MHz`. The frequency field also accepts `443mhz` or `145.252MHz`. Keyboard changes are session-only; save startup defaults through Settings. Live tuning releases and restarts the receiver and clears stale plots; an active listening session keeps running and simply follows the new frequency once the brief restart completes. Tuning is blocked while another job or survey holds the source. Audio input has no RF tuning.

Spectrum inspection: `]` zooms in, `[` zooms out, and `0` restores the full captured span. The mouse wheel also zooms. Click the trace or waterfall to select the strongest FFT bin in that terminal column; zoom follows the selected frequency. `t` tunes the receiver to the selection and `l` searches local frequency references. The marker reports frequency, uncalibrated dBFS, SNR and (when detected) approximate peak bandwidth. Frequency-band context is a candidate, not identification of a transmitter or protocol.

`m` cycles NFM (12.5 kHz), FM (50 kHz), WFM (200 kHz) and AM (10 kHz) receive presets. `b` accepts a custom receive filter width such as `12.5kHz`, from 3 to 200 kHz. The filter edges are marked on the spectrum. `a` starts/stops listening in Spectrum, running alongside the live plot rather than pausing it; Space independently starts/stops the receiver itself. Mode and bandwidth changes restart active listening only, not the receiver. Zoom changes the displayed span; it does not change the radio sample rate or improve the underlying FFT resolution.

`f` cycles 2048, 8192, 32768 and 65536 FFT bins, restarting active spectrum reception and clearing old waterfall rows. New configurations default to 8192 bins; existing configurations retain their setting. Higher FFT sizes resolve closer frequencies but use more CPU and memory. The waterfall retains the strongest bin in each terminal column so thin peaks are not skipped, with two time samples per character row. Zoom reveals the additional frequency detail; terminal size still limits the displayed pixels. `c` cycles palettes and `+`/`-` raise/lower the display threshold in 5 dB steps. These controls are session-only; Settings saves defaults.

RDS recording decoding:

```sh
thugsrf decode station.cs8 --mode rds --sample-rate 8000000
thugsrf --device rtl --sample-rate 1000000 decode station.cu8 --format cu8 --mode rds
thugsrf decode multiplex.wav --format wav --mode rds
```

IQ must be centered on the FM station and supplied with its original sample rate. WAV must contain mono 16-bit FM multiplex at 128 kHz or higher; ordinary 44.1/48 kHz audio recordings cannot retain RDS. The decoder processes up to 120 seconds and returns up to 2000 decoded groups as JSON, saved to investigation history. Empty results mean no RDS was recovered. The CLI `listen --mode wfm` includes the latest RDS summary when listening ends; the TUI displays it as it arrives.

Install the optional [redsea decoder](https://github.com/windytan/redsea) using its upstream build instructions, for example:

```sh
sudo apt-get install git build-essential meson libsndfile1-dev libliquid-dev nlohmann-json3-dev
git clone --depth 1 --branch v1.3.0 https://github.com/windytan/redsea.git
meson setup redsea/build redsea --wrap-mode=nodownload
meson compile -C redsea/build
sudo install -m755 redsea/build/redsea /usr/local/bin/redsea
```

`thugsrf doctor` checks decoder availability. Without redsea, WFM audio continues and the RDS panel reports the missing dependency. Input conventions follow [redsea's MPX documentation](https://github.com/windytan/redsea/wiki/Input-formats).

Press `s` in Spectrum to save `spectrum-<timestamp>.txt` and `waterfall-<timestamp>.txt` in the thugsrf config directory. Exports use one column per FFT bin in the current zoomed view, up to 65536 columns, rather than the terminal width. The graph has 131 power rows at 1 dB per row; the waterfall preserves every stored time row using an ASCII intensity ramp. Headers give the frequency of the first/last columns, Hz per column, FFT size and power scales. Use a monospace editor with line wrapping disabled. Existing files are never overwritten. Press `s` in Detections to save the analysis to SQLite instead.

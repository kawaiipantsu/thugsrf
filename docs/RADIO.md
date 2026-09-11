# Survey, listening and channels

**7 Survey** uses `hackrf_sweep` to assemble a sequential panorama, with latest measurements and peak hold. Space starts/stops; **s** saves the panorama to SQLite findings, including the pass in which each bin was last measured. Settings controls `sweep_start_mhz`, `sweep_end_mhz`, and `sweep_bin_hz`; defaults are 1–6000 MHz and 1 MHz bins. Coverage describes the current pass. Unvisited bins retain earlier values; a panorama is not simultaneous full-span reception or a wideband IQ recording. HackRF has at most about 20 MHz instantaneous bandwidth. Narrow/short-lived signals can be missed by coarse or sequential sweeps. Sweep levels are relative, not calibrated dBm. Choose a frequency and return to Spectrum for detailed recording/decoding.

**8 Listen** supplies broadcast FM, upper MW and SW tuning presets. Up/down selects, Enter tunes, Space starts/stops audio. Change `frequency`, `listen_mode`, `listen_bandwidth`, `squelch_dbfs` and `audio_device` through Settings. Presets are starting frequencies, not claims that stations are currently on air. Broadcast FM is mono with European 50 µs de-emphasis; no stereo/RDS. Narrow FM and AM use stateful channel/audio filtering and automatic audio gain. The audio worker uses bounded NumPy/SciPy blocks behind the Rust CLI. Diagnostic output is in `~/.local/share/thugsrf/audio.log` and errors appear in Workbench.

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

<p align="center"><img src="assets/banner.png" alt="THUGS(red) RF — capture, analyze, replay, research" width="100%"></p>

# THUGS(red) RF

**A native Linux signal intelligence workbench.** Capture radio signals, inspect spectra and waterfalls, extract pulse timings, run protocol decoders, and keep an investigation history from a full-color terminal.

Built by **Kawaiipantsu** from **[THUGS(red)](https://thugs.red)**, a Danish hacking community.

Rust **1.98.1**, edition 2024 · Ratatui · threaded reception and DSP · SQLite · MIT

## Build and install

```sh
make deps
make toolchain                   # official rustup installer / pinned stable compiler
make build
make check
make test
make deb                         # dist/thugsrf_0.1.0_<arch>.deb
sudo apt install ./dist/thugsrf_0.1.0_amd64.deb
thugsrf doctor
thugsrf
```

Alternatively, `sudo make install` installs under `/usr/local`. `PREFIX` and `DESTDIR` are supported. `make demo` opens a synthetic source; press **Space** to start it. `make addons` copies example addons into your user configuration without overwriting existing modules. Dependencies are locked in `Cargo.lock`.

Debian packages are built for the native architecture and target Debian 13 / Ubuntu 24.04 or later. There are no build-time SDR SDK requirements: maintained `hackrf_transfer`, `rtl_sdr`, and ALSA utilities provide the hardware transport. SQLite and TLS are compiled into the Rust application. The distribution packages install USB access rules. For non-root reception, check your distribution's HackRF/RTL-SDR udev group membership and reconnect the device after installation.

## Terminal workbench

Actual xterm screenshots of the running application. The spectrum views use the built-in **synthetic demo**, without opening a radio.

**170 × 50 — spectrum, waterfall, and receiver sidebar**

[![THUGS(red) RF running in a 170 by 50 terminal with a braille spectrum, color waterfall, and receiver sidebar](assets/screenshots/spectrum-wide.png)](assets/screenshots/spectrum-wide.png)

<details>
<summary>See the compact 80 × 24 layout and CLI commands</summary>

**80 × 24 — compact spectrum and waterfall**

![THUGS(red) RF running in an 80 by 24 terminal](assets/screenshots/spectrum-compact.png)

**Command-line interface — `thugsrf --help`**

![THUGS(red) RF command-line help in a terminal](assets/screenshots/cli-help.png)

</details>

The UI fits **80 × 24**, expands with the terminal, and adds a receiver sidebar at larger widths. **170 × 50** is recommended. It uses RGB color, box drawing, a braille spectrum, and two waterfall rows per terminal cell.

| Key | Action |
| --- | --- |
| Space | Start/stop passive reception |
| Tab / Shift-Tab / 1–6 | Switch panels |
| p | Freeze display while continuing to drain the receiver |
| s | Save spectrum and detections to SQLite |
| r | Prepare a finite recording command |
| : | Run a CLI command in the workbench |
| ↑↓, Enter | Select/edit settings or toggle a reviewed addon |
| PgUp / PgDn | Scroll workbench output |
| ? / q | Help / quit |

The command bar supports quoted paths. Jobs execute off the UI thread and stop live reception first to release the radio. The TUI never starts RF transmission on launch. `replay` needs `--confirm-tx` on every invocation.

## Hardware and capture

```sh
thugsrf doctor
thugsrf --frequency 433920000 record signal.cs8 --seconds 5
thugsrf analyze signal.cs8 --png spectrum.png
thugsrf --device rtl --sample-rate 2400000 record signal.cu8 --seconds 5
thugsrf --device audio record microphone.wav --seconds 5
thugsrf --device audio tui
thugsrf scan --start 433000000 --end 435000000 --step 1000000 --seconds 1
```

| Source | Input / output | Supported behavior |
| --- | --- | --- |
| HackRF One | Signed 8-bit interleaved IQ (`cs8`) | RX, finite recording, finite replay; 0–6 GHz driver tuning; 8–20 MS/s |
| Nooelec NESDR SMArTee v2 / RTL-SDR | Unsigned 8-bit interleaved IQ (`cu8`) | RX and recording; 24–1766 MHz tuning; 2.4 MS/s default; receive-only |
| ALSA sound card | S16 mono capture / WAV files | Live audio spectrum, recording, WAV playback, AFSK generation |
| Demo | Synthetic complex samples | UI exploration without hardware |

Rates and frequencies are in **Hz**, gains in dB (the `rtl_gain` setting uses tenths of a dB). Hardware limits still depend on the tuner, USB controller and firmware. HackRF's RF amplifier remains off by default. Its 8–20 MS/s operating range follows [HackRF's sampling/filter guidance](https://hackrf.readthedocs.io/en/stable/sampling_rate.html). The NESDR SMArTee has a powered bias tee: use compatible antennas/accessories.

Recordings never silently overwrite files. RF recordings receive a SigMF-style `.sigmf-meta` sidecar; the raw recording retains your chosen filename. Audio recordings receive a `.wav.json` metadata sidecar, described in `docs/ARCHITECTURE.md`. A finite scan retains each capture and stores its report. Scan steps are center frequencies; overlapping captures are expected when the step is smaller than sample rate.

## Analyze, decode, encode, replay

```sh
thugsrf decode signal.cs8 --mode ook
thugsrf decode signal.cs8 --mode fsk
thugsrf demod signal.cs8 audio.wav --mode fm
thugsrf play audio.wav
thugsrf encode test.cs8 --mode ook --bits 10110010 --rate 8000000 --baud 1000
thugsrf encode test.wav --mode afsk --bits 10110010 --rate 48000 --baud 1200
# A deliberate finite transmission in your authorized RF test setup:
thugsrf --frequency 433920000 replay test.cs8 --confirm-tx --gain 0
thugsrf history
thugsrf history --export 1 > investigation.json
```

Built-in analysis includes Hann-window FFT, DC removal, power spectra, median noise estimates, threshold-based peaks, approximate occupied bandwidth, RMS and crest factor. Frequency hints are **context, not protocol identification**. dBFS is uncalibrated; it is not received dBm. WAV analysis downmixes channels and uses the file's sample rate. Real audio shows a mirrored two-sided spectrum.

The built-in OOK decoder extracts envelope pulse durations; the FSK decoder extracts instantaneous frequency. Neither pretends to recover synchronized protocol packets. The **rtl433 addon** runs the installed [rtl_433](https://github.com/merbanan/rtl_433) decoder against recorded IQ for supported sensor protocols. No events is a valid result. AM/FM demodulation is a basic centered-carrier implementation, with a simple audio filter and a bounded input window. OOK and AFSK encoding generates symbols, not complete protocol framing/checksums.

## Addons and signal OSINT

```sh
make addons
thugsrf addon list
# Review addons before enabling: they execute with your user permissions.
thugsrf addon toggle rtl433
thugsrf addon run rtl433 signal.cs8
thugsrf addon toggle band-context
thugsrf addon run band-context signal.cs8
```

Addons live in `~/.config/thugsrf/decoders/<name>/` and `~/.config/thugsrf/identifiers/<name>/`. Each has an `addon.toml` manifest and an executable command. A versioned JSON request arrives on stdin; one JSON object leaves stdout. Rust, Go, Python and other languages all work. Addons have execution deadlines and a 1 MiB output limit. They are trusted local programs, **not sandboxed plugins**.

Included examples: OOK pulse analysis, rtl_433 offline decoding, and European frequency-context candidates with an EFIS reference. The latter is an offline aid; this release does not query live allocation registries or identify transmitter owners. See [the addon guide](docs/ADDONS.md) for the contract and extension points.

## OpenAI, Anthropic and local LLMs

Set `ai_provider` (`openai`, `anthropic`, `local`) and `ai_model` in the Settings panel or configuration file. Model names are configurable because availability and capabilities vary. Local servers use an OpenAI-compatible `/v1/chat/completions` endpoint (Ollama, llama.cpp or similar); set `local_url` to its `/v1` base URL.

```sh
thugsrf config set ai_provider local
thugsrf config set ai_model YOUR_LOCAL_MODEL
thugsrf config set local_url http://127.0.0.1:11434/v1
thugsrf ai --input microphone.wav --format wav
thugsrf ai --image spectrum.png --question 'What modulation candidates fit this spectrum?'
```

Cloud credentials come from `OPENAI_API_KEY` or `ANTHROPIC_API_KEY`; authenticated local servers may use `THUGSRF_LOCAL_API_KEY`. Credentials are never written to TOML or SQLite. AI requests only occur when you invoke `ai`. WAV and IQ inputs produce measured features locally; **raw audio is not uploaded or directly listened to by the model**. PNG/JPEG graphs are sent as image inputs and require a vision-capable model. Combine `--input` and `--image` to supply measurements and a graph together.

OpenAI uses the [Responses API](https://platform.openai.com/docs/api-reference/responses/create) with `store: false`; Anthropic uses Messages; local endpoints use Chat Completions. AI conclusions are stored separately as hypotheses. TLS is required except for loopback local servers. There is no automatic web browsing or autonomous RF action.

## Configuration and data

- `~/.config/thugsrf/config.toml`: editable, validated device, DSP and provider settings.
- `~/.config/thugsrf/{decoders,identifiers}/`: user addons.
- `~/.local/share/thugsrf/recordings/`: TUI and scan recordings.
- `~/.local/share/thugsrf/investigations.sqlite3`: reports, decoder findings and AI hypotheses; WAL mode.

`XDG_CONFIG_HOME` and `XDG_DATA_HOME` are honored. All settings can be edited in the TUI. `thugsrf config` prints the current configuration. Use `--device rtl` or `--device audio` to apply an appropriate default rate without changing persisted settings; changing device through Settings or `config set` resets its sample rate to a suitable default. When editing TOML manually, change device and rate together.

See [architecture and limits](docs/ARCHITECTURE.md), [addon development](docs/ADDONS.md), and [verification](docs/VERIFICATION.md). This is an initial working release with explicitly bounded analysis, not a universal protocol decoder. The GitHub banner uses the top section of the supplied artwork. The four lower logo variants are available separately in [the branding assets](assets/README.md), alongside the preserved original identity sheet; the terminal adapts its red/black palette and wordmark.

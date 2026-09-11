# Verification — initial 0.1.0 release

Verified on Debian GNU/Linux 13 (amd64), 2026-09-11, using Rust 1.98.1 and the locked dependencies. No RF transmission was performed during verification.

## Automated

- `make check`: rustfmt, Clippy with warnings denied, and unit tests for FFT tone frequency/power, signed/unsigned IQ conversion, silent input, provider request shapes, and all TUI panels at 80×24 / 170×50 / undersized terminals.
- `scripts/smoke.py`: actual CLI invocation; configuration validation; tone analysis; spectrum PNG; SQLite round-trip/export; OOK generation and pulse timing round-trip; AFSK WAV; WAV analysis; AM/FM output format; reviewed addon enable/run; malformed addon and timeout behavior; refusal to replay without the explicit TX flag.
- `scripts/tui-smoke.py`: actual PTY interaction at 80×24 and 170×50, demo streaming, all six panels, report save, clean exit, and terminal mode restoration.
- `scripts/ai-smoke.py`: actual local HTTP request to a loopback stub, configurable model, WAV feature submission and response parsing. No external AI call or credential required.
- `make deb`: native Debian package with shared-library dependencies derived by `dpkg-shlibdeps`.

The network sandbox prevents binding a loopback socket; the AI stub test was run outside it. Tests use temporary XDG directories and never replace user configuration.

## Connected hardware

HackRF One was detected with host/libhackrf 2024.02.1 and firmware 2021.03.1 (API 1.04). Firmware was left unchanged.

`scripts/hardware-smoke.py` verified:

1. A one-second passive capture at 433.92 MHz and 8 MS/s produced exactly **16,000,000 bytes** of signed 8-bit IQ.
2. The capture passed FFT analysis and investigation persistence.
3. Live HackRF USB reception fed the DSP/TUI; a frame with source `hackrf` was saved into SQLite, and the receiver stopped cleanly.
4. ALSA captured a one-second **48 kHz** mono WAV (**96,044 bytes**) and the application analyzed its **48,000 samples**.
5. WAV playback succeeded through the silent ALSA `null` output.

The smoke recordings stayed in temporary local directories and were removed after the checks. No captured RF/audio payloads are committed to Git.

## Not physically verified

- NESDR SMArTee v2: no RTL-SDR device was attached. The `rtl_sdr` transport and unsigned-IQ path are implemented; physical reception still needs this device.
- HackRF RF replay: command validation and the TX guard were tested; no signal was radiated.
- Audible speaker output: the host lists playback hardware; playback was exercised with the silent null sink.
- Cloud OpenAI/Anthropic and a real local model: request formats are tested; no provider credentials/model were supplied for an end-to-end inference call.
- rtl_433: the real installed offline decoder was exercised on a synthetic IQ fixture. A supported real sensor packet was not present, so successful over-the-air protocol recovery is not claimed.

This verification does not assert universal protocol recognition, calibrated signal levels, lossless real-time analysis at every sample rate, or compatibility with every Linux distribution.

## 0.1.1 receiver fix

On 2026-09-11, a direct HackRF test reproduced an intermittent startup failure: `Couldn't transfer any bytes for one second.` The first of three attempts failed, and the next two delivered samples. The two-line TUI status previously hid this diagnostic beneath the startup messages.

The live receiver now retries only that specific failure before any data has arrived, up to three attempts, and displays final errors with exit status in the Workbench. `scripts/receiver-smoke.py` tests recovery, exhaustion, busy-device failure and failure after samples arrive using a fake child process in an actual PTY. Three successive physical HackRF start/save/stop cycles also passed. No firmware change was made; this release handles the observed transient startup condition rather than claiming its underlying USB/firmware cause has been eliminated.

## 0.2.0 protocol, survey and audio expansion

`make check` and `make test` pass, including all nine TUI panels at 80×24 and 170×50. New offline checks cover a known ADS-B callsign and PPM fixture, checksum-valid NMEA, independently generated AX.25, POCSAG BCH/parity/alpha audio decoded by multimon-ng, BLE advertising CRC/name recovery and Wireshark rejection of a corrupt CRC, empty PCAP negatives, waveform-identifier paths and source-preserving frequency imports.

Fake-device audio tests recover a 1 kHz tone through live AM/NFM/WFM demodulation, verify finite AM/FM microphone IQ, reject unconfirmed TX and surface child-process errors. The actual connected HackRF completed a passive 1–6000 MHz sweep in approximately 0.75 seconds at 1 MHz bin resolution, and a two-second 100 MHz WFM capture streamed through the audio DSP into ALSA `null` successfully. The Survey README screenshot is an actual passive measurement. No RF transmission or private decoded payload was recorded for publication.

Satellite, GSM, digital voice, marine ATIS and GPS L1 backend integrations still need clean protocol-reference captures for end-to-end validation; see the capability matrix. Source-import parsing was tested with synthetic HTML/CSV, not fetched DKScan data.

PCAP exports of BLE and AX.25 frames were independently dissected by Wireshark/tshark. A malformed BLE CRC was flagged by Wireshark. GSMTAP IPv4/UDP framing and an empty GSM flowgraph were checked without network capture or RF transmission.

`scripts/reference-smoke.py` tests transactional SigID pagination/ranking/failure retention and suspicious-unit handling using mocked API data, plus actual Wireshark parsing of synthetic GSM cipher/identity/SI3 packets and masked IMSI/baseline behavior. A real API sync imported 586 usable SigID Wiki entries into the local user database; article prose and media were not copied.

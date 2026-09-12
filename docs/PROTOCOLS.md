# Protocol and OSINT catalog

Install with `thugsrf addon install` (or `make addons` from the checkout). Modules are disabled until enabled. Enter in the Addons panel toggles a module; the list scrolls at both supported terminal sizes.

```sh
thugsrf addon enable all --kind identifiers
thugsrf identify recording.cs8 --sample-rate 8000000 --frequency 433920000
thugsrf addon enable pocsag
thugsrf addon run pocsag pager.wav
thugsrf addon run pocsag pager.wav --options '{"invert":true}'
thugsrf addon run-enabled recording.cs8 --kind decoders
```

`identify` and `addon run-enabled` run up to three compatible modules concurrently, skip incompatible formats, retain per-module errors and save the combined result to SQLite. In the TUI, enabled compatible decoders also run against incoming Spectrum samples while reception is active. Press `d` for their shared rolling console, `D` to pause/resume addon decoding, `s` to toggle log saving, and `x` to clear the console. CLI `addon run` and `addon run-enabled` process existing files. Set sample rate and capture center correctly: the addon API does not automatically read SigMF sidecars. `--format auto` recognizes cs8/cu8/wav/pcap/pcapng/nmea/ts/hex extensions. WAV sample rate comes from its header.

Each module can have editable `options.toml`; per-run `--options` JSON overrides it. IQ narrowband adapters accept `channel_hz` for a carrier inside the recording. Ordinary waveform adapters inspect at most 8 million samples, while text/packet/output limits are separately bounded. Split long recordings into useful segments; no-result is a valid outcome and a decoder's output may still require validation.

| Addon | Kind | Inputs | Capability |
| --- | --- | --- | --- |
| `gsm-security` | identifiers | pcap, pcapng | Passive A5/GEA signalling, masked IMSI visibility, identity requests and cell baseline |
| `sigid-id` | identifiers | cs8, cu8 | Offline SigID Wiki reference ranking by frequency, bandwidth and optional modulation |
| `adsb` | decoders | cs8, cu8, hex | 1090 MHz ADS-B extended squitter with strict CRC-24 |
| `rds` | decoders | cs8, cu8, wav | Broadcast FM RDS station name, RadioText, PI and programme data via redsea |
| `ais` | decoders | cs8, cu8, wav, nmea | AIS A/B messages, MMSI and positions from IQ, audio or NMEA |
| `atis-symbols` | decoders | cs8, cu8, wav | Marine ATIS/DSC checked symbols (partial framing, not full vessel ID) |
| `aviation-atis-id` | identifiers | cs8, cu8, wav | Aviation AM / voice ATIS service context |
| `ble-advertising` | decoders | cs8, cu8 | BLE 1M advertisements: address, name, UUIDs, validated CRC |
| `bluetooth-id` | identifiers | cs8, cu8, wav, pcap, pcapng | Bluetooth/BLE band context or actual packet metadata |
| `bluetooth-pcap` | decoders | pcap, pcapng | Bluetooth/BLE capture metadata and advertised device information |
| `cellular-id` | identifiers | cs8, cu8, wav | GSM 900 / 1800 allocation context |
| `chirp-id` | identifiers | cs8, cu8 | Repeated linear chirp candidates (LoRa, radar or other chirped waveforms) |
| `danish-frequency` | identifiers | cs8, cu8, wav | User-imported Danish frequency lists with source, date and matching distance |
| `digital-audio` | decoders | cs8, cu8, wav | AFSK1200/2400, FSK9600 and FMSFSK messages |
| `digital-bursts` | identifiers | cs8, cu8, wav | Energy-burst timing: packet-like activity with uncertainty |
| `digital-voice` | decoders | cs8, cu8, wav | DMR / D-Star / dPMR / YSF metadata with DSDcc |
| `eas` | decoders | cs8, cu8, wav | EAS/SAME broadcast alert messages |
| `encryption-id` | identifiers | cs8, cu8, wav, pcap, pcapng | Explicit protection/encryption metadata; never guess from noise |
| `flex` | decoders | cs8, cu8, wav | FLEX pager messages |
| `gnss-id` | identifiers | cs8, cu8, wav | GPS / Galileo GNSS frequency context |
| `gps-l1` | decoders | cs8, cu8 | GPS L1 C/A acquisition, tracking and navigation using GNSS-SDR |
| `gps-nmea` | decoders | nmea | Checksum-validated GPS/GNSS receiver NMEA sentences |
| `gsm-bcch` | decoders | cs8, cu8 | Offline GSM broadcast-control-channel messages |
| `hid-id` | identifiers | cs8, cu8, wav, pcap, pcapng | Mouse / keyboard clues from Bluetooth HID UUIDs and device class |
| `maritime-id` | identifiers | cs8, cu8, wav | AIS / DSC / marine ATIS channel context |
| `morse` | decoders | cs8, cu8, wav | Morse/CW text from audio or centered CW IQ |
| `morse-id` | identifiers | cs8, cu8, wav | Morse/CW mark timing and estimated speed |
| `packet-radio` | decoders | cs8, cu8, wav | CRC-checked AX.25 / APRS from audio or IQ |
| `paging-id` | identifiers | cs8, cu8, wav | Paging allocation context: POCSAG / FLEX candidates |
| `pocsag` | decoders | cs8, cu8, wav | POCSAG 512/1200/2400 pager messages |
| `satellite` | decoders | cs8, cu8, wav | SatDump weather/satellite pipeline, configurable in options.toml |
| `satellite-id` | identifiers | cs8, cu8, wav | Satellite downlink candidates with source references |
| `selective-call` | decoders | cs8, cu8, wav | DTMF / ZVEI / CCIR / EEA / EIA selective-calling tones |
| `tv-id` | identifiers | cs8, cu8, wav | Terrestrial TV / DVB / DAB allocation candidates |
| `tv-transport` | decoders | ts | MPEG-TS PID, CRC-checked PAT programs and scrambling flags |
| `walkie-id` | identifiers | cs8, cu8, wav | VHF/UHF handheld / PMR446 context and CTCSS-tone candidates |
| `weather-id` | identifiers | cs8, cu8, wav | Weather sensor and radiosonde allocation candidates |
| `weather-sensors` | decoders | cs8, cu8 | rtl_433 sensor messages from recorded IQ |
| `wifi-id` | identifiers | cs8, cu8, wav, pcap, pcapng | Wi-Fi OFDM training candidates or actual PCAP metadata |
| `wifi-pcap` | decoders | pcap, pcapng | 802.11 packet metadata, SSIDs, addresses and protection flags |
| `zigbee-pcap` | decoders | pcap, pcapng | 802.15.4 / ZigBee packet metadata: PAN IDs, short addresses, security flag |

The original `ook-pulses`, `rtl433` and `band-context` addons are also included.

## Backend requirements and limits

- **POCSAG/FLEX/Morse/selective-call/digital-audio/EAS:** `multimon-ng`. Includes POCSAG 512/1200/2400, FLEX, Morse CW, DTMF and selective calls, supported AFSK/FSK and EAS. `invert=true` is available for inverted POCSAG discriminator polarity. These are audio/narrowband IQ adapters, not arbitrary digital-protocol decoders.
- **Packet radio/AIS audio:** Dire Wolf's `atest`. Packet radio uses CRC-checked AX.25 at 1200/2400/4800/9600 baud via `options.baud`. AIS handles one centered A/B carrier at a time; NMEA input validates its transport checksum and extracts MMSI/type and coordinates for supported position reports. No live vessel-owner registry is queried.
- **Marine ATIS:** partial 1300/2100 Hz symbol recognizer with zero-count checks and format-symbol candidates. It does **not** validate a complete DSC/ATIS message or recover a verified station identity. Aviation ATIS is spoken information and only gets airband context hints here.
- **ADS-B:** native Python PPM decoding or hexadecimal frames; DF17/18 CRC checks, address/type and callsign. No CPR position pairing or owner database.
- **BLE advertising:** legacy 1M advertising access address, dewhitening and CRC-24; address, advertised name, service UUIDs and manufacturer company ID. Choose `channel=37`, `38` or `39`. No connected-channel following, Bluetooth Classic raw-IQ decoder, Coded/2M PHY or encrypted payload recovery.
- **Wi-Fi/Bluetooth/HID/ZigBee PCAP:** `tshark`, reading existing captures only. Protocol fields, protected-frame flags, advertised HID UUID/device class and PAN/short addresses are evidence; names/classes are self-reported. No monitor-mode capture or raw Wi-Fi/ZigBee RF demodulation is implemented.
- **Digital voice:** `dsdccx` from `dsdcc`; metadata for supported DMR, D-Star, dPMR and YSF signals, limited NXDN detection. No P25 support, speech playback or encrypted speech recovery.
- **Weather:** installed `rtl_433` protocol library, not a separate implementation of every weather sensor.
- **Satellite:** `satdump`, selectable `meteor_m2_lrpt`, `noaa_apt`, `noaa_hrpt`, `metop_ahrpt`, `fengyun3_ab_ahrpt`, `fengyun3_c_ahrpt`. Use `--options '{"pipeline":"..."}'`. NOAA APT WAV support includes archival recordings; check active spacecraft separately. Output products go to the user data directory and do not by themselves establish a successful decode. These backend integrations have not been validated against clean satellite captures.
- **GPS L1:** `gnss-sdr`, centered 1575.42 MHz complex IQ. Streams signed IQ directly at capture rate; unsigned conversion is capped at 120 seconds and processing has a 100-second wall-clock deadline. A clean, sufficiently long capture is required for ephemerides and a fix. This integration has not been validated against a clean GPS capture. `gps-nmea` separately parses checksum-valid GGA/RMC sentences from an existing receiver.
- **GSM BCCH:** `grgsm_decode` from `gr-gsm`, offline broadcast-control-channel adapter. Emits decoded control bytes and local GSMTAP UDP on 127.0.0.1:4729; no subscriber traffic decryption. This integration has not been validated on a clean GSM capture.
- **TV:** MPEG-TS PID/scrambling flags and CRC-valid single-packet PAT program discovery from `.ts`. No raw DVB-T/T2 or satellite-TV RF demodulation is implemented.

`make deps` installs common dependencies. `make protocol-deps` adds the heavier optional backends on Debian. The package recommends common tools and suggests the heavy backends. Python DSP uses `/usr/bin/python3` with distribution NumPy/SciPy. Missing tools produce explicit errors; no fake decoded results are substituted.

## Packet export

BLE, AX.25 and GSM BCCH support `export-pcap` and the `pcap` output-path option. See [PCAP.md](PCAP.md) for real frame formats, source metadata and timestamp conventions.

## Interpreting evidence

Frequency-context matches are weak clues. Burst/chirp/Morse/OFDM-like patterns are waveform candidates, not protocol proof. Encryption cannot be identified from noise or entropy alone; the encryption identifier only reports explicit decoded protection/encryption fields. Mouse/keyboard identification needs an advertised HID service/device class or a known protocol; shared 2.4 GHz occupancy is insufficient. These modules do not identify a person behind a transmitter.

Source references include [multimon-ng](https://github.com/EliasOenal/multimon-ng), [Dire Wolf](https://github.com/wb2osz/direwolf), [Bluetooth Core](https://www.bluetooth.com/specifications/specs/core-specification/), [GNSS-SDR](https://gnss-sdr.org/), [gr-gsm](https://github.com/ptrkrysik/gr-gsm), [SatDump](https://github.com/SatDump/SatDump), and [Wireshark](https://www.wireshark.org/docs/). Allocation-source management is documented in [FREQUENCIES.md](FREQUENCIES.md).

See [SIGID.md](SIGID.md) for reference synchronization and [GSM-SECURITY.md](GSM-SECURITY.md) for passive cellular-security observations.

Live console entries include capture-end time (UTC), receiver frequency and decoder name. The latest 500 entries are retained, newest first; use Up/Down or Page Up/Page Down to scroll. Up to three compatible decoders run concurrently, with a 15-second limit per live invocation. Errors and empty results are shown alongside decoded events and diagnostics. These are repeated contiguous windows of up to two seconds / eight million samples, not persistent protocol sessions. Slow decoders skip intervening windows, and messages crossing window boundaries may be missed. Retuning cancels old work and starts fresh captures. Packet-capture-only addons cannot decode raw radio IQ and are skipped.

The live addon feed shares Spectrum's existing radio stream. While listening owns the radio, WFM RDS continues to feed the console, but other addon decoding waits for Spectrum reception to resume. Saved-file runs still place their complete JSON result in Workbench (6) and SQLite history; the rolling console retains 500 entries in memory. Press `s` in that console to start/stop writing new entries to `~/.config/thugsrf/decoder-output-<timestamp>.log` (or the XDG config directory). Each start creates a new private file; entries flush as they arrive and logging continues across retunes. `SAVING` appears in the console title. Clearing the on-screen list does not delete saved logs. The live feed does not continuously write IQ recordings or database entries.

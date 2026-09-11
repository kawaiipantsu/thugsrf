THUGS(red) RF 0.2.0 expands the workbench with 43 activatable protocol/OSINT addons, a sequential HackRF panorama, live AM/FM listening, and a local VHF/UHF channel directory.

- Survey tab 7 sweeps 1 MHz–6 GHz with coverage and peak hold; this is not simultaneous full-span IQ reception.
- Listen tab 8 adds AM, narrow FM, mono broadcast FM and SW/upper-MW presets; VHF/UHF tab 9 adds RX/TX channel entries and explicit finite microphone transmission.
- Addon installation from the Debian package, a scrolling addon list, format-aware batch identification/decoding, options and process-group cleanup.
- Protocol adapters include POCSAG/FLEX/Morse, AX.25, AIS, partial marine ATIS, weather, ADS-B, BLE, packet metadata and optional satellite/GNSS/GSM/digital-voice backends. The capability matrix distinguishes complete decodes, partial parsers and unvalidated backend integrations.
- PCAP export for CRC-checked BLE advertisements, AX.25 and optional GSM BCCH/GSMTAP, with source/timing sidecars and no-overwrite behavior.
- SigID Wiki factual metadata sync into local SQLite and offline multi-feature candidate ranking; 586 records imported during verification.
- Passive GSM A5/GEA signalling analysis, masked IMSI visibility, identity-request observations and public-cell baseline comparisons.
- Local Danish HTML/CSV frequency imports preserve provenance; US allocation starter hints and official reference links are region-labelled. DKScan blocked automated retrieval, so no DKScan entries are bundled.
- Correctly cropped branding and refreshed actual terminal screenshots, including a passive hardware survey.

Validation: formatting, strict Clippy, Rust layout/DSP tests, CLI/TUI/receiver/AI regressions, known protocol frames and independent decoder fixtures, fake-device audio RX/TX tests. The connected HackRF completed a full-range passive sweep at 1 MHz resolution. Physical RF transmission was not performed.

Install the Debian package, run `thugsrf addon install`, and restart the TUI. Existing configuration and addon files are preserved. Run `thugsrf addon enable all --kind identifiers` to activate the identification catalog. See docs/PROTOCOLS.md, docs/RADIO.md and docs/FREQUENCIES.md for the exact interfaces and limitations.

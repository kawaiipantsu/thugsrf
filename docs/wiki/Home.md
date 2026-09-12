# THUGS(red) RF manual

![THUGS(red) RF](images/banner.png)

A Linux radio investigation workbench by **Kawaiipantsu from THUGS(red)**, a Danish hacking community. [Community](https://thugs.red) · [Source](https://github.com/kawaiipantsu/thugsrf) · [Downloads](https://github.com/kawaiipantsu/thugsrf/releases)

This manual describes **0.3.0**. Begin with [[Installation]], then [[Quick-Start]] and [[Terminal-Manual]]. For a complete investigation follow [[Investigation-Guide]].

![Spectrum in a large terminal, synthetic demo](images/spectrum-wide.png)

## Manual and guides

- [[Installation]] — Debian packages, source builds, dependencies and hardware checks.
- [[Quick-Start]] — first reception and a reproducible hardware-free example.
- [[Terminal-Manual]] — every tab, keyboard controls, tuning and display interpretation.
- [[Configuration]] — every setting, readable frequencies and data locations.
- [[Investigation-Guide]] — capture, inspect, identify, decode and preserve evidence.
- [[Radio-and-Repeaters]] — survey, AM/FM, SW/MW and channel directory.
- [[Frequency-OSINT]] and [[SigID-Catalog]] — sourced frequency hints and waveform references.
- [[Protocol-Catalog]] — decoder capabilities and specific limitations.
- [[Wireshark-Export]] and [[GSM-Security]] — recovered packets and passive signalling observations.
- [[AI-Analysis]] — explicit feature/image analysis with cloud or local models.
- [[Addon-Development]] and [[Addon-Example]] — executable JSON extension API and a complete example.
- [[Command-Reference]] — help for every CLI command and subcommand.
- [[Troubleshooting]], [[Screenshots]], [[Architecture]], [[Verification]] and [[Building-and-Releasing]].

Frequency matches and AI suggestions are candidates. Distinguish measured samples, decoded fields, and reference-based guesses throughout an investigation. Survey scans sequentially; a HackRF does not receive its entire 1 MHz–6 GHz span simultaneously.

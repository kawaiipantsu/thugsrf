# Configuration reference

Settings (tab 5) edits validated persistent values. Select a row with Up/Down, Enter to edit, Enter to save, or Esc to cancel. Saving stops active reception; Space restarts it. `thugsrf config` prints configuration, and `thugsrf config set KEY VALUE` edits it from the shell.

```sh
thugsrf config set frequency 145.252MHz
thugsrf config set fine_tune_hz 12.5kHz
thugsrf config set coarse_tune_hz 10MHz
```

Those three fields accept case-insensitive Hz/kHz/MHz/GHz, optional space before units and exact decimals resolving to whole Hz. Bare integers mean Hz. TOML stores integer Hz. Other numeric fields use the numeric units below. Existing configurations automatically receive defaults for newly added settings.

| Setting | Default | Meaning / bounds |
|---|---|---|
| device | hackrf | hackrf, rtl, audio, demo; changing via Settings resets sample rate |
| frequency | 433920000 | Center Hz; HackRF driver 0–6 GHz, RTL 24–1766 MHz |
| fine_tune_hz | 500000 | Left/Right Spectrum step; 1 Hz–6 GHz |
| coarse_tune_hz | 10000000 | Up/Down and Page keys Spectrum step; 1 Hz–6 GHz |
| sample_rate | 8000000 | Samples/s; HackRF 8–20M, RTL supported ranges, audio 8k–192k |
| lna_gain | 16 | HackRF dB, 0–40 in steps of 8 |
| vga_gain | 20 | HackRF dB, 0–62 in steps of 2 |
| rtl_gain | 200 | RTL gain in tenths of dB |
| serial | empty | Optional device selection string |
| audio_device | default | ALSA device name |
| fft_size | 8192 | Power of two, 256–65536 |
| threshold_db | 12 | Detection threshold above estimated noise, 0–100 |
| ai_provider | local | local, openai, anthropic |
| ai_model | empty | Model name supplied by the user |
| local_url | http://127.0.0.1:11434/v1 | Local OpenAI-compatible API base |
| sweep_start_mhz | 1 | Sequential sweep lower bound |
| sweep_end_mhz | 6000 | Upper bound, above start and at most 6000 |
| sweep_bin_hz | 1000000 | Bin width, 100000–5000000 Hz |
| listen_mode | fm | am, nfm, fm, wfm (broadcast mono) |
| listen_bandwidth | 12500 | 3000–200000 Hz |
| squelch_dbfs | -65 | Audio squelch, -160–0 dBFS |

Global CLI options override the current invocation without saving. Keyboard tuning also remains session-only. A sound card has no RF center-frequency tuning. Practical antenna/front-end coverage differs from driver tuning bounds; see [[Radio-and-Repeaters]].

## Files and backups

| Location under your home | Contents |
|---|---|
| .config/thugsrf/config.toml | Settings |
| .config/thugsrf/decoders/ and identifiers/ | Executable addons and manifests |
| .config/thugsrf/repeaters.toml | Local channel directory |
| .config/thugsrf/decoder-output-*.log | Live console logs, toggled with s |
| .config/thugsrf/spectrum-*.txt and waterfall-*.txt | Full-bin ASCII exports from Spectrum s |
| .local/share/thugsrf/recordings/ | Default TUI captures |
| .local/share/thugsrf/investigations.sqlite3 | Reports, findings and AI hypotheses |
| .local/share/thugsrf/references.sqlite3 | SigID reference cache |
| .local/share/thugsrf/frequencies.json | Imported frequency references |
| .local/share/thugsrf/audio.log | Audio worker diagnostics |

Absolute `XDG_CONFIG_HOME` and `XDG_DATA_HOME` override the roots. Keep capture sidecars beside their raw files. For a simple consistent backup, quit the application and other writers before copying the configuration/data directories, including any SQLite WAL files. Recordings supplied with explicit paths may live elsewhere.

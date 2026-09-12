# Live decoders and saved output

Enable multiple decoders in Addons (4) with Enter. Start Spectrum reception with Space, then press `d` to open the shared rolling Decoder Console. Compatible enabled decoders automatically try the current center frequency and sample rate. You can return to Spectrum (1) while decoding continues.

| Key in Decoder Console | Action |
|---|---|
| Space | Start/stop the Spectrum receiver |
| D | Pause/resume live addon decoding |
| s | Start/stop saving new console output to a file |
| x | Clear the displayed history |
| Up/Down, Page Up/Page Down | Scroll |

The latest 500 entries appear first. Entries identify the capture-end time in UTC, receiver frequency and decoder. Decoded events, errors and decoder diagnostics share the same window. An empty result explicitly reports that no messages were recovered.

## Saving the rolling console

`s` creates `~/.config/thugsrf/decoder-output-<timestamp>.log`; `XDG_CONFIG_HOME` changes the root when configured. The timestamp is Unix time in milliseconds. The title shows **SAVING** while active. Entries flush as they arrive. Very long entries are shortened only on screen; the log retains the full entry. Press `s` again to stop; another start creates another file without overwriting earlier logs. Logging follows retuning and records new entries, not a backfill of old screen history. Clearing the list with `x` does not delete logs.

## AIS

Tune the center to the AIS carrier you want to inspect, enable `ais` in Addons, start Spectrum RX, and press `d`. The decoder handles one centered channel at a time. Decoded messages can include MMSI, message type and positions where available; missing `atest`/Dire Wolf appears as an error in the console. This does not identify the vessel owner through an online registry.

Saved recordings still work through the command bar:

```text
addon run ais /path/to/capture.cs8
```

File commands place the full JSON result in Workbench (6), with messages in `events` and decoder text in `diagnostics`. Supply the original capture frequency and sample rate if they differ from the current settings. See [[Protocol-Catalog]].

## FM RDS

Enable the `rds` addon to mix RDS output with other live Spectrum decoders. Alternatively, select WFM and start listening; RDS station name, PI, programme type, RadioText and traffic status appear in Listen and are mirrored to the console. WFM listening uses a continuous RDS decoder, while the Spectrum addon uses the same finite windows as the other addons.

For files:

```sh
thugsrf decode station.cs8 --mode rds --sample-rate 8000000
thugsrf --device rtl --sample-rate 1000000 decode station.cu8 --format cu8 --mode rds
thugsrf decode multiplex.wav --format wav --mode rds
```

Install the optional redsea backend as described in [[Radio-and-Repeaters]]. IQ must be centered on the FM carrier. WAV must contain mono 16-bit FM multiplex at 128 kHz or more; ordinary audio WAV cannot retain RDS. Built-in file decoding handles up to 120 seconds and 2000 groups.

## Live limits

Up to three decoders run concurrently on contiguous snapshots from the existing radio stream. Windows contain up to two seconds or eight million samples, whichever is smaller, and each live addon invocation has a 15-second deadline. Slow decoders skip windows, and messages split across boundaries can be missed. Unsupported input formats are skipped. Retuning cancels old work before the new receiver starts; previous console entries remain labelled with their original frequency.

Spectrum and live addon decoding share one hardware capture. Audio listening holds the spectrum and pauses the other addon feed; continuous WFM RDS still reaches the console. Space returns to Spectrum reception. The console is not a guaranteed lossless protocol capture; record IQ for reproducible offline analysis.

For high-resolution ASCII plot exports, press `s` in Spectrum instead. That writes separate graph and waterfall files; see [[Terminal-Manual]].

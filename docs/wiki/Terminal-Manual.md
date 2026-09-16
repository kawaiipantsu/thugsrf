# Terminal manual

![Spectrum, synthetic demo](images/spectrum-wide.png)

The interface adapts to terminal size. Use a UTF-8 terminal and a font with braille, box-drawing and block glyphs. True-color terminals show the full waterfall palette.

| Tab | Purpose | Main interaction |
|---|---|---|
| 1 Spectrum | FFT, waterfall, zoom and receiver filters | Space RX; Enter frequency; click peak; m mode; b width |
| 2 Detections | Current spectral findings | Inspect candidates; s saves a report |
| 3 Recordings | Investigation/recording view | w prepares packet export |
| 4 Addons | Installed decoder and identifier activation | Up/Down select; Enter toggles |
| 5 Settings | Validated persistent configuration | Up/Down select; Enter edit/save; Esc cancel |
| 6 Workbench | Command results and full diagnostics | Page Up/Down scroll |
| 7 Survey | Sequential wide-span HackRF panorama | Space sweep; s save |
| 8 Listen | AM/FM, SW and upper-MW presets | Up/Down select; Enter tune; Space listen |
| 9 VHF/UHF | Local channels and repeaters | Enter tune; a add; e edit; t prepare TX |
| d Decoder Console | Shared live output from enabled decoders | D pause/resume; s toggle log; x clear; arrows scroll |

## Spectrum tuning

| Key | Change |
|---|---|
| Left / Right | Down / up by fine_tune_hz; default 500 kHz |
| Down / Up | Down / up by coarse_tune_hz; default 10 MHz |
| Page Down / Page Up | Down / up by coarse_tune_hz |
| Enter | Enter a frequency such as `145.252MHz`; Enter applies, Esc cancels |
| [ / ] or mouse wheel | Zoom out/in, centered on the selected frequency |
| 0 | Full captured span |
| Left click | Select strongest FFT bin under that column, on graph or waterfall |
| t / l | Tune selected frequency / search local frequency references |
| m / b | Cycle NFM/FM/WFM/AM / enter receive bandwidth, e.g. `12.5kHz` |
| a | Listen/stop at the center frequency; spectrum and waterfall keep running |
| f | Cycle 2048, 8192, 32768 and 65536 FFT bins |
| c / + / - | Cycle waterfall palette / raise or lower display threshold |
| s | Export ASCII graph and waterfall at one column per visible FFT bin |

Edit both steps in Settings. Values accept `500kHz`, `0.5MHz`, or integer Hz. Frequency accepts `443mhz` and `145.252Mhz`. Input is case-insensitive and must resolve to whole Hz. Invalid values leave the existing setting unchanged.

Keyboard tuning changes the current session, not saved defaults. Live tuning releases the old receiver, clears stale plots, and starts reception at the new center; audio listening keeps running and simply follows the new frequency after that brief gap. A stopped receiver stays stopped. Tuning is blocked during jobs or survey; sound-card input has no RF tuning.

## Global controls

Tab / Shift-Tab cycle all ten panels; 1–9 select the first nine directly and `d` opens Decoder Console. `p` freezes the displayed spectrum while reception continues to drain. `s` exports ASCII in Spectrum, saves a report in Detections, saves the panorama in Survey, or toggles file logging in Decoder Console. `r` prepares a finite recording; review its path and press Enter. `l` looks up the tuned frequency in local references. `:` opens the command bar; quoted paths work. `?` opens help. `q` exits after active jobs finish.

Commands run on a worker thread and stop live reception first. The command bar accepts CLI subcommands, for example `record '/tmp/my capture.cs8' --seconds 5`, without the initial `thugsrf` word.

## Reading the plots

Spectrum positions represent frequencies around the configured center. Peaks show energy above the estimated noise floor; they do not establish a protocol. Levels are relative dBFS, not calibrated dBm. Waterfall rows show recent spectral history. Display freeze does not pause the transmitter or preserve an unlimited IQ recording. Save a recording for further decoding.

See [[Screenshots]] for compact and wide layouts, and [[Radio-and-Repeaters]] for the specialist tabs.

## Resolution, filtering and exports

Zoom crops the visible part of the current capture; receiver sample rate stays unchanged. `f` increases frequency resolution by using a larger FFT, restarting reception and clearing stale history. New settings default to 8192 bins; existing settings retain their value. The selected point shows frequency, dBFS, SNR and approximate detected-peak bandwidth. Allocation hints are candidates, not proof of a protocol or transmitter.

The receive filter edges are marked on the graph. NFM/FM/WFM/AM presets use 12.5/50/200/10 kHz widths; `b` accepts a custom 3–200 kHz width. These filters apply to listening. Spectrum still displays the captured band. `a` starts or stops listening alongside the live spectrum, at whatever frequency Spectrum is currently tuned to. WFM displays RDS when redsea is installed.

Spectrum `s` writes `spectrum-<timestamp>.txt` and `waterfall-<timestamp>.txt` under the thugsrf config directory. The graph uses 131 power rows (1 dB each); the waterfall exports all stored rows. Both use every visible FFT bin, up to 65536 columns, with frequency-scale headers. Open with line wrapping disabled. The normal waterfall uses peak-preserving column reduction and two time samples per terminal row.

See [[Live-Decoders]] for multiple-decoder output and toggleable log saving.

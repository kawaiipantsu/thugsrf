# Terminal manual

![Spectrum, synthetic demo](images/spectrum-wide.png)

The interface adapts to terminal size. Use a UTF-8 terminal and a font with braille, box-drawing and block glyphs. True-color terminals show the full waterfall palette.

| Tab | Purpose | Main interaction |
|---|---|---|
| 1 Spectrum | FFT, waterfall and receiver details | Space starts/stops; arrows tune |
| 2 Detections | Current spectral findings | Inspect candidates; s saves a report |
| 3 Recordings | Investigation/recording view | w prepares packet export |
| 4 Addons | Installed decoder and identifier activation | Up/Down select; Enter toggles |
| 5 Settings | Validated persistent configuration | Up/Down select; Enter edit/save; Esc cancel |
| 6 Workbench | Command results and full diagnostics | Page Up/Down scroll |
| 7 Survey | Sequential wide-span HackRF panorama | Space sweep; s save |
| 8 Listen | AM/FM, SW and upper-MW presets | Up/Down select; Enter tune; Space listen |
| 9 VHF/UHF | Local channels and repeaters | Enter tune; a add; e edit; t prepare TX |

## Spectrum tuning

| Key | Change |
|---|---|
| Left / Right | Down / up by fine_tune_hz; default 500 kHz |
| Down / Up | Down / up by coarse_tune_hz; default 10 MHz |
| Page Down / Page Up | Down / up by coarse_tune_hz |

Edit both steps in Settings. Values accept `500kHz`, `0.5MHz`, or integer Hz. Frequency accepts `443mhz` and `145.252Mhz`. Input is case-insensitive and must resolve to whole Hz. Invalid values leave the existing setting unchanged.

Keyboard tuning changes the current session, not saved defaults. Live tuning releases the old receiver, clears stale plots, and starts reception at the new center. A stopped receiver stays stopped. Tuning is blocked during jobs, survey or audio listening; sound-card input has no RF tuning.

## Global controls

Tab / Shift-Tab cycle panels; 1–9 select directly. `p` freezes the displayed spectrum while reception continues to drain. `s` saves the current report, or the panorama in Survey. `r` prepares a finite recording; review its path and press Enter. `l` looks up the tuned frequency in local references. `:` opens the command bar; quoted paths work. `?` opens help. `q` exits after active jobs finish.

Commands run on a worker thread and stop live reception first. The command bar accepts CLI subcommands, for example `record '/tmp/my capture.cs8' --seconds 5`, without the initial `thugsrf` word.

## Reading the plots

Spectrum positions represent frequencies around the configured center. Peaks show energy above the estimated noise floor; they do not establish a protocol. Levels are relative dBFS, not calibrated dBm. Waterfall rows show recent spectral history. Display freeze does not pause the transmitter or preserve an unlimited IQ recording. Save a recording for further decoding.

See [[Screenshots]] for compact and wide layouts, and [[Radio-and-Repeaters]] for the specialist tabs.

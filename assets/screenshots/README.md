# Terminal screenshots

Captured directly from xterm running the release binary. The spectrum screenshots use `thugsrf tui --demo`, with reception started by pressing Space; the data is synthetic. The CLI screenshot runs `thugsrf --help`.

- `spectrum-wide.png`: 170 columns × 50 rows.
- `spectrum-compact.png`: 80 columns × 24 rows.
- `cli-help.png`: 100 columns × 44 rows.

Font: DejaVu Sans Mono, 11 pt. Images are unretouched captures of the terminal window.

To regenerate on Debian:

```sh
sudo apt install xterm xvfb xauth xdotool imagemagick fonts-dejavu-core
make build
xvfb-run -a -s '-screen 0 2400x1600x24' python3 scripts/screenshots.py
```

The script uses temporary XDG directories and preserves user configuration. By default it does not open RF hardware. Add `--hardware` to also capture `survey-wide.png` from a passive HackRF sweep; that screenshot contains real relative-power measurements. `addons-wide.png`, `listen-wide.png` and `vhf-uhf-wide.png` show the new panels at 170×50 with no active audio or transmission.

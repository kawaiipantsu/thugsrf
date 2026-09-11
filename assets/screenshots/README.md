# Terminal screenshots

Captured directly from xterm running the release binary. The spectrum screenshots use `thugsrf tui --demo`, with reception started by pressing Space; the data is synthetic. The CLI screenshot runs `thugsrf --help`.

- `spectrum-wide.png`: 170 columns × 50 rows.
- `spectrum-compact.png`: 80 columns × 24 rows.
- `cli-help.png`: 100 columns × 32 rows.

Font: DejaVu Sans Mono, 11 pt. Images are unretouched captures of the terminal window.

To regenerate on Debian:

```sh
sudo apt install xterm xvfb xauth xdotool imagemagick fonts-dejavu-core
make build
xvfb-run -a -s '-screen 0 2400x1600x24' python3 scripts/screenshots.py
```

The script uses temporary XDG directories and does not change the user's configuration or open any RF hardware.

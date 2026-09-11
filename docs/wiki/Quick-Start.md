# Quick start

## Explore without a radio

```sh
thugsrf tui --demo
```

Press Space for synthetic reception. Resize the terminal; 80×24 is supported, while 170×50 gives more space for the plots and receiver details. Press Tab to explore, `?` for help, and `q` to exit.

## Receive with HackRF

```sh
thugsrf doctor
thugsrf --device hackrf --frequency 443mhz tui
```

Space starts reception. Left/Right changes frequency by 500 kHz. Up/Down or Page Up/Page Down changes it by 10 MHz. Press `5` for Settings to change `fine_tune_hz` or `coarse_tune_hz`; select with Up/Down, press Enter, type `12.5kHz`, and Enter to save. Frequency accepts `145.252MHz` too. Saved settings stop active reception; return to Spectrum and press Space.

## Reproducible file workflow

Use a new directory so output files do not already exist:

```sh
mkdir thugsrf-example
cd thugsrf-example
thugsrf encode test.cs8 --mode ook --bits 10110010 --rate 8000000 --baud 1000
thugsrf --frequency 433.92MHz analyze test.cs8 --png spectrum.png
thugsrf decode test.cs8 --mode ook
thugsrf history
```

This generates and analyzes synthetic symbols without RF transmission. OOK output reports envelope timing; it is not a complete framed protocol message. Continue with [[Investigation-Guide]] for actual captures.

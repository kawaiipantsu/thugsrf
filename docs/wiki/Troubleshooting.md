# Troubleshooting

## Space ends reception with HackRF setup messages

Setup lines such as `hackrf_set_sample_rate` and `hackrf_set_hw_sync_mode` are not by themselves the failure. Read the full Workbench diagnostics. Empty-start one-second USB timeouts are retried up to three times. Busy-device errors and failures after samples arrive are reported immediately.

Stop other SDR applications, verify the USB connection with `thugsrf doctor`, and retry with Space. Check a reliable USB cable/port and a supported sample rate. A timeout after reception has started needs a new manual start.

## Tuning arrows do not change frequency

Select tab 1. Other tabs use Up/Down for selection. Stop any active command, survey or listening session. Audio input has no RF tuning. Check the status line for device-range errors. Left/Right uses `fine_tune_hz`; Up/Down and Page keys use `coarse_tune_hz`.

## Invalid setting

Frequency and tuning steps accept units, but sample rate remains integer Hz. Use `145.252MHz`, not decimal commas. Tuning steps must be positive whole-Hz values within 6 GHz. HackRF and RTL have separate frequency/rate validation. Changing device through Settings or `config set device` also selects a suitable default sample rate.

## Addon missing, disabled or no events

Run `thugsrf addon install`, then `thugsrf addon list`. Enable the desired module and check its required programs in [[Protocol-Catalog]]. Existing user addon files are preserved, so an older user copy can differ from the newly packaged bundle. Back up customizations before updating it. No events is a valid decoder result; verify modulation, format, center frequency and sample rate.

## Audio is silent

Check `audio_device`, system mixer/output routing, selected mode and squelch. Inspect `~/.local/share/thugsrf/audio.log` and Workbench errors. WFM is mono; there is no stereo or RDS decoder in the listening path. Stop other radio owners before starting Listen.

## UI glyphs or colors look wrong

Use UTF-8, a font with braille/block characters and a true-color terminal. Try 170×50; 80×24 is the minimum supported layout. Demo mode isolates rendering from hardware issues.

## Source lists or AI are unavailable

Frequency imports use saved HTML/CSV. DKScan retrieval was blocked on the development host; no entries are bundled from that site. SigID requires an explicit network sync before offline lookup. AI requires a configured model and endpoint; see [[AI-Analysis]].

## Report a reproducible issue

Include `thugsrf --version`, distribution, terminal size, device/firmware details, exact command, full diagnostics and a small shareable sample if relevant. Remove credentials and private capture contents. [Open an issue](https://github.com/kawaiipantsu/thugsrf/issues).

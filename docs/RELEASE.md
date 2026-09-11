THUGS(red) RF 0.1.1 fixes live HackRF startup recovery and receiver diagnostics.

Some HackRF starts configured the radio but delivered no USB samples for one second, causing hackrf_transfer to exit. The previous TUI displayed only the first two startup lines and hid the actual failure.

- Retry this specific empty-startup failure up to three total attempts, releasing the previous process/USB handle and waiting 500 ms between attempts.
- Stop retries when the user stops reception; never retry busy-device errors or failures after samples have arrived.
- Show full receiver stderr and exit status in the scrollable Workbench on final failure.
- Keep lifecycle diagnostics separate from the bounded spectrum queue so display backpressure cannot hide errors.
- Include the correctly cropped banner/logos and real terminal screenshots in the documentation.

Regression tests exercise transient recovery, retry exhaustion, busy devices and mid-stream failure using a fake driver in the actual TUI. Physical HackRF reception was checked through three successive start/save/stop cycles. RF transmission and finite recording are never automatically retried.

Install the updated package with `sudo apt install ./thugsrf_0.1.1_amd64.deb`, restart `thugsrf`, and press Space to receive.

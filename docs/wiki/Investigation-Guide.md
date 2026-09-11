# Capture-to-evidence investigation

## 1. Choose a source and frequency

Use `thugsrf doctor`, then inspect Spectrum or the sequential Survey. Record the device, antenna, center frequency and sample rate. A broad scan can suggest a region to inspect, but short transmissions may occur between sweep visits.

## 2. Capture a bounded recording

```sh
thugsrf --device hackrf --frequency 433.92MHz --sample-rate 8000000 record sensor.cs8 --seconds 5
thugsrf --device rtl --frequency 433.92MHz --sample-rate 2400000 record sensor.cu8 --seconds 5
thugsrf --device audio record microphone.wav --seconds 5
```

These are independent examples; select the one matching your hardware. Raw IQ contains two bytes per complex sample: at 8 MS/s, five seconds is approximately 80 MB. Keep the metadata sidecar with its recording. Existing outputs are not silently overwritten.

## 3. Measure before identifying

```sh
thugsrf --frequency 433.92MHz analyze sensor.cs8 --png sensor-spectrum.png
thugsrf decode sensor.cs8 --mode ook
thugsrf decode sensor.cs8 --mode fsk
```

Review occupied bandwidth, peak frequencies, signal strength relative to noise and timing. Built-in OOK/FSK extraction produces waveform measurements rather than synchronized packets. A missing event may mean silence, unsuitable tuning, unsupported modulation, or inadequate capture quality.

## 4. Compare references and decode

```sh
thugsrf addon install
thugsrf addon enable all --kind identifiers
thugsrf identify sensor.cs8
thugsrf addon enable rtl433
thugsrf addon run rtl433 sensor.cs8
thugsrf frequency lookup --frequency 433.92MHz
```

Batch identification runs enabled compatible modules. A service-band match is weaker evidence than a CRC-checked packet. See [[Protocol-Catalog]] before interpreting confidence or coverage. [[SigID-Catalog]] adds offline waveform references after an explicit sync.

## 5. Save findings

```sh
thugsrf history
thugsrf history --export 1 > investigation.json
```

Replace `1` with your report ID. Preserve raw samples, sidecars, derived images, decoder JSON and software version together. Report what was measured, what was decoded and what remains a hypothesis. Use [[Wireshark-Export]] when a supported decoder recovers actual packets.

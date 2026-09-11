# RF recordings to Wireshark

`export-pcap` runs an enabled decoder and writes recovered packet bytes to a standard PCAP plus a `.pcap.json` provenance sidecar. In Recordings or Addons, **w** prepares an editable export command; all examples also work through the TUI `:` command bar.

```sh
thugsrf addon install
thugsrf addon enable ble-advertising
thugsrf addon enable packet-radio
thugsrf addon enable gsm-bcch

thugsrf export-pcap ble.cs8 ble.pcap --protocol ble \
  --frequency 2402000000 --sample-rate 8000000 --options '{"channel":37}'
thugsrf export-pcap packet-radio.wav ax25.pcap --protocol ax25
thugsrf export-pcap gsm.cs8 gsm.pcap --protocol gsm \
  --frequency 935000000 --sample-rate 8000000

wireshark ble.pcap
tshark -r ax25.pcap -V
```

| Protocol | Input | Exported bytes | Wireshark |
| --- | --- | --- | --- |
| BLE legacy 1M advertising | Centered signed/unsigned IQ, ≥2 MS/s | Access address + dewhitened PDU + valid CRC, without preamble | `LINKTYPE_BLUETOOTH_LE_LL` (251), `btle` / advertised names and services |
| AX.25 packet radio | WAV or centered narrow-FM IQ | Dire Wolf's CRC-checked frame bytes, FCS removed; bit-fixing disabled | `LINKTYPE_AX25` (3), `ax25`, APRS/application dissection where applicable |
| GSM BCCH | Centered GSM downlink IQ | Actual gr-gsm GSMTAP header and decoded BCCH message, wrapped in generated IPv4/UDP port 4729 headers | `LINKTYPE_RAW` (101), `gsmtap` / GSM radio-resource system information |

BLE export preserves repeated air packets; it suppresses duplicate recovery of the same packet by adjacent symbol-timing hypotheses. Packet radio supports `--options '{"baud":1200}'` (also 2400/4800/9600). GSM export collects BCCH only, excluding CCCH/paging and subscriber traffic. It does not send its generated IP/UDP envelopes to the network. GSM decoding requires the distribution `gr-gsm` GNU Radio Python modules; the CLI wrapper uses a dedicated offline flowgraph for export.

PCAP timestamps are **relative**, represented from Unix epoch zero: approximate sample positions for BLE, decoder-reported completion time for AX.25, and relative GSM frame numbers for BCCH. They are not an inferred wall-clock capture date. The GSM IP addresses describe an artificial localhost envelope, not RF transmitter addresses. Source recording parameters and timing conventions are retained in the sidecar. Exports refuse to overwrite either file. Zero recovered packets produces an explicitly empty PCAP; it does not imply no signal was present.

BLE and AX.25 exports are tested against Wireshark using known/generated RF/audio fixtures, including rejection of a corrupted BLE CRC. GSM encapsulation and the flowgraph's empty-input behavior are tested; successful BCCH RF decoding still needs validation against a clean capture.

This is packet export, not a way to make arbitrary raw IQ understandable to Wireshark. Noise, FFT bins, allocation guesses and tentative protocol labels remain JSON evidence. Existing Wi-Fi, Bluetooth and ZigBee PCAP/PCAPNG files can be analyzed by their respective addons; this release does not demodulate arbitrary Wi-Fi/ZigBee IQ into packets. AIS, POCSAG and other decoders currently retain structured/text results rather than exporting misleading synthetic packet formats.

Packet layout references: [libpcap BLE link type](https://www.tcpdump.org/linktypes/LINKTYPE_BLUETOOTH_LE_LL.html), [libpcap AX.25 link type](https://www.tcpdump.org/linktypes/LINKTYPE_AX25.html), [gr-gsm](https://github.com/ptrkrysik/gr-gsm), [Wireshark GSMTAP fields](https://www.wireshark.org/docs/dfref/g/gsmtap.html).

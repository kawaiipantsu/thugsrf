# Passive GSM security observations

The `gsm-security` identifier inspects an existing PCAP/PCAPNG using Wireshark's GSM dissectors. It reports signalled A5 cipher choices, GPRS GEA fields, identity requests, masked IMSI visibility and public cell identifiers. It can compare cells against a local baseline and an explicitly supplied home PLMN.

```sh
thugsrf addon enable gsm-security
thugsrf addon run gsm-security signalling.pcap
# Learn public cell identifiers from a reviewed reference capture:
thugsrf addon run gsm-security known-cells.pcap --options '{"learn_baseline":true}'
thugsrf addon run gsm-security new-cells.pcap --options '{"home_plmn":"23801"}'
```

- **A5/0** is represented by a command to disable ciphering. A5/1 and A5/2 are labelled with GSMA's legacy/weak-algorithm concerns. Other signalled A5 identifiers are named without claiming that they are secure or implemented by this tool. Cipher capability advertisements are not confused with a selected algorithm; a Ciphering Mode Command is still a command, not proof of subsequent air-interface state.
- **IMSI visibility** means the supplied capture contains a decoded IMSI field. Results mask all but the last three digits. A capture may already have been decrypted, so readable fields do not by themselves establish cleartext over the air. The source PCAP is left unchanged.
- **Identity requests** are legitimate GSM signalling as well as possible investigation evidence. The tool counts them and identifies the requested identity type; it does not solicit identities from handsets.
- **Cell changes** compare MCC/MNC/LAC/cell ID/ARFCN against `~/.local/share/thugsrf/gsm-cell-baseline.json`. Unknown cells or changed frequencies are anomalies, not proof of a false base station. Baselines only change with `learn_baseline=true`; no subscriber IMSIs are stored in them. A custom `baseline` path is supported.
- **Roaming context** compares an observed network's PLMN against the optional `home_plmn`. It cannot determine an individual subscriber's roaming status from broadcast cell data.

The BCCH PCAP exporter provides public cell information only. Cipher negotiations and identity responses require an existing capture containing those messages. The app has no active base-station impersonation, forced attachment, network downgrade, traffic interception or key-cracking feature. GSM false-base-station detection cannot reliably be reduced to one field or signal-strength threshold.

Tests use synthetic RR/MM/SI3 packets and a synthetic test-network IMSI, decoded by the real Wireshark dissector. They verify A5/0–3 interpretation, masking, identity-request counts, public cell parsing, baseline changes and PLMN context. They do not validate real-world IMSI-catcher detection.

References: [GSMA Security Algorithm Deployment Guidance](https://www.gsma.com/solutions-and-impact/technologies/security/wp-content/uploads/2022/09/FS.35-v3.0.pdf), [GSMA algorithms](https://www.gsma.com/solutions-and-impact/technologies/security/security-algorithms/), [Wireshark GSM fields](https://www.wireshark.org/docs/dfref/g/gsm_a.html).

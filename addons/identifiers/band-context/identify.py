#!/usr/bin/env python3
import json
import sys
r = json.load(sys.stdin)
f = r['input']['center_hz'] / 1e6
bands = [(433.05, 434.79, '433 MHz SRD', ['OOK', 'FSK']), (863, 870, 'European SRD', ['FSK', 'LoRa', 'OOK']), (2400, 2483.5, '2.4 GHz ISM', ['Wi-Fi', 'BLE', 'other'])]
json.dump({'api_version': 1, 'candidates': [{'label': label, 'modulations_to_test': mods, 'confidence': 'frequency-context-only'} for lo, hi, label, mods in bands if lo <= f <= hi], 'sources': ['https://efis.cept.org/'], 'note': 'Offline hints. Consult current national allocations. No live registry lookup performed.'}, sys.stdout)

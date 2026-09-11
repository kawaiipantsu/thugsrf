#!/usr/bin/env python3
"""THUGS(red) RF addon API v1; standard library only."""
import json
import math
import sys
request = json.load(sys.stdin)
assert request['api_version'] == 1
source = request['input']
assert source['format'] in ('cs8', 'cu8'), 'expected cs8/cu8'
with open(source['path'], 'rb') as stream:
    data = stream.read(2_000_000)
def value(x):
    return x - 127.5 if source['format'] == 'cu8' else (x if x < 128 else x - 256)
amplitudes = [math.hypot(value(data[i]), value(data[i+1])) for i in range(0, len(data)-1, 2)]
threshold = max(amplitudes, default=0) * 0.4
pulses = []
last = None
count = 0
for amp in amplitudes:
    state = amp > threshold
    if last is not None and state != last:
        pulses.append({'high': last, 'us': count / source['sample_rate'] * 1e6})
        count = 0
    count += 1
    last = state
if last is not None:
    pulses.append({'high': last, 'us': count / source['sample_rate'] * 1e6})
json.dump({'api_version': 1, 'decoder': 'ook-pulses', 'pulses': pulses[:10000], 'note': 'Envelope timings only; protocol unknown'}, sys.stdout)

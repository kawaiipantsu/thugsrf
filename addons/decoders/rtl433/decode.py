#!/usr/bin/env python3
"""Decode an existing IQ recording only; never open a radio."""
import json
import subprocess
import sys
import tempfile
r = json.load(sys.stdin)
s = r['input']
assert s['format'] in ('cs8', 'cu8'), 'rtl433 expects IQ, not WAV'
with open(s['path'], 'rb') as f:
    data = f.read(64_000_000)
if s['format'] == 'cs8':
    data = data.translate(bytes((i + 128) % 256 for i in range(256)))
with tempfile.NamedTemporaryFile(suffix='.cu8') as f:
    f.write(data)
    f.flush()
    result = subprocess.run(['rtl_433', '-c', '/dev/null', '-r', f.name, '-s', str(s['sample_rate']), '-f', str(s['center_hz']), '-F', 'json'], stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=45, check=False)
    events = []
    for line in result.stdout.splitlines():
        try:
            events.append(json.loads(line))
        except json.JSONDecodeError:
            pass
    if result.returncode != 0:
        raise RuntimeError(result.stderr.decode(errors='replace')[-2000:])
    json.dump({'api_version': 1, 'decoder': 'rtl_433', 'events': events[:200], 'sample_limit': 32_000_000, 'diagnostics': result.stderr.decode(errors='replace')[-2000:]}, sys.stdout)

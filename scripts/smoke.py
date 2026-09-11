#!/usr/bin/env python3
"""Real CLI integration tests using isolated user data; never opens RF hardware."""
import json
import math
import os
import pathlib
import shutil
import struct
import subprocess
import tempfile
import wave
root = pathlib.Path(__file__).resolve().parent.parent
binary = root / 'target/release/thugsrf'
if not binary.exists():
    binary = root / 'target/debug/thugsrf'
assert binary.exists(), 'run make build first'
with tempfile.TemporaryDirectory(prefix='thugsrf-smoke-') as tmp:
    tmp = pathlib.Path(tmp)
    env = dict(os.environ, XDG_CONFIG_HOME=str(tmp/'config'), XDG_DATA_HOME=str(tmp/'data'))
    def run(*args, ok=True):
        p = subprocess.run([str(binary), *map(str,args)], env=env, text=True, capture_output=True, timeout=30)
        assert (p.returncode == 0) == ok, (args,p.returncode,p.stdout,p.stderr)
        return p.stdout
    run('--help')
    run('config')
    run('config','set','frequency','7000000000',ok=False)
    run('config','set','fft_size','1000',ok=False)
    run('config','set','device','audio')
    assert '48000' in run('config')
    run('config','set','device','hackrf')
    samples = tmp/'tone.cs8'
    with samples.open('wb') as f:
        for i in range(16384):
            angle = 2*math.pi*128*i/2048
            f.write(struct.pack('bb', round(64*math.cos(angle)),round(64*math.sin(angle))))
    png = tmp/'spectrum.png'
    r = json.loads(run('analyze',samples,'--png',png))
    peak = r['report']['peaks'][0]
    assert abs(peak['frequency_hz'] - 434420000) < 1
    assert -7 < peak['power_dbfs'] < -5
    assert png.read_bytes().startswith(b'\x89PNG')
    stored = json.loads(run('history','--export',r['investigation_id']))
    assert stored['source'] == str(samples)
    empty = tmp/'empty.cs8'; empty.touch()
    run('analyze',empty,ok=False)
    run('replay',samples,ok=False)
    ook = tmp/'ook.cs8'
    run('encode',ook,'--bits','1100','--rate','8000000','--baud','1000')
    decoded = json.loads(run('decode',ook))
    assert len(decoded['pulses']) == 2
    assert decoded['pulses'][0]['high'] is True
    assert abs(decoded['pulses'][0]['microseconds']-2000) < .01
    run('encode',ook,'--bits','1',ok=False)
    afsk=tmp/'afsk.wav'
    run('encode',afsk,'--mode','afsk','--bits','10'*100)
    with wave.open(str(afsk)) as f:
        assert f.getframerate()==48000
        assert f.getnchannels()==1
    wav_report=json.loads(run('analyze',afsk,'--format','wav'))['report']
    assert wav_report['sample_rate']==48000 and wav_report['center_hz']==0
    audio=tmp/'fm.wav'
    run('demod',samples,audio)
    with wave.open(str(audio)) as f:
        assert 47000 <= f.getframerate() <= 49000
    target=tmp/'config/thugsrf/decoders/ook-pulses'
    shutil.copytree(root/'addons/decoders/ook-pulses',target)
    run('addon','run','ook-pulses',ook,ok=False)
    run('addon','toggle','ook-pulses')
    addon=json.loads(run('addon','run','ook-pulses',ook))
    assert len(addon['pulses'])==2
    assert abs(addon['pulses'][0]['us']-2000)<.01
    # Protocol wrapper exercises real rtl_433 if installed; silence is a valid no-events fixture.
    if shutil.which('rtl_433'):
        target=tmp/'config/thugsrf/decoders/rtl433'
        shutil.copytree(root/'addons/decoders/rtl433',target)
        run('addon','toggle','rtl433')
        result=json.loads(run('addon','run','rtl433',samples))
        assert isinstance(result['events'],list)
    # Malformed and hanging addons must fail without hanging the CLI.
    target=tmp/'config/thugsrf/identifiers/bad'
    target.mkdir(parents=True)
    (target/'addon.toml').write_text('api_version=1\nname="bad"\nkind="identifiers"\ndescription="test"\ncommand=["python3","bad.py"]\nenabled=true\ntimeout_seconds=1\n')
    (target/'bad.py').write_text('print("not json")')
    run('addon','run','bad',samples,ok=False)
    (target/'bad.py').write_text('import time; time.sleep(10)')
    run('addon','run','bad',samples,ok=False)
    run('ai','--input',samples,ok=False)  # Model must be selected explicitly.
print('PASS: CLI, FFT tone accuracy, PNG, SQLite export, WAV, OOK round-trip, addon execution/failures, TX guard')

#!/usr/bin/python3
"""RDS regression checks; optional MPX WAV fixture exercises real decoding.

Known fixture: windytan/redsea v1.3.0 test/resources/mpx-testfile-yksi.flac,
SHA256 c92b9c72f132e37cbe253a19a6ae17c52f17f5318672c391ce7f3b9657320eb1.
Convert to mono 16-bit WAV with sox, then pass its path to this script.
No radio hardware is opened.
"""
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import wave
import numpy as np
from scipy.signal import resample_poly

ROOT = Path(__file__).resolve().parent.parent
BINARY = ROOT/'target/release/thugsrf'
module = importlib.util.spec_from_file_location('rds', ROOT/'addons/_shared/v0_2/rds.py')
rds = importlib.util.module_from_spec(module)
module.loader.exec_module(rds)

with tempfile.TemporaryDirectory(prefix='thugsrf-rds-') as tmp:
    tmp = Path(tmp)
    env = dict(os.environ, XDG_CONFIG_HOME=str(tmp/'config'), XDG_DATA_HOME=str(tmp/'data'))
    def decode(path, fmt, rate=1000000):
        return subprocess.run([str(BINARY), '--device', 'rtl', '--sample-rate', str(rate), 'decode', str(path),
                               '--mode', 'rds', '--format', fmt], env=env,
                              capture_output=True, text=True, timeout=20)
    def wavfile(path, samples, rate):
        with wave.open(str(path), 'wb') as out:
            out.setnchannels(1); out.setsampwidth(2); out.setframerate(rate)
            out.writeframes(np.asarray(samples, dtype='<i2').tobytes())
    low = tmp/'audio.wav'
    wavfile(low, np.zeros(48000), 48000)
    bad = decode(low, 'wav')
    assert bad.returncode and 'ordinary audio' in bad.stderr, bad
    # Filtering must not change with I/O block boundaries.
    iq = .7*np.exp(1j*.5*np.sin(2*np.pi*57000*np.arange(600000)/1000000))
    whole = rds.FmMultiplex(1000000).process(iq)
    demod = rds.FmMultiplex(1000000)
    chunks = np.concatenate([demod.process(iq[i:i+12345]) for i in range(0, len(iq), 12345)])
    np.testing.assert_allclose(whole, chunks, atol=1e-12)
    if len(sys.argv) > 1:
        fixture = Path(sys.argv[1]).resolve()
        result = decode(fixture, 'wav')
        assert result.returncode == 0, result.stderr
        events = json.loads(result.stdout)['events']
        assert any(e.get('pi') == '0x6201' and e.get('prog_type') == 'Serious classical' for e in events), events
        with wave.open(str(fixture)) as w:
            rate = w.getframerate()
            mpx = np.frombuffer(w.readframes(w.getnframes()), '<i2').astype(float)/32768
        from math import gcd
        g = gcd(rate, 1000000)
        mpx = resample_poly(np.tile(mpx, 8), 1000000//g, rate//g)
        iq = .7*np.exp(1j*np.cumsum(mpx)*2*np.pi*75000/1000000)
        for fmt in ('cs8', 'cu8'):
            values = np.empty(len(iq)*2)
            values[::2], values[1::2] = iq.real*127, iq.imag*127
            raw = values.astype(np.int8) if fmt == 'cs8' else (values+127.5).astype(np.uint8)
            path = tmp/('station.'+fmt); path.write_bytes(raw.tobytes())
            result = decode(path, fmt)
            assert result.returncode == 0, result.stderr
            assert any(e.get('pi') == '0x6201' for e in json.loads(result.stdout)['events']), result.stdout
        # Feed actual FM IQ through the live worker with fake capture/audio devices.
        binpath = tmp/'bin'; binpath.mkdir()
        capture = binpath/'rtl_sdr'
        capture.write_text('#!/usr/bin/python3\nimport sys\nsys.stdout.buffer.write(open('+repr(str(tmp/'station.cu8'))+',"rb").read())\n')
        playback = binpath/'aplay'
        playback.write_text('#!/usr/bin/python3\nimport sys\nsys.stdin.buffer.read()\n')
        for path in (capture, playback): path.chmod(0o755)
        live_env = dict(env, PATH=str(binpath)+os.pathsep+env['PATH'])
        live = subprocess.run([str(BINARY), '--device', 'rtl', '--sample-rate', '1000000', '--frequency', '100MHz',
                               'listen', '--mode', 'wfm', '--seconds', '1'], env=live_env,
                              capture_output=True, text=True, timeout=20)
        assert live.returncode == 0 and '0x6201' in live.stdout, (live.stdout, live.stderr)
        print('PASS: real RDS MPX, signed/unsigned FM IQ, and live RDS with fake radio/audio devices')
    print('PASS: audio-only WAV rejected; FM multiplex filtering preserves block boundaries')

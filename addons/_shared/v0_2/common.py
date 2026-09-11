"""Bounded, offline signal I/O for the THUGS(red) RF protocol catalog."""
import json
import math
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import numpy as np
from scipy import signal

MAX_SAMPLES = 8_000_000

def result(module, events=None, candidates=None, notes=None, **extra):
    return dict(api_version=1, module=module, events=events or [], candidates=candidates or [],
                notes=notes or [], **extra)

def load_samples(request, limit=MAX_SAMPLES):
    src = request['input']; path = Path(src['path']); fmt = src['format']
    if fmt == 'wav':
        import wave
        try:
            with wave.open(str(path)) as w:
                rate, channels, width = w.getframerate(), w.getnchannels(), w.getsampwidth()
                raw = w.readframes(limit)
            if width == 1:
                values = (np.frombuffer(raw, np.uint8).astype(np.float32)-128)/128
            elif width in (2, 4):
                values = np.frombuffer(raw, '<i'+str(width)).astype(np.float32) / (2**(8*width-1))
            elif width == 3:
                b = np.frombuffer(raw, np.uint8).reshape(-1, 3).astype(np.int32)
                v = b[:, 0] | b[:, 1] << 8 | b[:, 2] << 16
                values = ((v ^ 0x800000)-0x800000).astype(np.float32)/8388608
            else:
                raise ValueError('unsupported WAV PCM width')
            samples = values.reshape(-1, channels).mean(axis=1)
        except wave.Error:
            from scipy.io import wavfile
            rate, samples = wavfile.read(path, mmap=True)
            samples = np.asarray(samples[:limit], dtype=np.float32)
            if samples.ndim > 1: samples = samples.mean(axis=1)
    elif fmt in ('cs8', 'cu8'):
        rate = int(src['sample_rate'])
        raw = np.fromfile(path, dtype=np.int8 if fmt == 'cs8' else np.uint8, count=limit*2)
        if len(raw) % 2: raise ValueError('truncated interleaved IQ sample')
        raw = raw.astype(np.float32)
        if fmt == 'cu8': raw -= 127.5
        raw = raw.reshape(-1, 2)/128
        samples = raw[:, 0]+1j*raw[:, 1]
    else:
        raise ValueError('expected cs8, cu8, or WAV; received '+fmt)
    if rate <= 0 or len(samples) < 256: raise ValueError('at least 256 samples and a positive sample rate required')
    if not np.isfinite(samples).all(): raise ValueError('non-finite input samples')
    return samples, int(rate)

def resample(samples, rate, target):
    g = math.gcd(int(rate), int(target))
    return signal.resample_poly(samples, target//g, rate//g)

def channel(request, target=48000, bandwidth=18000, mode='fm'):
    samples, rate = load_samples(request)
    if np.iscomplexobj(samples):
        wanted = float(request.get('options', {}).get('channel_hz', request['input']['center_hz']))
        offset = wanted-float(request['input']['center_hz'])
        if abs(offset)+bandwidth/2 >= rate/2: raise ValueError('selected channel is outside captured bandwidth')
        if offset:
            samples *= np.exp(-2j*np.pi*offset*np.arange(len(samples))/rate).astype(np.complex64)
        intermediate = max(target, 48000)
        if rate < intermediate: raise ValueError('IQ sample rate is too low for this decoder')
        # Filter before reducing rate; resample_poly provides antialias filtering.
        samples = resample(samples, rate, intermediate)
        cutoff = min(bandwidth/2, intermediate*.45)
        taps = signal.firwin(129, cutoff, fs=intermediate)
        samples = signal.lfilter(taps, [1], samples)
        if mode == 'am':
            samples = np.abs(samples)
        elif mode == 'real':
            samples = samples.real
        else:
            samples = np.angle(samples[1:]*samples[:-1].conj())
        rate = intermediate
    samples = samples.astype(np.float32)
    samples -= np.mean(samples)
    if rate != target: samples = resample(samples, rate, target)
    peak = np.max(np.abs(samples))
    if peak > 1e-9: samples = samples/peak*.8
    return samples, target

def pcm(samples):
    return (np.clip(samples, -1, 1)*32767).astype('<i2').tobytes()

def run_tool(args, data=None, timeout=40, limit=900_000):
    if shutil.which(args[0]) is None: raise ValueError('missing dependency: '+args[0])
    # Files bound parent memory even when a decoder is verbose.
    with tempfile.TemporaryFile() as out, tempfile.TemporaryFile() as err:
        p = subprocess.run(args, input=data, stdout=out, stderr=err, timeout=timeout, check=False)
        out.seek(0); text = out.read(limit+1)
        err.seek(0, 2); size = err.tell(); err.seek(max(0, size-8192)); log = err.read().decode(errors='replace')
    if p.returncode: raise ValueError(f'{args[0]} exited {p.returncode}: {log[-3000:]}')
    return text[:limit].decode(errors='replace'), log, len(text)>limit

def read_text(request, limit=2_000_000):
    with open(request['input']['path'], 'rb') as f:
        return f.read(limit).decode('utf-8', errors='replace')

def data_root():
    return Path(os.environ.get('XDG_DATA_HOME', str(Path.home()/'.local/share')))/'thugsrf'

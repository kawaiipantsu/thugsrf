"""FM multiplex/RDS helpers, embedded in the Rust audio and recording workers."""
import json
import math
import shutil
import subprocess
import tempfile
import threading
import wave

import numpy as np
from scipy import signal as dsp


class RdsDecoder:
    def __init__(self, rate, emit):
        if not shutil.which('redsea'):
            raise RuntimeError('RDS decoder unavailable: install redsea (see README)')
        if rate < 128000:
            raise ValueError('RDS needs FM multiplex at >=128 kHz, not ordinary audio WAV')
        self.log = tempfile.TemporaryFile()
        self.process = subprocess.Popen(
            ['redsea', '--input', 'mpx', '-r', str(rate)],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=self.log)
        self.error = None
        def read():
            try:
                for line in self.process.stdout:
                    event = json.loads(line)
                    if isinstance(event, dict):
                        emit(event)
            except Exception as exc:
                self.error = exc
        self.reader = threading.Thread(target=read, daemon=True)
        self.reader.start()

    def push(self, mpx):
        self.process.stdin.write((np.clip(mpx, -1, 1)*32767).astype('<i2').tobytes())

    def close(self):
        try:
            try:
                self.process.stdin.close()
            except BrokenPipeError:
                pass
            try:
                code = self.process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait()
                raise RuntimeError('RDS decoder timed out')
            self.reader.join(timeout=2)
            if code:
                self.log.seek(0)
                raise RuntimeError('RDS decoder failed: '+self.log.read(4000).decode(errors='replace'))
            if self.error:
                raise RuntimeError('RDS output failed: '+str(self.error))
        finally:
            if self.process.poll() is None:
                self.process.kill()
                self.process.wait()
            self.process.stdout.close()
            self.log.close()


class FmMultiplex:
    """Stateful channel filtering and phase demodulation across IQ blocks."""
    def __init__(self, rate):
        if not 240000 <= rate <= 20000000:
            raise ValueError('RDS IQ sample rate must be 240000..20000000 Hz')
        self.stages = []
        self.rate = rate
        while self.rate > 480000:
            factor = 4 if self.rate/4 >= 240000 else 2
            sos = dsp.butter(6, 0.8/factor, output='sos')
            self.stages.append([sos, np.zeros((len(sos), 2), complex), factor, 0])
            self.rate /= factor
        self.channel = dsp.butter(6, 100000, fs=self.rate, output='sos')
        self.state = np.zeros((len(self.channel), 2), complex)
        self.previous = 1+0j

    def process(self, iq):
        for stage in self.stages:
            sos, state, factor, offset = stage
            iq, stage[1] = dsp.sosfilt(sos, iq, zi=state)
            count = len(iq)
            iq = iq[offset::factor]
            stage[3] = (offset-count) % factor
        iq, self.state = dsp.sosfilt(self.channel, iq, zi=self.state)
        if not len(iq):
            return np.array([], dtype=float)
        mpx = np.angle(iq*np.conj(np.r_[self.previous, iq[:-1]]))/np.pi
        self.previous = iq[-1]
        return mpx


def decode_rds_file(spec):
    fmt, path, rate = spec['format'], spec['path'], spec['rate']
    if fmt not in ('cs8', 'cu8', 'wav'):
        raise ValueError('RDS input must be cs8, cu8, or mono 16-bit MPX WAV')
    events = []
    count = 0
    def emit(event):
        nonlocal count
        count += 1
        if len(events) < 2000:
            events.append(event)
    with open(path, 'rb') as source:
        wav = wave.open(source, 'rb') if fmt == 'wav' else None
        if wav:
            rate = wav.getframerate()
            if wav.getnchannels() != 1 or wav.getsampwidth() != 2:
                raise ValueError('RDS WAV must be mono 16-bit PCM FM multiplex')
            if rate < 128000 or rate > 2000000:
                raise ValueError('RDS MPX WAV sample rate must be 128000..2000000 Hz; ordinary audio has no RDS subcarrier')
        demod = None if wav else FmMultiplex(rate)
        decoder = RdsDecoder(rate if wav else demod.rate, emit)
        samples = 0
        limit = int(rate*120)
        truncated = False
        try:
            while samples < limit:
                size = min(131072, limit-samples)
                raw = wav.readframes(size) if wav else source.read(size*2)
                if not raw:
                    break
                if len(raw) % 2:
                    raise ValueError('truncated input sample')
                samples += len(raw)//2
                if wav:
                    mpx = np.frombuffer(raw, '<i2').astype(float)/32768
                else:
                    values = np.frombuffer(raw, np.int8 if fmt == 'cs8' else np.uint8).astype(float)
                    if fmt == 'cu8':
                        values -= 127.5
                    mpx = demod.process((values[0::2]+1j*values[1::2])/128)
                decoder.push(mpx)
            truncated = bool(wav.readframes(1) if wav else source.read(1))
        finally:
            decoder.close()
        if not samples:
            raise ValueError('empty RDS recording')
    return dict(decoder='redsea', events=events, groups=count,
                seconds=samples/rate, truncated=truncated or count > len(events),
                note='Tune the FM carrier to center. Up to 120 seconds and 2000 groups. '
                     'No decoded groups means no RDS was recovered, not proof that RDS is absent.')

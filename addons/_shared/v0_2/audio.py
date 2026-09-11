"""Digital audio and narrowband IQ decoder adapters."""
import re
import tempfile
import wave
import numpy as np
from scipy import signal
from common import channel, pcm, result, run_tool

MODES = {
    'pocsag': ['POCSAG512', 'POCSAG1200', 'POCSAG2400'],
    'flex': ['FLEX'],
    'morse': ['MORSE_CW'],
    'selective-call': ['DTMF', 'ZVEI1', 'ZVEI2', 'CCIR', 'EEA', 'EIA'],
    'digital-audio': ['AFSK1200', 'AFSK2400', 'FSK9600', 'FMSFSK'],
    'eas': ['EAS'],
}

def multimon(name, request):
    samples, _ = channel(request, 22050, mode='real' if name=='morse' else 'fm')
    command = ['multimon-ng', '-c', '-q', '-v', '0', '-t', 'raw']
    for mode in MODES[name]: command += ['-a', mode]
    if request.get('options',{}).get('invert',False): command+=['-i']
    command += ['-']
    text, log, truncated = run_tool(command, pcm(samples))
    events = [dict(text=line, evidence='decoder-output') for line in text.splitlines() if line.strip()][:200]
    return result(name, events=events, diagnostics=log, truncated=truncated,
                  notes=['Decoded from bounded mono audio/discriminator input. Tune the carrier to center or set channel_hz.',
                         'Decoder output may need external validation; an empty result means no supported message was recovered.'])

def packet_radio(request):
    baud = int(request.get('options', {}).get('baud', 1200))
    if baud not in (1200, 2400, 4800, 9600): raise ValueError('baud must be 1200, 2400, 4800, or 9600')
    samples, rate = channel(request)
    with tempfile.NamedTemporaryFile(suffix='.wav') as f:
        with wave.open(f.name, 'wb') as w:
            w.setnchannels(1); w.setsampwidth(2); w.setframerate(rate); w.writeframes(pcm(samples))
        text, log, trunc = run_tool(['atest', '-h', '-F', '0', '-B', str(baud), f.name])
    text=re.sub(r'\x1b\[[0-?]*[ -/]*[@-~]','',text)
    decoded = re.findall(r'DECODED\[.*?(?=\nDECODED\[|\Z)', text, flags=re.S)
    packets=[];events=[]
    for block in decoded[:1000]:
        length=re.search(r'length = (\d+)',block);timestamp=re.search(r'DECODED\[\d+\] (\d+):(\d+(?:\.\d+)?)',block)
        raw=bytearray();expected=0
        for line in block.splitlines():
            match=re.match(r'^\s+([0-9a-fA-F]{3,}):  (.{1,48})',line)
            if not match:continue
            if int(match[1],16)!=expected:raise ValueError('non-contiguous atest hexadecimal dump')
            chunk=bytes.fromhex(match[2].strip());raw.extend(chunk);expected+=len(chunk)
        if raw and length and len(raw)==int(length[1]) and timestamp:
            seconds=int(timestamp[1])*60+float(timestamp[2]);packets.append((seconds,bytes(raw)))
            events.append(dict(frame=block[:3000],packet_hex=raw.hex(),evidence='Dire Wolf CRC-checked AX.25 frame; FCS stripped by decoder'))
    from pcapio import export
    artifact=export(request,3,packets,'AX.25','Decoder-reported frame completion time, millisecond precision; FCS stripped')
    return result('packet-radio',events=events[:100],diagnostics=(text if not events else log)[-4000:],truncated=trunc,**artifact)

def morse_envelope(request):
    samples, rate = channel(request, 8000, mode='real')
    analytic = np.abs(signal.hilbert(samples))
    smooth = np.convolve(analytic, np.ones(80)/80, mode='same')
    lo, hi = np.percentile(smooth, [10, 95])
    if hi < .005 or hi < lo*2: return result('morse-id', notes=['No reliable on/off envelope.'])
    on = smooth > (lo+hi)/2
    boundaries = np.r_[0, np.flatnonzero(on[1:] != on[:-1])+1, len(on)]
    runs = [(bool(on[a]), (b-a)/rate) for a,b in zip(boundaries[:-1], boundaries[1:]) if (b-a)/rate>.008]
    marks = np.array([d for state,d in runs if state])
    if len(marks)<4: return result('morse-id', notes=['Too few marks to estimate Morse timing.'])
    unit = np.percentile(marks, 25)
    ratios = marks/unit
    fit = float(np.mean(np.minimum(abs(ratios-1), abs(ratios-3)) < .45))
    candidates=[]
    if fit>.75 and .018<unit<.4:
        candidates=[dict(label='Morse/CW timing candidate', evidence='on/off marks fit 1:3 timing',
                         timing_fit=fit, estimated_wpm=round(1.2/unit,1), confidence='waveform-candidate')]
    return result('morse-id', candidates=candidates, notes=['Timing similarity alone does not prove Morse; use the morse decoder to recover text.'])

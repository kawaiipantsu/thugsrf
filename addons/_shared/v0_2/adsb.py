"""DF17/DF18 extended squitter: PPM demodulation and strict CRC-24 validation."""
import re
import numpy as np
from common import load_samples, resample, read_text, result

def crc(data):
    value=int.from_bytes(data,'big')
    for bit in range(len(data)*8-1,23,-1):
        if value&(1<<bit):value^=0x1fff409<<(bit-24)
    return value&0xffffff

def decode_frame(data):
    if len(data)!=14 or data[0]>>3 not in (17,18) or crc(data)!=0:return None
    bits=''.join(f'{b:08b}' for b in data);tc=data[4]>>3
    event=dict(protocol='ADS-B',df=data[0]>>3,icao=data[1:4].hex().upper(),type_code=tc,frame=data.hex().upper(),evidence='valid DF17/18 CRC-24')
    if 1<=tc<=4:
        chars='#ABCDEFGHIJKLMNOPQRSTUVWXYZ#####_###############0123456789######'
        event['callsign']=''.join(chars[int(bits[i:i+6],2)] for i in range(40,88,6)).replace('_',' ').strip()
    return event

def decode(request):
    frames=[]
    if request['input']['format']=='hex':
        frames=[bytes.fromhex(x) for x in re.findall(r'(?i)\b[0-9a-f]{28}\b',read_text(request))]
    else:
        samples,rate=load_samples(request)
        if not np.iscomplexobj(samples) or rate<2_000_000:raise ValueError('ADS-B needs complex IQ at >=2 MS/s')
        power=np.abs(resample(samples,rate,2_000_000))
        if len(power)<240:return result('adsb')
        # At 2 MS/s, preamble high slots are 0,2,7,9 of 16 half-microsecond slots.
        n=len(power)-239
        highs=np.minimum.reduce([power[i:i+n] for i in (0,2,7,9)])
        lows=np.maximum.reduce([power[i:i+n] for i in (1,3,4,5,6,8,10,11,12,13,14,15)])
        indices=np.flatnonzero((highs>lows*1.8)&(highs>.05))[:10000]
        for start in indices:
            data=power[start+16:start+240].reshape(-1,2)
            frames.append(np.packbits(data[:,0]>data[:,1]).tobytes())
    events=[];seen=set()
    for f in frames:
        event=decode_frame(f)
        if event and f not in seen:events.append(event);seen.add(f)
    return result('adsb',events=events[:200],notes=['Decodes CRC-valid DF17/18 ICAO/type/callsign metadata; no CPR position pairing or aircraft-owner lookup. Tune to 1090 MHz.'])

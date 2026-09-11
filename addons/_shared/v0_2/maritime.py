import re
import tempfile
import wave
import numpy as np
from common import channel, pcm, result, run_tool, read_text

def sentence(line):
    line=line.strip()
    if '*' not in line or not line.startswith(('!','$')): return None
    body,check=line[1:].rsplit('*',1)
    value=0
    for c in body:value^=ord(c)
    try:
        if value!=int(check[:2],16):return None
    except ValueError:return None
    return body.split(',')

def ais_messages(text):
    events=[];pending={}
    for line in text.splitlines():
        at=line.find('!AI')
        if at<0:continue
        fields=sentence(line[at:])
        if not fields or fields[0] not in ('AIVDM','AIVDO') or len(fields)!=7:continue
        try:
            total,index=int(fields[1]),int(fields[2]);fill=int(fields[6])
            if not (1<=index<=total<=9 and 0<=fill<=5):continue
            key=(fields[3],fields[4],total)
            if index==1:pending[key]=[]
            if key not in pending or len(pending[key])!=index-1:continue
            pending[key].append(fields[5])
            if index!=total:continue
            payload=''.join(pending.pop(key));bits=''
            for c in payload:
                n=ord(c)-48
                if n>40:n-=8
                if not 0<=n<64:raise ValueError('invalid AIS payload character')
                bits+=f'{n:06b}'
            if fill:bits=bits[:-fill]
            if len(bits)<38:continue
            def u(start,count):return int(bits[start:start+count],2)
            def signed(start,count):
                v=u(start,count);return v-(1<<count) if v&(1<<(count-1)) else v
            typ=u(0,6);event=dict(message_type=typ,mmsi=f'{u(8,30):09d}',channel=fields[4],evidence='checksum-valid AIS NMEA payload',payload_bits=len(bits))
            if typ in (1,2,3) and len(bits)>=168:
                lon=signed(61,28)/600000;lat=signed(89,27)/600000
                if abs(lat)<=90 and abs(lon)<=180:event.update(latitude=lat,longitude=lon)
                if u(50,10)!=1023:event['speed_knots']=u(50,10)/10
                if u(116,12)<3600:event['course_degrees']=u(116,12)/10
            elif typ==18 and len(bits)>=168:
                lon=signed(57,28)/600000;lat=signed(85,27)/600000
                if abs(lat)<=90 and abs(lon)<=180:event.update(latitude=lat,longitude=lon)
            events.append(event)
        except (ValueError,IndexError):continue
    return events[:100]

def ais(request):
    if request['input']['format']=='nmea':text=read_text(request);log=''
    else:
        samples,rate=channel(request,48000,24000)
        with tempfile.NamedTemporaryFile(suffix='.wav') as f:
            with wave.open(f.name,'wb') as w:
                w.setnchannels(1);w.setsampwidth(2);w.setframerate(rate);w.writeframes(pcm(samples))
            text,log,_=run_tool(['atest','-B','AIS',f.name])
    return result('ais',events=ais_messages(text),diagnostics=(text+'\n'+log)[-4000:],
                  notes=['Single-channel AIS A or B; use channel_hz for the selected carrier. NMEA input checks transport checksum; audio decoding uses Dire Wolf frame checks.'])

def atis(request):
    samples,rate=channel(request,48000,18000)
    # Noncoherent AFSK mark/space energy, followed by symbol timing hypotheses.
    sps=40;t=np.arange(sps)/rate
    a=np.abs(np.convolve(samples,np.exp(2j*np.pi*1300*t),mode='valid'))**2
    b=np.abs(np.convolve(samples,np.exp(2j*np.pi*2100*t),mode='valid'))**2
    sequences=[]
    for phase in range(sps):
        bits=(a[phase::sps]>b[phase::sps]).astype(np.uint8)
        for offset in range(10):
            size=(len(bits)-offset)//10
            if size<12:continue
            words=bits[offset:offset+size*10].reshape(-1,10)
            values=words[:,:7]@(1<<np.arange(7))
            checks=words[:,7:]@np.array([4,2,1])
            valid=checks==7-words[:,:7].sum(axis=1)
            for start in np.flatnonzero(valid & (values==121)):
                end=min(start+32,size)
                if end-start>=12 and np.mean(valid[start:end])>.85:
                    seq=[int(v) for v,ok in zip(values[start:end],valid[start:end]) if ok]
                    if seq not in sequences:sequences.append(seq)
    return result('atis-symbols',events=[dict(symbols=s,evidence='121 format symbol and zero-count-valid 10-bit symbol run',validation='partial-symbol-sequence') for s in sequences[:20]],
                  notes=['Marine ATIS/DSC AFSK symbol extraction at 1200 baud, 1300/2100 Hz. Does not validate complete DSC diversity/repetition/ECC or assert a vessel identity. Aviation voice ATIS is a different service.'])

def nmea(request):
    events=[]
    for line in read_text(request).splitlines():
        f=sentence(line)
        if not f:continue
        typ=f[0][-3:]
        try:
            def coord(value,hem):
                if not value:return None
                n=float(value);v=int(n//100)+(n%100)/60
                return -v if hem in ('S','W') else v
            if typ=='GGA' and len(f)>=10:
                event=dict(sentence=f[0],utc=f[1],fix_quality=int(f[6] or 0),satellites=int(f[7] or 0),evidence='NMEA checksum')
                if event['fix_quality']>0:event.update(latitude=coord(f[2],f[3]),longitude=coord(f[4],f[5]),altitude_m=float(f[9] or 0))
                events.append(event)
            elif typ=='RMC' and len(f)>=10:
                event=dict(sentence=f[0],utc=f[1],status=f[2],date=f[9],evidence='NMEA checksum')
                if f[2]=='A':event.update(latitude=coord(f[3],f[4]),longitude=coord(f[5],f[6]))
                events.append(event)
        except ValueError:continue
    return result('gps-nmea',events=events[:100],notes=['Decodes receiver-generated NMEA text, not RF samples. A checksum does not authenticate the transmitter.'])

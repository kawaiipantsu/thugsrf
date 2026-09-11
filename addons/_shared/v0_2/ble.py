"""Legacy BLE 1M advertising demodulation, dewhitening and CRC validation."""
import numpy as np
from common import load_samples,resample,result

AA=np.unpackbits(np.frombuffer(bytes.fromhex('d6be898e'),np.uint8),bitorder='little')

def whitening(bits,channel):
    state=channel|0x40;out=[]
    for value in bits:
        feedback=state&1;out.append(int(value)^feedback)
        if feedback:state^=0x88
        state>>=1
    return np.array(out,dtype=np.uint8)

def crc_bytes(data):
    state=0x555555
    for byte in data:
        for i in range(8):
            feedback=((state>>23)&1)^((byte>>i)&1)
            state=(state<<1)&0xffffff
            if feedback:state^=0x65b
    return int(f'{state:024b}'[::-1],2).to_bytes(3,'little')

def advertising_fields(payload):
    fields={};i=0
    while i<len(payload):
        length=payload[i]
        if not length or i+1+length>len(payload):break
        typ=payload[i+1];data=payload[i+2:i+1+length]
        if typ in (8,9):fields['advertised_name']=data.decode(errors='replace')
        if typ in (2,3):fields['service_uuids16']=[f'{int.from_bytes(data[j:j+2],"little"):04x}' for j in range(0,len(data)-1,2)]
        if typ==0xff and len(data)>=2:fields['company_id']=int.from_bytes(data[:2],'little')
        i+=length+1
    return fields

def decode(request):
    samples,rate=load_samples(request)
    if not np.iscomplexobj(samples) or rate<2_000_000:raise ValueError('BLE 1M needs complex IQ >=2 MS/s')
    center=request['input']['center_hz'];options=request.get('options',{})
    channels={37:2402000000,38:2426000000,39:2480000000}
    ch=int(options.get('channel',min(channels,key=lambda k:abs(channels[k]-center))))
    if ch not in channels:raise ValueError('legacy advertising channel must be 37, 38, or 39')
    offset=channels[ch]-center
    if abs(offset)+1_000_000>rate/2:raise ValueError('BLE advertising carrier outside capture; choose channel 37/38/39 or retune')
    if offset:samples*=np.exp(-2j*np.pi*offset*np.arange(len(samples))/rate).astype(np.complex64)
    samples=resample(samples,rate,2_000_000)
    phase=np.angle(samples[1:]*samples[:-1].conj());events=[];seen=set();packets=[];last_packet={}
    for clock in (0,1):
        n=(len(phase)-clock)//2
        bits=(phase[clock:clock+n*2].reshape(-1,2).mean(axis=1)>0).astype(np.uint8)
        if len(bits)<100:continue
        correlation=np.correlate(bits.astype(np.int16)*2-1,AA.astype(np.int16)*2-1,'valid')
        for start in np.flatnonzero(correlation==32)[:2000]:
            remaining=bits[start+32:start+32+42*8]
            if len(remaining)<40:continue
            unwhite=np.packbits(whitening(remaining,ch),bitorder='little').tobytes()
            length=unwhite[1]&63;pdu_type=unwhite[0]&15
            if not 6<=length<=37 or pdu_type not in (0,2,4,6) or len(unwhite)<length+5:continue
            pdu=unwhite[:length+2];crc=unwhite[length+2:length+5]
            if crc_bytes(pdu)!=crc:continue
            timestamp=(start*2+clock)/2_000_000
            # Adjacent timing hypotheses can recover the same air packet twice.
            prior=last_packet.setdefault(pdu,[])
            if not any(abs(timestamp-t)<.000005 for t in prior):
                packets.append((timestamp,bytes.fromhex('d6be898e')+pdu+crc));prior.append(timestamp)
            if pdu in seen:continue
            seen.add(pdu)
            event=dict(protocol='BLE legacy 1M advertising',channel=ch,pdu_type=pdu_type,
                       advertiser=':'.join(f'{v:02X}' for v in pdu[2:8][::-1]),address_type='random' if pdu[0]&0x40 else 'public',
                       evidence='advertising access address + dewhitening + valid CRC-24')
            if pdu_type in (0,2,4,6):event.update(advertising_fields(pdu[8:]))
            events.append(event)
    from pcapio import export
    artifact=export(request,251,packets,'BLE legacy 1M advertising','Approximate access-address start time from IQ sample position')
    return result('ble-advertising',events=events[:100],**artifact,notes=['Legacy 1M advertising only; no hopping connection following, LE Coded/2M PHY or encrypted payload decoding. Names/services are device claims.'])

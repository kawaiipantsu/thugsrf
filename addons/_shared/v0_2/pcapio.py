"""Write real recovered frames to classic PCAP, with an explicit provenance sidecar."""
import json,math,struct
from pathlib import Path

def export(request,linktype,packets,protocol,timing):
    target=request.get('options',{}).get('pcap')
    if not target:return {}
    path=Path(target).absolute();sidecar=Path(str(path)+'.json')
    if path.exists() or sidecar.exists():raise ValueError('PCAP or sidecar already exists; choose a new output path')
    packets=sorted(packets,key=lambda p:p[0])
    if len(packets)>10000:raise ValueError('PCAP export limited to 10000 frames per run')
    source=dict(request['input'])
    if source.get('format')=='wav':
        import wave
        try:
            with wave.open(source['path'],'rb') as w:source['sample_rate']=w.getframerate()
        except wave.Error:
            from scipy.io import wavfile
            source['sample_rate']=wavfile.read(source['path'],mmap=True)[0]
    metadata=dict(protocol=protocol,linktype=linktype,packets=len(packets),source=source,timing=timing,note='Recovered protocol frames, not raw RF IQ. Timestamps are relative to the recording, represented from epoch zero; no absolute capture date is inferred.')
    created=[]
    try:
        with path.open('xb') as f:
            created.append(path)
            f.write(struct.pack('<IHHIIII',0xa1b2c3d4,2,4,0,0,65535,linktype))
            for timestamp,packet in packets:
                if not math.isfinite(timestamp) or timestamp<0 or timestamp>=2**32 or len(packet)>65535:raise ValueError('invalid packet timestamp/length')
                micros=round(timestamp*1e6);sec,usec=divmod(micros,1000000)
                f.write(struct.pack('<IIII',sec,usec,len(packet),len(packet)));f.write(packet)
        with sidecar.open('x') as f:created.append(sidecar);json.dump(metadata,f,indent=2)
    except Exception:
        for item in created:item.unlink(missing_ok=True)
        raise
    return dict(pcap=str(path),pcap_metadata=str(sidecar),packet_count=len(packets))

def udp_ip(payload,port=4729):
    """Synthetic localhost IPv4/UDP envelope for genuine GSMTAP message bytes. No network I/O."""
    size=20+8+len(payload)
    if size>65535:raise ValueError('GSMTAP packet too large')
    header=struct.pack('!BBHHHBBH4s4s',0x45,0,size,0,0,64,17,0,b'\x7f\x00\x00\x01',b'\x7f\x00\x00\x01')
    total=sum(struct.unpack('!10H',header))
    while total>>16:total=(total&65535)+(total>>16)
    header=header[:10]+struct.pack('!H',(~total)&65535)+header[12:]
    return header+struct.pack('!HHHH',port,port,len(payload)+8,0)+payload

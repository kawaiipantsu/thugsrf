"""MPEG-TS packet and PAT program discovery from an existing transport stream."""
from collections import Counter
from common import result

def decode(request):
    with open(request['input']['path'],'rb') as f:data=f.read(8_000_000)
    offset=next((i for i in range(min(188,len(data))) if all(i+j*188<len(data) and data[i+j*188]==0x47 for j in range(5))),None)
    if offset is None:return result('tv-transport',notes=['No repeated 188-byte MPEG transport packet sync found.'])
    pids=Counter();scrambled=Counter();errors=0;programs={}
    for i in range(offset,len(data)-187,188):
        p=data[i:i+188]
        if p[0]!=0x47:errors+=1;continue
        pid=((p[1]&31)<<8)|p[2];pids[pid]+=1
        if p[3]&0xc0:scrambled[pid]+=1
        # Parse single-packet PAT sections only. Validate their MPEG CRC before reporting programs.
        if pid!=0 or not p[1]&0x40 or p[1]&0x80:continue
        control=(p[3]>>4)&3
        if control not in (1,3):continue
        pos=4+(1+p[4] if control==3 else 0)
        if pos>=188:continue
        pos+=1+p[pos]
        if pos+8>=188 or p[pos]!=0:continue
        size=3+(((p[pos+1]&15)<<8)|p[pos+2]);section=p[pos:pos+size]
        if len(section)!=size or size<12:continue
        crc=0xffffffff
        for b in section:
            crc^=b<<24
            for _ in range(8):crc=((crc<<1)^0x04c11db7 if crc&0x80000000 else crc<<1)&0xffffffff
        if crc:continue
        for j in range(8,len(section)-4,4):
            number=int.from_bytes(section[j:j+2],'big');pmt=int.from_bytes(section[j+2:j+4],'big')&8191
            if number:programs[number]=pmt
    return result('tv-transport',events=[dict(program=k,pmt_pid=v,evidence='CRC-valid single-packet PAT') for k,v in programs.items()],
                  packet_pids=dict(pids),scrambled_packet_pids=dict(scrambled),sync_errors=errors,
                  notes=['Input is a demodulated MPEG-TS file. Scrambling-control flags are observable, but no TV RF demodulation or content decryption is performed.'])

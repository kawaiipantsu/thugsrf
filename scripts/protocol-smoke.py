#!/usr/bin/python3
"""Offline known frames, independent decoder fixture, negative evidence and reference import."""
import json, os, pathlib, struct, subprocess, tempfile, sys
sys.dont_write_bytecode=True
import numpy as np
ROOT=pathlib.Path(__file__).resolve().parent.parent
sys.path.insert(0,str(ROOT/'addons/_shared/v0_2'))
import adsb, maritime, frequencydb, ble
with tempfile.TemporaryDirectory() as t:
    temp=pathlib.Path(t);os.environ['XDG_DATA_HOME']=str(temp/'data')
    def run(name,path,fmt,rate=48000,center=433920000,options=None):
        request=dict(api_version=1,input=dict(path=str(path),format=fmt,sample_rate=rate,center_hz=center),options=options or {})
        out=subprocess.check_output(['/usr/bin/python3',str(ROOT/'addons/_shared/v0_2/runner.py'),name],input=json.dumps(request).encode(),cwd=temp,timeout=125)
        result=json.loads(out);assert result.get('status')!='error',(name,result);return result
    frame=bytes.fromhex('8D4840D6202CC371C32CE0576098')
    event=adsb.decode_frame(frame);assert event['callsign']=='KLM1023'
    assert adsb.decode_frame(frame[:-1]+bytes([frame[-1]^1])) is None
    path=temp/'adsb.hex';path.write_text(frame.hex());assert run('adsb',path,'hex')['events'][0]['icao']=='4840D6'
    # Independent known ADS-B frame rendered as PPM IQ.
    bits=np.unpackbits(np.frombuffer(frame,np.uint8));power=np.zeros(1024);power[[40,42,47,49]]=0.8
    for i,bit in enumerate(bits):power[56+i*2+(0 if bit else 1)]=0.8
    raw=np.zeros(2048,dtype=np.int8);raw[::2]=(power*127).astype(np.int8);path=temp/'adsb.cs8';raw.tofile(path)
    assert run('adsb',path,'cs8',2000000,1090000000)['events']
    body='GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,545.4,M,46.9,M,,'
    checksum=0
    for c in body:checksum^=ord(c)
    path=temp/'gps.nmea';path.write_text(f'${body}*{checksum:02X}\n');assert run('gps-nmea',path,'nmea')['events']
    assert maritime.sentence('$'+body+'*00') is None
    # Dire Wolf generator and decoder provide independent AX.25/FCS verification.
    path=temp/'packet.wav';subprocess.run(['gen_packets','-o',str(path)],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,check=True)
    r=run('packet-radio',path,'wav',options={'pcap':str(temp/'ax25-export.pcap')});assert r['events'],r
    assert r['packet_count']==4,r
    dissected=subprocess.check_output(['tshark','-r',str(temp/'ax25-export.pcap'),'-V'],stderr=subprocess.DEVNULL,text=True)
    assert 'WB2OSZ' in dissected and 'AX.25' in dissected
    pdu=bytes.fromhex('000c665544332211050954455354')
    assert ble.crc_bytes(pdu)==bytes.fromhex('c36b5a')
    packet=bytes.fromhex('d6be898e')+pdu+bytes.fromhex('c36b5a')
    path=temp/'ble.pcap';path.write_bytes(struct.pack('<IHHIIII',0xa1b2c3d4,2,4,0,0,65535,251)+struct.pack('<IIII',0,0,len(packet),len(packet))+packet)
    assert 'TEST' in json.dumps(run('bluetooth-pcap',path,'pcap'))
    bits=np.r_[np.tile([0,1],8),ble.AA,ble.whitening(np.unpackbits(np.frombuffer(pdu+ble.crc_bytes(pdu),np.uint8),bitorder='little'),37),np.zeros(64,dtype=np.uint8)]
    phase=np.cumsum(np.repeat(bits.astype(float)*2-1,2))*np.pi/4
    iq=np.exp(1j*phase)*90;raw=np.empty(len(iq)*2,dtype=np.int8);raw[::2]=iq.real.astype(np.int8);raw[1::2]=iq.imag.astype(np.int8)
    path=temp/'ble.cs8';raw.tofile(path)
    r=run('ble-advertising',path,'cs8',2000000,2402000000,{'pcap':str(temp/'ble-export.pcap')})
    assert r['events'][0]['advertised_name']=='TEST' and r['packet_count']==1,r
    dissected=subprocess.check_output(['tshark','-r',str(temp/'ble-export.pcap'),'-V'],stderr=subprocess.DEVNULL,text=True)
    assert 'TEST' in dissected and 'Incorrect CRC' not in dissected
    import pcapio
    try:pcapio.export({'input':{},'options':{'pcap':str(temp/'ble-export.pcap')}},251,[],'test','test');raise AssertionError('overwrote existing PCAP')
    except ValueError:pass
    gsmtap=struct.pack('!BBBBHbbIBBBB',2,4,1,0,1,-55,20,123,1,0,0,0)+bytes(23)
    pcapio.export({'input':{},'options':{'pcap':str(temp/'gsm-envelope.pcap')}},101,[(0.,pcapio.udp_ip(gsmtap))],'GSMTAP fixture','synthetic timing')
    dissected=subprocess.check_output(['tshark','-r',str(temp/'gsm-envelope.pcap'),'-T','fields','-e','gsmtap.arfcn'],stderr=subprocess.DEVNULL,text=True)
    assert dissected.strip()=='1',dissected
    # A corrupted BLE checksum must be rejected by an independent dissector.
    bad=bytearray(packet);bad[-1]^=1
    path=temp/'bad-ble.pcap';path.write_bytes(struct.pack('<IHHIIII',0xa1b2c3d4,2,4,0,0,65535,251)+struct.pack('<IIII',0,0,len(bad),len(bad))+bad)
    check=subprocess.check_output(['tshark','-r',str(path),'-Y','btle.crc.incorrect','-T','fields','-e','frame.number'],stderr=subprocess.DEVNULL)
    assert check.strip()==b'1'
    # POCSAG BCH(31,21), parity and alpha fixture, decoded by multimon-ng.
    def pocsag_word(data):
        value=data<<10
        for bit in range(30,9,-1):
            if value&(1<<bit):value^=0x769<<(bit-10)
        value=(data<<10)|(value&1023)
        return (value<<1)|(value.bit_count()&1)
    words=[0x7a89c197]*16;words[0]=pocsag_word((123456//8)<<2|3)
    bits=''.join(f'{ord(c):07b}'[::-1] for c in 'THUGSRF TEST ');bits=bits.ljust((len(bits)+19)//20*20,'0')
    for i in range(len(bits)//20):words[1+i]=pocsag_word((1<<20)|int(bits[i*20:i*20+20],2))
    symbols='10'*400+f'{0x7cd215d8:032b}'+''.join(f'{w:032b}' for w in words)+f'{0x7cd215d8:032b}'+f'{0x7a89c197:032b}'*16+'10'*100
    import wave
    path=temp/'pocsag.wav'
    with wave.open(str(path),'wb') as w:
        w.setnchannels(1);w.setsampwidth(2);w.setframerate(48000);w.writeframes((np.repeat(np.array([int(x)*2-1 for x in symbols]),40)*12000).astype('<i2').tobytes())
    assert 'THUGSRF TEST' in json.dumps(run('pocsag',path,'wav',options={'invert':True}))
    # Empty PCAP should not invent device identity or encryption.
    path=temp/'empty.pcap';path.write_bytes(struct.pack('<IHHIIII',0xa1b2c3d4,2,4,0,0,65535,127))
    for name in ['wifi-id','bluetooth-id','hid-id','encryption-id','wifi-pcap','bluetooth-pcap','zigbee-pcap']:
        r=run(name,path,'pcap');assert not r['events'] and not r['candidates'],r
    path=temp/'noise.cs8';np.random.default_rng(11).integers(-15,16,200000,dtype=np.int8).tofile(path)
    assert not run('encryption-id',path,'cs8',2000000)['candidates']
    assert not run('ble-advertising',path,'cs8',2000000,2402000000)['events']
    # Exercise every catalog module's declared input path without invoking heavy backends.
    catalog=json.loads((ROOT/'addons/_shared/v0_2/catalog.json').read_text())
    for name,spec in catalog.items():
        if spec['backend']=='identify' and 'cs8' in spec['formats']:run(name,path,'cs8',2000000)
    assert frequencydb.lookup(145600000,12500,'US')[0]['region']=='US'
    html=temp/'reference.html';html.write_text('<table><tr><td>145,600 MHz</td><td>Fixture repeater</td></tr><tr><td>2026-09-11</td><td>Not a frequency</td></tr></table>')
    assert frequencydb.import_file(html,'https://example.org/fixture')['imported']==1
    assert frequencydb.import_file(html,'https://example.org/fixture')['imported']==0
    assert frequencydb.lookup(145600000)[0]['label']=='Fixture repeater'
    assert frequencydb.lookup(144000000)==[]
print('PASS: ADS-B CRC/callsign/PPM, NMEA checksum, AX.25 WAV, BLE/AX.25/GSMTAP PCAP export/dissection, PCAP negatives, catalog identifiers, sourced frequency imports')

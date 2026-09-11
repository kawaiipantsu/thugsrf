#!/usr/bin/python3
"""Offline SigID pagination/ranking and GSM security observations through real Wireshark."""
import json,os,pathlib,struct,sys,tempfile
sys.dont_write_bytecode=True
ROOT=pathlib.Path(__file__).resolve().parent.parent
sys.path.insert(0,str(ROOT/'addons/_shared/v0_2'))
import sigid,gsm_security,pcapio
with tempfile.TemporaryDirectory() as t:
    p=pathlib.Path(t);os.environ['XDG_DATA_HOME']=str(p/'data')
    def row(title,freq,bw):return dict(fullurl='https://www.sigidwiki.com/wiki/'+title,printouts=dict(Frequencies=freq,Bandwidth=bw,Modulation=['FSK'],Mode=['NFM'],Location=['Worldwide']))
    responses=iter([{'query-continue-offset':1,'query':{'results':{'POCSAG':row('POCSAG',[25000000,932000000],[9000])}}},{'query':{'results':{'Suspect':row('Suspect',[452.9375,457937500],[8000])}}}])
    calls=[]
    def fetch(params):calls.append(params);return next(responses)
    sigid.fetch=fetch
    result=sigid.sync();assert result['signals']==2 and len(calls)==2
    match=sigid.lookup(153000000,9000,'FSK','NFM');assert [c['title'] for c in match['candidates']]==['POCSAG']
    assert match['candidates'][0]['confidence']=='reference-feature-candidate'
    sigid.fetch=lambda _: {'query':{'results':{}}}
    try:sigid.sync();raise AssertionError('accepted empty replacement')
    except ValueError:pass
    assert sigid.status()['signals']==2
    # Synthetic RR cipher commands, MM identities and SI3. Wireshark supplies parsed evidence.
    packets=[]
    def packet(l3,frame,channel=6,arfcn=1,bcch=False):
        payload=l3 if bcch else (bytes([3,0,(len(l3)<<2)|1])+l3).ljust(23,b'\x2b')
        header=struct.pack('!BBBBHbbIBBBB',2,4,1,0,arfcn,-55,20,frame,channel,0,0,0)
        return (float(frame),pcapio.udp_ip(header+payload))
    for index,setting in enumerate([0,1,3,5]):packets.append(packet(bytes([6,0x35,setting]),index))
    digits='001010123456789'
    identity=bytes([(int(digits[0])<<4)|9])+bytes(int(digits[i])|(int(digits[i+1])<<4) for i in range(1,15,2))
    packets.append(packet(bytes([5,0x18,1]),4))
    packets.append(packet(bytes([5,0x19,len(identity)])+identity,5))
    si3=bytes.fromhex('49 06 1b 12 34 32 f8 10 45 67 00 00 00 00 00 00 00 00 00').ljust(23,b'\x2b')
    packets.append(packet(si3,6,1,bcch=True))
    capture=p/'security.pcap';pcapio.export({'input':{},'options':{'pcap':str(capture)}},101,packets,'synthetic GSM','synthetic')
    report=gsm_security.analyze(dict(input=dict(path=str(capture)),options={'learn_baseline':True,'home_plmn':'00101'}))
    assert [e['algorithm'] for e in report['events'] if e['type']=='cipher-mode-command']==['A5/0 (no ciphering)','A5/1','A5/2','A5/3']
    assert report['identity_requests']==1 and report['imsi_fields']==1
    assert report['cells'][0]['mcc']=='238' and report['cells'][0]['mnc']=='01'
    baseline=pathlib.Path(report['baseline']);saved=baseline.read_bytes()
    assert digits not in json.dumps(report) and digits.encode() not in saved
    capture=p/'changed-cell.pcap';pcapio.export({'input':{},'options':{'pcap':str(capture)}},101,[packet(si3,0,1,arfcn=2,bcch=True)],'synthetic GSM','synthetic')
    changed=gsm_security.analyze(dict(input=dict(path=str(capture)),options={}))
    assert changed['candidates'][0]['label']=='Cell ARFCN differs from baseline'
    assert baseline.read_bytes()==saved,'baseline changed without explicit learning'
print('PASS: atomic SigID catalog refresh/ranking/unit ambiguity; GSM A5 fields, masked IMSI, identity request, cell baseline and PLMN context')

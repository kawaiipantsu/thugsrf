"""Passive, evidence-labelled GSM security observations from existing PCAPs."""
import datetime,json,re
from pathlib import Path
from common import result,run_tool,data_root
FIELDS=['frame.number','frame.time_relative','gsmtap.arfcn','gsm_a.dtap.msg_rr_type','gsm_a.dtap.msg_mm_type',
        'gsm_a.rr.SC','gsm_a.rr.algorithm_identifier','gsm_a.bssmap.cell_ci','gsm_a.lac','e212.lai.mcc','e212.lai.mnc',
        'e212.mcc','e212.mnc','e212.imsi','gsm_a.dtap.type_of_identity','gsm_a.gm.gmm.type_of_ciph_alg']
SOURCE='https://www.gsma.com/solutions-and-impact/technologies/security/wp-content/uploads/2022/09/FS.35-v3.0.pdf'

def first(fields,name,default=None):return next(iter(fields.get(name,[])),default)
def integer(fields,name):
    value=first(fields,name)
    return int(value,0) if value is not None else None

def inspect_packets(packets,options):
    events=[];candidates=[];cells={};requests=0;exposures=0
    baseline=Path(options.get('baseline',str(data_root()/'gsm-cell-baseline.json')))
    prior=json.loads(baseline.read_text()) if baseline.exists() else {'cells':[]}
    known={f"{r['mcc']}-{r['mnc']}-{r['lac']}-{r['cell_id']}":r for r in prior.get('cells',[])}
    for packet in packets:
        fields=packet.get('_source',{}).get('layers',{});frame=first(fields,'frame.number')
        rr=integer(fields,'gsm_a.dtap.msg_rr_type');mm=integer(fields,'gsm_a.dtap.msg_mm_type')
        sc=integer(fields,'gsm_a.rr.SC');algorithm=integer(fields,'gsm_a.rr.algorithm_identifier')
        if rr==0x35 and sc is not None:
            label='A5/0 (no ciphering)' if sc==0 else f'A5/{algorithm+1}' if algorithm is not None else 'unspecified'
            risk='no ciphering requested' if sc==0 else 'legacy low-security algorithm' if algorithm==0 else 'deprecated weak algorithm' if algorithm==1 else 'algorithm signalled; security not established'
            events.append(dict(frame=frame,type='cipher-mode-command',algorithm=label,assessment=risk,evidence='decoded RR Ciphering Mode Command; this is a command, not proof of the subsequent air-interface state',source=SOURCE))
        gea=integer(fields,'gsm_a.gm.gmm.type_of_ciph_alg')
        if gea is not None:
            events.append(dict(frame=frame,type='gprs-cipher-selection',algorithm=f'GEA/{gea}',assessment='no GPRS ciphering selected' if gea==0 else 'signalled GPRS cipher; no payload decryption performed',evidence='decoded GMM cipher algorithm field'))
        if mm==0x18:
            requests+=1;identity=integer(fields,'gsm_a.dtap.type_of_identity')
            events.append(dict(frame=frame,type='identity-request',requested_identity='IMSI' if identity==1 else str(identity),evidence='decoded MM Identity Request; legitimate networks can request identities'))
        for imsi in fields.get('e212.imsi',[]):
            digits=re.sub(r'\D','',imsi)
            if 5<=len(digits)<=16:
                exposures+=1;events.append(dict(frame=frame,type='imsi-visible-in-capture',imsi_masked='*'*(len(digits)-3)+digits[-3:],evidence='IMSI field present in supplied capture; capture may already have been decrypted'))
        cell=integer(fields,'gsm_a.bssmap.cell_ci');lac=integer(fields,'gsm_a.lac')
        mcc=first(fields,'e212.lai.mcc',first(fields,'e212.mcc'));mnc=first(fields,'e212.lai.mnc',first(fields,'e212.mnc'))
        if cell is not None and lac is not None and mcc and mnc:
            key=f'{mcc}-{mnc}-{lac}-{cell}';row=dict(mcc=mcc,mnc=mnc,lac=lac,cell_id=cell,arfcn=integer(fields,'gsmtap.arfcn'),frame=frame)
            if key not in cells:
                if known and key not in known:candidates.append(dict(label='Cell absent from local baseline',confidence='baseline-anomaly',cell=row,evidence='Incomplete baseline, maintenance and legitimate new cells can cause this'))
                elif key in known and known[key].get('arfcn')!=row['arfcn']:candidates.append(dict(label='Cell ARFCN differs from baseline',confidence='baseline-anomaly',cell=row,previous_arfcn=known[key].get('arfcn'),evidence='Frequency changes may be legitimate network reconfiguration'))
                home=options.get('home_plmn')
                if home and mcc+mnc!=str(home):candidates.append(dict(label='Observed PLMN differs from configured home PLMN',confidence='network-context-only',cell=row,evidence='Possible roaming context; does not establish any subscriber roaming state'))
                cells[key]=row
    if options.get('learn_baseline',False):
        known.update(cells);baseline.parent.mkdir(parents=True,exist_ok=True);tmp=baseline.with_suffix('.tmp')
        tmp.write_text(json.dumps(dict(updated_at=datetime.datetime.now(datetime.timezone.utc).isoformat(),cells=list(known.values())),indent=2));tmp.replace(baseline)
    return result('gsm-security',events=events[:200],candidates=candidates[:100],cells=list(cells.values()),identity_requests=requests,imsi_fields=exposures,baseline=str(baseline),
        notes=['Passive capture analysis only; no base-station emulation, forced identity requests, handset attachment, downgrade or key cracking.',
               'These observations do not prove an IMSI catcher. Raw IMSIs are masked in this result; the original PCAP remains unchanged.',
               'BCCH-only exports supply cell information, not subscriber cipher negotiations or IMSI responses. Those require an existing capture containing the relevant signalling.',
               'An explicit learn_baseline option records observed cells; review the capture before teaching the baseline. No raw IMSI is saved in the baseline.'])

def analyze(request):
    text,log,truncated=run_tool(['tshark','-n','-r',request['input']['path'],'-c','5000','-T','json',*[v for f in FIELDS for v in ['-e',f]]],timeout=30)
    if truncated:raise ValueError('GSM metadata exceeded output limit; supply a smaller capture')
    report=inspect_packets(json.loads(text or '[]'),request.get('options',{}));report['diagnostics']=log;return report

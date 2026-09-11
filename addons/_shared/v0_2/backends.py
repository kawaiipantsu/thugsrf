"""Finite offline wrappers for installed satellite, GNSS, GSM and digital voice tools."""
from pathlib import Path
import os
import json
import re
import tempfile
import time
import numpy as np
from common import load_samples,resample,channel,pcm,result,run_tool,data_root

def output_dir(prefix):
    base=data_root()/'products';base.mkdir(parents=True,exist_ok=True)
    return Path(tempfile.mkdtemp(prefix=prefix+'-',dir=base))

def satellite(request):
    options=request.get('options',{});pipeline=options.get('pipeline','meteor_m2_lrpt')
    allowed={'meteor_m2_lrpt','noaa_apt','noaa_hrpt','metop_ahrpt','fengyun3_ab_ahrpt','fengyun3_c_ahrpt'}
    if pipeline not in allowed:raise ValueError('supported pipelines: '+', '.join(sorted(allowed)))
    fmt=request['input']['format'];level='audio_wav' if fmt=='wav' and pipeline=='noaa_apt' else 'baseband'
    if fmt=='wav' and level!='audio_wav':raise ValueError('WAV supported only for noaa_apt; other pipelines need IQ')
    out=output_dir(pipeline)
    args=['satdump',pipeline,level,request['input']['path'],str(out),'--samplerate',str(request['input']['sample_rate'])]
    if level=='baseband':args+=['--baseband_format',fmt]
    text,log,truncated=run_tool(args,timeout=100)
    files=[str(p) for p in out.rglob('*') if p.is_file()][:100]
    return result('satellite',artifacts=files,output_directory=str(out),diagnostics=(text+'\n'+log)[-6000:],truncated=truncated,
                  notes=['Offline SatDump processing; select the exact pipeline and capture parameters. An output file is not proof of valid decoded imagery. NOAA APT support is useful for archival recordings; verify active spacecraft separately.'])

def gnss(request):
    if abs(request['input']['center_hz']-1575420000)>100000:raise ValueError('GPS L1 decoder expects capture centered on 1575.42 MHz')
    rate=int(request['input']['sample_rate']);fmt=request['input']['format']
    if fmt not in ('cs8','cu8') or not 2_000_000<=rate<=20_000_000:raise ValueError('GPS L1 needs complex IQ at 2..20 MS/s')
    out=output_dir('gps-l1');iq=Path(request['input']['path'])
    if fmt=='cu8':
        iq=out/'input.cs8'
        with open(request['input']['path'],'rb') as src,open(iq,'wb') as dst:
            remaining=rate*2*120
            while remaining:
                chunk=src.read(min(4_000_000,remaining))
                if not chunk:break
                remaining-=len(chunk);dst.write((np.frombuffer(chunk,np.uint8).astype(np.int16)-128).astype(np.int8).tobytes())
    config='''[GNSS-SDR]
GNSS-SDR.internal_fs_sps=2048000
SignalSource.implementation=File_Signal_Source
SignalSource.item_type=ibyte
SignalSource.sampling_frequency=2048000
SignalSource.samples=0
SignalSource.repeat=false
SignalConditioner.implementation=Signal_Conditioner
DataTypeAdapter.implementation=Ibyte_To_Complex
InputFilter.implementation=Pass_Through
Resampler.implementation=Pass_Through
Channels_1C.count=8
Channels.in_acquisition=1
Channel.signal=1C
Acquisition_1C.implementation=GPS_L1_CA_PCPS_Acquisition
Acquisition_1C.item_type=gr_complex
Acquisition_1C.pfa=0.01
Acquisition_1C.doppler_max=10000
Acquisition_1C.doppler_step=500
Tracking_1C.implementation=GPS_L1_CA_DLL_PLL_Tracking
Tracking_1C.item_type=gr_complex
Tracking_1C.pll_bw_hz=25
Tracking_1C.dll_bw_hz=2
TelemetryDecoder_1C.implementation=GPS_L1_CA_Telemetry_Decoder
Observables.implementation=Hybrid_Observables
PVT.implementation=RTKLIB_PVT
PVT.positioning_mode=Single
PVT.output_rate_ms=1000
PVT.display_rate_ms=1000
PVT.flag_rtcm_server=false
PVT.flag_rtcm_tty_port=false
PVT.flag_nmea_tty_port=false
'''
    config=config.replace('2048000',str(rate))
    if any(c in str(iq) for c in ('\n','\r')):raise ValueError('GNSS input path must not contain line breaks')
    config+=f'SignalSource.filename={iq}\nPVT.output_path={out}\nPVT.nmea_dump_filename=position.nmea\n'
    conf=out/'receiver.conf';conf.write_text(config)
    text,log,truncated=run_tool(['gnss-sdr','--config_file='+str(conf),'--log_dir='+str(out)],timeout=100)
    return result('gps-l1',artifacts=[str(p) for p in out.rglob('*') if p.is_file() and p!=iq][:100],diagnostics=(text+'\n'+log)[-6000:],truncated=truncated,
                  notes=['GPS L1 C/A streams signed IQ directly into GNSS-SDR at capture rate; unsigned IQ conversion is capped at 120 seconds. Processing deadline is 100 seconds. Navigation needs adequate signal quality and sufficient ephemeris data; short captures generally cannot yield a fix. Output artifacts are not automatically treated as a valid fix.'])

def gsm(request):
    options=request.get('options',{});wanted=float(options.get('channel_hz',request['input']['center_hz']))
    samples,rate=load_samples(request)
    if not np.iscomplexobj(samples) or rate<400000:raise ValueError('GSM requires complex IQ >=400 kS/s')
    offset=wanted-request['input']['center_hz']
    if abs(offset)+200000>rate/2:raise ValueError('selected GSM carrier outside capture')
    if offset:samples*=np.exp(-2j*np.pi*offset*np.arange(len(samples))/rate).astype(np.complex64)
    samples=resample(samples,rate,1_000_000).astype('<c8')
    with tempfile.NamedTemporaryFile(suffix='.cfile') as f:
        samples.tofile(f.name)
        if options.get('pcap'):
            spec=dict(path=f.name,frequency=int(wanted),request=request)
            text,log,truncated=run_tool(['/usr/bin/python3',str(Path(__file__).with_name('gsm_capture.py')),json.dumps(spec)],timeout=60)
            summary=next((line.removeprefix('THUGSRF_RESULT=') for line in text.splitlines() if line.startswith('THUGSRF_RESULT=')),None)
            if summary is None:raise ValueError('GSM packet collector did not return a result: '+log[-2000:])
            return result('gsm-bcch',**json.loads(summary),diagnostics=log,notes=['CRC-checked control decoder output; BCCH only. Real GSMTAP bytes inside a generated offline IPv4/UDP envelope. No network packets are sent.'])
        text,log,truncated=run_tool(['grgsm_decode','-c',f.name,'-s','1000000','-f',str(int(wanted)),'-m','BCCH','-t','0','-v'],timeout=60)
    lines=[line for line in text.splitlines() if re.search(r'\b(?:[0-9a-fA-F]{2} ){10,}',line)]
    return result('gsm-bcch',events=[dict(message=line,evidence='gr-gsm BCCH decoder') for line in lines[:100]],diagnostics=(text+'\n'+log)[-4000:],truncated=truncated,
                  notes=['Offline GSM broadcast-control-channel decoding only. gr-gsm also emits GSMTAP to localhost:4729. No traffic-channel decryption, subscriber targeting or radio transmission.'])

def voice(request):
    samples,_=channel(request,48000,12500)
    with tempfile.TemporaryDirectory(prefix='thugsrf-dsd-') as temp:
        messages=Path(temp)/'messages.log';logfile=Path(temp)/'decoder.log'
        text,log,_=run_tool(['dsdccx','-i','-','-o','/dev/null','-n','-fa','-M',str(messages),'-L',str(logfile)],pcm(samples),timeout=35)
        data=messages.read_text(errors='replace')[-10000:] if messages.exists() else ''
        diagnostics=logfile.read_text(errors='replace')[-4000:] if logfile.exists() else log
    return result('digital-voice',events=[dict(text=line,evidence='DSDcc formatted metadata') for line in data.splitlines() if line.strip()][:100],diagnostics=diagnostics,
                  notes=['DMR, D-Star, dPMR and YSF backend; NXDN detection where supported by DSDcc. No P25 support, key guessing or encrypted speech recovery. Audio output is disabled.'])

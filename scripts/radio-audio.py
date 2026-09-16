# Embedded live audio backend. No shell commands; all child processes are owned.
import json, os, signal, subprocess, sys, time, math, traceback
from pathlib import Path
children=[]
rds_decoder=None
def stop(*_):
    for p in children:
        if p.poll() is None: p.terminate()
    raise SystemExit(0)
signal.signal(signal.SIGTERM,stop)
signal.signal(signal.SIGINT,stop)
spec=json.loads(sys.argv[1]); c=spec['config']
def expired(*_):raise TimeoutError("audio worker exceeded its duration/deadline")
signal.signal(signal.SIGALRM,expired)
signal.alarm(spec['seconds']+20)
base=Path(os.environ.get('XDG_DATA_HOME',str(Path.home()/'.local/share')))/'thugsrf'
base.mkdir(parents=True,exist_ok=True)
log=open(base/'audio.log','w',buffering=1)
sys.stderr=log

def spawn(args,**kw):
    p=subprocess.Popen(args,stderr=log,**kw);children.append(p);return p

def main():
    global rds_decoder
    import numpy as np
    from scipy import signal as dsp
    rate=int(c['sample_rate']); seconds=spec['seconds']; tx=spec['tx']; mode=c['listen_mode']
    frequency=int(c['frequency']); serial=c['serial']; audio_rate=48000
    log.write(f"{'TX' if tx else 'RX'} {frequency} Hz {mode}, {seconds}s\n")
    if tx:
        source=spawn(['arecord','-q','-D',c['audio_device'],'-t','raw','-f','S16_LE','-c','1','-r','48000','-d',str(seconds)],stdout=subprocess.PIPE)
        args=['hackrf_transfer','-t','-','-f',str(frequency),'-s',str(rate),'-x','0','-a','0','-p','0','-n',str(rate*seconds)]
        if serial: args+=['-d',serial]
        sink=spawn(args,stdin=subprocess.PIPE,stdout=subprocess.DEVNULL)
        phase=0.; pos=0; previous=0.
        while True:
            raw=source.stdout.read(960) # 10 ms; bounded interpolation output
            if not raw: break
            audio=np.frombuffer(raw,dtype='<i2').astype(np.float64)/32768
            if not len(audio): break
            audio=np.clip(audio,-0.95,0.95)
            count=round(len(audio)*rate/audio_rate)
            up=np.interp(np.arange(count)*audio_rate/rate,np.arange(len(audio)+1),np.r_[previous,audio]);previous=audio[-1]
            if spec['tone'] and mode in ('fm','nfm'): up=0.85*up+0.15*np.sin(2*np.pi*spec['tone']*(np.arange(count)+pos)/rate)
            if mode=='am': iq=(0.45*(1+0.8*up)).astype(np.complex128)
            else:
                deviation=min(2500.,c['listen_bandwidth']/5)
                angles=phase+np.cumsum(up)*(2*np.pi*deviation/rate)
                phase=float(angles[-1]%(2*np.pi));iq=0.7*np.exp(1j*angles)
            out=np.empty(count*2,dtype=np.int8);out[0::2]=np.clip(iq.real*127,-127,127).astype(np.int8);out[1::2]=np.clip(iq.imag*127,-127,127).astype(np.int8)
            sink.stdin.write(out.tobytes());pos+=count
        sink.stdin.close()
    else:
        if spec.get('stdin'):
            reader=sys.stdin.buffer
        else:
            if c['device']=='hackrf':
                args=['hackrf_transfer','-r','-','-f',str(frequency),'-s',str(rate),'-l',str(c['lna_gain']),'-g',str(c['vga_gain']),'-n',str(rate*seconds)]
                if serial: args+=['-d',serial]
            else:
                args=['rtl_sdr','-f',str(frequency),'-s',str(rate),'-g',str(c['rtl_gain']/10),'-n',str(rate*seconds)]
                if serial: args+=['-d',serial]
                args+=['-']
            reader=spawn(args,stdout=subprocess.PIPE).stdout
        sink=spawn(['aplay','-q','-D',c['audio_device'],'-t','raw','-f','S16_LE','-c','1','-r','48000'],stdin=subprocess.PIPE,stdout=subprocess.DEVNULL)
        # Cascaded stateful anti-alias filters; integer decimation preserves phase across blocks.
        stages=[]; current=rate
        while current>480000:
            factor=4 if current/4>=240000 else 2
            sos=dsp.butter(6,0.8/factor,output='sos');stages.append([sos,np.zeros((len(sos),2),complex),factor,0]);current/=factor
        if mode=='wfm':
            try:
                rds_decoder=RdsDecoder(current, lambda event: print(json.dumps(event), flush=True))
            except Exception as exc:
                print(json.dumps({'rds_status':str(exc)}),flush=True)
        chan=dsp.butter(6,min(c['listen_bandwidth']/2,current*0.45),fs=current,output='sos');ci=np.zeros((len(chan),2),complex)
        af=dsp.butter(5,15000 if mode=='wfm' else min(4500,c['listen_bandwidth']/2),fs=current,output='sos');ai=np.zeros((len(af),2))
        prev=1+0j; dc=0.; gain=1.; offset=0.; deemphasis=0.
        while True:
            raw=reader.read(262144)
            if not raw: break
            raw=raw[:len(raw)//2*2]
            b=np.frombuffer(raw,dtype=np.int8 if c['device']=='hackrf' else np.uint8).astype(np.float64)
            if c['device']=='rtl': b-=127.5
            iq=(b[0::2]+1j*b[1::2])/128
            for stage in stages:
                sos,zi,factor,offset_stage=stage
                iq,stage[1]=dsp.sosfilt(sos,iq,zi=zi)
                n=len(iq);iq=iq[offset_stage::factor];stage[3]=(offset_stage-n)%factor
            iq,ci=dsp.sosfilt(chan,iq,zi=ci)
            if not len(iq): continue
            power=10*np.log10(float(np.mean(np.abs(iq)**2))+1e-16)
            if mode=='am': audio=np.abs(iq)
            else: audio=np.angle(iq*np.conj(np.r_[prev,iq[:-1]]));prev=iq[-1]
            if rds_decoder is not None:
                try:
                    rds_decoder.push(audio/np.pi)
                except (BrokenPipeError, OSError) as exc:
                    print(json.dumps({'rds_status':'RDS decoder stopped: '+str(exc)}),flush=True)
                    try: rds_decoder.close()
                    except Exception: pass
                    rds_decoder=None
            dc=0.95*dc+0.05*float(np.mean(audio));audio-=dc
            audio,ai=dsp.sosfilt(af,audio,zi=ai)
            if mode=='wfm':
                alpha=math.exp(-1/(current*50e-6));audio,z=dsp.lfilter([1-alpha],[1,-alpha],audio,zi=[deemphasis]);deemphasis=float(z[0])
            positions=np.arange(offset,len(audio),current/audio_rate)
            output=np.interp(positions,np.arange(len(audio)),audio);offset=(positions[-1]+current/audio_rate-len(audio)) if len(positions) else offset-len(audio)
            level=float(np.sqrt(np.mean(output**2))) if len(output) else 0
            gain=0.95*gain+0.05*min(100,0.15/max(level,1e-4))
            output=output*gain if power>=c['squelch_dbfs'] else output*0
            sink.stdin.write(np.clip(output*32767,-32767,32767).astype('<i2').tobytes())
        sink.stdin.close()
    for p in children:
        try:
            code=p.wait(timeout=10)
        except subprocess.TimeoutExpired:
            p.kill();p.wait();continue
        if code: raise RuntimeError(f"{p.args[0]} exited {code}")
try:
    main()
except SystemExit: pass
except Exception:
    traceback.print_exc(file=log);sys.exit(1)
finally:
    if rds_decoder is not None:
        try: rds_decoder.close()
        except Exception as exc: print(json.dumps({'rds_status':str(exc)}),flush=True)
    for p in children:
        if p.poll() is None: p.terminate()
    for p in children:
        try: p.wait(timeout=2)
        except subprocess.TimeoutExpired: p.kill();p.wait()

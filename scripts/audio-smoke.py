#!/usr/bin/python3
"""Live RX/TX process and DSP tests using fake devices; never opens a radio."""
import os,pathlib,subprocess,tempfile,json
import numpy as np
from scipy.signal import resample_poly
ROOT=pathlib.Path(__file__).resolve().parent.parent
BINARY=ROOT/'target/release/thugsrf'
DRIVER='''#!/usr/bin/python3
import sys,os,json
import numpy as np
args=sys.argv[1:];rate=int(args[args.index('-s')+1]);n=int(args[args.index('-n')+1])
if '-t' in args:
    data=sys.stdin.buffer.read();open(os.environ['AUDIO_TEST_OUTPUT'],'wb').write(data)
else:
    t=np.arange(n)/rate
    mode=os.environ['AUDIO_TEST_MODE']
    z=(0.5*(1+0.7*np.sin(2*np.pi*1000*t))) if mode=='am' else 0.6*np.exp(1j*2.5*np.sin(2*np.pi*1000*t))
    data=np.empty(n*2,dtype=np.int8);data[::2]=(z.real*127).astype(np.int8);data[1::2]=(z.imag*127).astype(np.int8)
    sys.stdout.buffer.write(data.tobytes())
'''
with tempfile.TemporaryDirectory() as temp:
    p=pathlib.Path(temp);bin=p/'bin';bin.mkdir()
    (bin/'hackrf_transfer').write_text(DRIVER)
    (bin/'aplay').write_text("#!/usr/bin/python3\nimport sys,os\nopen(os.environ['AUDIO_TEST_OUTPUT'],'wb').write(sys.stdin.buffer.read())\n")
    (bin/'arecord').write_text("#!/usr/bin/python3\nimport sys,numpy as np\nsys.stdout.buffer.write((np.sin(2*np.pi*1000*np.arange(48000)/48000)*12000).astype('<i2').tobytes())\n")
    for file in bin.iterdir():file.chmod(0o755)
    env=dict(os.environ,PATH=str(bin)+':'+os.environ['PATH'],XDG_CONFIG_HOME=str(p/'config'),XDG_DATA_HOME=str(p/'data'),AUDIO_TEST_OUTPUT=str(p/'output'))
    for mode in ['am','fm','wfm']:
        env['AUDIO_TEST_MODE']=mode
        subprocess.run([str(BINARY),'listen','--frequency','100000000','--mode',mode,'--bandwidth','200000' if mode=='wfm' else '12500','--seconds','1'],env=env,check=True,capture_output=True,timeout=25)
        audio=np.fromfile(p/'output',dtype='<i2').astype(float)
        assert 47000<len(audio)<49000,(mode,len(audio))
        audio=audio[10000:];peak=np.argmax(abs(np.fft.rfft(audio)))*48000/len(audio)
        assert abs(peak-1000)<10,(mode,peak)
    for mode in ['am','fm']:
        subprocess.run([str(BINARY),'talk','--frequency','145500000','--mode',mode,'--seconds','1','--confirm-tx'],env=env,check=True,capture_output=True,timeout=25)
        data=np.fromfile(p/'output',np.int8);assert len(data)==16000000
        iq=data[::2].astype(float)+1j*data[1::2].astype(float)
        iq=resample_poly(iq,1,40)
        audio=abs(iq) if mode=='am' else np.angle(iq[1:]*iq[:-1].conj())
        audio=resample_poly(audio,1,4)[1000:-1000];audio-=audio.mean();peak=np.argmax(abs(np.fft.rfft(audio)))*50000/len(audio)
        assert abs(peak-1000)<10,(mode,peak)
    rejected=subprocess.run([str(BINARY),'talk','--frequency','145500000'],env=env,capture_output=True)
    assert rejected.returncode and b'confirm-tx' in rejected.stderr
    (bin/'hackrf_transfer').write_text('#!/bin/sh\necho "fake radio failure" >&2\nexit 9\n')
    failed=subprocess.run([str(BINARY),'listen','--seconds','1'],env=env,capture_output=True,timeout=10)
    assert failed.returncode and b'fake radio failure' in failed.stderr,failed
    (bin/'hackrf_transfer').write_text('#!/usr/bin/python3\nimport os,signal,time\ntime.sleep(.1)\nos.kill(os.getppid(),signal.SIGALRM)\n')
    failed=subprocess.run([str(BINARY),'listen','--seconds','1'],env=env,capture_output=True,timeout=10)
    assert failed.returncode and b'exceeded its duration/deadline' in failed.stderr,failed
print('PASS: live AM/NFM/WFM tone recovery, finite AM/FM microphone IQ, explicit TX gate, child error diagnostics; fake hardware only')

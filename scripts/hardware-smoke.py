#!/usr/bin/env python3
"""Explicit passive host verification; never transmits RF or sends data to an AI provider."""
import json
import os
import pathlib
import sqlite3
import subprocess
import tempfile
root=pathlib.Path(__file__).resolve().parent.parent
binary=root/'target/release/thugsrf'
with tempfile.TemporaryDirectory(prefix='thugsrf-hardware-') as temp:
    tmp=pathlib.Path(temp)
    env=dict(os.environ,XDG_CONFIG_HOME=str(tmp/'config'),XDG_DATA_HOME=str(tmp/'data'))
    def run(*args):
        p=subprocess.run([str(binary),*map(str,args)],env=env,text=True,capture_output=True,timeout=30)
        if p.returncode: raise RuntimeError(p.stderr)
        return p.stdout
    iq=tmp/'hackrf.cs8'
    print(run('record',iq,'--seconds','1').strip())
    result=json.loads(run('analyze',iq))['report']
    print(json.dumps({'hackrf_bytes':iq.stat().st_size,'sample_rate':result['sample_rate'],'analyzed_samples':result['samples'],'rms_dbfs':result['rms_dbfs'],'peak_count':len(result['peaks'])}))
    assert iq.stat().st_size >= 16_000_000
    assert result['sample_rate']==8_000_000
    # Exercise live USB draining and ensure a saved frame is really from HackRF.
    import fcntl,pty,select,struct,termios,time
    master,slave=pty.openpty()
    fcntl.ioctl(slave,termios.TIOCSWINSZ,struct.pack('HHHH',50,170,0,0))
    p=subprocess.Popen([str(binary),'tui'],stdin=slave,stdout=slave,stderr=slave,env=dict(env,TERM='xterm-256color'))
    def drain(seconds):
        until=time.monotonic()+seconds
        while time.monotonic()<until:
            if select.select([master],[],[],.05)[0]:
                try: os.read(master,65536)
                except OSError: break
    drain(.2);os.write(master,b' ');drain(2);os.write(master,b's');drain(.2);os.write(master,b'q');drain(.3)
    assert p.wait(timeout=5)==0
    os.close(master);os.close(slave)
    db=sqlite3.connect(tmp/'data/thugsrf/investigations.sqlite3')
    assert db.execute("SELECT COUNT(*) FROM reports WHERE source='hackrf'").fetchone()[0]>=1
    print('PASS: live HackRF USB -> DSP -> TUI -> SQLite, receiver stopped and reaped')
    audio=tmp/'audio.wav'
    print(run('--device','audio','record',audio,'--seconds','1').strip())
    result=json.loads(run('analyze',audio,'--format','wav'))['report']
    assert result['sample_rate']==48000
    print(f"PASS: ALSA capture and WAV analysis ({result['samples']} samples)")
    run('config','set','audio_device','null')
    run('play',audio)
    print('PASS: audio output through ALSA null sink')

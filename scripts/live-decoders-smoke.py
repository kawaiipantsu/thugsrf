#!/usr/bin/python3
"""Exercise multiple live addons, retuning, pause and cancellation through a PTY.
Fake SDR and decoders only; no RF hardware or transmission.
"""
import fcntl
import json
import os
from pathlib import Path
import pty
import select
import struct
import subprocess
import tempfile
import termios
import time

ROOT = Path(__file__).resolve().parent.parent
BINARY = ROOT/'target/release/thugsrf'
with tempfile.TemporaryDirectory(prefix='thugsrf-live-test-') as tmp:
    tmp = Path(tmp)
    binpath = tmp/'bin'; binpath.mkdir()
    radio = binpath/'rtl_sdr'
    radio.write_text('''#!/usr/bin/python3
import sys,time
while True:
    sys.stdout.buffer.write(bytes([160,100])*65536)
    sys.stdout.buffer.flush()
    time.sleep(.005)
''')
    radio.chmod(0o755)
    calls = tmp/'calls'
    for name, token in [('ais-test', 'AIS_LIVE_123456789'), ('second-test', 'SECOND_LIVE_OK'), ('slow-test', 'SLOW')]:
        folder = tmp/'config/thugsrf/decoders'/name; folder.mkdir(parents=True)
        (folder/'addon.toml').write_text(f'''api_version = 1
name = "{name}"
kind = "decoders"
description = "test live output"
command = ["/usr/bin/python3", "decode.py"]
enabled = {'false' if name=='slow-test' else 'true'}
formats = ["cu8"]
timeout_seconds = 120
''')
        code = '''import json,os,sys,time,subprocess
r=json.load(sys.stdin)
with open(os.environ['LIVE_TEST_CALLS'],'a') as f:
    f.write(json.dumps(dict(name=NAME,frequency=r['input']['center_hz'],rate=r['input']['sample_rate'],size=os.path.getsize(r['input']['path'])))+'\\n')
'''.replace('NAME',repr(name))
        if name == 'slow-test':
            code += "p=subprocess.Popen(['sleep','60'])\nopen(os.environ['LIVE_TEST_SLOW'],'w').write(str(p.pid))\ntime.sleep(60)\n"
        code += "print(json.dumps({'events':[{'message':"+repr(token)+"}]}))\n"
        (folder/'decode.py').write_text(code)
    env = dict(os.environ, TERM='xterm-256color', PATH=str(binpath)+':'+os.environ['PATH'],
               XDG_CONFIG_HOME=str(tmp/'config'), XDG_DATA_HOME=str(tmp/'data'),
               LIVE_TEST_CALLS=str(calls), LIVE_TEST_SLOW=str(tmp/'slow.pid'))
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 40, 170, 0, 0))
    process = subprocess.Popen([str(BINARY),'--device','rtl','--sample-rate','1000000','--frequency','162MHz','tui'],
                               env=env,stdin=slave,stdout=slave,stderr=slave)
    output = bytearray()
    def drain(seconds=.15):
        deadline = time.monotonic()+seconds
        while time.monotonic()<deadline:
            if select.select([master],[],[],.02)[0]:
                output.extend(os.read(master,65536))
    def key(seq):
        os.write(master,seq); drain()
    def until(check, timeout=8):
        deadline=time.monotonic()+timeout
        while time.monotonic()<deadline:
            drain()
            if check(): return
        raise AssertionError(output[-4000:].decode(errors='replace'))
    def records():
        return [json.loads(line) for line in calls.read_text().splitlines()] if calls.exists() else []
    try:
        drain(.3)
        key(b'ds ')
        until(lambda: b'AIS_LIVE_123456789' in output and b'SECOND_LIVE_OK' in output)
        logs=list((tmp/'config/thugsrf').glob('decoder-output-*.log'))
        assert len(logs)==1
        assert 'AIS_LIVE_123456789' in logs[0].read_text() and 'SECOND_LIVE_OK' in logs[0].read_text()
        assert logs[0].stat().st_mode & 0o777 == 0o600
        key(b's')
        size=logs[0].stat().st_size;drain(.4)
        assert logs[0].stat().st_size==size, 'log continued writing after stop'
        key(b's')
        until(lambda: len(list((tmp/'config/thugsrf').glob('decoder-output-*.log')))==2)
        assert all(r['rate']==1000000 and r['size']==4000000 for r in records()), records()
        key(b'1\r162.025MHz\rd')
        until(lambda: any(r['frequency']==162025000 for r in records()))
        key(b'D');drain(.4)
        count=len(records());drain(.5)
        assert len(records())==count, 'pause still invokes decoders'
        key(b'D')
        until(lambda: len(records())>count)
        manifest=tmp/'config/thugsrf/decoders/slow-test/addon.toml'
        manifest.write_text(manifest.read_text().replace('enabled = false','enabled = true'))
        until(lambda: (tmp/'slow.pid').exists())
        started=time.monotonic();key(b'q')
        assert process.wait(timeout=3)==0
        assert time.monotonic()-started < 3
        pid=int((tmp/'slow.pid').read_text())
        # A killed child can briefly remain as a zombie awaiting init's reap.
        status=Path(f'/proc/{pid}/stat')
        assert not status.exists() or status.read_text().split()[2]=='Z', 'decoder descendant survived quit'
        print('PASS: multiple live outputs in one console, exact contiguous windows, retune metadata, log start/stop/new-file, pause/resume, prompt cancellation and descendant cleanup')
    finally:
        if process.poll() is None:
            process.kill();process.wait()
        os.close(master);os.close(slave)

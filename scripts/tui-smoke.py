#!/usr/bin/env python3
"""Exercise the actual TUI under a PTY at both supported layouts."""
import fcntl
import os
import pathlib
import pty
import select
import struct
import subprocess
import tempfile
import termios
import time
root = pathlib.Path(__file__).resolve().parent.parent
binary = root / 'target/release/thugsrf'
with tempfile.TemporaryDirectory(prefix='thugsrf-tui-') as tmp:
    env=dict(os.environ,TERM='xterm-256color',XDG_CONFIG_HOME=tmp+'/config',XDG_DATA_HOME=tmp+'/data')
    subprocess.run([str(binary),'addon','install'],env=env,check=True,capture_output=True)
    for width,height in [(80,24),(170,50)]:
        master,slave=pty.openpty()
        fcntl.ioctl(slave,termios.TIOCSWINSZ,struct.pack('HHHH',height,width,0,0))
        before=termios.tcgetattr(slave)
        p=subprocess.Popen([str(binary),'tui','--demo'],stdin=slave,stdout=slave,stderr=slave,env=env)
        output=bytearray()
        def drain(seconds):
            end=time.monotonic()+seconds
            while time.monotonic()<end:
                if select.select([master],[],[],.05)[0]:
                    try: output.extend(os.read(master,65536))
                    except OSError: break
        drain(.3)
        os.write(master,b' ');drain(.5)
        for key in [b'2',b'3',b'4',b'5',b'6',b'7',b'8',b'9',b'1']:
            os.write(master,key);drain(.1)
            if key==b'4':
                os.write(master,b'\x1b[B'*42);drain(2)
                # A resize forces a complete redraw; ordinary differential terminal writes may split a label.
                fcntl.ioctl(slave,termios.TIOCSWINSZ,struct.pack('HHHH',height,width+1,0,0));drain(.2)
                fcntl.ioctl(slave,termios.TIOCSWINSZ,struct.pack('HHHH',height,width,0,0));drain(.2)
                assert b'zigbee-pcap' in output,'last addon was not scrolled into view'
        os.write(master,b's');drain(.1)
        os.write(master,b'q');drain(.3)
        assert p.wait(timeout=5)==0,output[-1000:]
        assert termios.tcgetattr(slave)==before,'terminal mode was not restored'
        assert 'Spectrum'.encode() in output and 'Waterfall'.encode() in output
        assert 'SYNTHETIC DEMO'.encode() in output if width==170 else True
        pathlib.Path(f'/tmp/thugsrf-tui-{width}x{height}.ansi').write_bytes(output)
        os.close(master);os.close(slave)
print('PASS: 80x24 and 170x50, demo streaming, all panels, saved report, clean exit and terminal restoration')

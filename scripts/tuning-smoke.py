#!/usr/bin/env python3
"""Exercise actual Spectrum keys and USB child replacement in a PTY, using a fake radio."""
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

ROOT = pathlib.Path(__file__).resolve().parent.parent
BINARY = ROOT / 'target/release/thugsrf'
with tempfile.TemporaryDirectory(prefix='thugsrf-tuning-') as temp:
    tmp = pathlib.Path(temp)
    driver = tmp / 'hackrf_transfer'
    driver.write_text('''#!/usr/bin/python3
import fcntl, os, pathlib, sys, time
lock = open(os.environ['TUNE_LOG'] + '.lock', 'w')
try:
    fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
except BlockingIOError:
    pathlib.Path(os.environ['TUNE_LOG'] + '.overlap').touch()
    sys.exit(1)
with open(os.environ['TUNE_LOG'], 'a') as out:
    out.write(sys.argv[sys.argv.index('-f') + 1] + '\\n')
while True:
    sys.stdout.buffer.write(bytes([64, 32]) * 32768)
    sys.stdout.buffer.flush()
    time.sleep(.04)
''')
    driver.chmod(0o755)
    log = tmp / 'frequencies'
    env = dict(os.environ, TERM='xterm-256color', PATH=str(tmp)+os.pathsep+os.environ['PATH'],
               XDG_CONFIG_HOME=str(tmp/'config'), XDG_DATA_HOME=str(tmp/'data'), TUNE_LOG=str(log))
    def cli(*args):
        return subprocess.run([str(BINARY), *args], env=env, check=True, capture_output=True)
    cli('config', 'set', 'frequency', '145.252MHz')
    cli('config', 'set', 'fine_tune_hz', '12.5kHz')
    cli('config', 'set', 'coarse_tune_hz', '2MHz')
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 24, 80, 0, 0))
    process = subprocess.Popen([str(BINARY), 'tui'], stdin=slave, stdout=slave, stderr=slave, env=env)
    def drain(seconds=.35):
        end = time.monotonic()+seconds
        while time.monotonic()<end:
            if select.select([master], [], [], .02)[0]:
                os.read(master, 65536)
    def key(value):
        os.write(master, value)
        drain()
    def frequencies():
        return [int(n) for n in log.read_text().splitlines()] if log.exists() else []
    try:
        drain()
        key(b'\x1b[C')  # right while stopped
        assert frequencies() == []
        key(b' ')
        assert frequencies() == [145264500], frequencies()
        expected = [145264500]
        for seq, hz in [(b'\x1b[D',145252000), (b'\x1b[A',147252000),
                        (b'\x1b[B',145252000), (b'\x1b[5~',147252000), (b'\x1b[6~',145252000)]:
            key(seq)
            expected.append(hz)
            assert frequencies() == expected, frequencies()
        key(b' ')
        key(b'\x1b[C')
        assert frequencies() == expected
        assert not pathlib.Path(str(log)+'.overlap').exists()
        key(b'q')
        assert process.wait(timeout=5) == 0
        assert b'frequency = 145252000' in cli('config').stdout  # session tuning is not persisted
        print('PASS: Spectrum fine/coarse keys, custom steps, live restart without overlap, stopped tuning, persistence')
    finally:
        if process.poll() is None:
            process.kill()
            process.wait()
        os.close(master)
        os.close(slave)

#!/usr/bin/env python3
"""Regression tests for receiver startup retries and full TUI error reporting.

Uses a fake hackrf_transfer child inside an actual PTY. Never opens hardware.
"""
import fcntl
import os
import pathlib
import pty
import select
import sqlite3
import struct
import subprocess
import tempfile
import termios
import time

ROOT = pathlib.Path(__file__).resolve().parent.parent
BINARY = ROOT / 'target/release/thugsrf'
DRIVER = r'''#!/usr/bin/python3
import os, pathlib, sys, time
counter = pathlib.Path(os.environ['THUGSRF_TEST_COUNTER'])
n = int(counter.read_text())+1 if counter.exists() else 1
counter.write_text(str(n))
mode = os.environ['THUGSRF_TEST_MODE']
print('call hackrf_set_sample_rate(8000000 Hz/8.000 MHz)', file=sys.stderr)
print('call hackrf_set_hw_sync_mode(0)', file=sys.stderr)
if mode == 'busy':
    print('hackrf_open() failed: Resource busy (-1000)', file=sys.stderr)
    sys.exit(1)
if mode == 'always' or (mode == 'recover' and n == 1):
    print("Couldn't transfer any bytes for one second.", file=sys.stderr)
    sys.exit(1)
while True:
    sys.stdout.buffer.write(bytes([64, 32]) * 32768)
    sys.stdout.buffer.flush()
    time.sleep(.08)
    if mode == 'late':
        print("Couldn't transfer any bytes for one second.", file=sys.stderr)
        sys.exit(1)
'''
with tempfile.TemporaryDirectory(prefix='thugsrf-rx-test-') as temp:
    tmp = pathlib.Path(temp)
    driver = tmp / 'hackrf_transfer'
    driver.write_text(DRIVER)
    driver.chmod(0o755)
    for mode, expected in [('recover', 2), ('always', 3), ('busy', 1), ('late', 1)]:
        case = tmp / mode
        case.mkdir()
        counter = case / 'attempts'
        env = dict(os.environ, TERM='xterm-256color', PATH=str(tmp)+os.pathsep+os.environ['PATH'],
                   XDG_CONFIG_HOME=str(case/'config'), XDG_DATA_HOME=str(case/'data'),
                   THUGSRF_TEST_COUNTER=str(counter), THUGSRF_TEST_MODE=mode)
        master, slave = pty.openpty()
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack('HHHH', 24, 80, 0, 0))
        process = subprocess.Popen([str(BINARY), 'tui'], stdin=slave, stdout=slave, stderr=slave, env=env)
        output = bytearray()
        def drain(seconds):
            end = time.monotonic()+seconds
            while time.monotonic()<end:
                if select.select([master], [], [], .03)[0]:
                    try:
                        output.extend(os.read(master, 65536))
                    except OSError:
                        break
        try:
            drain(.2)
            os.write(master, b' ')
            drain(2.3)
            assert int(counter.read_text()) == expected, (mode, counter.read_text())
            if mode == 'recover':
                os.write(master, b'2s')
                drain(.2)
                with sqlite3.connect(case/'data/thugsrf/investigations.sqlite3') as db:
                    assert db.execute("SELECT COUNT(*) FROM reports WHERE source='hackrf'").fetchone()[0] > 0
            else:
                assert b'Receiver diagnostics' in output, (mode, output[-2000:])
                assert b'exit status: 1' in output, (mode, output[-2000:])
                if mode == 'busy':
                    assert b'Resource busy' in output
                else:
                    assert b"Couldn't transfer any bytes" in output
            os.write(master, b'q')
            drain(.2)
            assert process.wait(timeout=5) == 0
            print(f'PASS: {mode} ({expected} attempts)', flush=True)
        finally:
            if process.poll() is None:
                process.kill()
                process.wait()
            os.close(master)
            os.close(slave)

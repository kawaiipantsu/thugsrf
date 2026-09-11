#!/usr/bin/env python3
"""Capture the real release binary inside xterm, with synthetic RF data.

Dependencies: xterm xvfb xauth xdotool imagemagick fonts-dejavu-core
Run: xvfb-run -a -s '-screen 0 2400x1600x24' python3 scripts/screenshots.py
"""
import argparse
import os
import pathlib
import subprocess
import tempfile
import time

parser=argparse.ArgumentParser()
parser.add_argument('--hardware',action='store_true',help='Also capture the real passive HackRF Survey panel')
options=parser.parse_args()
ROOT = pathlib.Path(__file__).resolve().parent.parent
BINARY = ROOT / 'target/release/thugsrf'
DEST = ROOT / 'assets/screenshots'
DEST.mkdir(parents=True, exist_ok=True)

with tempfile.TemporaryDirectory(prefix='thugsrf-screenshots-') as temp:
    env = dict(os.environ, TERM='xterm-256color', COLORTERM='truecolor',
               XDG_CONFIG_HOME=temp + '/config', XDG_DATA_HOME=temp + '/data')
    env.pop('NO_COLOR', None)
    subprocess.run([str(BINARY),"addon","install"],env=env,check=True,stdout=subprocess.DEVNULL)
    shots = [
        ('spectrum-wide', '170x50', ['tui', '--demo'], True, None),
        ('spectrum-compact', '80x24', ['tui', '--demo'], True, None),
        ('cli-help', '100x44', ['--help'], False, None),
        ('addons-wide','170x50',['tui'],False,'4'),
        ('listen-wide','170x50',['tui'],False,'8'),
        ('vhf-uhf-wide','170x50',['tui'],False,'9'),
    ]
    if options.hardware: shots.append(('survey-wide','170x50',['tui'],True,'7'))
    for name,geometry,args,live,key in shots:
        title = 'THUGSRF-SCREENSHOT-' + name
        process = subprocess.Popen([
            'xterm', '-hold', '-title', title, '-geometry', geometry,
            '-fa', 'DejaVu Sans Mono', '-fs', '11', '-b', '12',
            '-bg', '#090c13', '-fg', '#e3eaf2', '+sb',
            '-e', str(BINARY), *args,
        ], env=env)
        try:
            window = subprocess.check_output(
                ['xdotool', 'search', '--sync', '--onlyvisible', '--name', '^' + title + '$'],
                env=env, text=True, timeout=10).splitlines()[0]
            subprocess.run(['xdotool', 'windowfocus', '--sync', window], env=env, check=True)
            time.sleep(0.5)
            if key:
                subprocess.run(['xdotool','key',key],env=env,check=True)
                time.sleep(.4)
            if live:
                subprocess.run(['xdotool', 'key', 'space'], env=env, check=True)
                time.sleep(6)
            subprocess.run(['import', '-window', window, str(DEST / (name + '.png'))],
                           env=env, check=True)
            if args!=["--help"]:
                subprocess.run(['xdotool', 'key', 'q'], env=env, check=True)
                time.sleep(0.2)
            print('Captured', name, geometry, flush=True)
        finally:
            process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()

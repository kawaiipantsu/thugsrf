#!/usr/bin/env python3
"""Build a reviewable GitHub Wiki checkout; never publishes automatically."""
import pathlib
import re
import shutil
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
DEST = pathlib.Path(sys.argv[1]).resolve()
DEST.mkdir(parents=True, exist_ok=True)
BINARY = ROOT / 'target/release/thugsrf'
MAPPING = {'RADIO':'Radio-and-Repeaters', 'FREQUENCIES':'Frequency-OSINT',
           'SIGID':'SigID-Catalog', 'PROTOCOLS':'Protocol-Catalog', 'PCAP':'Wireshark-Export',
           'GSM-SECURITY':'GSM-Security', 'ADDONS':'Addon-Development',
           'ARCHITECTURE':'Architecture', 'VERIFICATION':'Verification'}
for path in (ROOT/'docs/wiki').glob('*.md'):
    shutil.copy2(path, DEST/path.name)
for source, page in MAPPING.items():
    content = (ROOT/'docs'/f'{source}.md').read_text()
    for old, new in MAPPING.items():
        content = content.replace(f']({old}.md)', f']({new})')
    content = content.replace('](../README.md)', '](https://github.com/kawaiipantsu/thugsrf#readme)')
    (DEST/f'{page}.md').write_text(content)
# Keep media local to the wiki so screenshots survive changes to main.
images = DEST/'images'
images.mkdir(exist_ok=True)
for path in (ROOT/'assets/screenshots').glob('*.png'):
    shutil.copy2(path, images/path.name)
shutil.copy2(ROOT/'assets/banner.png', images/'banner.png')
shots = [('spectrum-wide','170×50 Spectrum — synthetic demo'),
         ('spectrum-compact','80×24 Spectrum — synthetic demo'),
         ('cli-help','CLI help'), ('settings-wide','Settings — configurable tuning steps'), ('survey-wide','Sequential Survey — passive HackRF acquisition'),
         ('addons-wide','Activatable addon catalog'), ('listen-wide','Listening presets — stopped'),
         ('vhf-uhf-wide','Channel directory — stopped')]
(DEST/'Screenshots.md').write_text('# Terminal screenshots\n\nActual xterm captures from the application. Screenshots illustrate the layout from 0.2.1; the passive hardware Survey capture is from 0.2.0. The 0.3.0 controls and live console are documented in [[Terminal-Manual]] and [[Live-Decoders]].\n\n' + '\n\n'.join(f'## {title}\n\n![{title}](images/{name}.png)' for name,title in shots) + '\n\nReproduce with `xvfb-run -a python3 scripts/screenshots.py` in the main checkout. `--hardware` additionally starts a passive HackRF survey.\n')
# Walk the actual Clap help tree, including nested subcommands.
reference = ['# Command reference', 'Generated from `thugsrf 0.3.0 --help`. Global `--frequency` accepts Hz/kHz/MHz/GHz; other numeric options retain the units shown in their help.']
def help_tree(args):
    result = subprocess.run([str(BINARY), *args, '--help'], check=True, text=True, capture_output=True).stdout
    reference.append('## '+ ' '.join(['thugsrf', *args])+'\n\n```text\n'+result+'```')
    commands = False
    for line in result.splitlines():
        if line == 'Commands:':
            commands = True
            continue
        if commands and not line.strip():
            commands = False
        if commands:
            match = re.match(r'  ([a-z][a-z-]+)\s', line)
            if match and match[1] != 'help':
                help_tree([*args, match[1]])
help_tree([])
(DEST/'Command-Reference.md').write_text('\n\n'.join(reference)+'\n')
pages = sorted(p.stem for p in DEST.glob('*.md') if not p.name.startswith('_'))
(DEST/'_Sidebar.md').write_text('**THUGS(red) RF**\n\n'+'\n'.join(f'- [[{page}]]' for page in ['Home', *[p for p in pages if p!='Home']])+'\n')
(DEST/'_Footer.md').write_text('[THUGS(red)](https://thugs.red) · [Source](https://github.com/kawaiipantsu/thugsrf) · [Report an issue](https://github.com/kawaiipantsu/thugsrf/issues) · Manual for 0.3.0\n')
# Ensure internal wiki links and local images resolve before publication.
for path in DEST.glob('*.md'):
    text = path.read_text()
    for target in re.findall(r'\[\[([^]|]+)(?:\|[^]]+)?\]\]', text):
        assert (DEST/(target+'.md')).exists(), (path, target)
    for target in re.findall(r'\]\(([^)]+)\)', text):
        if '://' in target or target.startswith('#'):
            continue
        target = target.split('#')[0]
        assert (DEST/target).exists() or (DEST/(target+'.md')).exists(), (path, target)
# GitHub Wiki serves committed media through its raw wiki endpoint.
for path in DEST.glob('*.md'):
    path.write_text(path.read_text().replace('](images/', '](https://raw.githubusercontent.com/wiki/kawaiipantsu/thugsrf/images/'))
print(f'Built and checked {len(pages)} wiki pages in {DEST}')

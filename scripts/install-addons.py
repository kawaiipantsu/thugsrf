#!/usr/bin/env python3
import os
import pathlib
import shutil
root = pathlib.Path(__file__).resolve().parent.parent
config = pathlib.Path(os.environ.get('XDG_CONFIG_HOME', pathlib.Path.home() / '.config')) / 'thugsrf'
for source in (root / 'addons').glob('*/*'):
    target = config / source.parent.name / source.name
    if target.exists():
        print(f'Preserved existing {target}')
        continue
    shutil.copytree(source, target)
    print(f'Installed disabled addon: {target}')

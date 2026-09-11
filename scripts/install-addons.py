#!/usr/bin/env python3
import os
import pathlib
import shutil
root = pathlib.Path(__file__).resolve().parent.parent
config = pathlib.Path(os.environ.get('XDG_CONFIG_HOME', pathlib.Path.home() / '.config')) / 'thugsrf'
shared = root / 'addons/_shared/v0_2'
target_shared = config / '_shared/v0_2'
if not target_shared.exists():
    shutil.copytree(shared, target_shared, ignore=shutil.ignore_patterns("__pycache__","*.pyc"))
for source in (root / 'addons').glob('*/*'):
    if source.parent.name not in ('decoders', 'identifiers') or not (source / 'addon.toml').exists():
        continue
    target = config / source.parent.name / source.name
    if target.exists():
        print(f'Preserved existing {target}')
        continue
    shutil.copytree(source, target, ignore=shutil.ignore_patterns("__pycache__","*.pyc"))
    print(f'Installed disabled addon: {target}')

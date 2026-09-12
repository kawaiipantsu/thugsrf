#!/usr/bin/python3
"""RDS addon; shared helper also powers built-in recording and live WFM decoding."""
import json
import os
from pathlib import Path
import sys
os.environ['OPENBLAS_NUM_THREADS'] = '1'
os.environ['OMP_NUM_THREADS'] = '1'
sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parents[2]/'_shared/v0_2'))
try:
    from rds import decode_rds_file
    request = json.load(sys.stdin)
    if request.get('api_version') != 1:
        raise ValueError('unsupported addon API')
    source = request['input']
    result = decode_rds_file(dict(path=source['path'], format=source['format'], rate=source['sample_rate']))
    print(json.dumps(dict(api_version=1, module='rds', **result)))
except Exception as exc:
    print(json.dumps(dict(api_version=1, module='rds', status='error', error=str(exc))))

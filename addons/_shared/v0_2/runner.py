#!/usr/bin/python3
"""Language-neutral addon entry point. One JSON request in, one JSON object out."""
import json
import os
from pathlib import Path
import sys
import tomllib
# Keep optional numeric libraries from starting a large thread pool per addon.
os.environ['OPENBLAS_NUM_THREADS']='1'
os.environ['OMP_NUM_THREADS']='1'
sys.dont_write_bytecode=True

def main():
    name=sys.argv[1]
    catalog=json.loads(Path(__file__).with_name('catalog.json').read_text())
    spec=catalog[name];request=json.load(sys.stdin)
    if request.get('api_version')!=1:raise ValueError('unsupported addon API')
    if request['input']['format'] not in spec['formats']:raise ValueError('supported formats: '+', '.join(spec['formats']))
    path=Path('options.toml')
    options=tomllib.loads(path.read_text()) if path.exists() else {}
    options.update(request.get('options',{}));request['options']=options
    backend=spec['backend']
    if backend=='frequency':
        from frequencydb import identify
        return identify(request)
    if backend=='identify':
        if request['input']['format'] in ('pcap','pcapng'):
            from packets import capture
            return capture(name,request)
        from identifiers import identify
        return identify(name,request)
    if backend=='sigid':
        from sigid import identify
        return identify(request)
    if backend=='gsm-security':
        from gsm_security import analyze
        return analyze(request)
    if backend=='packets':
        from packets import capture
        return capture(name,request)
    if backend=='multimon':
        from audio import multimon
        return multimon(name,request)
    if backend in ('packet-radio','morse-id'):
        from audio import packet_radio,morse_envelope
        return (packet_radio if backend=='packet-radio' else morse_envelope)(request)
    if backend in ('ais','atis','nmea'):
        import maritime
        return getattr(maritime,backend)(request)
    if backend in ('gnss','gsm','voice','satellite'):
        import backends
        return getattr(backends,backend)(request)
    if backend in ('transport','adsb','ble'):
        import importlib
        return importlib.import_module(backend).decode(request)
    if backend=='weather':
        import subprocess
        p=subprocess.run(['/usr/bin/python3',str(Path(__file__).resolve().parents[2]/'decoders/rtl433/decode.py')],input=json.dumps(request),capture_output=True,text=True,timeout=60)
        if p.returncode:raise ValueError(p.stderr[-2000:])
        return json.loads(p.stdout)
    raise ValueError('unknown backend')
try:
    json.dump(main(),sys.stdout,allow_nan=False)
except Exception as error:
    json.dump({'api_version':1,'module':sys.argv[1] if len(sys.argv)>1 else 'unknown','status':'error','error':str(error)},sys.stdout)

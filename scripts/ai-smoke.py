#!/usr/bin/env python3
"""Test the local LLM HTTP contract against a loopback stub, without API credentials."""
import http.server
import json
import os
import pathlib
import subprocess
import tempfile
import threading
root=pathlib.Path(__file__).resolve().parent.parent
requests=[]
class Handler(http.server.BaseHTTPRequestHandler):
    def do_POST(self):
        requests.append((self.path,json.loads(self.rfile.read(int(self.headers['Content-Length'])))))
        body=json.dumps({'choices':[{'message':{'content':'Test hypothesis: inspect the measured tone.'}}]}).encode()
        self.send_response(200);self.send_header('Content-Type','application/json');self.send_header('Content-Length',str(len(body)));self.end_headers();self.wfile.write(body)
    def log_message(self,*args): pass
server=http.server.HTTPServer(('127.0.0.1',0),Handler)
thread=threading.Thread(target=server.serve_forever,daemon=True);thread.start()
try:
    with tempfile.TemporaryDirectory(prefix='thugsrf-ai-') as tmp:
        env=dict(os.environ,XDG_CONFIG_HOME=tmp+'/config',XDG_DATA_HOME=tmp+'/data')
        def run(*args):
            return subprocess.run([str(root/'target/release/thugsrf'),*args],env=env,text=True,capture_output=True,check=True).stdout
        run('config','set','ai_model','test-model')
        run('config','set','local_url',f'http://127.0.0.1:{server.server_port}/v1')
        path=tmp+'/tone.wav';run('encode',path,'--mode','afsk','--bits','10'*100)
        out=run('ai','--input',path,'--format','wav')
        assert 'Test hypothesis' in out
        assert requests[0][0]=='/v1/chat/completions'
        assert requests[0][1]['model']=='test-model'
        assert '48000' in requests[0][1]['messages'][0]['content'][0]['text']
finally:
    server.shutdown();server.server_close()
print('PASS: local AI HTTP payload, model selection, WAV feature submission and result parsing')

"""Explicit SigID Wiki factual-metadata sync and offline candidate ranking."""
import datetime,json,math,os,sqlite3,subprocess,sys,time
from pathlib import Path
API='https://www.sigidwiki.com/api.php'
SOURCE='https://www.sigidwiki.com/wiki/Database'

def db_path():return Path(os.environ.get('XDG_DATA_HOME',str(Path.home()/'.local/share')))/'thugsrf/references.sqlite3'
def connect():
    path=db_path();path.parent.mkdir(parents=True,exist_ok=True);db=sqlite3.connect(path)
    db.execute('CREATE TABLE IF NOT EXISTS sigid (title TEXT PRIMARY KEY, lo REAL, hi REAL, data TEXT NOT NULL)')
    db.execute('CREATE TABLE IF NOT EXISTS reference_meta (source TEXT PRIMARY KEY, data TEXT NOT NULL)')
    return db

def fetch(params):
    args=['curl','--fail','--silent','--show-error','--location','--proto','=https','--proto-redir','=https','--max-time','20','--max-filesize','8000000','--get',API]
    for k,v in params.items():args+=['--data-urlencode',f'{k}={v}']
    result=subprocess.run(args,capture_output=True,timeout=25)
    if result.returncode:raise ValueError('SigID Wiki API request failed: '+result.stderr.decode(errors='replace')[-1000:])
    data=json.loads(result.stdout)
    if 'error' in data:raise ValueError('SigID Wiki API error: '+str(data['error']))
    return data

def numeric(values):
    result=[]
    for value in values:
        try:
            v=float(value)
            if math.isfinite(v) and 0<v<=1e15:result.append(v)
        except (TypeError,ValueError):pass
    return sorted(set(result))

def normalize(title,row):
    props=row.get('printouts',{});freq=numeric(props.get('Frequencies',[]));bw=numeric(props.get('Bandwidth',[]))
    if not freq:return None
    url=row.get('fullurl','')
    if not url.startswith('https://www.sigidwiki.com/wiki/'):return None
    flags=['Frequency units/range need review: exceptionally wide ratio'] if min(freq)<1000 and max(freq)/min(freq)>100000 else []
    return dict(title=title,url=url,quality_flags=flags,frequencies_hz=freq,bandwidths_hz=bw,modulation=[str(x) for x in props.get('Modulation',[])],mode=[str(x) for x in props.get('Mode',[])],location=[str(x) for x in props.get('Location',[])])

def sync():
    records={};offset=0;start=time.monotonic();pages=0;queried=set()
    while True:
        if pages>=20 or time.monotonic()-start>90:raise ValueError('catalog pagination/deadline limit exceeded; existing database kept unchanged')
        query=f'[[Category:Signal]]|?Frequencies|?Bandwidth|?Modulation|?Mode|?Location|limit=250|offset={offset}'
        data=fetch(dict(action='ask',query=query,format='json'));rows=data.get('query',{}).get('results')
        if not isinstance(rows,dict):raise ValueError('unexpected semantic API response; existing database unchanged')
        for title,row in rows.items():
            queried.add(title)
            record=normalize(title,row)
            if record:records[title]=record
        pages+=1;next_offset=data.get('query-continue-offset')
        if next_offset is None:break
        if int(next_offset)<=offset:raise ValueError('non-advancing API pagination')
        offset=int(next_offset);time.sleep(.2)
    if not records:raise ValueError('no frequency metadata returned; existing database unchanged')
    metadata=dict(source=SOURCE,api=API,retrieved_at=datetime.datetime.now(datetime.timezone.utc).isoformat(),signals=len(records),queried_entries=len(queried),skipped_without_usable_frequency=len(queried)-len(records),pages=pages,content='Factual structured metadata only; no article prose, audio, IQ archives or images downloaded',rights='MediaWiki rightsinfo did not specify a license when integration was checked; retain source attribution, refer to the original pages for media/content reuse')
    with connect() as db:
        db.execute('DELETE FROM sigid')
        db.executemany('INSERT INTO sigid VALUES (?,?,?,?)',[(r['title'],min(r['frequencies_hz']),max(r['frequencies_hz']),json.dumps(r)) for r in records.values()])
        db.execute('INSERT OR REPLACE INTO reference_meta VALUES (?,?)',('sigidwiki',json.dumps(metadata)))
    return dict(metadata,database=str(db_path()))

def status():
    with connect() as db:
        row=db.execute('SELECT data FROM reference_meta WHERE source=?',('sigidwiki',)).fetchone()
    return json.loads(row[0]) if row else dict(signals=0,note='Run thugsrf sigid sync to fetch structured factual metadata')

def lookup(hz,bandwidth=None,modulation=None,mode=None,limit=15):
    hz=float(hz)
    if not math.isfinite(hz) or hz<=0:raise ValueError('positive RF frequency required')
    if bandwidth is not None and (not math.isfinite(float(bandwidth)) or float(bandwidth)<=0):raise ValueError('bandwidth must be positive Hz')
    with connect() as db:rows=db.execute('SELECT data FROM sigid WHERE lo<=? AND hi>=?',(hz,hz)).fetchall()
    candidates=[]
    for (raw,) in rows:
        row=json.loads(raw)
        flags=row.get('quality_flags',[])
        # Keep suspect source values unchanged, but do not let them imply a huge continuous band.
        if flags and min(abs(hz-f) for f in row['frequencies_hz'])>float(bandwidth or 25000):continue
        span=max(row['frequencies_hz'])-min(row['frequencies_hz']);scale=float(bandwidth or 25000)
        score=30/(1+math.log10(1+span/scale));evidence=['Within published frequency envelope; endpoints may summarize non-contiguous uses']
        if bandwidth and row['bandwidths_hz']:
            difference=min(abs(math.log2(float(bandwidth)/b)) for b in row['bandwidths_hz']);score+=40*math.exp(-difference)
            evidence.append(f'Bandwidth comparison: measured/supplied {float(bandwidth):.0f} Hz')
        if modulation:
            match=any(modulation.casefold()==x.casefold() for x in row['modulation']);score+=30 if match else -20;evidence.append('Modulation label '+('matches' if match else 'differs'))
        if mode:
            match=any(mode.casefold()==x.casefold() for x in row['mode']);score+=10 if match else -10;evidence.append('Receiver-mode label '+('matches' if match else 'differs'))
        candidates.append(dict(row,rank_score=round(score,2),confidence='reference-feature-candidate',evidence=evidence))
    return dict(frequency_hz=hz,bandwidth_hz=bandwidth,candidates=sorted(candidates,key=lambda r:(-r['rank_score'],r['title']))[:max(1,min(int(limit),100))],catalog=status(),notes=['Rank scores order reference matches; they are not probabilities or protocol confirmation. Frequency envelopes are not national allocations or station ownership records.'])

def identify(request):
    options=request.get('options',{});hz=float(options.get('frequency_hz',request['input']['center_hz']));bandwidth=options.get('bandwidth_hz');measurement=None
    if bandwidth is None:
        from common import load_samples
        import numpy as np
        from scipy import signal
        samples,rate=load_samples(request,1000000)
        freqs,power=signal.welch(samples,fs=rate,return_onesided=False,nperseg=min(16384,len(samples)),scaling='spectrum')
        freqs=np.fft.fftshift(freqs);power=np.fft.fftshift(power)
        peak=int(np.argmax(power));floor=float(np.median(power));threshold=max(floor*6.3,float(power[peak])*.01)
        if power[peak]>floor*10:
            left=right=peak
            while left>0 and power[left-1]>threshold:left-=1
            while right+1<len(power) and power[right+1]>threshold:right+=1
            bandwidth=float((right-left+1)*rate/len(power));hz+=float(freqs[peak])
            measurement=dict(method='Dominant Welch peak, contiguous above noise+8 dB or peak-20 dB; approximate and may be interference',bandwidth_hz=bandwidth,peak_offset_hz=float(freqs[peak]))
    report=lookup(hz,bandwidth,options.get('modulation'),options.get('mode'),options.get('limit',15));report.update(api_version=1,module='sigid-id',events=[],measurement=measurement);return report

if __name__=='__main__':
    try:
        args=json.loads(sys.argv[1]);action=args.pop('action')
        print(json.dumps(sync() if action=='sync' else status() if action=='status' else lookup(**args),indent=2))
    except Exception as e:print(str(e),file=sys.stderr);sys.exit(1)

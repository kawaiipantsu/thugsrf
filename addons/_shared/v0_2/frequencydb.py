"""User-owned frequency references, with provenance; never proof of a transmitter identity."""
import csv, datetime, io, json, os, re
from html.parser import HTMLParser
from pathlib import Path

SOURCES = [
    {'name':'DKScan Danish frequency lists','region':'DK','url':'http://www.dkscan.dk/frekvens.htm','kind':'community reference; import saved HTML'},
    {'name':'CEPT EFIS national allocations and applications','region':'EU/DK','url':'https://efis.cept.org/','kind':'official national information; export and review before import'},
    {'name':'NTIA US allocation chart','region':'US','url':'https://www.ntia.gov/page/united-states-frequency-allocation-chart','kind':'official allocation chart; September 2025 edition, March 2025 data'},
    {'name':'FCC allocation table','region':'US','url':'https://www.ecfr.gov/current/title-47/chapter-I/subchapter-A/part-2/subpart-B/section-2.106','kind':'official current regulations'},
    {'name':'FCC broadcast station public files','region':'US','url':'https://publicfiles.fcc.gov/','kind':'station reference; frequency matches alone do not prove identity'},
]
# Small source-backed starting set. This is deliberately not a complete allocation table.
BUILTINS = [
    (0.535,1.705,'US AM broadcast band','US','https://docs.fcc.gov/public/attachments/FCC-25-17A1_Rcd.pdf'),
    (88,108,'US FM broadcast band','US','https://docs.fcc.gov/public/attachments/FCC-25-17A1_Rcd.pdf'),
    (144,148,'US 2 m amateur band; other uses/sharing conditions may apply','US','https://docs.fcc.gov/public/attachments/FCC-19-130A1_Rcd.pdf'),
    (420,450,'US 70 cm amateur band; shared with other services','US','https://docs.fcc.gov/public/attachments/FCC-19-130A1_Rcd.pdf'),
    (902,928,'US 33 cm amateur band; shared with other services','US','https://docs.fcc.gov/public/attachments/FCC-19-130A1_Rcd.pdf'),
    (118,136.975,'US aeronautical VHF communications','US','https://docs.fcc.gov/public/attachments/DA-98-1984A1.pdf'),
]

def database():
    return Path(os.environ.get('XDG_DATA_HOME',str(Path.home()/'.local/share')))/'thugsrf/frequencies.json'

class Tables(HTMLParser):
    def __init__(self): super().__init__();self.rows=[];self.row=[];self.cell=None
    def handle_starttag(self,tag,attrs):
        if tag=='tr': self.row=[]
        if tag in ('td','th'): self.cell=[]
        if tag=='br' and self.cell is not None:self.cell.append(' ')
    def handle_data(self,data):
        if self.cell is not None:self.cell.append(data)
    def handle_endtag(self,tag):
        if tag in ('td','th') and self.cell is not None:self.row.append(' '.join(''.join(self.cell).split()));self.cell=None
        if tag=='tr' and self.row:self.rows.append(self.row);self.row=[]

def load():
    p=database()
    return json.loads(p.read_text()) if p.exists() else []

def import_file(path,source,region="DK"):
    p=Path(path)
    if p.stat().st_size>20_000_000:raise ValueError('reference file exceeds 20 MB')
    text=p.read_bytes().decode('utf-8-sig',errors='replace');rows=[]
    if p.suffix.lower()=='.csv':
        for row in csv.DictReader(io.StringIO(text)):
            low=float(row['low_hz']) if row.get('low_hz') else float(row['frequency_mhz'].replace(',','.'))*1e6
            high=float(row.get('high_hz') or low)
            rows.append(dict(low_hz=low,high_hz=high,label=row.get('label') or row.get('service') or '',region=row.get('region') or region,source=row.get('source') or source))
    else:
        parser=Tables();parser.feed(text)
        for cells in parser.rows:
            for index,cell in enumerate(cells):
                # Only whole frequency cells; dates, callsigns and embedded prose cannot become channels.
                match=re.fullmatch(r'\s*(\d{1,4}[.,]\d{1,6})(?:\s*[-–]\s*(\d{1,4}[.,]\d{1,6}))?\s*(MHz|kHz)?\s*',cell,re.I)
                if not match:continue
                scale=1000 if (match[3] or '').lower()=='khz' else 1e6
                low=float(match[1].replace(',','.'))*scale;high=float((match[2] or match[1]).replace(',','.'))*scale
                label=' | '.join(c for i,c in enumerate(cells) if i!=index)
                rows.append(dict(low_hz=low,high_hz=high,label=label,source=source))
                break
    now=datetime.datetime.now(datetime.timezone.utc).isoformat()
    for row in rows:
        if not (30000<=row['low_hz']<=row['high_hz']<=6e9):raise ValueError('frequency outside 30 kHz..6 GHz')
        if not row['label'].strip():raise ValueError('every row requires a label/service')
        row.update(region=row.get('region',region),imported_at=now,evidence='reference-list match; unverified current allocation')
    if not rows:raise ValueError('no usable frequency rows; use CSV frequency_mhz,label,source or low_hz,high_hz,label,source')
    existing=load();keys={(r['low_hz'],r['high_hz'],r['label'],r['source'],r.get('region','DK')) for r in existing}
    added=[r for r in rows if (r['low_hz'],r['high_hz'],r['label'],r['source'],r.get('region','DK')) not in keys]
    unique={ (r['low_hz'],r['high_hz'],r['label'],r['source'],r.get('region','DK')):r for r in existing+added }
    target=database();target.parent.mkdir(parents=True,exist_ok=True);temp=target.with_suffix('.tmp');temp.write_text(json.dumps(list(unique.values()),indent=2,ensure_ascii=False));temp.replace(target)
    return dict(imported=len(unique)-len(keys),total=len(unique),database=str(target),note='Review HTML imports: unitless decimal frequency cells are interpreted as MHz. Original sources are retained; no transmitter ownership is established.')

def lookup(hz,tolerance=12500,region="DK"):
    if not 0<=tolerance<=1e6:raise ValueError('tolerance must be 0..1000000 Hz')
    matches=[]
    builtin=[dict(low_hz=lo*1e6,high_hz=hi*1e6,label=label,region=reg,source=source,evidence='allocation context only; incomplete starter table',reference_checked='2026-09-11') for lo,hi,label,reg,source in BUILTINS]
    for row in load()+builtin:
        if region!='ALL' and row.get('region','DK') not in (region,'ALL'):continue
        distance=max(row['low_hz']-hz,hz-row['high_hz'],0)
        if distance<=tolerance:matches.append(dict(row,distance_hz=distance))
    return sorted(matches,key=lambda r:r['distance_hz'])[:100]

def identify(request):
    hz=float(request['input']['center_hz']);matches=lookup(hz,float(request.get('options',{}).get('tolerance_hz',12500)),request.get('options',{}).get('region','DK'))
    return dict(api_version=1,module='danish-frequency',candidates=matches,events=[],notes=['Frequency-list clues only; shared frequencies and outdated entries are possible.','Import saved DKScan HTML or your own CSV with thugsrf frequency import.'],database=str(database()))

if __name__=='__main__':
    import sys
    try:
        args=json.loads(sys.argv[1])
        result=SOURCES if args['action']=='sources' else import_file(args['path'],args['source'],args['region']) if args['action']=='import' else dict(frequency_hz=args['hz'],region=args['region'],matches=lookup(args['hz'],args['tolerance'],args['region']))
        print(json.dumps(result,indent=2,ensure_ascii=False))
    except Exception as e:print(str(e),file=sys.stderr);sys.exit(1)

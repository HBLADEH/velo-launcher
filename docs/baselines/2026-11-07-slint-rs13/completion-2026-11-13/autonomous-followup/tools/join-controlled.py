"""Join bounded WGC clips without changing native frame pixels or timestamps."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil

parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('source',type=Path)
parser.add_argument('session',type=Path)
parser.add_argument('direction',choices=('enter','exit'))
parser.add_argument('output',type=Path)
args=parser.parse_args()
if args.output.exists():raise ValueError('Preserve earlier joined records')
events=[json.loads(line) for line in (args.source/'events.jsonl').read_text().splitlines()]
ready=next(e for e in events if e['event']=='capture_ready')
end=next(e for e in events if e['event']=='end')
session=json.loads(args.session.read_text(encoding='utf-8-sig'))
assert ready['pid']==session['ProcessId'] and ready['creation_filetime_100ns']==session['StartedTicks']-504911232000000000
assert not end['storage_limited'] and end['gpu_queue_rejected']==end['write_queue_rejected']==0
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest().upper()
unique={}
for event in events:
 if event['event']!='frame' or not event.get('saved'):continue
 path=args.source/event['clip']/f"frame-{event['index']}.bmp"
 key=event['frame_time_100ns']
 if key in unique:assert sha(path)==sha(unique[key]['path'])
 if key not in unique or (unique[key]['event'].get('baseline_cached') and not event.get('baseline_cached')):
  unique[key]={'event':event,'path':path}
rows=sorted(unique.values(),key=lambda row:row['event']['frame_time_100ns'])
assert len(rows)>=6
name=f'settings-{args.direction}-1'
folder=args.output/name;folder.mkdir(parents=True)
trace=[ready,{'event':'clip_start','clip':name,'qpc':next(e['qpc'] for e in events if e['event']=='clip_start')}]
manifest=[]
for row in rows:
 event=dict(row['event']);event['clip']=name
 assert event['dpi']==96 and event['inset']==12 and event['width']==616 and event['height']==636
 dest=folder/row['path'].name
 shutil.copyfile(row['path'],dest);assert sha(dest)==sha(row['path'])
 trace.append(event)
 manifest.append({'OriginalFile':str(row['path']),'File':name+'/'+dest.name,'SHA256':sha(dest),'Index':event['index'],'SourceTime100ns':event['frame_time_100ns']})
trace.append(end)
(args.output/'events.jsonl').write_text('\n'.join(json.dumps(e) for e in trace)+'\n')
shutil.copyfile(args.session,args.output/'session.json')
(args.output/'source-manifest.json').write_text(json.dumps({'OriginalTrace':str(args.source/'events.jsonl'),'OriginalTraceSHA256':sha(args.source/'events.jsonl'),'ParentEnd':end,'UniqueFrames':len(rows),'Frames':manifest,'Scope':'Whole original sequence; identical cached boundary duplicates removed after SHA check. No pixels, source clocks, endpoint amplitudes, thresholds or durations modified.'},indent=2)+'\n')
print('Joined',len(rows),'unmodified',session['Edition'],args.direction,'source frames.')

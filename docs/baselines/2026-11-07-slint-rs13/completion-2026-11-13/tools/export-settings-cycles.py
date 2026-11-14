"""Locate complete settings intervals; preserve original pixels, clocks and SHA."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil

import numpy as np
from PIL import Image

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('source', type=Path)
parser.add_argument('session', type=Path)
parser.add_argument('panel_reference', type=Path)
parser.add_argument('output', type=Path)
parser.add_argument('--home-reference', type=Path)
args = parser.parse_args()
if args.output.exists():
    raise ValueError('Keep previous exports')
events = [json.loads(line) for line in (args.source / 'events.jsonl').read_text().splitlines()]
ready = next(event for event in events if event['event'] == 'capture_ready')
end = next(event for event in events if event['event'] == 'end')
session = json.loads(args.session.read_text(encoding='utf-8-sig'))
assert ready['pid'] == session['ProcessId']
assert ready['creation_filetime_100ns'] == session['StartedTicks'] - 504911232000000000
assert not end['storage_limited'] and end['gpu_queue_rejected'] == end['write_queue_rejected'] == 0

def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest().upper()

unique = {}
for event in events:
    if event['event'] != 'frame' or not event.get('saved'):
        continue
    path = args.source / event['clip'] / f"frame-{event['index']}.bmp"
    key = event['frame_time_100ns']
    if key in unique:
        assert sha(unique[key]['path']) == sha(path)
    if key not in unique or (unique[key]['event'].get('baseline_cached') and not event.get('baseline_cached')):
        unique[key] = {'event': event, 'path': path}
rows = sorted(unique.values(), key=lambda row: row['event']['frame_time_100ns'])
roi = (62, 17, 74, 50)  # Client title x74..86/y29..62 minus the original 12px inset.
for row in rows:
    assert row['event']['dpi'] == 96 and row['event']['inset'] == 12
    with Image.open(row['path']) as image:
        assert image.size == (616, 636)
        row['roi'] = np.asarray(image.convert('RGB').crop(roi)).astype(float)
home_row = next((row for row in rows if row['roi'].mean() > 200 and row['roi'].std() < 10), None)
if args.home_reference:
    home_path = args.home_reference
    home = np.asarray(Image.open(home_path).convert('RGB').crop(roi)).astype(float)
else:
    if home_row is None:
        raise ValueError('No uniform home ROI; select an inspected home frame explicitly')
    home_path = home_row['path']
    home = home_row['roi']
panel = np.asarray(Image.open(args.panel_reference).convert('RGB').crop(roi)).astype(float)
for row in rows:
    a = float(np.sqrt(np.mean((row['roi'] - home) ** 2)))
    b = float(np.sqrt(np.mean((row['roi'] - panel) ** 2)))
    row['state'] = 'home' if a < 1 else 'settings' if b < 1 else 'moving'
    if row['roi'].mean() < 50:
        row['state'] = 'invalid-black'

changes = []
previous = 'home'
previous_index = None
for index, row in enumerate(rows):
    if row['state'] in ('moving', 'invalid-black'):
        continue
    if row['state'] != previous:
        assert previous_index is not None
        changes.append({'direction': 'enter' if row['state'] == 'settings' else 'exit',
                        'start': previous_index, 'end': index})
        previous = row['state']
    previous_index = index
assert changes, 'No complete settings intervals recorded'
args.output.mkdir()
trace = [ready]
manifest = []
counts = {'enter': 0, 'exit': 0}
for change in changes:
    direction = change['direction']
    counts[direction] += 1
    name = f"settings-{direction}-{counts[direction]}"
    folder = args.output / name
    folder.mkdir()
    first = max(0, change['start'] - 2)
    last = min(len(rows) - 1, change['end'] + 7)
    while first < change['start'] and rows[first]['state'] == 'invalid-black':
        first += 1
    trace.append({'event': 'clip_start', 'clip': name, 'qpc': rows[first]['event']['observed_qpc']})
    frames = []
    for row in rows[first:last + 1]:
        assert row['state'] != 'invalid-black'
        event = dict(row['event'])
        event['clip'] = name
        copy = folder / row['path'].name
        shutil.copyfile(row['path'], copy)
        assert sha(copy) == sha(row['path'])
        trace.append(event)
        frames.append({'OriginalFile': str(row['path']), 'File': name + '/' + copy.name,
                       'Index': event['index'], 'SourceTime100ns': event['frame_time_100ns'], 'SHA256': sha(copy)})
    manifest.append({'Clip': name, 'OriginalFrames': frames})
trace.append(end)
(args.output / 'events.jsonl').write_text('\n'.join(json.dumps(row) for row in trace) + '\n')
shutil.copyfile(args.session, args.output / 'session.json')
(args.output / 'source-manifest.json').write_text(json.dumps({
    'SourceTrace': str(args.source / 'events.jsonl'), 'SourceTraceSHA256': sha(args.source / 'events.jsonl'),
    'HomeReference': str(home_path), 'PanelReference': str(args.panel_reference),
    'ParentEnd': end, 'Clips': manifest, 'SourceFramesUnmodified': True,
    'Scope': 'Endpoint-distance locator only. All candidates keep original pre-transition/rest and settled frames, pixels, timestamps and hashes; no opacity renormalization.'
}, indent=2) + '\n')
print(f'Exported {counts} complete native transition intervals and {sum(len(row["OriginalFrames"]) for row in manifest)} frame hash mappings.')

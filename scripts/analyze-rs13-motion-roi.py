"""Check animation changes inside fixed 96-DPI native control regions.

Only selected-window source frames are used. This excludes carets and most
tooltips so unrelated activity cannot prove a decorative animation changed.
"""
import argparse
import json
from pathlib import Path

from PIL import Image, ImageChops, ImageStat


def region(clip, edition, recording_center=None):
    family = clip.removesuffix('-1').removesuffix('-2')
    if family.startswith('settings-'):
        return [74, 29, 160, 66]
    if family.startswith('custom-'):
        return [24, 22, 210, 74]
    if family.startswith('dropdown-'):
        return [30, 245, 600, 410]
    if family == 'recording':
        center = recording_center if recording_center is not None else 554 if edition == 'wails' else 514
        return [center - 11, 251, center + 11, 274]
    if family == 'list-selection':
        return [12, 84, 626, 195]
    if family == 'list-enter':
        return [12, 84, 626, 500]
    if family == 'home-entry':
        return [24, 300, 600, 514]
    if family.startswith('tile-'):
        return [134, 200, 194, 260]
    if family == 'gear-hover':
        return [590, 27, 616, 54]
    if family == 'scanning':
        return [600, 628, 626, 651]
    raise ValueError('Unknown motion family')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('recording_directory', type=Path)
    parser.add_argument('session_json', type=Path)
    parser.add_argument('output_json', type=Path)
    parser.add_argument('--recording-center', type=int, help='Measured client x center for this build; historical Slint clips use 514')
    args = parser.parse_args()
    if args.output_json.exists():
        raise ValueError('Use a fresh ROI result')
    events = [json.loads(line) for line in (args.recording_directory / 'events.jsonl').read_text().splitlines()]
    session = json.loads(args.session_json.read_text(encoding='utf-8-sig'))
    ready = next(event for event in events if event['event'] == 'capture_ready')
    if ready['pid'] != session['ProcessId'] or ready['creation_filetime_100ns'] != session['StartedTicks'] - 504911232000000000:
        raise ValueError('Unexpected target identity')
    result = []
    for start in [event for event in events if event['event'] == 'clip_start']:
        clip = start['clip']
        if Path(clip).name != clip or not clip.replace('-', '').isalnum():
            raise ValueError('Invalid clip path')
        rows = [event for event in events if event['event'] == 'frame' and event.get('clip') == clip and event.get('saved')]
        if not rows or any(row['dpi'] != 96 or row['inset'] != 12 for row in rows):
            result.append({'Clip': clip, 'Measured': False, 'Reason': 'No frames or uncalibrated native DPI/inset'})
            continue
        box = region(clip, session['Edition'], args.recording_center)
        stored = [coordinate - 12 for coordinate in box]
        images = []
        for row in rows:
            path = args.recording_directory / clip / f"frame-{row['index']}.bmp"
            with Image.open(path) as image:
                # A tooltip/caret elsewhere is deliberately excluded. Any
                # needed edge padding stays constant and is reported below.
                images.append(image.convert('RGB').crop(stored))
        energies = [sum(ImageStat.Stat(ImageChops.difference(a, b)).sum) for a, b in zip(images, images[1:])]
        live = [row for row in rows if not row.get('baseline_cached')]
        result.append({'Clip': clip, 'Measured': True, 'ClientROILogicalPixels': box,
                       'Frames': len(rows), 'ChangedROIIntervals': sum(value > 40 for value in energies),
                       'MaximumROIChangeEnergy': max(energies, default=0),
                       'ControlChangeObserved': sum(value > 40 for value in energies) >= 2,
                       'SourceSpanWithoutCachedBaselineMilliseconds': (live[-1]['frame_time_100ns'] - live[0]['frame_time_100ns']) / 10000 if len(live) > 1 else 0,
                       'Scope': 'Local control pixels changed; this does not by itself certify exact amplitude, timing, easing or a completed rotation/pulse period.'})
    args.output_json.write_text(json.dumps({'Edition': session['Edition'], 'ExecutableSHA256': session['SHA256'], 'Rows': result,
        'PhysicalInput': False, 'Scope': 'Fixed 96-DPI regions exclude unrelated caret/tooltip changes. No ROI observation automatically accepts an exact trajectory.'}, indent=2) + '\n', encoding='utf8')
    print(f"Checked {len(result)} native control regions; {sum(row.get('ControlChangeObserved', False) for row in result)} show local changes.")


if __name__ == '__main__':
    main()

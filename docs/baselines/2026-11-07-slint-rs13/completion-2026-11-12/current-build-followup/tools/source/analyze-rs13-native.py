"""Describe native surface sampling and finite transition progress.

Uses original WGC clocks and BMPs. A scalar endpoint fit is a diagnostic, not a
substitute for geometric/opacity curves or final desktop presentation timing.
"""
import argparse
import hashlib
import json
from pathlib import Path

import numpy as np
from PIL import Image


def ease(progress, curve):
    low = np.zeros_like(progress)
    high = np.ones_like(progress)
    x1, y1, x2, y2 = curve
    for _ in range(24):
        t = (low + high) / 2
        x = 3 * (1 - t) ** 2 * t * x1 + 3 * (1 - t) * t ** 2 * x2 + t ** 3
        low = np.where(x < progress, t, low)
        high = np.where(x >= progress, t, high)
    return 3 * (1 - t) ** 2 * t * y1 + 3 * (1 - t) * t ** 2 * y2 + t ** 3


def describe(rows, folder):
    images = []
    for row in rows:
        path = folder / f"frame-{row['index']}.bmp"
        with Image.open(path) as image:
            if image.size != (row['width'], row['height']):
                raise ValueError('Native frame geometry mismatch')
            # The source stays full-size. Sample only the body for this
            # diagnostic to exclude caret, pointer tooltips and fixed footer.
            images.append(np.asarray(image.convert('RGB').crop((12, 128, min(image.width, 580), min(image.height, 540))))[::2, ::2].astype(np.float32))
    times = np.array([row['frame_time_100ns'] for row in rows], dtype=np.float64) / 10000
    times -= times[0]
    if np.any(np.diff(times) <= 0):
        raise ValueError('WGC source clocks are not strictly increasing')
    valid = [index for index, row in enumerate(rows) if not row.get('baseline_cached')]
    intervals = np.diff(times[valid])
    result = {
        'Frames': len(rows), 'CachedBaseline': bool(rows[0].get('baseline_cached')),
        'SourceSpanMilliseconds': float(times[-1]),
        'SourceIntervalsMilliseconds': [float(value) for value in intervals],
        'MaximumSourceGapMilliseconds': float(intervals.max()) if len(intervals) else None,
    }
    if not images or any(image.shape != images[0].shape for image in images):
        result['FiniteFitAvailable'] = False
        result['Reason'] = 'Window size changes require separately aligned geometry.'
        return result
    first, last = images[0], images[-1]
    difference = last - first
    mask = np.max(np.abs(difference), axis=2) >= 24
    if int(mask.sum()) < 100:
        result['FiniteFitAvailable'] = False
        result['Reason'] = 'Endpoints nearly equal or cyclic decoration; no finite endpoint model.'
        return result
    vector = difference[mask]
    denominator = float(np.sum(vector * vector))
    progress = np.array([np.sum((frame - first)[mask] * vector) / denominator for frame in images])
    result['EndpointProgress'] = [{'SourceMilliseconds': float(t), 'Progress': float(p)} for t, p in zip(times, progress)]
    intermediate = (progress > .03) & (progress < .97)
    result['IntermediateFrames'] = int(intermediate.sum())
    if intermediate.sum() < 4:
        result['FiniteFitAvailable'] = False
        result['Reason'] = 'Fewer than four intermediate native frames; exact curve unverified.'
        return result
    first_change = times[np.argmax(progress > .03)]
    best = None
    onsets = np.arange(first_change - 80, first_change + 2, 2)
    grid = np.linspace(0, 1, 4097)
    for label, curve in [('wails-main', (.22, 1, .36, 1)), ('css-ease', (.25, .1, .25, 1))]:
        values = ease(grid, curve)
        for duration in range(40, 501, 2):
            predicted = np.interp(np.clip((times[None, :] - onsets[:, None]) / duration, 0, 1), grid, values)
            errors = np.sqrt(np.mean((predicted - progress) ** 2, axis=1))
            index = int(np.argmin(errors))
            if best is None or errors[index] < best['RMSE']:
                best = {'Curve': label, 'DurationMilliseconds': duration, 'OnsetSourceMilliseconds': float(onsets[index]), 'RMSE': float(errors[index])}
    result['FiniteFitAvailable'] = True
    result['ScalarEndpointFit'] = best
    result['FitScope'] = 'Projection between body endpoints; blends opacity and translation. This fit alone cannot certify either independent CSS property.'
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('recording_directory', type=Path)
    parser.add_argument('session_json', type=Path)
    parser.add_argument('output_json', type=Path)
    args = parser.parse_args()
    if args.output_json.exists():
        raise ValueError('Preserve earlier diagnostics; use a fresh output path')
    session = json.loads(args.session_json.read_text(encoding='utf-8-sig'))
    events = [json.loads(line) for line in (args.recording_directory / 'events.jsonl').read_text(encoding='utf-8-sig').splitlines()]
    ready = next(event for event in events if event['event'] == 'capture_ready')
    if ready['pid'] != session['ProcessId'] or ready['creation_filetime_100ns'] != session['StartedTicks'] - 504911232000000000:
        raise ValueError('Recorder process identity differs')
    end = next((event for event in events if event['event'] == 'end'), None)
    clips = []
    for start in [event for event in events if event['event'] == 'clip_start']:
        name = start['clip']
        if Path(name).name != name or not name.replace('-', '').isalnum():
            raise ValueError('Unexpected native clip name')
        if name.startswith('private-'):
            continue
        rows = [event for event in events if event['event'] == 'frame' and event.get('clip') == name and event.get('saved')]
        if not rows:
            clips.append({'Name': name, 'FiniteFitAvailable': False, 'Reason': 'No frames'})
            continue
        clips.append({'Name': name, **describe(rows, args.recording_directory / name)})
    report = {
        'ExecutableSHA256': session['SHA256'], 'PID': session['ProcessId'],
        'RawTraceSHA256': hashlib.sha256((args.recording_directory / 'events.jsonl').read_bytes()).hexdigest().upper(),
        'CompleteTrace': bool(end), 'WriterQueueRejected': end.get('write_queue_rejected') if end else None,
        'GPUQueueRejected': end.get('gpu_queue_rejected') if end else None,
        'Clips': clips, 'PhysicalInput': False, 'RS13Complete': False,
        'Scope': 'Original selected-window WGC surface times; source intervals distinguish cached idle baselines. Scalar fits are diagnostic and do not certify final desktop or isolated opacity/translation curves.',
    }
    args.output_json.write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    print(f"Described {len(clips)} native clips with source-frame clocks.")


if __name__ == '__main__':
    main()

"""Measure panel title opacity/vertical shift from original WGC surfaces.

The small title ROI excludes the incoming home tiles. Results retain sampling
uncertainty; low-opacity shift estimates and sparse sequences are not accepted.
"""
import argparse
import importlib.util
import json
from pathlib import Path

import numpy as np
from PIL import Image

spec = importlib.util.spec_from_file_location('rs13_native', Path(__file__).with_name('analyze-rs13-native.py'))
native = importlib.util.module_from_spec(spec)
spec.loader.exec_module(native)
ease = native.ease


def fit_curve(times, values, curve):
    moving = np.flatnonzero((values > .03) & (values < .97))
    if len(moving) < 4:
        return None
    onset_candidates = np.arange(times[moving[0]] - 100, times[moving[0]] + 1, 1)
    grid = np.linspace(0, 1, 4097)
    reference = ease(grid, curve)
    fits = []
    for duration in range(40, 501, 2):
        predicted = np.interp(np.clip((times[None, :] - onset_candidates[:, None]) / duration, 0, 1), grid, reference)
        errors = np.sqrt(np.mean((predicted - values) ** 2, axis=1))
        index = int(np.argmin(errors))
        fits.append({'DurationMilliseconds': duration, 'OnsetSourceMilliseconds': float(onset_candidates[index]), 'RMSE': float(errors[index])})
    best = min(fits, key=lambda fit: fit['RMSE'])
    plausible = [fit['DurationMilliseconds'] for fit in fits if fit['RMSE'] <= best['RMSE'] + .02]
    return {**best, 'DurationRangeAtRMSEPlus002': [min(plausible), max(plausible)]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('recording_directory', type=Path)
    parser.add_argument('session_json', type=Path)
    parser.add_argument('output_json', type=Path)
    parser.add_argument('--local-background', action='store_true', help='Fit a per-frame planar background separately from the title glyph')
    parser.add_argument('--roi-client', type=int, nargs=4, default=[74, 29, 86, 62], metavar=('LEFT', 'TOP', 'RIGHT', 'BOTTOM'), help='Explicit measured 96-DPI client title region')
    parser.add_argument('--luma', action='store_true', help='Fit luminance instead of subpixel color fringes')
    args = parser.parse_args()
    if args.output_json.exists():
        raise ValueError('Use a fresh output')
    session = json.loads(args.session_json.read_text(encoding='utf-8-sig'))
    events = [json.loads(line) for line in (args.recording_directory / 'events.jsonl').read_text().splitlines()]
    ready = next(event for event in events if event['event'] == 'capture_ready')
    if ready['pid'] != session['ProcessId'] or ready['creation_filetime_100ns'] != session['StartedTicks'] - 504911232000000000:
        raise ValueError('Foreign native process identity')
    results = []
    for start in [event for event in events if event['event'] == 'clip_start' and event['clip'].startswith('settings-')]:
        name = start['clip']
        if Path(name).name != name or not name.replace('-', '').isalnum():
            raise ValueError('Invalid clip name')
        rows = [event for event in events if event['event'] == 'frame' and event.get('clip') == name and event.get('saved')]
        if len(rows) < 3 or any(row['dpi'] != 96 or row['inset'] != 12 for row in rows):
            results.append({'Clip': name, 'Measured': False, 'Reason': 'Insufficient frames or ROI not calibrated for this DPI/inset'})
            continue
        images = []
        for row in rows:
            with Image.open(args.recording_directory / name / f"frame-{row['index']}.bmp") as image:
                images.append(np.asarray(image.convert('RGB')).astype(np.float32))
        if args.luma:
            images = [np.repeat(np.sum(image * np.array([.2126, .7152, .0722]), axis=2)[:, :, None], 3, axis=2) for image in images]
        if len({image.shape for image in images}) != 1:
            results.append({'Clip': name, 'Measured': False, 'Reason': 'Window geometry changed'})
            continue
        entering = name.startswith('settings-enter')
        panel = images[-1] if entering else images[0]
        background = images[0] if entering else images[-1]
        # Client x74..86 contains the V in "Velo 设置" while excluding the
        # launcher's search icon at x90. Stored images start at client (12,12).
        left, top, right, bottom = args.roi_client
        if not (12 <= left < right <= images[0].shape[1]+12 and 22 <= top < bottom <= images[0].shape[0]+12):
            raise ValueError('Title ROI is outside calibrated client bounds')
        x0, x1, y0, y1 = left-12, right-12, top-12, bottom-12
        base = background[y0:y1, x0:x1]
        shifts = np.arange(-1, 9.01, .25)
        candidates = []
        for shift in shifts:
            ys = np.arange(y0, y1) - shift
            top = np.floor(ys).astype(int)
            fraction = (ys - top)[:, None, None]
            candidates.append(panel[top, x0:x1] * (1 - fraction) + panel[top + 1, x0:x1] * fraction)
        candidates = np.asarray(candidates)
        delta = candidates - base
        mask = np.max(np.abs(delta), axis=(0, 3)) > 20
        if mask.sum() < 40:
            results.append({'Clip': name, 'Measured': False, 'Reason': 'Title has insufficient endpoint contrast'})
            continue
        vectors = delta[:, mask]
        if args.local_background:
            # The old and new Wails pages fade simultaneously. Treating the
            # first page's background as constant biases the title opacity.
            # Remove only a local plane (no glyph-shaped basis) before fitting.
            yy, xx = np.mgrid[0:y1-y0, 0:x1-x0]
            basis = np.stack([np.ones_like(xx), xx, yy], axis=-1)[mask].astype(float)
            orthogonal, _ = np.linalg.qr(basis)
            vectors = vectors - np.einsum('nk,mkc->mnc', orthogonal, np.einsum('nk,mnc->mkc', orthogonal, vectors))
        denominators = np.sum(vectors ** 2, axis=(1, 2))
        parameters = []
        for image in images:
            current = (image[y0:y1, x0:x1] - base)[mask]
            if args.local_background:
                current = current - orthogonal @ (orthogonal.T @ current)
            opacity = np.clip(np.sum(vectors * current, axis=(1, 2)) / denominators, 0, 1)
            errors = np.mean((vectors * opacity[:, None, None] - current) ** 2, axis=(1, 2))
            index = int(np.argmin(errors))
            parameters.append({'Opacity': float(opacity[index]), 'VerticalShiftLogicalPixels': float(shifts[index]), 'PixelRMSE': float(np.sqrt(errors[index]))})
        times = np.asarray([row['frame_time_100ns'] for row in rows], dtype=np.float64) / 10000
        times -= times[0]
        if np.any(np.diff(times) <= 0):
            raise ValueError('Panel source frame clock is not strictly increasing')
        values = np.asarray([parameter['Opacity'] if entering else 1 - parameter['Opacity'] for parameter in parameters])
        fit = fit_curve(times, values, (.22, 1, .36, 1) if entering else (.25, .1, .25, 1))
        geometry_fit = None
        visible = np.array([parameter['Opacity'] > .15 for parameter in parameters])
        if entering and fit and int(visible.sum()) >= 4:
            relative = times[visible] - fit['OnsetSourceMilliseconds']
            observed = np.array([parameter['VerticalShiftLogicalPixels'] for parameter in parameters])[visible]
            grid = np.linspace(0, 1, 4097)
            reference = ease(grid, (.22, 1, .36, 1))
            fits = []
            for duration in range(100, 501, 2):
                predicted = 8 * (1 - np.interp(np.clip(relative / duration, 0, 1), grid, reference))
                fits.append({'DurationMilliseconds': duration, 'RMSELogicalPixels': float(np.sqrt(np.mean((predicted - observed) ** 2)))})
            geometry_fit = min(fits, key=lambda candidate: candidate['RMSELogicalPixels'])
        moving = np.flatnonzero((values > .03) & (values < .97))
        active = range(max(0, moving[0] - 1), min(len(rows), moving[-1] + 2)) if len(moving) else []
        active_gaps = [float(times[b] - times[a]) for a, b in zip(active, list(active)[1:]) if not rows[a].get('baseline_cached')]
        results.append({
            'Clip': name, 'Measured': True, 'ROIClientLogicalPixels': args.roi_client,
            'Frames': len(rows), 'IntermediateFrames': len(moving),
            'MaximumActiveSourceGapMilliseconds': max(active_gaps) if active_gaps else None,
            'DeclaredOpacityMilliseconds': 220 if entering else 100,
            'OpacityFit': fit,
            'DeclaredEnterShiftDurationMilliseconds': 280 if entering else None,
            'VerticalShiftFit': geometry_fit,
            'VisibleExitMaximumAbsoluteShiftLogicalPixels': float(max(abs(parameter['VerticalShiftLogicalPixels']) for parameter in parameters if parameter['Opacity'] > .15)) if not entering and np.any(visible) else None,
            'Samples': [{'SourceMilliseconds': float(t), **parameter} for t, parameter in zip(times, parameters)],
            'BackgroundModel': 'Per-frame local plane removed independently of glyph' if args.local_background else 'Fixed first/last page endpoint',
            'ColorModel': 'Luminance' if args.luma else 'RGB',
            'Scope': 'Title-only optical estimate. Shift is poorly identifiable at near-zero opacity; fit error and original frames must be reviewed before acceptance.',
        })
    args.output_json.write_text(json.dumps({'Renderer': session['Edition'], 'ExecutableSHA256': session['SHA256'], 'Rows': results,
        'PhysicalInput': False, 'RS13Complete': False, 'Scope': 'Optical native surface measurements, not GPU execution time or final desktop presentation. No fitted curve is automatically accepted.'}, indent=2) + '\n', encoding='utf-8')
    print(f'Measured {len(results)} settings transition title sequences.')


if __name__ == '__main__':
    main()

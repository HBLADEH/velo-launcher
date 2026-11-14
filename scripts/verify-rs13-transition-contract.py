"""Compare independently measured native title samples with declared motion.

The duration/easing are fixed from the Wails contract; only the unknown trigger
onset is fitted. Passing means consistency at the recorded source-frame and
pixel resolution, never an exact display-time or unique-curve assertion.
"""
import argparse
import importlib.util
import hashlib
import json
from pathlib import Path
import re

import numpy as np

spec = importlib.util.spec_from_file_location('rs13_native_curve', Path(__file__).with_name('analyze-rs13-native.py'))
native = importlib.util.module_from_spec(spec)
spec.loader.exec_module(native)


def reference_contract(path):
    text = path.read_text(encoding='utf-8-sig')
    main = re.search(r'--ease:\s*cubic-bezier\(([^)]+)\)', text)
    incoming = re.search(r'\.panel-enter-active\s*\{[^}]*opacity\s+(\d+)ms\s+var\(--ease\),\s*transform\s+(\d+)ms', text)
    outgoing = re.search(r'\.panel-leave-active\s*\{[^}]*transition:\s*opacity\s+(\d+)ms\s+ease', text)
    shift = re.search(r'\.panel-enter-from\s*\{[^}]*translateY\((\d+)px\)', text)
    if not all((main, incoming, outgoing, shift)):
        raise ValueError('Independent Wails CSS panel contract is missing or ambiguous')
    return {'MainCurve': [float(value.strip()) for value in main[1].split(',')],
            'IncomingOpacityMilliseconds': int(incoming[1]), 'IncomingTranslationMilliseconds': int(incoming[2]),
            'OutgoingOpacityMilliseconds': int(outgoing[1]), 'IncomingTranslationLogicalPixels': int(shift[1]),
            'CSSSHA256': hashlib.sha256(path.read_bytes()).hexdigest().upper()}


def check(row, reference=None):
    reference = reference or {'MainCurve': [.22, 1, .36, 1], 'IncomingOpacityMilliseconds': 220,
                              'IncomingTranslationMilliseconds': 280, 'OutgoingOpacityMilliseconds': 100,
                              'IncomingTranslationLogicalPixels': 8}
    if not row.get('Measured'):
        return {'Clip': row['Clip'], 'Status': 'insufficient', 'Reason': row.get('Reason', 'No native title measurements')}
    samples = row['Samples']
    times = np.array([sample['SourceMilliseconds'] for sample in samples])
    opacity = np.array([sample['Opacity'] for sample in samples])
    shifts = np.array([sample['VerticalShiftLogicalPixels'] for sample in samples])
    residual = np.array([sample['PixelRMSE'] for sample in samples])
    if (len(times) < 3 or any(np.any(~np.isfinite(value)) for value in (times, opacity, shifts, residual))
            or np.any(np.diff(times) <= 0) or np.any((opacity < 0) | (opacity > 1)) or np.any(residual < 0)):
        raise ValueError('Native title data has an invalid source clock or values')
    entering = row['Clip'].startswith('settings-enter')
    progress = opacity if entering else 1 - opacity
    moving = (progress > .03) & (progress < .97)
    visible = (opacity > .15) & moving
    reported_gap = row.get('MaximumActiveSourceGapMilliseconds')
    if reported_gap is not None and (not np.isfinite(reported_gap) or reported_gap <= 0):
        raise ValueError('Reported active source gap must be finite and positive')
    indices = np.flatnonzero(moving)
    if len(indices) < 4:
        return {'Clip': row['Clip'], 'Status': 'insufficient', 'IntermediateFrames': int(moving.sum()),
                'Reason': 'Need at least four intermediate samples'}
    first_index, last_index = int(indices[0]), int(indices[-1])
    # Include the rest->moving and moving->settled boundaries. A cached idle
    # frame can be minutes old even when every intermediate frame is dense.
    # Derive gaps from source samples instead of trusting summary metadata.
    lead_gap = float(times[first_index] - times[first_index - 1]) if first_index else None
    tail_gap = float(times[last_index + 1] - times[last_index]) if last_index + 1 < len(times) else None
    start = max(0, first_index - 1)
    end = min(len(times) - 1, last_index + 1)
    gap = float(np.diff(times[start:end + 1]).max())
    coverage = {'IntermediateFrames': len(indices), 'MaximumActiveSourceGapMilliseconds': gap,
                'ReportedMaximumActiveSourceGapMilliseconds': reported_gap,
                'TriggerBracketGapMilliseconds': lead_gap, 'CompletionBracketGapMilliseconds': tail_gap}
    if (lead_gap is None or progress[first_index - 1] > .03 or tail_gap is None
            or progress[last_index + 1] < .97 or gap > 55 or (reported_gap is not None and reported_gap > 55)):
        return {'Clip': row['Clip'], 'Status': 'insufficient', **coverage,
                'Reason': 'Need rest, intermediate and settled samples with source gaps <=55ms, including the trigger boundary'}
    first = times[np.argmax(progress > .03)]
    previous = times[max(0, np.argmax(progress > .03) - 1)]
    # The <=3% threshold can occur just after the first source event. Search a
    # bounded onset interval, using live data near the transition only so a long
    # idle tail cannot dilute the residual or create a false green check.
    onset_candidates = np.arange(max(previous - 55, first - 100), first + 1, .5)
    grid = np.linspace(0, 1, 8193)
    curve = reference['MainCurve'] if entering else (.25, .1, .25, 1)
    curve_values = native.ease(grid, curve)
    duration = reference['IncomingOpacityMilliseconds'] if entering else reference['OutgoingOpacityMilliseconds']
    model = np.interp(np.clip((times[None, :] - onset_candidates[:, None]) / duration, 0, 1), grid, curve_values)
    near = (times >= first - 55) & (times <= times[np.flatnonzero(moving)[-1]] + 55)
    errors = np.sqrt(np.mean((model[:, near] - progress[near]) ** 2, axis=1))
    best = int(np.argmin(errors))
    onset = onset_candidates[best]
    alpha_rmse = float(errors[best])
    expected_shift = reference['IncomingTranslationLogicalPixels'] * (1 - np.interp(np.clip((times - onset) / reference['IncomingTranslationMilliseconds'], 0, 1), grid, native.ease(grid, reference['MainCurve']))) if entering else np.zeros(len(times))
    shift_error = float(np.sqrt(np.mean((expected_shift[visible] - shifts[visible]) ** 2))) if visible.any() else None
    max_shift_error = float(np.max(np.abs(expected_shift[visible] - shifts[visible]))) if visible.any() else None
    endpoint = float(opacity[-1] if entering else 1 - opacity[-1])
    pixel_residual = float(np.percentile(residual[moving], 95))
    passed = alpha_rmse <= .05 and shift_error is not None and shift_error <= .75 and max_shift_error <= 1.5 and endpoint >= .97
    # A poor optical model is evidence insufficiency, not evidence that the
    # application's declared animation is wrong. Never relax the quality gate
    # merely because the Wails reference fails this particular title fit.
    status = 'insufficient' if pixel_residual > 18 else 'compatible' if passed else 'mismatch'
    return {'Clip': row['Clip'], 'Status': status,
            'FixedOpacityDurationMilliseconds': duration, 'FixedOpacityCurve': list(curve),
            'FixedEnterTranslationLogicalPixels': reference['IncomingTranslationLogicalPixels'], 'FixedEnterTranslationDurationMilliseconds': reference['IncomingTranslationMilliseconds'],
            'EstimatedTriggerOnsetSourceMilliseconds': float(onset), **coverage, 'OpacityRMSE': alpha_rmse,
            'TranslationRMSELogicalPixels': shift_error, 'MaximumTranslationErrorLogicalPixels': max_shift_error,
            'FinalProgress': endpoint, 'P95TitleOpticalResidualRGB': pixel_residual,
            'Reason': 'Title optical residual exceeds the calibrated model quality limit' if pixel_residual > 18 else None,
            'Tolerance': {'OpacityRMSE': .05, 'TranslationRMSELogicalPixels': .75, 'MaximumTranslationErrorLogicalPixels': 1.5,
                          'FinalProgressMinimum': .97, 'P95TitleOpticalResidualRGB': 18},
            'Scope': 'Comparison of the fixed declared waveform with independent optical samples at their resolution. Nearby waveforms may be indistinguishable; no exact display/presentation latency claim.'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('title_measurements', type=Path)
    parser.add_argument('output_json', type=Path)
    parser.add_argument('--reference-css', type=Path, default=Path(__file__).resolve().parents[1] / 'frontend/src/style.css')
    args = parser.parse_args()
    if args.output_json.exists():
        raise ValueError('Use a fresh contract result')
    source = json.loads(args.title_measurements.read_text(encoding='utf-8-sig'))
    reference = reference_contract(args.reference_css)
    rows = [check(row, reference) for row in source['Rows']]
    result = {'Renderer': source['Renderer'], 'ExecutableSHA256': source['ExecutableSHA256'], 'Rows': rows,
              'AllRecordedSettingsCompatible': bool(rows) and all(row['Status'] == 'compatible' for row in rows),
              'IndependentWailsReference': reference,
              'ExactCurveUniquenessEstablished': False, 'PhysicalInput': False,
              'Scope': 'Native settings opacity and vertical translation only, at recorded source-frame/pixel tolerance. Does not certify other animation families or physical IME/DPI.'}
    args.output_json.write_text(json.dumps(result, indent=2) + '\n', encoding='utf8')
    print(f"{source['Renderer']}: {sum(row['Status'] == 'compatible' for row in rows)}/{len(rows)} fixed native transition contracts compatible.")


if __name__ == '__main__':
    main()

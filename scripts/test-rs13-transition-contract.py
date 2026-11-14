"""Calibrate native motion verification and reject unobserved transition boundaries."""
import argparse
import copy
import importlib.util
import json
from pathlib import Path

import numpy as np

spec = importlib.util.spec_from_file_location('rs13_contract', Path(__file__).with_name('verify-rs13-transition-contract.py'))
contract = importlib.util.module_from_spec(spec)
spec.loader.exec_module(contract)


def samples(enter=True, opacity_duration=None, translation_duration=280, linear=False, sliding_exit=False):
    times = np.arange(0, 800, 1000 / 60)
    duration = opacity_duration or (220 if enter else 100)
    curve = (.22, 1, .36, 1) if enter else (.25, .1, .25, 1)
    phase = np.clip((times - 35) / duration, 0, 1)
    progress = phase if linear else contract.native.ease(phase, curve)
    shifts = (8 * (1 - contract.native.ease(np.clip((times - 35) / translation_duration, 0, 1), (.22, 1, .36, 1)))
              if enter else progress * 8 if sliding_exit else np.zeros_like(times))
    return {'Clip': 'settings-enter-calibration' if enter else 'settings-exit-calibration', 'Measured': True,
            'MaximumActiveSourceGapMilliseconds': 1000 / 60,
            'Samples': [{'SourceMilliseconds': float(time), 'Opacity': float(value if enter else 1 - value),
                         'VerticalShiftLogicalPixels': float(shift), 'PixelRMSE': .1}
                        for time, value, shift in zip(times, progress, shifts)]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output_json', type=Path)
    args = parser.parse_args()
    if args.output_json.exists():
        raise ValueError('Use a fresh calibration output')
    reference = contract.reference_contract(Path(__file__).resolve().parents[1] / 'frontend/src/style.css')
    results = []

    def check(name, row, expected):
        actual = contract.check(row, reference)
        if actual['Status'] != expected:
            raise AssertionError((name, expected, actual))
        results.append({'Case': name, 'Status': actual['Status'], 'Passed': True})

    check('declared-enter', samples(), 'compatible')
    check('declared-exit', samples(False), 'compatible')
    check('slow-opacity', samples(opacity_duration=500), 'mismatch')
    check('linear-opacity', samples(linear=True), 'mismatch')
    check('slow-translation', samples(translation_duration=650), 'mismatch')
    check('sliding-exit', samples(False, sliding_exit=True), 'mismatch')
    poor = samples()
    for sample in poor['Samples']:
        sample['PixelRMSE'] = 30
    check('poor-optical-model', poor, 'insufficient')
    sparse = samples()
    sparse['MaximumActiveSourceGapMilliseconds'] = 80
    check('reported-source-gap', sparse, 'insufficient')
    # Dense moving frames with a stale cached baseline previously passed.
    stale = samples()
    first = next(index for index, sample in enumerate(stale['Samples']) if sample['Opacity'] > .03)
    for sample in stale['Samples'][first:]:
        sample['SourceMilliseconds'] += 500
    check('stale-baseline-dense-motion', stale, 'insufficient')
    for enter in (True, False):
        dense = samples(enter)
        moving = [index for index, sample in enumerate(dense['Samples']) if .03 < sample['Opacity'] < .97]
        no_start = copy.deepcopy(dense)
        no_start['Samples'] = no_start['Samples'][moving[0]:]
        check(f'missing-rest-{enter}', no_start, 'insufficient')
        no_end = copy.deepcopy(dense)
        no_end['Samples'] = no_end['Samples'][:moving[-1] + 1]
        check(f'missing-settled-{enter}', no_end, 'insufficient')
    internal = samples(opacity_duration=500)
    internal['Samples'] = internal['Samples'][:5] + internal['Samples'][11:]
    check('actual-gap-despite-dense-metadata', internal, 'insufficient')
    distant_tail = samples()
    last = max(index for index, sample in enumerate(distant_tail['Samples']) if .03 < sample['Opacity'] < .97)
    for sample in distant_tail['Samples'][last + 1:]:
        sample['SourceMilliseconds'] += 500
    check('distant-settled-sample', distant_tail, 'insufficient')
    idle = samples()
    idle['Samples'][-1]['SourceMilliseconds'] += 60_000
    check('distant-idle-tail-after-settled', idle, 'compatible')
    missing_metadata = samples()
    del missing_metadata['MaximumActiveSourceGapMilliseconds']
    check('derive-gaps-from-source', missing_metadata, 'compatible')
    for name, mutate in (
        ('reversed-clock', lambda row: row['Samples'][4].update(SourceMilliseconds=-1)),
        ('nonfinite-opacity', lambda row: row['Samples'][4].update(Opacity=float('nan'))),
        ('negative-optical-residual', lambda row: row['Samples'][4].update(PixelRMSE=-1)),
        ('nonfinite-reported-gap', lambda row: row.update(MaximumActiveSourceGapMilliseconds=float('nan'))),
    ):
        row = samples()
        mutate(row)
        try:
            contract.check(row, reference)
        except ValueError:
            results.append({'Case': name, 'RefusedInvalidData': True, 'Passed': True})
        else:
            raise AssertionError(name)
    args.output_json.write_text(json.dumps({'Passed': True, 'Checks': results,
        'Reference': reference, 'Scope': 'Known waveforms and source-coverage rejection; synthetic calibration only, no product or display acceptance.'}, indent=2) + '\n', encoding='utf8')
    print(f'{len(results)} native transition contract/calibration checks passed.')


if __name__ == '__main__':
    main()

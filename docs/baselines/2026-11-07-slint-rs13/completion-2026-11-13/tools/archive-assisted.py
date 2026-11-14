"""Publish the assisted physical matrix with its original executable identities."""
from collections import Counter, defaultdict
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import re
import shutil

repo = Path(__file__).resolve().parents[2]
local = Path(__file__).resolve().parent
out = repo / 'docs/baselines/2026-11-07-slint-rs13/completion-2026-11-13'
if out.exists():
    raise ValueError('Preserve previous baseline output')
out.mkdir()


def read(path):
    return json.loads(path.read_text(encoding='utf-8-sig'))


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest().upper()


def write(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    if path.exists():
        raise ValueError(f'Output exists: {path}')
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n', encoding='utf8')


def copy(source, target):
    target.parent.mkdir(parents=True, exist_ok=True)
    if target.exists():
        raise ValueError(f'Output exists: {target}')
    shutil.copyfile(source, target)
    assert sha(source) == sha(target)


builds = []
for renderer in ('software', 'femtovg'):
    folder = repo / 'rust/target/artifacts' / renderer
    identity = read(folder / 'build-identity.json')
    receipt = read(folder / 'verification.json')
    assert sha(folder / 'velo-slint.exe') == receipt['ExecutableSHA256'] == identity['ExecutableSHA256']
    assert sha(folder / 'build-identity.json') == receipt['BuildIdentitySHA256']
    assert receipt['ChecksPassed'] and receipt['SmokeTestPassed']
    assert len(identity['Sources']) == 206
    assert all(sha(repo / 'rust' / source['Path']) == source['SHA256'] for source in identity['Sources'])
    assert sha(folder / 'build-identity.json') == sha(local / 'identities' / renderer / 'build-identity.json')
    builds.append({'Renderer': renderer, 'Identity': identity, 'Verification': receipt,
                   'CurrentInputsMatched': 206, 'ProductRebuiltThisPass': False})
write(out / 'build-identities.json', builds)

reports = [json.loads(line) for line in (local / 'operator-reports.jsonl').read_text(encoding='utf-8-sig').splitlines()]
write(out / 'operator-reports.json', {'Reports': reports, 'SourceSHA256': sha(local / 'operator-reports.jsonl'),
    'Scope': 'Direct human replies preserved verbatim; native checks and acceptance scopes are recorded separately.'})
sessions = []
input_by_dpi = {}
for run in sorted((local / 'light/runs').iterdir()):
    session = read(run / 'session.json')
    completion = read(run / 'completion.json')
    assert completion['Completed'] and completion['ExitCode'] == 0 and completion['ForcedCleanup'] is False
    assert session['SHA256'] == completion['SHA256']
    assert sha(run / 'local/Velo/history.json') == session['Inputs']['history.json']
    assert sha(run / 'local/Velo/config.json') == session['Inputs']['config.json']
    raw = (run / 'input-events.log').read_text()
    events = [dict(re.findall(r'([A-Za-z_][A-Za-z0-9_]*)=([^ ]+)', line)) for line in raw.splitlines()]
    assert not any(event.get('event') == 'launch_started' for event in events)
    by_dpi = defaultdict(Counter)
    dpi = 96
    for event in events:
        if event.get('event') == 'window_layout':
            dpi = int(event['dpi'])
        if event.get('event') == 'input_accepted':
            assert event.get('synthetic') == 'false', 'No automated typing belongs in this pass'
            by_dpi[dpi]['Edits'] += 1
            by_dpi[dpi]['Composing'] += event['composing'] == 'true'
            by_dpi[dpi]['NonComposing'] += event['composing'] == 'false'
            by_dpi[dpi]['ThreeByteNonComposing'] += event['composing'] == 'false' and event['utf8_bytes'] == '3'
    if run.name.endswith('-physical'):
        assert all(by_dpi[dpi]['Composing'] and by_dpi[dpi]['ThreeByteNonComposing'] for dpi in (96, 120, 144, 168))
        input_by_dpi[session['Edition']] = by_dpi
    observations = []
    for path in sorted(run.glob('native-window-observation*.json')):
        observation = read(path)
        assert observation['SHA256'] == session['SHA256'] and not observation['Capture']['Truncated']
        copy(path, out / 'sessions' / run.name / path.name)
        observations.append({'File': path.name, 'SHA256': sha(path), 'TargetEnded': observation['Capture']['TargetEnded']})
    copy(run / 'completion.json', out / 'sessions' / run.name / 'completion.json')
    sessions.append({'Run': run.name, 'Renderer': session['Edition'], 'ProcessId': session['ProcessId'],
        'StartedTicks': session['StartedTicks'], 'ExecutableSHA256': session['SHA256'], 'Completion': completion,
        'RawInputLogSHA256': sha(run / 'input-events.log'), 'HistoryUnchanged': True, 'ConfigUnchanged': True,
        'LaunchCallbacks': 0, 'InputEditsByActualLoggedDPI': {str(d): dict(counts) for d, counts in by_dpi.items()},
        'WindowObservations': observations})
write(out / 'sessions.json', sessions)

matrix = []
for renderer in ('software', 'femtovg'):
    for scale in (100, 125, 150, 175):
        checkpoint_path = local / f'{renderer}-{scale}-physical.json'
        checkpoint = read(checkpoint_path)
        native_path = local / f'{renderer}-{scale}-native.json'
        assert sha(native_path) == checkpoint['NativeObservationSHA256']
        assert checkpoint['ExecutableSHA256'] == next(row['Identity']['ExecutableSHA256'] for row in builds if row['Renderer'] == renderer)
        copy(checkpoint_path, out / 'physical' / checkpoint_path.name)
        copy(native_path, out / 'physical' / native_path.name)
        dpi = scale * 96 // 100
        if renderer == 'femtovg' and scale == 150:
            interval_path = local / 'femtovg-150-validated-log-interval.json'
            interval = read(interval_path)
            assert interval['InputAtRequestedDPIConfirmedByApplicationLog'] and interval['InputIntervalDPI'] == dpi
            assert not checkpoint['NativeDPIMatched'] and checkpoint['NativeDPIs'] == [96]
            copy(interval_path, out / 'physical' / interval_path.name)
            proof = 'Direct operator reply and 144 DPI application layouts enclosing the actual edits. Later passive 96 DPI checkpoint kept as a mismatching post-restoration snapshot.'
        else:
            assert checkpoint['NativeDPIMatched'] and checkpoint['NativeDPIs'] == [dpi]
            proof = 'Direct operator reply plus passive native DPI checkpoint and application layouts/edits.'
        matrix.append({'Renderer': renderer, 'ScalePercent': scale, 'ActualInputDPI': dpi,
            'ExecutableSHA256': checkpoint['ExecutableSHA256'], 'OperatorReply': checkpoint['OperatorReply'],
            'OperatorConfirmed': ['Layout', 'Pinyin candidate location', 'Candidate paging', 'Word selection', 'Escape cancellation', 'English abc'],
            'Evidence': proof, 'Checkpoint': 'physical/' + checkpoint_path.name,
            'LoggedEditCountsAtDPIWholeRun': dict(input_by_dpi[renderer][dpi]),
            'CandidateUIIndependentlyInstrumented': False,
            'Scope': 'Physical-keyboard behavior is operator-confirmed. Logs retain composition/visibility/focus and redacted byte counts; synthetic=false alone is not physical-input proof.'})
write(out / 'physical-input.json', {'Date': '2026-11-13 Asia/Shanghai', 'Rows': matrix,
    'CurrentExecutableMatrixRows': 8, 'PhysicalInputSource': 'Direct user participation in this chat, with no automated typing in these sessions.',
    'Scale200Percent': 'Not tested: prior user display choices stop at 175%; not relabeled as passed.',
    'OriginalPhysicalMatricesUnchanged': True, 'RS13Complete': False})

cross = []
for renderer in ('software', 'femtovg'):
    path = local / f'{renderer}-crossscreen-physical.json'
    value = read(path)
    assert value['NativeDPIs'] == [96]
    copy(path, out / 'physical' / path.name)
    copy(local / f'{renderer}-crossscreen-native.json', out / 'physical' / f'{renderer}-crossscreen-native.json')
    capture_path = local / 'light/runs' / f'{renderer}-physical/native-window-observation-2.json'
    capture = read(capture_path)['Capture']
    assert set(row['DPI'] for row in capture['Changes']) == {96, 168}
    assert capture['TargetEnded'] and not capture['Truncated']
    cross.append({'Renderer': renderer, 'ExecutableSHA256': value['ExecutableSHA256'], 'OperatorReply': value['OperatorReply'],
        'PhysicalCrossScreenInputOperatorConfirmed': True, 'ObservedDPIs': [168, 96],
        'PassiveTrace': f'sessions/{renderer}-physical/native-window-observation-2.json',
        'PassiveTraceSHA256': sha(capture_path), 'Scope': 'Physical drag/input is user-confirmed; passive polling and application logs support actual DPI/position changes, not exact per-frame monitor handoff timing.'})
write(out / 'cross-screen.json', {'Rows': cross, 'DisplayRestoredTo100PercentOperatorConfirmed': True, 'FinalNativeDPI': 96})

copy(local / 'appearance-acceptance.json', out / 'appearance/acceptance.json')
copy(local / 'appearance-review.png', out / 'appearance/review.png')
copy(local / 'appearance-review-sources.json', out / 'appearance/sources.json')
copy(local / 'cleanup.json', out / 'cleanup.json')
assert read(out / 'cleanup.json')['CleanupPassed']

attempts = []
for name in ('femtovg-pulse-native-1', 'femtovg-transitions-native-1', 'femtovg-transitions-native-2', 'femtovg-transitions-native-3', 'femtovg-transitions-native-4'):
    directory = local / name
    result = read(directory / 'capture-result.json') if (directory / 'capture-result.json').exists() else None
    attempts.append({'Run': name, 'CaptureResult': result,
        'RawTraceSHA256': sha(directory / 'events.jsonl') if (directory / 'events.jsonl').exists() else None,
        'ErrorText': (directory / 'stderr.log').read_text(encoding='utf-8-sig').strip() if (directory / 'stderr.log').exists() else '',
        'Acceptance': 'No capture: hidden window refused by WGC' if result is None else 'No settings action inside the fixed recording window' if name.endswith('native-2') else 'Analyze source timing/ROI before accepting'})
write(out / 'native/attempts.json', attempts)

frame_references = 0
pulse = local / 'femtovg-pulse-joined-1'
pulse_manifest = read(pulse / 'source-manifest.json')
for row in pulse_manifest['CopiesSHA256Checked']:
    assert sha(repo / row['OriginalFile']) == sha(pulse / row['DerivedFile']) == row['SHA256']
    frame_references += 1
for filename in ('source-manifest.json', 'motion-roi.json'):
    copy(pulse / filename, out / 'native/pulse' / filename)
for path in (local / 'femtovg-pulse-previews').iterdir():
    copy(path, out / 'native/pulse/preview' / path.name)
contracts = []
for sequence in (1, 2):
    directory = local / f'femtovg-settings-cycles-{sequence}'
    manifest = read(directory / 'source-manifest.json')
    for clip in manifest['Clips']:
        for frame in clip['OriginalFrames']:
            assert sha(repo / frame['OriginalFile']) == sha(directory / frame['File']) == frame['SHA256']
            frame_references += 1
    report = read(directory / 'transition-contract.json')
    for row in report['Rows']:
        contracts.append({'Sequence': sequence, **row})
    for filename in ('source-manifest.json', 'panel-analysis.json', 'transition-contract.json'):
        copy(directory / filename, out / f'native/settings-{sequence}' / filename)
    if (directory / 'previews').exists():
        for path in (directory / 'previews').iterdir():
            copy(path, out / f'native/settings-{sequence}/preview' / path.name)
write(out / 'native/transition-summary.json', {'Rows': contracts,
    'Compatible': sum(row['Status'] == 'compatible' for row in contracts),
    'Insufficient': sum(row['Status'] == 'insufficient' for row in contracts),
    'Scope': 'All candidate intervals retained. Compatible means consistency with declared source waveforms at calibrated source-frame/pixel tolerance, never exact GPU/photon timing or curve uniqueness.'})
tool_rows = []
for path in (local / 'checkpoint-physical.ps1', local / 'capture-recording-pulse.ps1', local / 'capture-manual-transitions.ps1',
             local / 'export-settings-cycles.py', local / 'check-cleanup.ps1', Path(__file__),
             repo / 'scripts/verify-rs13-transition-contract.py', repo / 'scripts/analyze-rs13-panel.py', repo / 'scripts/analyze-rs13-motion-roi.py'):
    copy(path, out / 'tools' / path.name)
    tool_rows.append({'Path': path.relative_to(repo).as_posix(), 'SHA256': sha(path)})
write(out / 'tools/identities.json', tool_rows)
write(out / 'verification.json', {'PhysicalMatrixRowsVerified': len(matrix), 'PhysicalCrossScreenRowsVerified': len(cross),
    'CurrentInputsMatchedPerRenderer': 206, 'OriginalPhysicalMatrixRelabeled': False,
    'FemtoVGPulseNativeCaptureCompleted': True, 'PulseLiveFrames': 153, 'PulseLiveSourceSpanMilliseconds': 2702.6964,
    'PulseMaximumLiveGapMilliseconds': 33.3688, 'FemtoVGSettingsCompatibleIntervals': 2,
    'SettingsTimingFullyAccepted': False, 'AppearanceDifferencesOperatorAccepted': True,
    'NormalProductExits': 3, 'CompletedRecorderExits': 4, 'FailedCaptureAttemptsPreserved': 1,
    'NativeFrameSHA256ReferencesChecked': frame_references, 'GPUAndWriterQueueRejected': 0,
    'CompletedCapturesStorageLimited': False, 'FormalFilesUnchanged': 5, 'CleanupPassed': True,
    'Scale200PercentTested': False, 'DefaultRendererChosen': False, 'G4Passed': False, 'RS13Complete': False,
    'RecordedAtUtc': datetime.now(timezone.utc).isoformat(),
    'Scope': 'Current executable physical/operator and captured native evidence; remaining exact native exit and Software transition coverage are explicit.'})
print(f'Archived {len(matrix)} physical rows, {len(cross)} cross-screen rows, {len(contracts)} native intervals, {frame_references} native frame SHA references; RS-13 remains in progress.')

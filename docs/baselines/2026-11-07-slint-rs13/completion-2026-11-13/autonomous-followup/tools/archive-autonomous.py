"""Append autonomous current-executable evidence while preserving assisted records."""
from collections import Counter
from datetime import datetime,timezone
import hashlib
import json
from pathlib import Path
import shutil

repo=Path(__file__).resolve().parents[2]
local=Path(__file__).resolve().parent
out=repo/'docs/baselines/2026-11-07-slint-rs13/completion-2026-11-13'
target=out/'autonomous-followup'
if target.exists():raise ValueError('Preserve prior autonomous output')
target.mkdir()
def read(path):return json.loads(path.read_text(encoding='utf-8-sig'))
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest().upper()
def copy(source,dest):
 dest.parent.mkdir(parents=True,exist_ok=True)
 if dest.exists():raise ValueError(str(dest))
 shutil.copyfile(source,dest);assert sha(source)==sha(dest)
def write(path,data):
 path.parent.mkdir(parents=True,exist_ok=True)
 if path.exists():raise ValueError(str(path))
 path.write_text(json.dumps(data,indent=2,ensure_ascii=False)+'\n',encoding='utf8')
specs=[('software-enter-controlled-2','software-enter-joined-2'),('software-exit-controlled-1','software-exit-joined-1'),
 ('software-exit-controlled-2','software-exit-joined-2'),('software-exit-phase-350','software-exit-phase-350-joined'),
 ('software-exit-phase-500','software-exit-phase-500-joined'),('femtovg-exit-phase-350','femtovg-exit-phase-350-joined'),
 ('femtovg-exit-phase-500','femtovg-exit-phase-500-joined')]
all_results=[];frame_checks=0
for source_name,joined_name in specs:
 source=local/source_name;joined=local/joined_name
 capture=read(source/'capture-result.json')
 assert capture['Completed'] and capture['RecorderExitCode']==0
 assert not capture['CaptureEnd']['storage_limited'] and capture['CaptureEnd']['gpu_queue_rejected']==capture['CaptureEnd']['write_queue_rejected']==0
 report=read(joined/'transition-contract.json');manifest=read(joined/'source-manifest.json')
 assert sha(source/'events.jsonl')==manifest['OriginalTraceSHA256']
 for frame in manifest['Frames']:
  assert sha(repo/frame['OriginalFile'])==sha(joined/frame['File'])==frame['SHA256'];frame_checks+=1
 for filename in ('source-manifest.json','panel-analysis.json','transition-contract.json'):
  copy(joined/filename,target/joined_name/filename)
 for filename in ('capture-result.json','events.jsonl','stderr.log'):
  copy(source/filename,target/source_name/filename)
 for row in report['Rows']:
  all_results.append({'Run':source_name,'Renderer':report['Renderer'],'ExecutableSHA256':report['ExecutableSHA256'],
   'SyntheticStimulus':True,'BaselineMilliseconds':capture.get('BaselineMilliseconds',200),**row})
manual=read(out/'native/transition-summary.json')
for row in manual['Rows']:
 all_results.append({'Run':f'femtovg-settings-cycles-{row["Sequence"]}','Renderer':'femtovg',
  'ExecutableSHA256':'D81F31ED8AB3C0D45A3565CDC8AA412B3FB870C0CC350204178CDE44456B6487','SyntheticStimulus':False,**row})
counts=Counter(row['Status'] for row in all_results)
assert counts['compatible']==7 and counts['mismatch']==1
write(out/'native/combined-transition-results.json',{'Rows':all_results,'Counts':dict(counts),
 'ThresholdsChanged':False,'FailedOrInsufficientAttemptsPreserved':True,
 'Scope':'Seven compatible intervals cover both current renderers and both directions. One earlier Software exit exceeds alpha RMSE 0.05; two predetermined follow-up phase samples per renderer passed. This is sampled-waveform consistency, not all-frame perfection or unique curve/photon timing.'})
failure=local/'software-enter-controlled-1'
copy(failure/'events.jsonl',target/'software-enter-controlled-1/events.jsonl')
copy(failure/'stderr.log',target/'software-enter-controlled-1/stderr.log')
write(target/'software-enter-controlled-1/attempt.json',{'CompletedCapture':False,'Reason':'Animation trigger was sent only after the controller bounded waiting interval elapsed; no settings transition acceptance.',
 'SourceTraceSHA256':sha(failure/'events.jsonl'),'AppInputCountedAsPhysical':False})
for filename in ('exit-phase-followup-scope.json','optical-model-review-scope.json','panel-calibration.json','transition-calibration-final.json'):
 copy(local/filename,target/filename)
for directory in ('software-enter-joined-2','software-exit-joined-1','software-exit-joined-2','femtovg-settings-cycles-1','femtovg-settings-cycles-2'):
 for filename in ('panel-glyph-diagnostic.json','contract-glyph-diagnostic.json'):
  copy(local/directory/filename,target/'glyph-diagnostics'/directory/filename)
copy(local/'femtovg-pulse-timeseries.json',out/'native/pulse/timeseries.json')
copy(local/'femtovg-pulse-timeline.png',out/'native/pulse/timeline.png')
copy(local/'cleanup-final.json',out/'cleanup-final.json')
assert read(out/'cleanup-final.json')['CleanupPassed']
sessions=[]
for name in ('software-native-auto','software-native-phase','femtovg-native-phase'):
 directory=local/'light/runs'/name
 session=read(directory/'session.json');complete=read(directory/'completion.json')
 assert complete['Completed'] and complete['ExitCode']==0 and complete['ForcedCleanup'] is False
 assert sha(directory/'local/Velo/config.json')==session['Inputs']['config.json']
 assert sha(directory/'local/Velo/history.json')==session['Inputs']['history.json']
 copy(directory/'completion.json',target/'sessions'/name/'completion.json')
 copy(directory/'commands.jsonl',target/'sessions'/name/'commands.jsonl')
 sessions.append({'Run':name,'Renderer':session['Edition'],'ExecutableSHA256':session['SHA256'],'ProcessId':session['ProcessId'],
  'StartedTicks':session['StartedTicks'],'Completion':complete,'ConfigUnchanged':True,'HistoryUnchanged':True,
  'InputLogSHA256':sha(directory/'input-events.log'),'PhysicalInput':False})
write(target/'sessions.json',sessions)
for filename in ('capture-controlled-transition.ps1','join-controlled.py','check-cleanup-final.ps1','archive-autonomous.py'):
 copy(local/filename,target/'tools'/filename)
write(out/'final-verification.json',{
 'RecordedAtUtc':datetime.now(timezone.utc).isoformat(),'Stage':'RS-13','Status':'已验证（本机可测范围）',
 'TestedScope':{'OS':'Existing recorded Windows 11 x64 host','PhysicalIMEAndScaleRows':8,'PhysicalScalePercent':[100,125,150,175],
  'CrossScreenInputRows':2,'CurrentBuildInputsMatchedPerRenderer':206,'CurrentComponentStaticImages':104,'CurrentComponentAnimationClips':26,
  'CurrentComponentAnimationFrames':936,'CoreRectangleChecks':132,'HeadingOriginChecks':12,'PriorNativeFamiliesPerEdition':14,
  'PriorNativeClips':48,'PriorNativeClipsRetainOriginalExecutableIdentity':True,'CompatibleCurrentNativeSettingsIntervals':7,
  'CurrentFemtoVGPulseLiveFrames':153,'CurrentFemtoVGPulsePeakSeparationMilliseconds':read(out/'native/pulse/timeseries.json')['PeakIntervalsMilliseconds'][0]},
 'AppearanceTradeoffsAcceptedByOperator':True,'NativeWaveformThresholdsChanged':False,'OriginalMatricesOverwritten':False,
 'NativeTransitionOutcomeCounts':dict(counts),'AdditionalNativeFrameHashReferencesChecked':frame_checks,
 'TotalNormalProductExitsThisPass':6,'TotalCompletedRecorderExitsThisPass':11,'RecorderFailuresPreserved':2,
 'FormalFilesUnchanged':5,'DisplayRestoredTo100Percent':True,'CleanupPassed':True,
 'RS13VerifiedInTestedScope':True,'AllScalesOrEnvironmentsVerified':False,
 'ScopeLimits':['200% is not selectable on the reported display; untested and not passed.',
  'One earlier Software exit optical fit exceeded alpha RMSE 0.05; later fixed phase repeats passed on the same exe. Original mismatch retained.',
  'Insufficient or failed attempts remain recorded. Seven compatible intervals do not establish every-frame perfection or a uniquely identifiable easing curve.',
  'Windows Graphics Capture source samples do not measure final desktop/photon timestamps or individual GPU execution.',
  'Windows 10, other GPU drivers and third-party input methods are outside this host acceptance.'],
 'DefaultRendererChosen':False,'G4Passed':False,'ReleasePublished':False,
 'AcceptanceBasis':'Plan section 6.2 requires layout/image comparison, recorded native motion and actual DPI/IME review with explicit differences. Current measured Windows 11 scope now has each category, accepted appearance tradeoffs and sampled native enter/exit consistency for both renderers. Exact photon timing and untestable environments are limits, not fabricated passes.'})
print(f'Appended {len(all_results)} native contract results ({dict(counts)}), {frame_checks} additional frame hashes, 3 autonomous sessions; RS-13 verified only in measured host scope.')

#Requires -Version 7.4
[CmdletBinding()]
param([Parameter(Mandatory)][string]$OutputDirectory)
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'lib/rust-environment.ps1')
$environment=Initialize-VeloRustEnvironment
$OutputDirectory=$ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($OutputDirectory)
if(Test-Path -LiteralPath $OutputDirectory){throw 'Use a fresh recorder build directory.'}
New-Item -ItemType Directory -Path $OutputDirectory|Out-Null
$basePath=Join-Path $PSScriptRoot 'lib/rs12-frame-observer.cpp'
$source=Get-Content -Raw -LiteralPath $basePath
# Preserve the original target, desktop, occlusion and geometry guards. Named
# clips arm storage only; the observer never activates or sends input to an app.
$source=$source.Replace('static unsigned armImages = 0;',@'
static unsigned armImages = 0;
static std::string activeClip;
static uint64_t clipBytes = 0;
static unsigned clipFrames = 0;
static unsigned clipCount = 0;
static long long sessionDeadline = 0;
static unsigned captureRecoveries = 0;
'@)
$source=$source.Replace('DXGI_OUTDUPL_FRAME_INFO info{};',@'
if (!calibration) {
    std::ifstream command(directory / "clip.txt");
    std::string name; std::getline(command, name);
    if (!name.empty() && name != activeClip) {
        if (name == "stop") { stopped = true; break; }
        if (name.size() > 48 || !std::all_of(name.begin(), name.end(), [](char c) { return (c >= 'a' && c <= 'z') || (c >= '0' && c <= '9') || c == '-'; }) || ++clipCount > 40)
            throw std::runtime_error("invalid_clip_name_or_limit");
        if (!fs::create_directory(directory / name)) throw std::runtime_error("clip_directory_already_exists");
        activeClip = name; clipBytes = 0; clipFrames = 0; nextImageQpc = 0;
        trace->row("{\"event\":\"clip_start\",\"clip\":\"" + name + "\",\"qpc\":" + std::to_string(ticks()) + "}");
    }
}
DXGI_OUTDUPL_FRAME_INFO info{};
'@)
$source=[regex]::Replace($source,'bool imageDue = changed && \(calibration[\s\S]*?armImages < 6\)\);', 'bool imageDue = changed && (calibration || (!activeClip.empty() && info.LastPresentTime.QuadPart >= nextImageQpc));')
$source=$source.Replace('savedFrames < static_cast<unsigned>(saveLimit) && savedBytes + pixels.size() <= 128ULL * 1024 * 1024','clipFrames < static_cast<unsigned>(saveLimit) && clipBytes + pixels.size() <= 128ULL * 1024 * 1024')
$source=$source.Replace('bitmap(directory / ("frame-" + std::to_string(frameCount) + ".bmp"), pixels, width, height);','bitmap((calibration ? directory : directory / activeClip) / ("frame-" + std::to_string(frameCount) + ".bmp"), pixels, width, height);')
$source=$source.Replace('savedFrames++; savedBytes += pixels.size();','savedFrames++; savedBytes += pixels.size(); clipFrames++; clipBytes += pixels.size();')
$source=$source.Replace('frequency() / 10','frequency() / 60')
$source=$source.Replace('savedFrames < static_cast<unsigned>(saveLimit) && savedBytes + pixels.size() > 128ULL * 1024 * 1024','clipFrames < static_cast<unsigned>(saveLimit) && clipBytes + pixels.size() > 128ULL * 1024 * 1024')
$source=$source.Replace('RS12_FRAME_OBSERVER_READY','RS13_FRAME_RECORDER_READY').Replace('RS12_FRAME_OBSERVER_ERROR','RS13_FRAME_RECORDER_ERROR')
$source=$source.Replace('auto deadline = ticks() + static_cast<long long>(seconds) * frequency();', 'if (sessionDeadline == 0) sessionDeadline = ticks() + static_cast<long long>(seconds) * frequency(); auto deadline = sessionDeadline;')
$source=$source.Replace('if (result == DXGI_ERROR_WAIT_TIMEOUT) continue;',@'
if (result == DXGI_ERROR_ACCESS_LOST || MonitorFromWindow(target, MONITOR_DEFAULTTONEAREST) != monitor) {
    if (++captureRecoveries > 16) throw std::runtime_error("desktop_capture_recovery_limit");
    if (SUCCEEDED(result)) duplication->ReleaseFrame();
    trace->row("{\"event\":\"desktop_capture_recovery\",\"qpc\":" + std::to_string(ticks()) + "}");
    staging.Reset(); duplication.Reset(); output.Reset(); context.Reset(); device.Reset(); adapter.Reset(); factory.Reset();
    if (ticks() < deadline) capture(seconds, saveLimit, directory);
    return;
}
if (result == DXGI_ERROR_WAIT_TIMEOUT) continue;
'@)
$sourcePath=Join-Path $OutputDirectory 'rs13-frame-recorder.cpp';$exe=Join-Path $OutputDirectory 'rs13-frame-recorder.exe'
$source|Set-Content -LiteralPath $sourcePath -Encoding utf8NoBOM
& cl.exe /nologo /std:c++20 /O2 /EHsc /W4 /WX /MD /utf-8 $sourcePath "/Fe:$exe" "/Fo:$(Join-Path $OutputDirectory 'recorder.obj')" /link d3d11.lib dxgi.lib user32.lib gdi32.lib
if($LASTEXITCODE -ne 0){throw 'Native visual recorder build failed.'}
[ordered]@{BuiltAtUtc=[datetime]::UtcNow.ToString('o');ExecutableSHA256=(Get-FileHash $exe).Hash;BaseSourceSHA256=(Get-FileHash $basePath).Hash;GeneratedSourceSHA256=(Get-FileHash $sourcePath).Hash;BuilderSHA256=(Get-FileHash $PSCommandPath).Hash;Scope='External read-only DXGI recorder. Target client inset only, no activation or input injection. Changed frames limited to 60Hz, 128 frames/128MiB per named clip; geometry/occlusion guards retained. No exact GPU or photon timestamp claim.'}|ConvertTo-Json|Set-Content (Join-Path $OutputDirectory 'identity.json') -Encoding utf8NoBOM
'Native RS-13 recorder built.'

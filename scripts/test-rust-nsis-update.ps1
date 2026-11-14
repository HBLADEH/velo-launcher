#Requires -Version 7.4
[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidateSet('Prepare','Schedule','Verify','Finalize')][string]$Action,
    [Parameter(Mandatory)][string]$OutputDirectory,
    [ValidateSet('software','femtovg')][string]$Renderer,
    [ValidateSet('success','cancel')][string]$Case,
    [string]$Makensis,
    [string]$FixtureHelper,
    [string]$UpdateHelper
)
$ErrorActionPreference = 'Stop'
$repository = Split-Path -Parent $PSScriptRoot
$targetRoot = (Resolve-Path -LiteralPath (Join-Path $repository 'rust/target')).Path
$OutputDirectory = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($OutputDirectory)
if (-not $OutputDirectory.StartsWith($targetRoot + '\', [StringComparison]::OrdinalIgnoreCase)) { throw 'Actual NSIS update test must stay inside rust/target.' }
$planPath = Join-Path $OutputDirectory 'assisted-plan.json'
function Get-TestProcess {
    @(Get-CimInstance Win32_Process -Filter "name='velo-slint.exe'" | Where-Object {
        $_.ExecutablePath -eq $plan.Target -and $_.CommandLine -match '--instance-id=rs11-installer(?:\s|$)'
    })
}
function Assert-ConfigAndBackup {
    if ((Get-FileHash -LiteralPath $plan.Config).Hash -ne $plan.ConfigSHA256 -or
        (Get-FileHash -LiteralPath $plan.Backup).Hash -ne $plan.ConfigSHA256) { throw 'Actual NSIS update changed config or its original backup.' }
}
function Get-AccessRuleSignature($Acl) {
    @($Acl.GetAccessRules($true, $true, [Security.Principal.SecurityIdentifier]) | ForEach-Object {
        '{0}|{1}|{2}|{3}|{4}|{5}' -f $_.IdentityReference.Value, $_.FileSystemRights, $_.AccessControlType,
            $_.InheritanceFlags, $_.PropagationFlags, $_.IsInherited
    } | Sort-Object) -join "`n"
}
if ($Action -eq 'Prepare') {
    if (-not $Renderer -or -not $Case -or -not $Makensis -or -not $FixtureHelper) { throw 'Preparation requires Renderer, Case, Makensis and FixtureHelper.' }
    if (Test-Path -LiteralPath $OutputDirectory) { throw 'Preserve previous UAC attempts; use a fresh test directory.' }
    if (@(Get-CimInstance Win32_Process -Filter "name='velo-slint.exe' OR name='velo-launcher.exe'").Count) { throw 'Exit Velo before preparing the actual NSIS update.' }
    $FixtureHelper = (Resolve-Path -LiteralPath $FixtureHelper).Path
    if (-not $UpdateHelper) { $UpdateHelper = Join-Path $targetRoot 'x86_64-pc-windows-msvc/debug/examples/rs14_update_plan.exe' }
    $UpdateHelper = (Resolve-Path -LiteralPath $UpdateHelper).Path
    New-Item -ItemType Directory -Path $OutputDirectory | Out-Null
    $preview = Join-Path $OutputDirectory 'preview'
    & (Join-Path $PSScriptRoot 'package-rust-preview.ps1') -Renderer $Renderer -OutputDirectory $preview
    $payload = Join-Path $preview 'content/velo-slint.exe'
    $fixture = Join-Path $OutputDirectory 'fixture'
    # The original helper prepares a real ConfigStore backup and isolated data.
    # The second helper emits core::update::script for the actual NSIS payload.
    & $FixtureHelper --prepare $fixture $payload
    if ($LASTEXITCODE) { throw 'Production update template/backup preparation failed.' }
    $installed = Join-Path $fixture '受保护 安装目录/velo-slint.exe'
    $script = Join-Path $fixture 'install-update.ps1'
    $installer = Join-Path $fixture 'package/velo-slint-x64-installer.exe'
    $build = Join-Path $OutputDirectory 'installer'
    & (Join-Path $PSScriptRoot 'package-rust-installer.ps1') -PreviewContent (Join-Path $preview 'content') -OutputDirectory $build -Makensis $Makensis -ValidationInstallDirectory (Split-Path -Parent $installed)
    Copy-Item -LiteralPath (Get-ChildItem -LiteralPath $build -Filter '*installer.exe').FullName -Destination $installer -Force
    $installerHash = (Get-FileHash -LiteralPath $installer).Hash
    & $UpdateHelper $installed $installer $script
    if ($LASTEXITCODE) { throw 'Actual NSIS production update plan emission failed.' }
    # Appending harmless overlay bytes makes the previous PE observably distinct.
    [IO.File]::WriteAllBytes($installed, ([IO.File]::ReadAllBytes($installed) + [byte[]](1,2,3,4)))
    $installDirectory = Split-Path -Parent $installed
    $originalAcl = Get-Acl -LiteralPath $installDirectory
    $originalAcl.Sddl | Set-Content -LiteralPath (Join-Path $OutputDirectory 'original-target-acl.txt') -Encoding utf8NoBOM
    $protectedAcl = [Security.AccessControl.DirectorySecurity]::new()
    $protectedAcl.SetAccessRuleProtection($true, $false)
    foreach ($entry in @(@('S-1-5-32-544','FullControl'), @('S-1-5-18','FullControl'), @('S-1-5-32-545','ReadAndExecute'))) {
        $protectedAcl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new(
            [Security.Principal.SecurityIdentifier]::new($entry[0]), [Security.AccessControl.FileSystemRights]$entry[1],
            [Security.AccessControl.InheritanceFlags]'ContainerInherit,ObjectInherit',
            [Security.AccessControl.PropagationFlags]::None, [Security.AccessControl.AccessControlType]::Allow))
    }
    [IO.FileSystemAclExtensions]::SetAccessControl([IO.DirectoryInfo]::new($installDirectory), $protectedAcl)
    $writeDenied = $false
    try {
        $probe = Join-Path $installDirectory 'write-probe.tmp'
        $stream = [IO.File]::Open($probe, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write)
        $stream.Dispose()
        Remove-Item -LiteralPath $probe
    } catch [UnauthorizedAccessException] { $writeDenied = $true }
    if (-not $writeDenied) {
        [IO.FileSystemAclExtensions]::SetAccessControl([IO.DirectoryInfo]::new($installDirectory), $originalAcl)
        throw 'The normal test token can write the protected target; elevation boundary not established.'
    }
    $config = Join-Path $fixture 'data/config.json'
    $backup = @(Get-ChildItem -LiteralPath (Join-Path $fixture 'data/backups') -File)
    if ($backup.Count -ne 1) { throw 'Exactly one original configuration backup is required.' }
    [ordered]@{
        Renderer=$Renderer; Case=$Case; Fixture=$fixture; FixtureHelper=$FixtureHelper
        FixtureHelperSHA256=(Get-FileHash -LiteralPath $FixtureHelper).Hash
        UpdateHelperSHA256=(Get-FileHash -LiteralPath $UpdateHelper).Hash
        Target=$installed; PreviousSHA256=(Get-FileHash -LiteralPath $installed).Hash; PayloadSHA256=(Get-FileHash -LiteralPath $payload).Hash
        Installer=$installer; InstallerSHA256=$installerHash; Script=$script; ScriptSHA256=(Get-FileHash -LiteralPath $script).Hash
        Config=$config; ConfigSHA256=(Get-FileHash -LiteralPath $config).Hash; Backup=$backup[0].FullName
        InstallDirectory=$installDirectory; OriginalAcl=$originalAcl.Sddl; NormalTokenWriteDenied=$true
        PreparedAtUtc=[datetime]::UtcNow.ToString('o')
    } | ConvertTo-Json | Set-Content -LiteralPath $planPath -Encoding utf8NoBOM
    'Prepared actual pinned NSIS with the production update script and original data backup. No UAC or update started.'
    return
}
$plan = Get-Content -Raw -LiteralPath $planPath | ConvertFrom-Json
foreach ($path in $plan.Target,$plan.Installer,$plan.Script,$plan.Config,$plan.Backup) {
    if (-not ([IO.Path]::GetFullPath($path)).StartsWith($OutputDirectory + '\', [StringComparison]::OrdinalIgnoreCase)) { throw 'Assisted update plan leaves its test directory.' }
}
if ($Action -eq 'Schedule') {
    if (Test-Path -LiteralPath (Join-Path $OutputDirectory 'scheduled.json')) { throw 'Do not replay an assisted attempt.' }
    $policy = Get-ItemProperty -LiteralPath 'HKLM:/SOFTWARE/Microsoft/Windows/CurrentVersion/Policies/System'
    if ($policy.EnableLUA -ne 1 -or $policy.ConsentPromptBehaviorAdmin -eq 0) { throw 'A real consent prompt must be enabled for assisted UAC validation.' }
    if ((Get-FileHash -LiteralPath $plan.Target).Hash -ne $plan.PreviousSHA256 -or
        (Get-FileHash -LiteralPath $plan.Installer).Hash -ne $plan.InstallerSHA256 -or
        (Get-FileHash -LiteralPath $plan.Script).Hash -ne $plan.ScriptSHA256 -or
        (Get-FileHash -LiteralPath $plan.FixtureHelper).Hash -ne $plan.FixtureHelperSHA256) { throw 'Prepared actual update inputs changed.' }
    if (@(Get-TestProcess).Count) { throw 'An old isolated update instance is running.' }
    Assert-ConfigAndBackup
    & $plan.FixtureHelper --schedule $plan.Fixture
    if ($LASTEXITCODE) { throw 'Actual production updater schedule failed.' }
    [ordered]@{ScheduledAtUtc=[datetime]::UtcNow.ToString('o');ExpectedOperatorAction=$plan.Case;ConsentPromptBehaviorAdmin=$policy.ConsentPromptBehaviorAdmin} |
        ConvertTo-Json | Set-Content -LiteralPath (Join-Path $OutputDirectory 'scheduled.json') -Encoding utf8NoBOM
    'Production updater scheduled. The operator must choose the planned Yes or No in the actual UAC prompt.'
    return
}
$processes = @(Get-TestProcess)
if ($Action -eq 'Finalize') {
    if ($processes.Count) { throw 'Exit the isolated restarted program from its tray before finalizing.' }
    $resultPath = Join-Path $OutputDirectory 'results.json'
    $result = Get-Content -Raw -LiteralPath $resultPath | ConvertFrom-Json
    Assert-ConfigAndBackup
    $restore = [Security.AccessControl.DirectorySecurity]::new()
    $restore.SetSecurityDescriptorSddlForm($plan.OriginalAcl, [Security.AccessControl.AccessControlSections]::Access)
    [IO.FileSystemAclExtensions]::SetAccessControl([IO.DirectoryInfo]::new($plan.InstallDirectory), $restore)
    $expectedAcl = [Security.AccessControl.DirectorySecurity]::new()
    $expectedAcl.SetSecurityDescriptorSddlForm($plan.OriginalAcl)
    $actualAcl = Get-Acl -LiteralPath $plan.InstallDirectory
    # Windows can rewrite SDDL control flags while preserving each ACE. Compare
    # the effective rule set, inheritance protection, owner and group instead.
    if ((Get-AccessRuleSignature $actualAcl) -ne (Get-AccessRuleSignature $expectedAcl) -or
        $actualAcl.AreAccessRulesProtected -ne $expectedAcl.AreAccessRulesProtected -or
        $actualAcl.Owner -ne $expectedAcl.Owner -or $actualAcl.Group -ne $expectedAcl.Group) {
        throw 'Original update-target access rules were not restored.'
    }
    $result | Add-Member -NotePropertyName FinalizedAtUtc -NotePropertyValue ([datetime]::UtcNow.ToString('o')) -Force
    $result | Add-Member -NotePropertyName RestartedProcessExited -NotePropertyValue $true -Force
    $result | Add-Member -NotePropertyName TargetPermissionsRestored -NotePropertyValue $true -Force
    $result | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $resultPath -Encoding utf8NoBOM
    'Actual NSIS assisted update finalized; the restarted task process exited and config/backup remain unchanged.'
    return
}
if ($processes.Count -ne 1 -or $processes[0].CommandLine -notmatch '--background' -or
    -not $processes[0].CommandLine.Contains((Join-Path $plan.Fixture 'data'))) { throw 'Exactly one restarted process with original isolated/background args is required.' }
$actualHash = (Get-FileHash -LiteralPath $plan.Target).Hash
$errorLog = $plan.Script + '.log'
if ($plan.Case -eq 'success') {
    if ($actualHash -ne $plan.PayloadSHA256 -or (Test-Path -LiteralPath $plan.Installer) -or
        (Test-Path -LiteralPath $plan.Script) -or (Test-Path -LiteralPath $errorLog)) { throw 'Actual NSIS replacement or successful cleanup failed.' }
} else {
    if ($actualHash -ne $plan.PreviousSHA256 -or -not (Test-Path -LiteralPath $plan.Installer) -or
        -not (Test-Path -LiteralPath $plan.Script) -or -not (Test-Path -LiteralPath $errorLog)) { throw 'Cancelled NSIS update did not preserve old program/package/script and report failure.' }
    if ((Get-Content -Raw -LiteralPath $errorLog) -notmatch '(?i)cancel|取消|0x800704c7|1223') { throw 'The failed update did not report the expected UAC cancellation.' }
}
Assert-ConfigAndBackup
[ordered]@{
    RecordedAtUtc=[datetime]::UtcNow.ToString('o'); Renderer=$plan.Renderer; Case=$plan.Case; Passed=$true
    PreviousSHA256=$plan.PreviousSHA256; PayloadSHA256=$plan.PayloadSHA256; ActualSHA256=$actualHash
    InstallerSHA256=$plan.InstallerSHA256; ConfigAndOriginalBackupUnchanged=$true
    RestartedProcessId=$processes[0].ProcessId; RestartIsolatedAndBackground=$true; RestartedProcessExited=$false
    ActualNSISInstaller=$true; ProductionUpdateScript=$true; OperatorUACReportRequired=$true
    NormalTokenWriteDenied=$plan.NormalTokenWriteDenied
    Scope='Actual NSIS elevated by the production updater in an isolated target with an explicit original backup. No formal installation or data used.'
} | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $OutputDirectory 'results.json') -Encoding utf8NoBOM
'Actual NSIS update outcome and restarted process verified; normal tray exit is still required.'

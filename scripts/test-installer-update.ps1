param(
    [Parameter(Mandatory)][string]$Executable,
    [Parameter(Mandatory)][string]$PreviousExecutable,
    [Parameter(Mandatory)][string]$FixtureHelper,
    [Parameter(Mandatory)][string]$OutputDirectory
)
$ErrorActionPreference = 'Stop'
$Executable = (Resolve-Path -LiteralPath $Executable).Path
$PreviousExecutable = (Resolve-Path -LiteralPath $PreviousExecutable).Path
$FixtureHelper = (Resolve-Path -LiteralPath $FixtureHelper).Path
$repository = Split-Path -Parent $PSScriptRoot
$allowedRoot = (Resolve-Path -LiteralPath (Join-Path $repository 'target')).Path
$OutputDirectory = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($OutputDirectory)
if (-not $OutputDirectory.StartsWith($allowedRoot + '\', [StringComparison]::OrdinalIgnoreCase)) {
    throw 'Installer verification must stay inside target.'
}
if (Test-Path -LiteralPath $OutputDirectory) { throw 'Installer verification needs a fresh directory.' }
$parent = (Resolve-Path -LiteralPath (Split-Path -Parent $OutputDirectory)).Path
if ($parent -ne $allowedRoot -and -not $parent.StartsWith($allowedRoot + '\', [StringComparison]::OrdinalIgnoreCase)) {
    throw 'Resolved output parent leaves target.'
}
$policy = Get-ItemProperty -LiteralPath 'HKLM:/SOFTWARE/Microsoft/Windows/CurrentVersion/Policies/System'
# This optional native check must never open an unattended consent dialog. It
# reads the existing policy and does not change system or security settings.
if ($policy.EnableLUA -ne 1 -or $policy.ConsentPromptBehaviorAdmin -ne 0) {
    throw 'This unattended installer check needs the existing no-consent administrator policy; use assisted verification elsewhere.'
}
$currentIdentity = [Security.Principal.WindowsIdentity]::GetCurrent()
$principal = [Security.Principal.WindowsPrincipal]::new($currentIdentity)
# The filtered token can omit Administrators from Identity.Groups. Resolve
# account/group membership independently from whether its admin SID is enabled.
$administratorGroup = Get-CimInstance Win32_Group -Filter "SID='S-1-5-32-544'"
$memberSids = @(Get-CimAssociatedInstance -InputObject $administratorGroup -Association Win32_GroupUser | Select-Object -ExpandProperty SID)
$identitySids = @($currentIdentity.User.Value) + @($currentIdentity.Groups | ForEach-Object Value)
if (-not @($identitySids | Where-Object { $_ -in $memberSids }).Count -or $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw 'Run this check with a non-elevated administrator token; credential prompts and an already elevated writer do not establish this test.'
}
if (Get-CimInstance Win32_Process -Filter "name='velo-slint.exe'" | Where-Object CommandLine -Match '--instance-id=rs11-installer(?:\s|$)') {
    throw 'The isolated rs11-installer instance is already running.'
}
$payloadHash = (Get-FileHash -LiteralPath $Executable).Hash
$previousHash = (Get-FileHash -LiteralPath $PreviousExecutable).Hash
if ($payloadHash -eq $previousHash) { throw 'Use distinct old and new programs to prove replacement and failure preservation.' }
New-Item -ItemType Directory -Path $OutputDirectory | Out-Null

# Remove only this fixture's notification icon before terminating its background
# test process. This is resource cleanup, not evidence of a user tray interaction.
if (-not ('VeloInstallerCleanup' -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class VeloInstallerCleanup {
    [StructLayout(LayoutKind.Sequential, CharSet=CharSet.Unicode)]
    struct NotifyIconData {
        public uint cbSize; public IntPtr hwnd; public uint id, flags, message; public IntPtr icon;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst=128)] public string tip;
        public uint state, stateMask;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst=256)] public string info;
        public uint version;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst=64)] public string title;
        public uint infoFlags; public Guid guid; public IntPtr balloonIcon;
    }
    [DllImport("user32.dll", CharSet=CharSet.Unicode)]
    static extern IntPtr FindWindow(string cls, string title);
    [DllImport("user32.dll")]
    static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);
    [DllImport("shell32.dll", CharSet=CharSet.Unicode)]
    static extern bool Shell_NotifyIcon(uint message, ref NotifyIconData data);
    public static bool RemoveTray(uint expectedPid) {
        IntPtr hwnd = FindWindow("VeloSlintDesktopService", "Velo Slint instance rs11-installer");
        if (hwnd == IntPtr.Zero) return false;
        uint pid; GetWindowThreadProcessId(hwnd, out pid);
        if (pid != expectedPid) return false;
        var data = new NotifyIconData { cbSize=(uint)Marshal.SizeOf(typeof(NotifyIconData)), hwnd=hwnd, id=1 };
        return Shell_NotifyIcon(2, ref data);
    }
}
'@
}

function Invoke-Fixture([string[]]$Arguments, [string]$LogPrefix) {
    $process = Start-Process -FilePath $FixtureHelper -ArgumentList $Arguments -WindowStyle Hidden -PassThru `
        -RedirectStandardError ($LogPrefix + '.stderr.log') -RedirectStandardOutput ($LogPrefix + '.stdout.log')
    try {
        # Start-Process -Wait includes detached descendants. The helper only
        # schedules a script; wait for its own PID, then observe the restarted app.
        if (-not $process.WaitForExit(15000)) { throw 'Installer fixture helper timed out.' }
        $process.Refresh()
        if ($process.ExitCode -ne 0) { throw "Installer fixture helper failed: $($process.ExitCode)" }
    } finally {
        if (-not $process.HasExited) { Stop-Process -Id $process.Id -ErrorAction SilentlyContinue }
    }
}

function Get-FixtureProcess([string]$Target) {
    @(Get-CimInstance Win32_Process -Filter "name='velo-slint.exe'" | Where-Object {
        $_.ExecutablePath -eq $Target -and $_.CommandLine -match '--instance-id=rs11-installer(?:\s|$)'
    })
}

function Access-Rules($Acl) {
    @($Acl.GetAccessRules($true, $true, [Security.Principal.SecurityIdentifier]) | ForEach-Object {
        '{0}|{1}|{2}|{3}|{4}|{5}' -f $_.IdentityReference.Value, $_.FileSystemRights, $_.AccessControlType,
            $_.InheritanceFlags, $_.PropagationFlags, $_.IsInherited
    } | Sort-Object) -join "`n"
}

$results = @()
foreach ($mode in @('success', 'failure')) {
    $root = Join-Path $OutputDirectory $mode
    Invoke-Fixture -Arguments @('--prepare', ('"' + $root + '"'), ('"' + $Executable + '"')) -LogPrefix (Join-Path $OutputDirectory ($mode + '-prepare'))
    $targetDirectory = (Resolve-Path -LiteralPath (Join-Path $root '受保护 安装目录')).Path
    if (-not $targetDirectory.StartsWith($OutputDirectory + '\', [StringComparison]::OrdinalIgnoreCase)) {
        throw 'Resolved installer target leaves its fixture.'
    }
    $target = Join-Path $targetDirectory 'velo-slint.exe'
    Copy-Item -LiteralPath $PreviousExecutable -Destination $target
    $planPath = Join-Path $root 'package/plan.json'
    $plan = Get-Content -LiteralPath $planPath -Raw | ConvertFrom-Json -AsHashtable
    $plan['fail'] = $mode -eq 'failure'
    $plan | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $planPath -Encoding utf8NoBOM
    $configPath = Join-Path $root 'data/config.json'
    $configHash = (Get-FileHash -LiteralPath $configPath).Hash
    $backup = @(Get-ChildItem -LiteralPath (Join-Path $root 'data/backups') -File)
    if ($backup.Count -ne 1 -or (Get-FileHash -LiteralPath $backup[0].FullName).Hash -ne $configHash) {
        throw 'Installation did not preserve the saved configuration bytes.'
    }
    $saved = Get-Acl -LiteralPath $targetDirectory
    $saved.Sddl | Set-Content -LiteralPath (Join-Path $root 'original-acl.txt')
    $result = $null
    try {
        $protected = [Security.AccessControl.DirectorySecurity]::new()
        $protected.SetAccessRuleProtection($true, $false)
        foreach ($entry in @(@('S-1-5-32-544','FullControl'), @('S-1-5-18','FullControl'), @('S-1-5-32-545','ReadAndExecute'))) {
            $protected.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new(
                [Security.Principal.SecurityIdentifier]::new($entry[0]), [Security.AccessControl.FileSystemRights]$entry[1],
                [Security.AccessControl.InheritanceFlags]'ContainerInherit,ObjectInherit',
                [Security.AccessControl.PropagationFlags]::None, [Security.AccessControl.AccessControlType]::Allow))
        }
        [IO.FileSystemAclExtensions]::SetAccessControl([IO.DirectoryInfo]::new($targetDirectory), $protected)
        $writeDenied = $false
        $probe = Join-Path $targetDirectory 'write-probe.tmp'
        try {
            $stream = [IO.File]::Open($probe, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write)
            $stream.Dispose()
            Remove-Item -LiteralPath $probe
        } catch [UnauthorizedAccessException] { $writeDenied = $true }
        if (-not $writeDenied) { throw 'The normal token can still write the isolated install directory.' }
        Invoke-Fixture -Arguments @('--schedule', ('"' + $root + '"')) -LogPrefix (Join-Path $OutputDirectory ($mode + '-schedule'))
        $markerPath = Join-Path $root 'installer-result.json'
        $scriptPath = Join-Path $root 'install-update.ps1'
        $installerPath = Join-Path $root 'package/velo-slint-x64-installer.exe'
        $deadline = [DateTime]::UtcNow.AddSeconds(45)
        $restarted = @()
        do {
            $restarted = @(Get-FixtureProcess $target)
            $scriptFinished = if ($mode -eq 'success') { -not (Test-Path -LiteralPath $scriptPath) } else { Test-Path -LiteralPath ($scriptPath + '.log') }
            if ((Test-Path -LiteralPath $markerPath) -and $restarted.Count -eq 1 -and $scriptFinished) { break }
            Start-Sleep -Milliseconds 200
        } while ([DateTime]::UtcNow -lt $deadline)
        if ($restarted.Count -ne 1 -or -not $scriptFinished -or -not (Test-Path -LiteralPath $markerPath)) {
            throw 'Installer completion or isolated restart was not observed within 45 seconds.'
        }
        $marker = Get-Content -LiteralPath $markerPath -Raw | ConvertFrom-Json
        if (-not $marker.elevated) { throw 'The installer did not have an administrator token.' }
        $arguments = $restarted[0].CommandLine
        $expectedData = (Join-Path $root 'data').Replace('/', '\')
        if ($arguments -notmatch '--background' -or -not $arguments.Contains($expectedData)) {
            throw 'The restarted app lost its background or isolated data arguments.'
        }
        $targetHash = (Get-FileHash -LiteralPath $target).Hash
        if ($mode -eq 'success') {
            if ($targetHash -ne $payloadHash -or (Test-Path -LiteralPath $installerPath)) { throw 'Verified installer replacement/cleanup failed.' }
        } else {
            if (-not $marker.injected_failure -or $targetHash -ne $previousHash -or -not (Test-Path -LiteralPath $installerPath)) {
                throw 'Failed installation did not preserve and restart the original program.'
            }
            if ((Get-Content -LiteralPath ($scriptPath + '.log') -Raw) -notmatch 'Installer failed') { throw 'The failure was not reported by the update script.' }
        }
        if ((Get-FileHash -LiteralPath $configPath).Hash -ne $configHash -or
            (Get-FileHash -LiteralPath $backup[0].FullName).Hash -ne $configHash) { throw 'Installation changed the saved configuration or its backup.' }
        $result = [ordered]@{
            mode=$mode; passed=$true; normal_token_write_denied=$true; installer_elevated=$true
            target_before_sha256=$previousHash; payload_sha256=$payloadHash; target_after_sha256=$targetHash
            original_config_and_backup_unchanged=$true; restart_isolated=$true; restart_background_argument=$true
            installer_preserved_on_failure=($mode -eq 'failure'); script_error_observed=($mode -eq 'failure')
            scope='local installer fixture; does not validate a published NSIS package or a consent dialog'
        }
    } finally {
        $removed = $true
        foreach ($process in @(Get-FixtureProcess $target)) {
            $removed = [VeloInstallerCleanup]::RemoveTray([uint32]$process.ProcessId) -and $removed
            Stop-Process -Id $process.ProcessId -ErrorAction SilentlyContinue
        }
        $restore = [Security.AccessControl.DirectorySecurity]::new()
        $restore.SetSecurityDescriptorSddlForm($saved.Sddl, [Security.AccessControl.AccessControlSections]::Access)
        [IO.FileSystemAclExtensions]::SetAccessControl([IO.DirectoryInfo]::new($targetDirectory), $restore)
        $actual = Get-Acl -LiteralPath $targetDirectory
        if ((Access-Rules $actual) -ne (Access-Rules $saved) -or $actual.AreAccessRulesProtected -ne $saved.AreAccessRulesProtected) {
            throw 'Installer fixture access rules were not restored.'
        }
        if (@(Get-FixtureProcess $target).Count) { throw 'A test process survived cleanup.' }
        if ($null -ne $result) {
            $result.permissions_restored = $true
            $result.test_processes_remaining = 0
            $result.tray_cleanup = $removed
            $result.process_cleanup = 'terminated isolated test copy after validation'
        }
    }
    $results += $result
    Write-Output "Installer update $mode passed; original access rules restored and test process removed."
}
[ordered]@{
    schema_version=1; passed=$true; executable_sha256=$payloadHash; previous_executable_sha256=$previousHash
    fixture_helper_sha256=(Get-FileHash -LiteralPath $FixtureHelper).Hash
    consent_prompt_behavior_admin=[int]$policy.ConsentPromptBehaviorAdmin
    normal_administrator_token_verified=$true
    cases=$results
} | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $OutputDirectory 'results.json') -Encoding utf8NoBOM

#Requires -Version 5.1
# Embedded in the elevated NSIS installer/uninstaller. Never used by a resident process.
[CmdletBinding()]
param(
    [ValidateSet('Install','Rollback','Uninstall')][string]$Action = 'Install',
    [Parameter(Mandatory)][string]$InstallDirectory,
    [string]$Payload
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2
$registration = 'SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\HBLADEHVelo'
$root = [IO.Path]::GetFullPath($InstallDirectory).TrimEnd('\')
$archive = Join-Path $root '.velo-rollback'
$journal = Join-Path $archive 'pending.json'
$machine = $null
$transaction = $null
$canRecover = $false
$installMutex = $null
$installLockHeld = $false
$code = 30
function Assert-Ordinary([string]$Path) {
    $next = [IO.Path]::GetFullPath($Path)
    while ($next) {
        if (Test-Path -LiteralPath $next) {
            if ((Get-Item -LiteralPath $next -Force).Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Reparse points are not supported by the machine installer.' }
        }
        $parent = Split-Path -Parent $next
        if ($parent -eq $next) { break }
        $next = $parent
    }
}
function Write-DurableJson([string]$Path, $Value) {
    $temporary = $Path + '.' + [guid]::NewGuid().ToString('N') + '.tmp'
    $bytes = [Text.UTF8Encoding]::new($false).GetBytes(($Value | ConvertTo-Json -Depth 12))
    $stream = [IO.File]::Open($temporary, 'CreateNew', 'Write', 'None')
    try { $stream.Write($bytes,0,$bytes.Length); $stream.Flush($true) } finally { $stream.Dispose() }
    # Windows PowerShell 5.1 binds $null string arguments as empty strings.
    # NullString is required for Replace's optional backup file name.
    if (Test-Path -LiteralPath $Path) { [IO.File]::Replace($temporary,$Path,[System.Management.Automation.Language.NullString]::Value) } else { [IO.File]::Move($temporary,$Path) }
}
function Copy-Durable([string]$Source,[string]$Destination) {
    Assert-Ordinary $Source
    Assert-Ordinary $Destination
    [IO.Directory]::CreateDirectory((Split-Path -Parent $Destination)) | Out-Null
    $inputFile = [IO.File]::Open($Source,'Open','Read','Read')
    try {
        $outputFile = [IO.File]::Open($Destination,'Create','Write','None')
        try { $inputFile.CopyTo($outputFile); $outputFile.Flush($true) } finally { $outputFile.Dispose() }
    } finally { $inputFile.Dispose() }
    if ((Get-FileHash -LiteralPath $Source).Hash -ne (Get-FileHash -LiteralPath $Destination).Hash) { throw 'A durable program copy failed verification.' }
}
function Relative-Files([string]$Directory) {
    if (Test-Path -LiteralPath $Directory) {
        foreach ($item in Get-ChildItem -LiteralPath $Directory -Force) {
            Assert-Ordinary $item.FullName
            if ($item.PSIsContainer) { Relative-Files $item.FullName } else { $item.FullName }
        }
    }
}
function Remove-Owned([string]$Path) {
    # Only three fixed program resources or a checked transaction directory.
    Assert-Ordinary $Path
    if (Test-Path -LiteralPath $Path) {
        foreach ($file in @(Relative-Files $Path)) { [IO.File]::Delete($file) }
        if ([IO.Directory]::Exists($Path)) {
            foreach ($directory in @(Get-ChildItem -LiteralPath $Path -Directory -Recurse -Force | Sort-Object { $_.FullName.Length } -Descending)) { [IO.Directory]::Delete($directory.FullName) }
            [IO.Directory]::Delete($Path)
        } else { [IO.File]::Delete($Path) }
    }
}
function Registry-State {
    $key = $machine.OpenSubKey($registration)
    if (-not $key) { return [pscustomobject]@{ Exists=$false; Values=@() } }
    try {
        if ($key.GetSubKeyNames().Count) { throw 'Unrecognized uninstall registry subkeys must be preserved.' }
        $values = @(foreach ($name in $key.GetValueNames()) {
            $kind = $key.GetValueKind($name).ToString()
            if ($kind -notin 'String','ExpandString','DWord','QWord','MultiString','Binary','None') { throw 'Unsupported uninstall registry value type.' }
            [ordered]@{ Name=$name; Kind=$kind; Value=$key.GetValue($name,$null,[Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames) }
        })
        return [pscustomobject]@{ Exists=$true; Values=$values }
    } finally { $key.Dispose() }
}
function Restore-Registry($State) {
    $machine.DeleteSubKeyTree($registration,$false)
    if ($State.Exists) {
        $key = $machine.CreateSubKey($registration)
        try {
            foreach ($value in $State.Values) {
                $data = $value.Value
                switch ($value.Kind) {
                    DWord { $data = [int]$data }
                    QWord { $data = [long]$data }
                    MultiString { $data = [string[]]$data }
                    Binary { $data = [byte[]]$data }
                    None { $data = [byte[]]$data }
                    default { $data = [string]$data }
                }
                $key.SetValue($value.Name,$data,[Microsoft.Win32.RegistryValueKind]::$($value.Kind))
            }
            $key.Flush()
        } finally { $key.Dispose() }
    }
}
function Shortcut-Paths {
    @((Join-Path ([Environment]::GetFolderPath('CommonPrograms')) 'Velo.lnk'),
      (Join-Path ([Environment]::GetFolderPath('CommonDesktopDirectory')) 'Velo.lnk'))
}
function Assert-Stopped {
    $exe = Join-Path $root 'velo-launcher.exe'
    if (Test-Path -LiteralPath $exe) {
        $handle = [IO.File]::Open($exe,'Open','ReadWrite','None')
        $handle.Dispose()
    }
    foreach ($name in 'Local\VeloSlint.Instance.product','wails-app-6ab2e7d8-15d5-456b-a8eb-f924ac650291sim') {
        try { $mutex = [Threading.Mutex]::OpenExisting($name) } catch [Threading.WaitHandleCannotBeOpenedException] { continue }
        $mutex.Dispose()
        throw 'Exit Velo from its tray menu before installing, rolling back or uninstalling.'
    }
}
function Snapshot([string]$Directory) {
    [IO.Directory]::CreateDirectory($Directory) | Out-Null
    $entries = @()
    foreach ($name in 'velo-launcher.exe','uninstall.exe','.velo-rust') {
        $path = Join-Path $root $name
        $files = if ([IO.File]::Exists($path)) { @($path) } else { @(Relative-Files $path) }
        foreach ($file in $files) {
            $relative = $file.Substring($root.Length+1)
            Copy-Durable $file (Join-Path $Directory ('files\'+$relative))
            $entries += [ordered]@{ Path=$relative; SHA256=(Get-FileHash -LiteralPath $file).Hash }
        }
    }
    $shortcuts = @(); $i = 0
    foreach ($path in Shortcut-Paths) {
        $exists = Test-Path -LiteralPath $path
        if ($exists) { Copy-Durable $path (Join-Path $Directory ('shortcut-'+$i+'.lnk')) }
        $shortcuts += [ordered]@{ Index=$i; Exists=[bool]$exists; SHA256=$(if ($exists) { (Get-FileHash -LiteralPath $path).Hash } else { $null }) }; $i++
    }
    Write-DurableJson (Join-Path $Directory 'snapshot.json') ([ordered]@{ Schema=1; Files=$entries; Registry=(Registry-State); Shortcuts=$shortcuts })
    Verify-Snapshot $Directory | Out-Null
}
function Verify-Snapshot([string]$Directory) {
    Assert-Ordinary $Directory
    $state = Get-Content -LiteralPath (Join-Path $Directory 'snapshot.json') -Raw | ConvertFrom-Json
    if ($state.Schema -ne 1) { throw 'Unsupported rollback snapshot.' }
    $seen = @{}
    foreach ($entry in $state.Files) {
        $name = [string]$entry.Path
        if ($name -notmatch '^(velo-launcher\.exe|uninstall\.exe|\.velo-rust\\[^:]+)$' -or
            $name -match '(^|\\)\.\.(\\|$)' -or $seen.ContainsKey($name)) { throw 'Unsafe rollback file list.' }
        $seen[$name] = $true
        $path = Join-Path $Directory ('files\'+$name)
        Assert-Ordinary $path
        if ((Get-FileHash -LiteralPath $path).Hash -ne $entry.SHA256) { throw 'Rollback program checksum mismatch.' }
    }
    if (@($state.Shortcuts).Count -ne 2) { throw 'Rollback shortcut snapshot incomplete.' }
    foreach ($item in $state.Shortcuts) {
        if ($item.Index -notin 0,1) { throw 'Unsafe shortcut snapshot.' }
        if ($item.Exists -and (Get-FileHash -LiteralPath (Join-Path $Directory ('shortcut-'+$item.Index+'.lnk'))).Hash -ne $item.SHA256) { throw 'Rollback shortcut checksum mismatch.' }
    }
    return $state
}
function Restore-Snapshot([string]$Directory) {
    $state = Verify-Snapshot $Directory
    Assert-Stopped
    foreach ($name in 'velo-launcher.exe','uninstall.exe','.velo-rust') { Remove-Owned (Join-Path $root $name) }
    foreach ($entry in $state.Files) { Copy-Durable (Join-Path $Directory ('files\'+$entry.Path)) (Join-Path $root $entry.Path) }
    $paths = @(Shortcut-Paths)
    foreach ($item in $state.Shortcuts) {
        if ($item.Exists) { Copy-Durable (Join-Path $Directory ('shortcut-'+$item.Index+'.lnk')) $paths[$item.Index] }
        elseif (Test-Path -LiteralPath $paths[$item.Index]) { [IO.File]::Delete($paths[$item.Index]) }
    }
    Restore-Registry $state.Registry
}
function Validate-Payload {
    $list = @{}
    foreach ($line in Get-Content -LiteralPath (Join-Path $Payload 'SHA256SUMS.txt')) {
        if ($line -notmatch '^([a-fA-F0-9]{64})  (.+)$') { throw 'Invalid product payload checksum list.' }
        $hash = $Matches[1]; $name = $Matches[2].Replace('/','\')
        $file = [IO.Path]::GetFullPath((Join-Path $Payload $name))
        if (-not $file.StartsWith($Payload+'\',[StringComparison]::OrdinalIgnoreCase) -or $list.ContainsKey($name)) { throw 'Unsafe product payload path.' }
        Assert-Ordinary $file
        if ((Get-FileHash -LiteralPath $file).Hash -ne $hash) { throw 'Product payload checksum mismatch.' }
        $list[$name] = $true
    }
    foreach ($file in Relative-Files $Payload) {
        $name = $file.Substring($Payload.Length+1)
        if ($name -notin 'SHA256SUMS.txt','uninstall.exe' -and -not $list.ContainsKey($name)) { throw 'Unlisted product payload file.' }
    }
    $package = Get-Content -LiteralPath (Join-Path $Payload 'package.json') -Raw | ConvertFrom-Json
    if (-not $package.ProductIdentity) { throw 'Product identity is required.' }
    if ($package.InternalValidationOnly) {
        if ($package.RedistributionCleared) { throw 'Internal product metadata is inconsistent.' }
    } elseif (-not $package.RedistributionCleared -or -not $package.G4Passed -or
        -not $package.PublicReleaseApproved -or $package.ReleaseTag -notmatch '^v?\d+\.\d+\.\d+$' -or
        ($package.ReleaseTag -replace '^v', '') -ne $package.Version) {
        throw 'Product release metadata is not approved.'
    }
    if ((Get-FileHash -LiteralPath (Join-Path $Payload 'velo-slint.exe')).Hash -ne $package.ExecutableSHA256) { throw 'Product executable identity mismatch.' }
    return $package
}
try {
    $principal = [Security.Principal.WindowsPrincipal]::new([Security.Principal.WindowsIdentity]::GetCurrent())
    if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator) -or -not [Environment]::Is64BitProcess) { throw 'The machine installer requires an elevated x64 process.' }
    # All install directories share one HKLM registration. Exclude other
    # installers before reading a journal or changing any program resources.
    $installMutex = [Threading.Mutex]::new($false,'Global\Velo.MachineInstaller')
    try { $installLockHeld = $installMutex.WaitOne(0) }
    catch [Threading.AbandonedMutexException] { $installLockHeld = $true }
    if (-not $installLockHeld) { $code = 33; throw 'Another Velo installer or uninstaller is running; wait for it to finish.' }
    $programFiles = [Environment]::GetFolderPath('ProgramFiles').TrimEnd('\')
    if (-not $root.StartsWith($programFiles+'\',[StringComparison]::OrdinalIgnoreCase) -or $root -eq $programFiles) { throw 'Machine installs must use a protected Program Files subdirectory.' }
    Assert-Ordinary $root
    $machine = [Microsoft.Win32.RegistryKey]::OpenBaseKey([Microsoft.Win32.RegistryHive]::LocalMachine,[Microsoft.Win32.RegistryView]::Registry64)
    Assert-Stopped
    [IO.Directory]::CreateDirectory($root) | Out-Null
    # Capability ACE names (APPLICATION PACKAGE AUTHORITY) have no name-to-SID lookup; read SIDs directly.
    foreach ($rule in (Get-Acl -LiteralPath $root).GetAccessRules($true,$true,[Security.Principal.SecurityIdentifier])) {
        $sid = $rule.IdentityReference.Value
        if ($rule.AccessControlType -eq 'Allow' -and $sid -in 'S-1-1-0','S-1-5-11','S-1-5-32-545' -and
            ($rule.FileSystemRights -band [Security.AccessControl.FileSystemRights]'Write,Delete,ChangePermissions,TakeOwnership')) { throw 'The install directory permits non-administrator modification.' }
    }
    [IO.Directory]::CreateDirectory($archive) | Out-Null
    Assert-Ordinary $archive
    if (Test-Path -LiteralPath $journal) {
        $pending = Get-Content -Raw -LiteralPath $journal | ConvertFrom-Json
        if ($pending.Transaction -notmatch '^[0-9a-f]{32}$') { throw 'Invalid pending install journal.' }
        Restore-Snapshot (Join-Path $archive $pending.Transaction)
        [IO.File]::Delete($journal)
    }
    $state = Registry-State
    if ($state.Exists) {
        $uninstall = @($state.Values | Where-Object Name -eq 'UninstallString')
        if ($uninstall.Count -ne 1 -or $uninstall[0].Value -notmatch '^"([^"]+\\uninstall\.exe)"$' -or
            (Split-Path -Parent $Matches[1]) -ne $root) { throw 'Preserve the registered installation; choose its existing directory.' }
    }
    if ($Action -eq 'Install') {
        $Payload = [IO.Path]::GetFullPath($Payload).TrimEnd('\')
        $package = Validate-Payload
        $hasProduct = Test-Path -LiteralPath (Join-Path $root '.velo-rust/installation.json')
        $exe = Join-Path $root 'velo-launcher.exe'
        if ((Test-Path -LiteralPath $exe) -and -not $hasProduct) {
            # Initial takeover is intentionally limited to the published baseline.
            if ((Get-FileHash -LiteralPath $exe).Hash -ne '9A1A898A1E3D70267481BCC68E3368F0B6594F68E8AE43B3F0DCDC06A9B329A6' -or -not $state.Exists) { throw 'Only the verified official Go v0.9.11 system baseline can be taken over.' }
        } elseif (-not $hasProduct -and ($state.Exists -or (Test-Path -LiteralPath (Join-Path $root 'uninstall.exe')) -or (Test-Path -LiteralPath (Join-Path $root '.velo-rust')))) { throw 'Existing program resources must be preserved; repair or select a fresh installation.' }
        $transaction = [guid]::NewGuid().ToString('N')
        $backup = Join-Path $archive $transaction
        Snapshot $backup
        if ((Test-Path -LiteralPath $exe) -and -not $hasProduct -and -not (Test-Path -LiteralPath (Join-Path $archive 'original-go'))) {
            [IO.Directory]::Move($backup,(Join-Path $archive 'original-go'))
            # The immutable Go baseline is retained separately from crash recovery.
            Snapshot $backup
        }
        Write-DurableJson $journal ([ordered]@{ Schema=1; Transaction=$transaction })
        $canRecover = $true
        foreach ($name in 'velo-launcher.exe','uninstall.exe','.velo-rust') { Remove-Owned (Join-Path $root $name) }
        Copy-Durable (Join-Path $Payload 'velo-slint.exe') $exe
        Copy-Durable (Join-Path $Payload 'uninstall.exe') (Join-Path $root 'uninstall.exe')
        $resources = Join-Path $root '.velo-rust'
        [IO.Directory]::CreateDirectory($resources) | Out-Null
        foreach ($file in Relative-Files $Payload) {
            $name = $file.Substring($Payload.Length+1)
            if ($name -notin 'velo-slint.exe','uninstall.exe') { Copy-Durable $file (Join-Path $resources $name) }
        }
        Write-DurableJson (Join-Path $resources 'installation.json') ([ordered]@{ Schema=1; MigrationID=$transaction; Renderer=$package.Renderer; ExecutableSHA256=$package.ExecutableSHA256 })
        $shell = New-Object -ComObject WScript.Shell
        try {
            foreach ($path in Shortcut-Paths) {
                $link = $shell.CreateShortcut($path)
                try { $link.TargetPath=$exe; $link.Arguments=''; $link.WorkingDirectory=$root; $link.Save() } finally { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($link) }
            }
        } finally { [void][Runtime.InteropServices.Marshal]::FinalReleaseComObject($shell) }
        $key = $machine.CreateSubKey($registration)
        try {
            $displayName = if ($package.InternalValidationOnly) { 'Velo (Rust candidate)' } else { 'Velo' }
            foreach ($item in @{ Publisher='HBLADEH'; DisplayName=$displayName; DisplayVersion=$package.Version; DisplayIcon=$exe; InstallLocation=$root; Runtime='rust'; UninstallString=('"'+(Join-Path $root 'uninstall.exe')+'"'); QuietUninstallString=('"'+(Join-Path $root 'uninstall.exe')+'" /S') }.GetEnumerator()) { $key.SetValue($item.Key,$item.Value,[Microsoft.Win32.RegistryValueKind]::String) }
            $key.SetValue('NoModify',1,[Microsoft.Win32.RegistryValueKind]::DWord)
            $key.SetValue('NoRepair',1,[Microsoft.Win32.RegistryValueKind]::DWord)
            $key.Flush()
        } finally { $key.Dispose() }
        if ((Get-FileHash -LiteralPath $exe).Hash -ne $package.ExecutableSHA256) { throw 'Installed program verification failed.' }
        [IO.File]::Delete($journal)
        $canRecover = $false
    } elseif ($Action -eq 'Rollback') {
        if (-not (Test-Path -LiteralPath (Join-Path $root '.velo-rust/installation.json'))) { throw 'Only an installed Rust product can be rolled back.' }
        $original = Join-Path $archive 'original-go'
        $null = Verify-Snapshot $original
        # Prepare recovery of the current Rust before restoring the original Go.
        $transaction = [guid]::NewGuid().ToString('N')
        Snapshot (Join-Path $archive $transaction)
        Write-DurableJson $journal ([ordered]@{ Schema=1; Transaction=$transaction })
        $canRecover = $true
        Restore-Snapshot $original
        [IO.File]::Delete($journal)
        $canRecover = $false
    } else {
        if (-not (Test-Path -LiteralPath (Join-Path $root '.velo-rust/installation.json'))) { throw 'Uninstall refused: Rust installation receipt missing.' }
        $transaction = [guid]::NewGuid().ToString('N')
        Snapshot (Join-Path $archive $transaction)
        Write-DurableJson $journal ([ordered]@{ Schema=1; Transaction=$transaction })
        $canRecover = $true
        foreach ($name in 'velo-launcher.exe','uninstall.exe','.velo-rust') { Remove-Owned (Join-Path $root $name) }
        foreach ($path in Shortcut-Paths) { if (Test-Path -LiteralPath $path) { [IO.File]::Delete($path) } }
        $machine.DeleteSubKeyTree($registration,$false)
        [IO.File]::Delete($journal)
        $canRecover = $false
        # Keep program rollback archives and every unknown/user file for recovery.
    }
    Write-DurableJson (Join-Path $archive 'last-result.json') ([ordered]@{ Schema=1; AtUtc=[datetime]::UtcNow.ToString('o'); Action=$Action; Passed=$true; UserDataModified=$false })
    $code = 0
} catch {
    $message = $_.Exception.Message
    if ($canRecover) {
        try { Restore-Snapshot (Join-Path $archive $transaction); [IO.File]::Delete($journal); $code=31 } catch { $code=32 }
    }
    # Local diagnostic only; caller exports selected fields, never user paths.
    try { if ($installLockHeld -and (Test-Path -LiteralPath $archive)) { Write-DurableJson (Join-Path $archive 'last-error.json') ([ordered]@{ AtUtc=[datetime]::UtcNow.ToString('o'); Action=$Action; ExitCode=$code; Message=$message; RecoveryRequired=($code -eq 32) }) } } catch { }
    Write-Error -Message $message -ErrorAction Continue
} finally {
    if ($machine) { $machine.Dispose() }
    if ($installMutex) {
        if ($installLockHeld) { $installMutex.ReleaseMutex() }
        $installMutex.Dispose()
    }
}
exit $code

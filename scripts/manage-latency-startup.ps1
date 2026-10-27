#Requires -Version 7.4
[CmdletBinding()]
param(
    [ValidateSet('Backup', 'Status', 'Disable', 'Restore')][string]$Action = 'Status',
    [Parameter(Mandatory)][string]$BackupPath
)
$ErrorActionPreference = 'Stop'
$BackupPath = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($BackupPath)
$subKey = 'Software\Microsoft\Windows\CurrentVersion\Run'
$valueName = 'Velo'
$expectedValue = '"C:\Program Files\HBLADEH\Velo\velo-launcher.exe" --background'
$owner = [Security.Principal.WindowsIdentity]::GetCurrent().User.Value
$actualSubKey = $owner + '\' + $subKey
# The system provider avoids the local HKCU overlay of packaged/x86 callers.
function Registry-Call([string]$Method, [hashtable]$Arguments) {
    $reply = Invoke-CimMethod -Namespace 'root/default' -ClassName StdRegProv -MethodName $Method -Arguments $Arguments
    if ($reply.ReturnValue -notin @(0, 2)) { throw "System registry $Method failed: $($reply.ReturnValue)." }
    $reply
}
function Read-Startup {
    # GetStringValue reports a generic 1 for an absent value on this host.
    # Enumerate the exact name first, so absence never masks a provider error.
    $values = Registry-Call 'EnumValues' @{ hDefKey = [uint32]2147483651; sSubKeyName = $actualSubKey }
    $index = $(if ($values.sNames) { [Array]::IndexOf([string[]]$values.sNames, $valueName) } else { -1 })
    if ($index -lt 0) { return [pscustomobject]@{ Present = $false; Value = $null; Kind = $null } }
    $kind = $values.Types[$index]
    $value = $null
    if ($kind -eq 1) {
        $reply = Registry-Call 'GetStringValue' @{ hDefKey = [uint32]2147483651; sSubKeyName = $actualSubKey; sValueName = $valueName }
        if ($reply.ReturnValue -ne 0) { throw 'Startup value changed during inspection.' }
        $value = $reply.sValue
    }
    [pscustomobject]@{ Present = $true; Value = $value; Kind = $kind }
}
$current = Read-Startup
$before = $current.Present
$approvalEnabled = $true
$approvalSubKey = $owner + '\Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run'
$approvalNames = Registry-Call 'EnumValues' @{ hDefKey = [uint32]2147483651; sSubKeyName = $approvalSubKey }
if ($approvalNames.sNames -contains $valueName) {
    $approval = Registry-Call 'GetBinaryValue' @{ hDefKey = [uint32]2147483651; sSubKeyName = $approvalSubKey; sValueName = $valueName }
    $approvalEnabled = $(if ($approval.uValue -and $approval.uValue[0] -in @(2, 6)) { $true } elseif ($approval.uValue -and $approval.uValue[0] -in @(3, 7)) { $false } else { $null })
}
if ($Action -eq 'Backup') {
    if (Test-Path -LiteralPath $BackupPath) { throw 'BackupPath already exists; keep the original backup.' }
    if (-not $current.Present -or $current.Kind -ne 1 -or $current.Value -cne $expectedValue) { throw 'The actual Velo startup value differs from the inspected installed instance; no backup or change made.' }
    [pscustomobject]@{
        SchemaVersion = 1; BackedUpAtUtc = [DateTime]::UtcNow.ToString('o'); OwnerSID = $owner
        SubKey = $subKey; ValueName = $valueName; ValueKind = 'String'; Value = $current.Value
    } | ConvertTo-Json | Set-Content -LiteralPath $BackupPath -Encoding UTF8
} elseif ($Action -in @('Disable', 'Restore')) {
    $backup = Get-Content -LiteralPath $BackupPath -Raw -Encoding UTF8 | ConvertFrom-Json
    if ($backup.SchemaVersion -ne 1 -or $backup.OwnerSID -ne $owner -or $backup.SubKey -cne $subKey -or
        $backup.ValueName -cne $valueName -or $backup.ValueKind -cne 'String' -or $backup.Value -cne $expectedValue) {
        throw 'Backup identity does not match this user and the inspected Velo startup value; no change made.'
    }
    if ($current.Present -and ($current.Kind -ne 1 -or $current.Value -cne $expectedValue)) {
        throw 'The startup value changed after backup; refusing to overwrite it.'
    }
    if ($Action -eq 'Disable' -and $current.Present) {
        $null = Registry-Call 'DeleteValue' @{ hDefKey = [uint32]2147483651; sSubKeyName = $actualSubKey; sValueName = $valueName }
    }
    if ($Action -eq 'Restore' -and -not $current.Present) {
        $null = Registry-Call 'SetStringValue' @{ hDefKey = [uint32]2147483651; sSubKeyName = $actualSubKey; sValueName = $valueName; sValue = $backup.Value }
    }
    $current = Read-Startup
    if (($Action -eq 'Disable' -and $current.Present) -or ($Action -eq 'Restore' -and (-not $current.Present -or $current.Kind -ne 1 -or $current.Value -cne $expectedValue))) {
        throw 'Actual startup value verification failed; retain the backup and inspect before rebooting.'
    }
    if ($Action -eq 'Restore') {
        # Undo the earlier local API overlay deletion as well as the real value.
        $local = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey($subKey, $true)
        if (-not $local) { throw 'Actual startup was restored, but its local API view is unavailable.' }
        try {
            $localValue = $local.GetValue($valueName, $null)
            if ($null -ne $localValue -and $localValue -cne $expectedValue) { throw 'Actual startup was restored; refusing to overwrite a changed local API value.' }
            if ($null -eq $localValue) { $local.SetValue($valueName, $backup.Value, [Microsoft.Win32.RegistryValueKind]::String) }
        } finally { $local.Dispose() }
    }
}
[pscustomobject]@{
    Action = $Action; RegistryProvider = 'System StdRegProv, current user SID only'
    StartupValuePresent = $current.Present; BackupExists = (Test-Path -LiteralPath $BackupPath)
    StartupApprovalEnabled = $approvalEnabled
    AutoStartExpected = $(if ($current.Present) { $approvalEnabled } else { $false })
    RegistryChanged = $before -ne $current.Present
    RunningAppStopped = $false; ConfigFilesModified = $false; RebootPerformed = $false
} | ConvertTo-Json

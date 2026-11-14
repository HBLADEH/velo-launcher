#Requires -Version 7.4
# Destructive machine-registration checks are restricted to the disposable RS14 VM.
# Each stage is invoked elevated by the unelevated guest coordinator. Real app/IME
# sessions run between stages with the user's normal filtered token.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidateSet('Prepare','Switch','Rollback','Cleanup')][string]$Stage,
    [Parameter(Mandatory)][string]$InputDirectory,
    [Parameter(Mandatory)][string]$EvidenceDirectory
)
$ErrorActionPreference='Stop'
Set-StrictMode -Version 2
if ($env:COMPUTERNAME -ne 'VELO-RS14-GUEST') { throw 'Machine installer acceptance is restricted to the disposable Velo RS14 guest.' }
$principal=[Security.Principal.WindowsPrincipal]::new([Security.Principal.WindowsIdentity]::GetCurrent())
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) { throw 'An elevated guest token is required.' }
$InputDirectory=(Resolve-Path -LiteralPath $InputDirectory).Path
$EvidenceDirectory=[IO.Path]::GetFullPath($EvidenceDirectory)
if (-not $EvidenceDirectory.StartsWith($env:USERPROFILE+'\VeloRS14-Product\',[StringComparison]::OrdinalIgnoreCase)) { throw 'Use the task-only guest evidence directory.' }
New-Item -ItemType Directory -Path $EvidenceDirectory -Force | Out-Null
$manifest=Get-Content -LiteralPath (Join-Path $InputDirectory 'machine-test-input.json') -Raw | ConvertFrom-Json
foreach ($file in $manifest.Files) {
    if ($file.Name -notmatch '^[a-zA-Z0-9._-]+$' -or (Get-FileHash -LiteralPath (Join-Path $InputDirectory $file.Name)).Hash -ne $file.SHA256) { throw 'Machine acceptance input identity differs.' }
}
$root=Join-Path ${env:ProgramFiles} 'HBLADEH/Velo'
$exe=Join-Path $root 'velo-launcher.exe'
$registry='SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\HBLADEHVelo'
$cases=[Collections.Generic.List[object]]::new()
function Registration {
    $base=[Microsoft.Win32.RegistryKey]::OpenBaseKey('LocalMachine','Registry64')
    try {
        $key=$base.OpenSubKey($registry)
        if(-not $key){return $null}
        try {@(foreach($name in ($key.GetValueNames()|Sort-Object)){[ordered]@{Name=$name;Kind=$key.GetValueKind($name).ToString();Value=$key.GetValue($name,$null,[Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)}})|ConvertTo-Json -Depth 6 -Compress}finally{$key.Dispose()}
    }finally{$base.Dispose()}
}
function Program-State {
    $files=@(foreach($file in @($exe,(Join-Path $root 'uninstall.exe'),
        (Join-Path ([Environment]::GetFolderPath('CommonPrograms')) 'Velo.lnk'),
        (Join-Path ([Environment]::GetFolderPath('CommonDesktopDirectory')) 'Velo.lnk'))){
        if(Test-Path -LiteralPath $file){(Get-FileHash -LiteralPath $file).Hash}else{'absent'}
    })
    [ordered]@{Files=$files;Registration=(Registration)}|ConvertTo-Json -Compress -Depth 5
}
function Invoke-Installer([string]$Name,[int]$Expected=0) {
    $p=Start-Process -FilePath (Join-Path $InputDirectory $Name) -ArgumentList '/S' -WindowStyle Hidden -PassThru
    try {
        # Each license copy is flushed and rehashed; a slow guest can need
        # several minutes for an upgrade plus an injected rollback.
        if(-not $p.WaitForExit(900000)){throw 'Machine installer timed out.'}
        $p.Refresh()
        if($null -eq $p.ExitCode -or $p.ExitCode -ne $Expected){throw "Machine installer result differs: $Name, observed $($p.ExitCode), expected $Expected."}
    }finally{$p.Dispose()}
}
function Invoke-Uninstaller([string]$Action,[bool]$Success) {
    $result=Join-Path $root ('.velo-rollback/'+$(if($Success){'last-result.json'}else{'last-error.json'}))
    $before=$(if(Test-Path -LiteralPath $result){(Get-Item -LiteralPath $result).LastWriteTimeUtc}else{[datetime]::MinValue})
    $arguments='/S'+$(if($Action -eq 'Rollback'){' /ROLLBACK'}else{''})
    $p=Start-Process -FilePath (Join-Path $root 'uninstall.exe') -ArgumentList $arguments -WindowStyle Hidden -PassThru
    try {if(-not $p.WaitForExit(900000)){throw 'Uninstaller launcher timed out.'};$launcherExit=$p.ExitCode}finally{$p.Dispose()}
    $deadline=[datetime]::UtcNow.AddSeconds(900)
    while([datetime]::UtcNow -lt $deadline){
        if((Test-Path -LiteralPath $result) -and (Get-Item -LiteralPath $result).LastWriteTimeUtc -gt $before){
            $actual=Get-Content -LiteralPath $result -Raw|ConvertFrom-Json
            if($actual.Action -eq $Action){
                if($Success -and -not $actual.Passed){throw 'Uninstaller did not report completed cleanup.'}
                return [ordered]@{LauncherExitCode=$launcherExit;OperationResult=$actual}
            }
        }
        Start-Sleep -Milliseconds 200
    }
    throw 'No fresh semantic uninstaller result; the launcher exit alone is insufficient.'
}
function Assert-Go {
    if((Get-FileHash -LiteralPath $exe).Hash -ne $manifest.GoExeSHA256){throw 'Official Go program identity changed.'}
}
function Assert-Rust([string]$Renderer) {
    $installation=Get-Content -LiteralPath (Join-Path $root '.velo-rust/installation.json') -Raw|ConvertFrom-Json
    if($installation.Renderer -ne $Renderer -or (Get-FileHash -LiteralPath $exe).Hash -ne $manifest.ExecutableHashes.$Renderer){throw 'In-place Rust program identity differs.'}
    $base=[Microsoft.Win32.RegistryKey]::OpenBaseKey('LocalMachine','Registry64')
    try{$key=$base.OpenSubKey($registry);try{if($key.GetValue('Runtime') -ne 'rust' -or $key.GetValue('InstallLocation') -ne $root){throw 'Machine Rust registration differs.'}}finally{$key.Dispose()}}finally{$base.Dispose()}
}
try {
    switch($Stage){
        Prepare {
            if((Registration) -or (Test-Path -LiteralPath $exe)){throw 'A fresh disposable guest machine installation is required.'}
            Invoke-Installer 'GoInstaller.exe'
            Assert-Go
            $original=Program-State
            $original|Set-Content -LiteralPath (Join-Path $EvidenceDirectory 'original-go-state.json') -Encoding utf8NoBOM
            [IO.File]::WriteAllText((Join-Path $root 'user-owned.keep'),'Synthetic unknown file must survive takeover, rollback and Rust uninstall.')
            $locked=[IO.File]::Open($exe,'Open','Read','Read')
            try{Invoke-Installer 'software-installer.exe' 30}finally{$locked.Dispose()}
            if((Program-State) -ne $original){throw 'Locked Go takeover changed the original program or registration.'}
            $cases.Add(@{Name='locked-Go-takeover-rejected-before-change';Passed=$true})
            $originalExe=Join-Path $EvidenceDirectory 'original-go.exe'
            Copy-Item -LiteralPath $exe -Destination $originalExe
            try{
                [IO.File]::WriteAllText($exe,'Unsupported baseline must not be taken over.')
                $unsupported=Program-State
                Invoke-Installer 'software-installer.exe' 30
                if((Program-State) -ne $unsupported){throw 'Unsupported baseline rejection changed state.'}
            }finally{Copy-Item -LiteralPath $originalExe -Destination $exe -Force}
            $cases.Add(@{Name='unknown-baseline-rejected-without-change';Passed=$true})
            Invoke-Installer 'software-installer.exe'
            Assert-Rust 'software'
            if((Get-FileHash -LiteralPath (Join-Path $root '.velo-rollback/original-go/files/velo-launcher.exe')).Hash -ne $manifest.GoExeSHA256){throw 'Original Go rollback archive differs.'}
            $cases.Add(@{Name='actual-Go-to-Software-same-path-HKLM-takeover';Passed=$true})
        }
        Switch {
            Assert-Rust 'software'
            $before=Program-State
            $locked=[IO.File]::Open($exe,'Open','Read','Read')
            try{
                Invoke-Installer 'femtovg-installer.exe' 30
                $null=Invoke-Uninstaller 'Uninstall' $false
            }finally{$locked.Dispose()}
            if((Program-State) -ne $before){throw 'Locked Rust upgrade/uninstall changed state.'}
            $cases.Add(@{Name='locked-Rust-upgrade-and-uninstall-rejected';Passed=$true})
            # This separate fixture adds one throw AFTER replacement to the exact
            # embedded script, then rehashes its payload. It is never published.
            Invoke-Installer 'fault-installer.exe' 30
            $failure=Get-Content -LiteralPath (Join-Path $root '.velo-rollback/last-error.json') -Raw|ConvertFrom-Json
            if($failure.ExitCode -ne 31 -or $failure.RecoveryRequired -or (Program-State) -ne $before -or (Test-Path -LiteralPath (Join-Path $root '.velo-rollback/pending.json'))){throw 'Post-replacement failure did not restore byte-exact program/registry/shortcuts.'}
            $cases.Add(@{Name='post-replacement-fault-restores-exact-Software-state';Passed=$true;FixtureOnly=$true})
            Invoke-Installer 'femtovg-installer.exe'
            Assert-Rust 'femtovg'
            $cases.Add(@{Name='actual-Software-to-FemtoVG-machine-upgrade';Passed=$true})
        }
        Rollback {
            Assert-Rust 'femtovg'
            $result=Invoke-Uninstaller 'Rollback' $true
            Assert-Go
            if((Program-State) -ne (Get-Content -LiteralPath (Join-Path $EvidenceDirectory 'original-go-state.json') -Raw).Trim()){throw 'Rollback did not restore exact Go files, registration and shortcuts.'}
            if(-not(Test-Path -LiteralPath (Join-Path $root 'user-owned.keep'))){throw 'Rollback removed an unknown file.'}
            $cases.Add(@{Name='actual-Go-machine-rollback-byte-exact';Passed=$true;SemanticUninstallResult=$result})
        }
        Cleanup {
            Assert-Go
            Invoke-Installer 'software-installer.exe'
            Assert-Rust 'software'
            $result=Invoke-Uninstaller 'Uninstall' $true
            if((Registration) -or (Test-Path -LiteralPath $exe) -or (Test-Path -LiteralPath (Join-Path $root '.velo-rust'))){throw 'Rust system uninstall left active product resources.'}
            foreach($folder in 'CommonPrograms','CommonDesktopDirectory'){if(Test-Path -LiteralPath (Join-Path ([Environment]::GetFolderPath($folder)) 'Velo.lnk')){throw 'Machine shortcut remains.'}}
            if(-not(Test-Path -LiteralPath (Join-Path $root 'user-owned.keep')) -or -not(Test-Path -LiteralPath (Join-Path $root '.velo-rollback/original-go/snapshot.json'))){throw 'Unknown file or retained recovery archive was deleted.'}
            $cases.Add(@{Name='actual-Rust-system-uninstall-keeps-unknown-and-rollback-archives';Passed=$true;SemanticUninstallResult=$result})
        }
    }
    [ordered]@{AtUtc=[datetime]::UtcNow.ToString('o');Stage=$Stage;Passed=$true;Cases=$cases.ToArray();HostModified=$false}|ConvertTo-Json -Depth 10|Set-Content -LiteralPath (Join-Path $EvidenceDirectory ($Stage.ToLowerInvariant()+'.json')) -Encoding utf8NoBOM
}catch{
    [ordered]@{AtUtc=[datetime]::UtcNow.ToString('o');Stage=$Stage;Passed=$false;Cases=$cases.ToArray();Error=($_|Out-String);HostModified=$false}|ConvertTo-Json -Depth 10|Set-Content -LiteralPath (Join-Path $EvidenceDirectory ($Stage.ToLowerInvariant()+'.json')) -Encoding utf8NoBOM
    throw
}

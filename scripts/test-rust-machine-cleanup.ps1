$ErrorActionPreference='Stop'
Set-StrictMode -Version 2
# Final elevated cleanup of resources this acceptance task created in the
# disposable guest. Runs only after every stage assertion has passed.
if($env:COMPUTERNAME -ne 'VELO-RS14-GUEST'){throw 'Disposable guest only.'}
$principal=[Security.Principal.WindowsPrincipal]::new([Security.Principal.WindowsIdentity]::GetCurrent())
if(-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'An elevated guest token is required.'}
$evidence=Join-Path $env:USERPROFILE 'VeloRS14-Product/acceptance'
$program=Join-Path ${env:ProgramFiles} 'HBLADEH/Velo'
$registry='SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\HBLADEHVelo'
$removed=[Collections.Generic.List[string]]::new()
try{
    foreach($name in 'velo-launcher','velo-slint'){
        if(@(Get-Process -Name $name -ErrorAction SilentlyContinue).Count){throw 'A Velo process is still running; exit it from the tray first.'}
    }
    $base=[Microsoft.Win32.RegistryKey]::OpenBaseKey('LocalMachine','Registry64')
    try{
        $key=$base.OpenSubKey($registry)
        if($key){$key.Dispose();throw 'Machine uninstall registration remains before final cleanup.'}
    }finally{$base.Dispose()}
    foreach($folder in 'CommonPrograms','CommonDesktopDirectory'){
        $link=Join-Path ([Environment]::GetFolderPath($folder)) 'Velo.lnk'
        if(Test-Path -LiteralPath $link){throw 'A machine shortcut remains before final cleanup.'}
    }
    foreach($path in @($program,(Join-Path $env:LOCALAPPDATA 'Velo'),(Join-Path $env:LOCALAPPDATA 'VeloRustMigrationBackups'))){
        if(Test-Path -LiteralPath $path){
            Remove-Item -LiteralPath $path -Recurse -Force
            $removed.Add($path)
        }
    }
    $company=Join-Path ${env:ProgramFiles} 'HBLADEH'
    if((Test-Path -LiteralPath $company) -and -not @(Get-ChildItem -LiteralPath $company -Force).Count){
        Remove-Item -LiteralPath $company -Force
        $removed.Add($company)
    }
    $remaining=@($removed|Where-Object {Test-Path -LiteralPath $_})
    if($remaining.Count){throw 'Final cleanup left task resources behind.'}
    [ordered]@{AtUtc=[datetime]::UtcNow.ToString('o');Passed=$true;Removed=$removed.ToArray();
        MachineRegistrationAbsent=$true;MachineShortcutsAbsent=$true;NoVeloProcess=$true;GuestOnly=$true}|
        ConvertTo-Json -Depth 6|Set-Content -LiteralPath (Join-Path $evidence 'final-cleanup.json') -Encoding utf8NoBOM
}catch{
    [ordered]@{AtUtc=[datetime]::UtcNow.ToString('o');Passed=$false;Removed=$removed.ToArray();Error=($_|Out-String)}|
        ConvertTo-Json -Depth 6|Set-Content -LiteralPath (Join-Path $evidence 'final-cleanup.json') -Encoding utf8NoBOM
    throw
}
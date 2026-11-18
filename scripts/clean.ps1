#Requires -Version 7.4
[CmdletBinding(SupportsShouldProcess)]
param(
    [switch]$All,
    [switch]$LegacyReports
)
$ErrorActionPreference = 'Stop'
$repository = (Resolve-Path -LiteralPath (Split-Path -Parent $PSScriptRoot)).Path
if (-not (Test-Path -LiteralPath (Join-Path $repository 'Cargo.toml')) -or
    -not (Test-Path -LiteralPath (Join-Path $repository 'src/main.rs'))) {
    throw 'Run cleanup from the Velo Cargo project.'
}
$target = Join-Path $repository 'target'
$roots = @($target)
if ($LegacyReports) { $roots += @('.reasonix', '.playwright-mcp') | ForEach-Object { Join-Path $repository $_ } }
# Development data lives in .dev-data, outside the disposable build tree.
$keep = @('artifacts', 'product-artifacts', 'previews', 'tools')
$removedBytes = 0L
foreach ($root in $roots) {
    if (-not (Test-Path -LiteralPath $root)) { continue }
    $resolved = (Resolve-Path -LiteralPath $root).Path
    if ([IO.Path]::GetDirectoryName($resolved) -ne $repository -or
        (Get-Item -LiteralPath $resolved -Force).Attributes.HasFlag([IO.FileAttributes]::ReparsePoint)) {
        throw 'Cleanup root must be a real, direct child of this repository.'
    }
    if ($root -eq $target) {
        $running = @(Get-CimInstance Win32_Process | Where-Object {
            $_.ExecutablePath -and $_.ExecutablePath.StartsWith($resolved + '\', [StringComparison]::OrdinalIgnoreCase)
        })
        if ($running.Count) { throw 'Exit Velo previews and test processes in target before cleaning.' }
        # Cargo runs outside target. Probe the locked toolchain's profile-local
        # lock files, not cargo.exe/rustc.exe processes from unrelated projects.
        foreach ($lock in Get-ChildItem -LiteralPath $resolved -Recurse -Depth 2 -File -Force -Filter '.cargo-*lock') {
            if ($lock.Attributes.HasFlag([IO.FileAttributes]::ReparsePoint)) { throw 'Remove generated file links separately before cleaning.' }
            try { [IO.File]::Open($lock.FullName, 'Open', 'Read', 'None').Dispose() }
            catch [IO.IOException] { throw 'Wait for the Cargo build using this target directory before cleaning.' }
        }
    }
    if ($root -eq $target -and $All) {
        if ($PSCmdlet.ShouldProcess($resolved, 'Remove all build outputs')) {
            if (@(Get-ChildItem -LiteralPath $resolved -Force -Recurse -Attributes ReparsePoint).Count) { throw 'Remove generated directory links separately before cleaning.' }
            $removedBytes += (Get-ChildItem -LiteralPath $resolved -Recurse -Force -File | Measure-Object Length -Sum).Sum
            Remove-Item -LiteralPath $resolved -Recurse -Force
        }
        continue
    }
    $items = @(Get-ChildItem -LiteralPath $resolved -Force | Where-Object { $root -ne $target -or $_.Name -notin $keep })
    foreach ($item in $items) {
        $path = [IO.Path]::GetFullPath($item.FullName)
        if (-not $path.StartsWith($resolved + '\', [StringComparison]::OrdinalIgnoreCase)) { throw 'Cleanup path left its generated directory.' }
        if ($item.Attributes.HasFlag([IO.FileAttributes]::ReparsePoint) -or
            ($item.PSIsContainer -and @(Get-ChildItem -LiteralPath $path -Force -Recurse -Attributes ReparsePoint -ErrorAction Stop).Count)) {
            throw 'Remove generated directory links separately before cleaning.'
        }
        if ($PSCmdlet.ShouldProcess($path, 'Remove generated files')) {
            $bytes = if ($item.PSIsContainer) { (Get-ChildItem -LiteralPath $path -File -Recurse -Force | Measure-Object Length -Sum).Sum } else { $item.Length }
            Remove-Item -LiteralPath $path -Recurse -Force
            $removedBytes += $bytes
        }
    }
}
Write-Output ('Removed {0:N2} GiB of generated files.' -f ($removedBytes / 1GB))

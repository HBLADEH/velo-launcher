# Explicit choices for the expressions in the locked dependency graph. An
# unfamiliar expression must be reviewed instead of silently picking a license.
function Get-VeloRustLicenseSelection([string]$Expression) {
    $choices = @{
        '(MIT OR Apache-2.0) AND Unicode-3.0' = 'MIT AND Unicode-3.0'
        '0BSD OR MIT OR Apache-2.0' = 'MIT'
        'Apache-2.0' = 'Apache-2.0'
        'Apache-2.0 AND MIT' = 'Apache-2.0 AND MIT'
        'Apache-2.0 OR MIT' = 'MIT'
        'Apache-2.0/MIT' = 'MIT'
        'Apache-2.0 / MIT' = 'MIT'
        'BSD-2-Clause' = 'BSD-2-Clause'
        'BSD-2-Clause OR Apache-2.0 OR MIT' = 'MIT'
        'BSD-3-Clause' = 'BSD-3-Clause'
        'BSD-3-Clause OR Apache-2.0' = 'BSD-3-Clause'
        'BSD-3-Clause OR MIT OR Apache-2.0' = 'MIT'
        'BSL-1.0' = 'BSL-1.0'
        'CC0-1.0 OR Apache-2.0' = 'Apache-2.0'
        'GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0' = 'LicenseRef-Slint-Royalty-free-2.0'
        'ISC' = 'ISC'
        'MIT' = 'MIT'
        'MIT OR Apache-2.0' = 'MIT'
        'MIT OR Apache-2.0 OR Zlib' = 'MIT'
        'MIT OR Zlib OR Apache-2.0' = 'MIT'
        'MIT/Apache-2.0' = 'MIT'
        'Unicode-3.0' = 'Unicode-3.0'
        'Unlicense OR MIT' = 'MIT'
        'Zlib' = 'Zlib'
        'Zlib OR Apache-2.0 OR MIT' = 'MIT'
    }
    if (-not $choices.ContainsKey($Expression)) { throw "Unreviewed dependency license expression: $Expression" }
    $choices[$Expression]
}

function Get-VeloRustLicenseFiles($Package) {
    $directory = Split-Path -Parent $Package.manifest_path
    # Retain paths: nested Unicode tables and copied spin code carry notices
    # that a scan of crate-root LICENSE files alone does not collect.
    $files = @(Get-ChildItem -LiteralPath $directory -Recurse -File | Where-Object Name -Match '^(LICENSE|LICENCE|COPYING|NOTICE|COPYRIGHT)([._-]|$)')
    if ($Package.license_file) {
        $explicit = [IO.Path]::GetFullPath((Join-Path $directory $Package.license_file))
        if (-not $explicit.StartsWith($directory + '\', [StringComparison]::OrdinalIgnoreCase)) { throw 'Dependency license file leaves its crate.' }
        if (Test-Path -LiteralPath $explicit -PathType Leaf) { $files += Get-Item -LiteralPath $explicit }
    }
    @($files | Sort-Object FullName -Unique)
}

function Get-VeloRustEmbeddedNotices($Package) {
    if ($Package.name -ne 'khronos_api') { return }
    $directory = Split-Path -Parent $Package.manifest_path
    foreach ($file in Get-ChildItem -LiteralPath $directory -Recurse -File | Where-Object Extension -In '.xml','.idl') {
        $contents = [IO.File]::ReadAllText($file.FullName)
        $text = if ($file.Extension -eq '.xml') {
            @([regex]::Matches($contents, '(?s)<comment>(.*?)</comment>|<!--(.*?)-->') | ForEach-Object {
                $value = if ($_.Groups[1].Success) { $_.Groups[1].Value } else { $_.Groups[2].Value }
                if ($value -match '(?i)copyright|permission is hereby granted|licensed under') { $value }
            }) -join "`n"
        } else {
            [regex]::Match($contents, '(?s)\A(?:(?://[^\r\n]*|[\r\n\s])+)' ).Value
        }
        if ($text -notmatch '(?i)copyright|permission is hereby granted|licensed under') { continue }
        $bytes = [Text.Encoding]::UTF8.GetBytes($text)
        [pscustomobject]@{
            SourcePath = [IO.Path]::GetRelativePath($directory, $file.FullName).Replace('\','/')
            SourceSHA256 = (Get-FileHash -LiteralPath $file.FullName).Hash
            NoticeSHA256 = [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($bytes))
            Text = $text
        }
    }
}

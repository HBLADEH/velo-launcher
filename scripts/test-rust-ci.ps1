#Requires -Version 7.4
[CmdletBinding()]
param([Parameter(Mandatory)][ValidateSet('software','femtovg')][string]$Renderer)

$ErrorActionPreference = 'Stop'
& (Join-Path $PSScriptRoot 'verify-rust.ps1') -Renderer $Renderer -Product -HostedCI -ChecksOnly

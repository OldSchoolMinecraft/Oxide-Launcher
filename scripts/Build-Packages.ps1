[CmdletBinding()]
param([string] $Target, [string] $OutputDir, [switch] $SkipBuild)
$ErrorActionPreference = 'Stop'
$buildArguments = @(Join-Path $PSScriptRoot 'package-build.py')
if ($Target) { $buildArguments += @('--target', $Target) }
if ($OutputDir) { $buildArguments += @('--output-dir', $OutputDir) }
if ($SkipBuild) { $buildArguments += '--skip-build' }
& python @buildArguments
if ($LASTEXITCODE -ne 0) { throw 'Launcher build/packaging failed.' }

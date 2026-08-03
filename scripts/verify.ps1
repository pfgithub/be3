<#
.SYNOPSIS
Lints, formats and tests the workspace.

.DESCRIPTION
Runs clippy, rustfmt and the test suite over the whole workspace. This is the
one command to run after making changes.

By default clippy applies the fixes it can and rustfmt rewrites the sources.
With -Check nothing is written: clippy and rustfmt only report, which is what
CI wants. Either way the run fails if any clippy warning survives.

.PARAMETER Check
Report problems instead of fixing them, and fail if there are any.

.EXAMPLE
.\scripts\verify.ps1

.EXAMPLE
.\scripts\verify.ps1 -Check
#>
[CmdletBinding()]
param(
    [switch]$Check
)

$ErrorActionPreference = 'Stop'

$repository = Split-Path -Parent $PSScriptRoot
Push-Location $repository
try {
    # Tests and examples are linted too, so a warning cannot hide in them.
    $clippyTargets = @('--workspace', '--all-targets', '--all-features')

    if (-not $Check) {
        Write-Host 'Applying clippy fixes...'
        # The tree is expected to be mid-change; fixing only a pristine
        # checkout would make the script useless during development.
        & cargo clippy --fix --allow-dirty --allow-staged @clippyTargets
        if ($LASTEXITCODE -ne 0) { throw 'cargo clippy --fix failed' }
    }

    # Everything clippy could not fix by itself, as an error. In fix mode this
    # re-runs against the freshly fixed sources, which cargo has already built.
    Write-Host 'Checking for clippy warnings...'
    & cargo clippy @clippyTargets -- -D warnings
    if ($LASTEXITCODE -ne 0) { throw 'cargo clippy reported warnings' }

    Write-Host 'Formatting...'
    $formatArguments = @('--all')
    if ($Check) { $formatArguments += '--check' }
    & cargo fmt @formatArguments
    if ($LASTEXITCODE -ne 0) { throw 'cargo fmt failed' }

    Write-Host 'Running tests...'
    $testArguments = @('--workspace')
    if ($Check) { $testArguments += @('--profile', 'ci') }
    & cargo nextest run @testArguments
    if ($LASTEXITCODE -ne 0) { throw 'cargo nextest run failed' }

    Write-Host ''
    Write-Host 'All checks passed.'
}
finally {
    Pop-Location
}

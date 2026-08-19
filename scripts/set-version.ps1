#!/usr/bin/env pwsh
<#
.SYNOPSIS
    Manual pre-release tool: sets the release version across all version-carrying manifests in
    the workspace. Run this locally, commit the result, and only then tag the release.

.DESCRIPTION
    The release workflow (.github/workflows/release.yml) treats the committed manifests as the
    single source of truth for the release version — it never writes to them, and on a tag push
    it fails the build if the tag doesn't match what's already committed. This script is how you
    make that be true: run it, review the diff, commit it, then `git tag`.

    Rewrites the `version` field in:
      - rtmapp/src-tauri/tauri.conf.json  (drives MSI ProductVersion and exe metadata)
      - rtmapp/src-tauri/Cargo.toml
      - rtmcli/Cargo.toml
      - todotxt/Cargo.toml
      - rtmapp/package.json               (cosmetic, kept in sync for consistency)

    For Cargo.toml files, only the `version` key inside the `[package]` table is touched. This is
    scoped deliberately, not just anchored to the start of a line: todotxt/Cargo.toml also has a
    `[dependencies.uuid]` table with its own `version = "..."` line, and a naive "first line
    starting with version" match would be correct today only by accident of ordering. Scoping the
    match to the `[package]` table body makes it correct by construction, and safe even if a
    dependency table is ever reordered to appear first.

    Each target is patched via a narrow, targeted regex rather than full JSON/TOML
    (de)serialization, to avoid reformatting files that are otherwise hand-maintained. If a
    target's version key cannot be found, the script fails loudly rather than silently leaving
    that file unversioned — a silent no-op here would ship a release with a stale version forever
    and would specifically break MSI upgrade detection.

    This script performs no git operations and is intentionally idempotent: running it twice with
    the same version is a no-op on the second run, not an error.

.PARAMETER Version
    The target version, e.g. "26.1.0" or "v26.1.0" (a leading v/V is stripped). Must be exactly
    major.minor.patch, all-numeric. Pre-release/build-metadata suffixes (e.g. "-rc1", "+build5")
    are rejected, because MSI ProductVersion cannot represent them.

.PARAMETER RepoRoot
    Root of the workspace containing rtmapp/, rtmcli/ and todotxt/. Defaults to the parent of this
    script's directory (i.e. the repo root, assuming the conventional scripts/ layout). Overridable
    for testing against an isolated directory tree.

.EXAMPLE
    ./scripts/set-version.ps1 26.1.0

.EXAMPLE
    ./scripts/set-version.ps1 -Version v26.1.1 -RepoRoot C:\temp\rtm-test
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true, Position = 0)]
    [string]$Version,

    [Parameter(Mandatory = $false)]
    [string]$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

# ---------------------------------------------------------------------------
# 1. Validate and normalise the version string.
# ---------------------------------------------------------------------------

$normalised = $Version -replace '^[vV]', ''

if ($normalised -notmatch '^(\d+)\.(\d+)\.(\d+)$') {
    throw "Invalid version '$Version'. Expected 'major.minor.patch' (optionally prefixed with " +
          "'v'), all-numeric, no pre-release or build-metadata suffix. Got: '$normalised'."
}

$major = [int]$Matches[1]
$minor = [int]$Matches[2]
$patch = [int]$Matches[3]

# MSI ProductVersion field limits: https://learn.microsoft.com/windows/win32/msi/productversion
if ($major -gt 255) {
    throw "Version '$normalised' invalid: major ($major) exceeds the MSI ProductVersion limit of 255."
}
if ($minor -gt 255) {
    throw "Version '$normalised' invalid: minor ($minor) exceeds the MSI ProductVersion limit of 255."
}
if ($patch -gt 65535) {
    throw "Version '$normalised' invalid: patch ($patch) exceeds the MSI ProductVersion limit of 65535."
}

Write-Host "Setting version to $normalised"

# ---------------------------------------------------------------------------
# 2. Patch helpers, one per file shape.
# ---------------------------------------------------------------------------

function Set-JsonTopLevelVersion {
    <#
        Replaces a top-level (2-space-indented) "version": "..." key. Anchored to that specific
        indentation so it cannot match a nested version key elsewhere in the document (e.g. a
        future `bundle.windows.wix.version`, which would be indented further).
    #>
    param([string]$Content, [string]$NewVersion)

    $pattern = '(?m)^  "version": "[^"]*"'
    $replacement = "  `"version`": `"$NewVersion`""
    $regex = [regex]::new($pattern)
    $match = $regex.Match($Content)

    if (-not $match.Success) {
        return $null
    }

    return @{
        Before  = $match.Value
        After   = $replacement
        Content = $regex.Replace($Content, $replacement, 1)
    }
}

function Set-TomlPackageVersion {
    <#
        Replaces the `version = "..."` key inside the `[package]` table specifically — not the
        first line-anchored `version = ` match in the whole file, which could belong to a
        dependency table (e.g. `[dependencies.uuid]` also declaring its own `version`).
    #>
    param([string]$Content, [string]$NewVersion)

    # Capture the [package] table's body: everything after the header up to the next top-level
    # table header or end of file.
    $blockRegex = [regex]::new('(?ms)^\[package\](?<body>.*?)(?=^\[|\z)')
    $blockMatch = $blockRegex.Match($Content)

    if (-not $blockMatch.Success) {
        return $null
    }

    $body = $blockMatch.Groups['body']
    $versionRegex = [regex]::new('(?m)^version = "[^"]*"')
    $versionMatch = $versionRegex.Match($body.Value)

    if (-not $versionMatch.Success) {
        return $null
    }

    $replacement = "version = `"$NewVersion`""
    $newBody = $versionRegex.Replace($body.Value, $replacement, 1)
    $newContent = $Content.Substring(0, $body.Index) + $newBody + $Content.Substring($body.Index + $body.Length)

    return @{
        Before  = $versionMatch.Value
        After   = $replacement
        Content = $newContent
    }
}

# ---------------------------------------------------------------------------
# 3. Define the patch targets.
# ---------------------------------------------------------------------------

$targets = @(
    @{ Path = Join-Path $RepoRoot 'rtmapp/src-tauri/tauri.conf.json'; Handler = ${function:Set-JsonTopLevelVersion} },
    @{ Path = Join-Path $RepoRoot 'rtmapp/src-tauri/Cargo.toml';       Handler = ${function:Set-TomlPackageVersion} },
    @{ Path = Join-Path $RepoRoot 'rtmcli/Cargo.toml';                 Handler = ${function:Set-TomlPackageVersion} },
    @{ Path = Join-Path $RepoRoot 'todotxt/Cargo.toml';                Handler = ${function:Set-TomlPackageVersion} },
    @{ Path = Join-Path $RepoRoot 'rtmapp/package.json';               Handler = ${function:Set-JsonTopLevelVersion} }
)

# ---------------------------------------------------------------------------
# 4. Apply patches. Fail loudly on any target whose pattern is not found — see script header.
# ---------------------------------------------------------------------------

foreach ($target in $targets) {
    $path = $target.Path

    if (-not (Test-Path -LiteralPath $path)) {
        throw "Version target not found: '$path'. Refusing to continue silently."
    }

    $content = [System.IO.File]::ReadAllText($path)
    $result = & $target.Handler $content $normalised

    if ($null -eq $result) {
        throw "Version key pattern not found in '$path'. The file may have been reformatted or " +
              'the key renamed/relocated; refusing to silently leave this file unversioned.'
    }

    # UTF-8 without BOM, preserving the file's existing line endings verbatim (we only ever
    # touch the matched substring).
    [System.IO.File]::WriteAllText($path, $result.Content, (New-Object System.Text.UTF8Encoding($false)))

    if ($result.Before -eq $result.After) {
        Write-Host "  $path : already at $normalised (no change)"
    } else {
        Write-Host "  $path : $($result.Before) -> $($result.After)"
    }
}

Write-Host "Version set to $normalised across $($targets.Count) file(s)."

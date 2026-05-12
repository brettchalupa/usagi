# Usagi Engine installer (Windows)
#
# Usage:
#   irm https://usagiengine.com/install.ps1 | iex
#
# To pin a version or change the install dir, set environment variables
# before piping into iex:
#   $env:UsagiVersion = 'v0.7.0'; irm https://usagiengine.com/install.ps1 | iex
#   $env:UsagiInstall = 'C:\tools\usagi'; irm https://usagiengine.com/install.ps1 | iex
#
# Source: https://github.com/brettchalupa/usagi/blob/main/website/install.ps1

#Requires -Version 5

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# Windows PowerShell 5.1 defaults to TLS 1.0/1.1, which GitHub rejects.
[Net.ServicePointManager]::SecurityProtocol =
    [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

$GithubRepo = 'brettchalupa/usagi'
$UserAgent = 'usagi-installer'

if (-not [Environment]::Is64BitOperatingSystem) {
    throw 'Usagi only publishes 64-bit Windows builds.'
}

# Version: env var > GitHub "latest" redirect
$Version = if ($env:UsagiVersion) { $env:UsagiVersion } else { '' }
if (-not $Version) {
    Write-Host 'Resolving latest release...'
    $api = "https://api.github.com/repos/$GithubRepo/releases/latest"
    $rel = Invoke-RestMethod -Uri $api -UseBasicParsing -Headers @{ 'User-Agent' = $UserAgent }
    $Version = $rel.tag_name
}

if ($Version -notmatch '^v[0-9]') {
    throw "Invalid version: '$Version' (expected vMAJOR.MINOR.PATCH)"
}

$VerNoV = $Version.TrimStart('v')
$Archive = "usagi-$VerNoV-windows-x86_64.zip"
$Checksum = "$Archive.sha256"
$BaseUrl = "https://github.com/$GithubRepo/releases/download/$Version"

$InstallDir = if ($env:UsagiInstall) { $env:UsagiInstall } else { Join-Path $env:USERPROFILE '.usagi' }
$BinDir = Join-Path $InstallDir 'bin'
$Exe = Join-Path $BinDir 'usagi.exe'

New-Item -ItemType Directory -Force -Path $BinDir | Out-Null

$Tmp = Join-Path ([IO.Path]::GetTempPath()) ([IO.Path]::GetRandomFileName())
New-Item -ItemType Directory -Force -Path $Tmp | Out-Null

try {
    Write-Host "Installing Usagi $Version (windows-x86_64) to $Exe"

    $ArchivePath = Join-Path $Tmp $Archive
    $ChecksumPath = Join-Path $Tmp $Checksum

    Write-Host "Downloading $Archive..."
    Invoke-WebRequest -Uri "$BaseUrl/$Archive" -OutFile $ArchivePath `
        -UseBasicParsing -Headers @{ 'User-Agent' = $UserAgent }

    Write-Host "Downloading $Checksum..."
    Invoke-WebRequest -Uri "$BaseUrl/$Checksum" -OutFile $ChecksumPath `
        -UseBasicParsing -Headers @{ 'User-Agent' = $UserAgent }

    Write-Host 'Verifying checksum...'
    $ExpectedLine = (Get-Content -Path $ChecksumPath -Raw).Trim()
    $Expected = (($ExpectedLine -split '\s+')[0]).ToLower()
    if (-not ($Expected -match '^[0-9a-f]{64}$')) {
        throw "Malformed checksum file: $ExpectedLine"
    }
    $Actual = (Get-FileHash -Algorithm SHA256 -Path $ArchivePath).Hash.ToLower()
    if ($Expected -ne $Actual) {
        throw "Checksum mismatch for ${Archive}: expected=$Expected actual=$Actual"
    }

    Write-Host 'Extracting...'
    $ExtractDir = Join-Path $Tmp 'extract'
    Expand-Archive -Path $ArchivePath -DestinationPath $ExtractDir -Force

    $Src = Get-ChildItem -Path $ExtractDir -Recurse -Filter 'usagi.exe' -File |
        Select-Object -First 1
    if (-not $Src) {
        throw 'Could not find usagi.exe inside archive'
    }

    if (Test-Path $Exe) { Remove-Item -Path $Exe -Force }
    Move-Item -Path $Src.FullName -Destination $Exe

    # Append to user PATH (HKCU) if missing. Never touch machine PATH.
    $UserPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    if (-not $UserPath) { $UserPath = '' }
    $Entries = $UserPath -split ';' | Where-Object { $_ -ne '' }
    if ($Entries -notcontains $BinDir) {
        $NewPath = if ($UserPath) { "$UserPath;$BinDir" } else { $BinDir }
        [Environment]::SetEnvironmentVariable('Path', $NewPath, 'User')
        $env:Path = "$env:Path;$BinDir"
        Write-Host ''
        Write-Host "Added $BinDir to your user PATH (new terminals will pick it up)."
    }

    Write-Host ''
    Write-Host "Installed: $Exe"
    Write-Host ''
    Write-Host 'Get started: usagi help'
}
finally {
    Remove-Item -Recurse -Force -Path $Tmp -ErrorAction SilentlyContinue
}

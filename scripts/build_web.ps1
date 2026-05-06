param(
  [switch] $Release,
  [string] $Example = "snake",
  [string] $EmsdkDir = ""
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Resolve-EmsdkDir {
  param([string] $Requested)

  if ($Requested) {
    return (Resolve-Path -LiteralPath $Requested).Path
  }

  if ($env:EMSDK) {
    return (Resolve-Path -LiteralPath $env:EMSDK).Path
  }

  $default = Join-Path $env:USERPROFILE ".local\share\emsdk"
  if (Test-Path -LiteralPath $default) {
    return (Resolve-Path -LiteralPath $default).Path
  }

  throw "emsdk not found. Install it with scripts/setup_emscripten.sh or pass -EmsdkDir."
}

function Import-EmsdkEnvironment {
  param([string] $Root)

  $envScript = Join-Path $Root "emsdk_env.bat"
  if (!(Test-Path -LiteralPath $envScript)) {
    throw "Missing emsdk_env.bat at $envScript"
  }

  $lines = cmd /c "`"$envScript`" >nul && set"
  foreach ($line in $lines) {
    $idx = $line.IndexOf("=")
    if ($idx -le 0) {
      continue
    }
    $name = $line.Substring(0, $idx)
    $value = $line.Substring($idx + 1)
    [Environment]::SetEnvironmentVariable($name, $value, "Process")
  }
}

function Copy-WebArtifact {
  param(
    [string] $Source,
    [string] $Destination
  )

  if (!(Test-Path -LiteralPath $Source)) {
    throw "Expected web build artifact missing: $Source"
  }
  Copy-Item -LiteralPath $Source -Destination $Destination -Force
}

$repoRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")).Path
Set-Location $repoRoot

$emsdkRoot = Resolve-EmsdkDir $EmsdkDir
$env:EMSDK_QUIET = "1"
Import-EmsdkEnvironment $emsdkRoot

$emscriptenRoot = Join-Path $emsdkRoot "upstream\emscripten"
$env:EMCMAKE = Join-Path $emscriptenRoot "emcmake.bat"
$env:EMMAKE = Join-Path $emscriptenRoot "emmake.bat"
$env:EMCC_CFLAGS = "-fwasm-exceptions -sSUPPORT_LONGJMP=wasm -s USE_LIBPNG=1 -s USE_OGG=1 -s USE_VORBIS=1"

$profileDir = if ($Release) { "release" } else { "debug" }
$cargoArgs = @("build", "--target", "wasm32-unknown-emscripten")
if ($Release) {
  $cargoArgs += "--release"
}

& cargo @cargoArgs
if ($LASTEXITCODE -ne 0) {
  exit $LASTEXITCODE
}

$targetWeb = Join-Path $repoRoot "target\web"
New-Item -ItemType Directory -Force -Path $targetWeb | Out-Null
Get-ChildItem -LiteralPath $targetWeb -Force | Remove-Item -Recurse -Force

Copy-Item -LiteralPath (Join-Path $repoRoot "web\shell.html") -Destination (Join-Path $targetWeb "index.html") -Force
Copy-WebArtifact (Join-Path $repoRoot "target\wasm32-unknown-emscripten\$profileDir\usagi.wasm") (Join-Path $targetWeb "usagi.wasm")
Copy-WebArtifact (Join-Path $repoRoot "target\wasm32-unknown-emscripten\$profileDir\usagi.js") (Join-Path $targetWeb "usagi.js")

$exportArgs = @("run")
if ($Release) {
  $exportArgs += "--release"
}
$exportArgs += @("--quiet", "--", "export", "examples/$Example", "--target", "bundle", "-o", "target/web/game.usagi")

& cargo @exportArgs
if ($LASTEXITCODE -ne 0) {
  exit $LASTEXITCODE
}

Write-Host "[usagi] wrote target/web/index.html, usagi.js, usagi.wasm, and game.usagi"

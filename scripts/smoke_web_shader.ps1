param(
  [switch] $Release,
  [int] $Port = 3535,
  [int] $DebugPort = 9223,
  [string] $BrowserPath = "",
  [string] $EmsdkDir = ""
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Test-NodeProbeSupport {
  $node = Get-Command node -ErrorAction SilentlyContinue
  if ($null -eq $node) {
    throw "Node.js is required for the Chrome DevTools smoke probe."
  }

  & node -e "process.exit(typeof WebSocket === 'function' ? 0 : 1)"
  if ($LASTEXITCODE -ne 0) {
    throw "Node.js 22+ is required because the smoke probe uses the built-in WebSocket client."
  }
}

function Test-PortFree {
  param([int] $Candidate)
  $listener = Get-NetTCPConnection -LocalPort $Candidate -State Listen -ErrorAction SilentlyContinue |
    Select-Object -First 1
  return $null -eq $listener
}

function Get-FreeTcpPort {
  param([int] $Start)
  for ($candidate = $Start; $candidate -lt ($Start + 200); $candidate++) {
    if (Test-PortFree $candidate) {
      return $candidate
    }
  }
  throw "No free TCP port found from $Start through $($Start + 199)."
}

function Resolve-BrowserPath {
  param([string] $Requested)

  if ($Requested) {
    return (Resolve-Path -LiteralPath $Requested).Path
  }

  $candidates = @(
    "$env:ProgramFiles\Google\Chrome\Application\chrome.exe",
    "${env:ProgramFiles(x86)}\Google\Chrome\Application\chrome.exe",
    "$env:LocalAppData\Google\Chrome\Application\chrome.exe",
    "$env:ProgramFiles\Microsoft\Edge\Application\msedge.exe",
    "${env:ProgramFiles(x86)}\Microsoft\Edge\Application\msedge.exe",
    "$env:LocalAppData\Microsoft\Edge\Application\msedge.exe"
  )

  foreach ($candidate in $candidates) {
    if ($candidate -and (Test-Path -LiteralPath $candidate)) {
      return (Resolve-Path -LiteralPath $candidate).Path
    }
  }

  throw "Chrome or Edge was not found. Pass -BrowserPath to a Chromium-based browser."
}

function Wait-HttpOk {
  param(
    [string] $Url,
    [int] $TimeoutSeconds = 20
  )

  $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
  do {
    try {
      $response = Invoke-WebRequest -Uri $Url -UseBasicParsing -TimeoutSec 2
      if ([int] $response.StatusCode -eq 200) {
        return
      }
    } catch {
      Start-Sleep -Milliseconds 250
    }
  } while ((Get-Date) -lt $deadline)

  throw "Timed out waiting for $Url"
}

function Wait-ChromeDebug {
  param(
    [int] $Port,
    [int] $TimeoutSeconds = 20
  )

  $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
  do {
    try {
      Invoke-RestMethod -Uri "http://127.0.0.1:$Port/json/version" -TimeoutSec 2 | Out-Null
      return
    } catch {
      Start-Sleep -Milliseconds 250
    }
  } while ((Get-Date) -lt $deadline)

  throw "Timed out waiting for Chrome remote debugging on port $Port"
}

function Stop-PortListener {
  param([int] $Port)

  $listeners = Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction SilentlyContinue
  foreach ($listener in $listeners) {
    $pidToStop = $listener.OwningProcess
    $proc = Get-Process -Id $pidToStop -ErrorAction SilentlyContinue
    if ($proc) {
      Stop-Process -Id $pidToStop -Force
    }
  }
}

$repoRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")).Path
Set-Location $repoRoot

$serverProc = $null
$browserProc = $null
$servePort = Get-FreeTcpPort $Port
$remoteDebugPort = Get-FreeTcpPort $DebugPort
$smokeRoot = Join-Path $repoRoot "target\web-shader-smoke"
$chromeProfile = Join-Path $smokeRoot "chrome-profile"

try {
  Test-NodeProbeSupport

  $buildArgs = @{ Example = "shader" }
  if ($Release) {
    $buildArgs.Release = $true
  }
  if ($EmsdkDir) {
    $buildArgs.EmsdkDir = $EmsdkDir
  }
  & (Join-Path $PSScriptRoot "build_web.ps1") @buildArgs

  New-Item -ItemType Directory -Force -Path $smokeRoot | Out-Null
  if (Test-Path -LiteralPath $chromeProfile) {
    Remove-Item -LiteralPath $chromeProfile -Recurse -Force
  }
  New-Item -ItemType Directory -Force -Path $chromeProfile | Out-Null

  $serverOut = Join-Path $smokeRoot "server.out.log"
  $serverErr = Join-Path $smokeRoot "server.err.log"
  Remove-Item -LiteralPath $serverOut, $serverErr -ErrorAction SilentlyContinue

  $serverCommand = "Set-Location -LiteralPath '$repoRoot'; `$env:PORT='$servePort'; just serve-web"
  $serverProc = Start-Process `
    -FilePath "pwsh" `
    -ArgumentList @("-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", $serverCommand) `
    -WindowStyle Hidden `
    -RedirectStandardOutput $serverOut `
    -RedirectStandardError $serverErr `
    -PassThru

  Wait-HttpOk "http://127.0.0.1:$servePort/"
  Wait-HttpOk "http://127.0.0.1:$servePort/usagi.js"
  Wait-HttpOk "http://127.0.0.1:$servePort/usagi.wasm"
  Wait-HttpOk "http://127.0.0.1:$servePort/game.usagi"

  $browser = Resolve-BrowserPath $BrowserPath
  $browserOut = Join-Path $smokeRoot "browser.out.log"
  $browserErr = Join-Path $smokeRoot "browser.err.log"
  Remove-Item -LiteralPath $browserOut, $browserErr -ErrorAction SilentlyContinue
  $browserArgs = @(
    "--headless=new",
    "--remote-debugging-port=$remoteDebugPort",
    "--user-data-dir=$chromeProfile",
    "--no-first-run",
    "--no-default-browser-check",
    "--enable-webgl",
    "--ignore-gpu-blocklist",
    "about:blank"
  )
  $browserProc = Start-Process `
    -FilePath $browser `
    -ArgumentList $browserArgs `
    -WindowStyle Hidden `
    -RedirectStandardOutput $browserOut `
    -RedirectStandardError $browserErr `
    -PassThru

  Wait-ChromeDebug $remoteDebugPort

  & node (Join-Path $PSScriptRoot "smoke_web_shader_probe.js") `
    --url "http://127.0.0.1:$servePort/" `
    --debug-port "$remoteDebugPort" `
    --out-dir $smokeRoot
  if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
  }
} finally {
  if ($browserProc -and !$browserProc.HasExited) {
    Stop-Process -Id $browserProc.Id -Force
  }
  Stop-PortListener $remoteDebugPort

  if ($serverProc -and !$serverProc.HasExited) {
    Stop-Process -Id $serverProc.Id -Force
  }
  Stop-PortListener $servePort
}

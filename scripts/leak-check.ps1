<#
.SYNOPSIS
  Verifies that a Raygun desktop build writes nothing outside its portable folder (ARCHITECTURE §3).

.DESCRIPTION
  Copies the exe into target\leak-check\Raygun, snapshots %APPDATA%, %LOCALAPPDATA% and
  HKCU\Software, runs the app until its webview profile appears in raygun-data, stops it,
  and snapshots again.

  Fails when:
    - any new file, folder or registry key whose name contains "raygun" appears outside the portable folder
    - raygun-data\logs or raygun-data\webview\EBWebView is missing next to the exe

  Other new entries (Windows and other apps write constantly) are listed for review only.

.EXAMPLE
  pnpm --filter @raygun/app tauri build --no-bundle
  powershell -ExecutionPolicy Bypass -File scripts\leak-check.ps1
#>
param(
  [string]$Exe = (Join-Path $PSScriptRoot '..\target\release\raygun.exe'),
  [int]$TimeoutSeconds = 60
)

$ErrorActionPreference = 'Stop'

$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$exePath = (Resolve-Path $Exe).Path
$portableDir = Join-Path $repo 'target\leak-check\Raygun'
$dataDir = Join-Path $portableDir 'raygun-data'

# This tool's own temp files live under a folder named after the repo; they are not the app's.
$ignorePattern = '\\Temp\\claude\\'

function Get-Entries {
  $entries = New-Object 'System.Collections.Generic.HashSet[string]'
  foreach ($root in @($env:APPDATA, $env:LOCALAPPDATA)) {
    foreach ($item in Get-ChildItem -LiteralPath $root -Force -ErrorAction SilentlyContinue) {
      [void]$entries.Add($item.FullName)
      if ($item.PSIsContainer) {
        foreach ($child in Get-ChildItem -LiteralPath $item.FullName -Force -ErrorAction SilentlyContinue) {
          [void]$entries.Add($child.FullName)
        }
      }
    }
  }
  foreach ($key in Get-ChildItem 'HKCU:\Software' -ErrorAction SilentlyContinue) {
    [void]$entries.Add($key.Name)
    foreach ($child in Get-ChildItem $key.PSPath -ErrorAction SilentlyContinue) {
      [void]$entries.Add($child.Name)
    }
  }
  return , $entries
}

function Get-RaygunNamed {
  $found = New-Object 'System.Collections.Generic.HashSet[string]'
  foreach ($root in @($env:APPDATA, $env:LOCALAPPDATA)) {
    Get-ChildItem -LiteralPath $root -Recurse -Depth 3 -Force -Filter '*raygun*' -ErrorAction SilentlyContinue |
      ForEach-Object { [void]$found.Add($_.FullName) }
  }
  Get-ChildItem 'HKCU:\Software' -Recurse -Depth 3 -ErrorAction SilentlyContinue |
    Where-Object { $_.PSChildName -match 'raygun' } |
    ForEach-Object { [void]$found.Add($_.Name) }
  return , $found
}

function Get-NewEntries($before, $after) {
  $after | Where-Object { -not $before.Contains($_) -and $_ -notmatch $ignorePattern } | Sort-Object
}

Write-Host "Preparing portable copy in $portableDir"
if (Test-Path $portableDir) { Remove-Item $portableDir -Recurse -Force }
New-Item -ItemType Directory -Force $portableDir | Out-Null
Copy-Item $exePath (Join-Path $portableDir 'Raygun.exe')

Write-Host 'Taking snapshot before launch...'
$entriesBefore = Get-Entries
$raygunBefore = Get-RaygunNamed

Write-Host 'Launching Raygun...'
$process = Start-Process -FilePath (Join-Path $portableDir 'Raygun.exe') -PassThru
$webviewProfile = Join-Path $dataDir 'webview\EBWebView'
$deadline = (Get-Date).AddSeconds($TimeoutSeconds)
while (-not (Test-Path $webviewProfile) -and (Get-Date) -lt $deadline -and -not $process.HasExited) {
  Start-Sleep -Milliseconds 500
}
# Give the webview time to finish writing its profile and the page time to load.
Start-Sleep -Seconds 8
if (-not $process.HasExited) {
  & taskkill.exe /PID $process.Id /T /F | Out-Null
}
Start-Sleep -Seconds 3

Write-Host 'Taking snapshot after exit...'
$entriesAfter = Get-Entries
$raygunAfter = Get-RaygunNamed

$failures = @()
if (-not (Test-Path (Join-Path $dataDir 'logs'))) { $failures += 'raygun-data\logs is missing' }
if (-not (Test-Path $webviewProfile)) { $failures += 'raygun-data\webview\EBWebView is missing (webview profile went elsewhere or app failed to start)' }

$leaks = @(Get-NewEntries $raygunBefore $raygunAfter)
foreach ($leak in $leaks) { $failures += "leak: $leak" }

$otherNew = @(Get-NewEntries $entriesBefore $entriesAfter | Where-Object { $leaks -notcontains $_ })

Write-Host ''
Write-Host "Portable folder contents:"
Get-ChildItem $dataDir -ErrorAction SilentlyContinue | ForEach-Object { Write-Host "  $($_.Name)" }

if ($otherNew.Count -gt 0) {
  Write-Host ''
  Write-Host "Other new entries during the run (review; usually Windows or other apps):"
  $otherNew | ForEach-Object { Write-Host "  $_" }
}

Write-Host ''
if ($failures.Count -gt 0) {
  Write-Host 'LEAK CHECK FAILED' -ForegroundColor Red
  $failures | ForEach-Object { Write-Host "  $_" -ForegroundColor Red }
  exit 1
}
Write-Host 'LEAK CHECK PASSED: nothing named raygun was written outside the portable folder.' -ForegroundColor Green
exit 0

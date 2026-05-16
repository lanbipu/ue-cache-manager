# UECM remote deploy script. Runs on lanPC.
# Triggered by scripts/deploy-lanpc.sh on mac.

$ErrorActionPreference = 'Stop'

$BuildDir   = 'E:\uecm-plan4-test'
$DeployDir  = 'C:\Tools\UECM'
$StagingDir = 'E:\uecm-plan4-test\.deploy-staging'
$TarFile    = Join-Path $StagingDir 'deploy.tar.gz'

function Step($n, $msg) { Write-Host "[lanPC $n] $msg" -ForegroundColor Cyan }

# 1. Sanity check
Step '1/7' "verify staging tar exists"
if (-not (Test-Path $TarFile)) {
  throw "staging tar not found: $TarFile"
}
$tarBytes = (Get-Item $TarFile).Length
Write-Host "    tar size: $([Math]::Round($tarBytes/1MB, 2)) MB"

# 2. Clean stale source files from build dir, then extract.
#    Without the cleanup, files deleted/renamed locally would linger on lanPC
#    and either pollute the build or shadow current source.
#    Preserve: node_modules, src-tauri/target, src-tauri/gen, .deploy-staging.
Step '2/7' "clean stale tree + extract tar over $BuildDir"
if (-not (Test-Path $BuildDir)) {
  New-Item -ItemType Directory -Path $BuildDir | Out-Null
}

$rootKeep    = @('node_modules', '.deploy-staging')
$tauriKeep   = @('target', 'gen')

# Wipe root-level files (lockfiles, configs — will be re-written from tar)
Get-ChildItem -LiteralPath $BuildDir -File -Force `
  | Remove-Item -Force -ErrorAction SilentlyContinue

# Wipe root-level dirs except cache and staging
Get-ChildItem -LiteralPath $BuildDir -Directory -Force `
  | Where-Object { $rootKeep -notcontains $_.Name } `
  | ForEach-Object {
      if ($_.Name -eq 'src-tauri') {
        # Inside src-tauri, preserve target/ and gen/, wipe everything else
        Get-ChildItem -LiteralPath $_.FullName -File -Force `
          | Remove-Item -Force -ErrorAction SilentlyContinue
        Get-ChildItem -LiteralPath $_.FullName -Directory -Force `
          | Where-Object { $tauriKeep -notcontains $_.Name } `
          | Remove-Item -Recurse -Force -ErrorAction SilentlyContinue
      } else {
        Remove-Item -LiteralPath $_.FullName -Recurse -Force -ErrorAction SilentlyContinue
      }
    }

# tar is bundled with Windows 10+; supports gz natively.
& tar -xzf $TarFile -C $BuildDir
if ($LASTEXITCODE -ne 0) { throw "tar extract failed (exit $LASTEXITCODE)" }

# 3. pnpm install (idempotent; respects pnpm-lock)
Step '3/7' "pnpm install --frozen-lockfile"
Push-Location $BuildDir
try {
  & pnpm install --frozen-lockfile
  if ($LASTEXITCODE -ne 0) { throw "pnpm install failed (exit $LASTEXITCODE)" }

  # 4. Tauri release build (--no-bundle: skip msi/exe installer, only produce target\release\uecm.exe)
  Step '4/7' "pnpm tauri build --no-bundle"
  & pnpm tauri build --no-bundle
  if ($LASTEXITCODE -ne 0) { throw "tauri build failed (exit $LASTEXITCODE)" }
} finally {
  Pop-Location
}

$BuildExe = Join-Path $BuildDir 'src-tauri\target\release\uecm.exe'
if (-not (Test-Path $BuildExe)) {
  throw "expected build artifact missing: $BuildExe"
}
$BuildHash = (Get-FileHash $BuildExe -Algorithm SHA256).Hash
Write-Host "    built exe sha256: $BuildHash"

# 5. Kill running uecm.exe so we can overwrite it
Step '5/7' "stop running uecm.exe (if any)"
Get-Process -Name 'uecm' -ErrorAction SilentlyContinue | ForEach-Object {
  Write-Host "    killing PID $($_.Id)"
  Stop-Process -Id $_.Id -Force
}
Start-Sleep -Milliseconds 500

# 6. Copy artifacts to C:\Tools\UECM (backup old exe first)
Step '6/7' "deploy to $DeployDir"
if (-not (Test-Path $DeployDir)) {
  New-Item -ItemType Directory -Path $DeployDir | Out-Null
}
$LiveExe = Join-Path $DeployDir 'uecm.exe'
if (Test-Path $LiveExe) {
  $stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
  Move-Item -Force $LiveExe (Join-Path $DeployDir "uecm.exe.bak-$stamp")
}
Copy-Item -Force $BuildExe $LiveExe

# Copy ps-scripts (mirror)
$BuildPsScripts  = Join-Path $BuildDir 'ps-scripts'
$DeployPsScripts = Join-Path $DeployDir 'ps-scripts'
if (Test-Path $BuildPsScripts) {
  if (Test-Path $DeployPsScripts) { Remove-Item -Recurse -Force $DeployPsScripts }
  Copy-Item -Recurse $BuildPsScripts $DeployPsScripts
}

# Copy vendor (PsExec64.exe et al.)
$BuildVendor  = Join-Path $BuildDir 'vendor'
$DeployVendor = Join-Path $DeployDir 'vendor'
if (Test-Path $BuildVendor) {
  if (Test-Path $DeployVendor) { Remove-Item -Recurse -Force $DeployVendor }
  Copy-Item -Recurse $BuildVendor $DeployVendor
}

# 7. SHA256 align check: built vs deployed must match
Step '7/7' "verify SHA256 alignment"
$DeployHash = (Get-FileHash $LiveExe -Algorithm SHA256).Hash
if ($BuildHash -ne $DeployHash) {
  throw "SHA256 mismatch! build=$BuildHash deploy=$DeployHash"
}
Write-Host "    OK: $DeployHash"

Write-Host ""
Write-Host "[lanPC] deploy complete." -ForegroundColor Green
Write-Host "    build exe:  $BuildExe"
Write-Host "    deploy exe: $LiveExe"
Write-Host "    sha256:     $DeployHash"

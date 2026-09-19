# install-opencode2.ps1: install the opencode2 release archive (windows-x64).
# Fail-closed: missing/corrupt archive or hash mismatch aborts before any
# write. Never touches an existing `opencode` binary or user data.
# Upgrade preserves history; uninstall removes only opencode2.
param(
  [string]$Archive = "",
  [string]$Checksum = "",
  [string]$InstallDir = (Join-Path $HOME ".local\bin"),
  [string]$Version = $env:OPENCODE2_VERSION,
  [switch]$Uninstall,
  [switch]$Help
)

$Bin = "opencode2.exe"

function Usage {
  Write-Host "usage: install-opencode2.ps1 [-Archive FILE] [-Checksum SHA256] [-InstallDir DIR] [-Uninstall]"
}

if ($Help) { Usage; exit 0 }

function Uninstall-Bin {
  $target = Join-Path -Path $InstallDir -ChildPath $Bin
  if (Test-Path -LiteralPath $target) {
    Write-Host "note: preserving user data; only removing $target"
    Remove-Item -LiteralPath $target -Force
    Write-Host "uninstalled $target (user data untouched)"
  } else {
    Write-Host "nothing to uninstall at $target"
  }
  exit 0
}

if ($Uninstall) { Uninstall-Bin }

if ([string]::IsNullOrEmpty($Archive)) { Write-Host "missing -Archive"; Usage; exit 64 }
if ([string]::IsNullOrEmpty($Checksum)) { Write-Host "missing -Checksum (fail-closed)"; Usage; exit 64 }
if (-not (Test-Path -LiteralPath $Archive)) { Write-Host "archive not found: $Archive"; exit 66 }

# Architecture gate: release ships windows-x64.
$arch = $env:PROCESSOR_ARCHITECTURE
if ($arch -ne "AMD64") { Write-Host "unsupported arch: $arch (need AMD64/windows-x64)"; exit 64 }

# Hash gate BEFORE any write.
$actual = (Get-FileHash -LiteralPath $Archive -Algorithm SHA256).Hash.ToLower()
if ($actual -ne $Checksum.ToLower()) { Write-Host "checksum mismatch, aborting"; exit 65 }

$legacy = Join-Path -Path $InstallDir -ChildPath "opencode.exe"
if (Test-Path -LiteralPath $legacy) {
  Write-Host "refusing: $legacy exists; opencode2 installs side-by-side only"
  exit 73
}

$stage = Join-Path ([System.IO.Path]::GetTempPath()) ("opencode2-" + [System.Guid]::NewGuid())
New-Item -ItemType Directory -Path $stage | Out-Null
try {
  Expand-Archive -LiteralPath $Archive -DestinationPath $stage -Force
  $src = Join-Path -Path $stage -ChildPath $Bin
  if (-not (Test-Path -LiteralPath $src)) { Write-Host "archive missing $Bin binary"; exit 65 }
  New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
  $target = Join-Path -Path $InstallDir -ChildPath $Bin
  if (Test-Path -LiteralPath $target) {
    Write-Host "upgrade: preserving existing install (history lives in user data dir, untouched)"
  }
  Copy-Item -LiteralPath $src -Destination $target -Force
  Write-Host "installed $target (windows-x64)"
  & $target --version
} finally {
  Remove-Item -LiteralPath $stage -Recurse -Force -ErrorAction SilentlyContinue
}

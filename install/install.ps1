[CmdletBinding()]
param(
  [string] $Version = $env:DSTACK_VERSION,
  [string] $InstallDir = $env:DSTACK_INSTALL_DIR,
  [switch] $DryRun
)

$ErrorActionPreference = 'Stop'
$Repository = 'KyanJeuring/dstack'

function Normalize-Version {
  param([Parameter(Mandatory = $true)][string] $Value)

  if ($Value -notmatch '^(?:v)?(2\.\d+\.\d+)$') {
    throw 'Version must be a stable v2.X.Y release.'
  }
  return "v$($Matches[1])"
}

if ([string]::IsNullOrWhiteSpace($InstallDir)) {
  if ([string]::IsNullOrWhiteSpace($HOME)) {
    throw 'HOME is not set; use -InstallDir or DSTACK_INSTALL_DIR.'
  }
  $InstallDir = Join-Path $HOME '.local/bin'
}

if ([string]::IsNullOrWhiteSpace($Version)) {
  $latest = Invoke-RestMethod `
    -Uri "https://api.github.com/repos/$Repository/releases/latest" `
    -Headers @{ Accept = 'application/vnd.github+json' }
  $Version = Normalize-Version -Value ([string] $latest.tag_name)
} else {
  $Version = Normalize-Version -Value $Version
}

if (-not [Environment]::Is64BitOperatingSystem) {
  throw 'DStack native Windows releases require x86_64 Windows.'
}
$architecture = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture
if ($architecture -ne [System.Runtime.InteropServices.Architecture]::X64) {
  throw "Unsupported Windows architecture: $architecture"
}

$Platform = 'windows-x86_64'
$Archive = "dstack-$Version-$Platform.zip"
$Destination = Join-Path $InstallDir 'dstack.exe'

Write-Output 'DStack installer plan'
Write-Output "  Version: $Version"
Write-Output "  Platform: $Platform"
Write-Output "  Release asset: $Archive"
Write-Output "  Destination: $Destination"

if ($DryRun) {
  Write-Output 'Dry run: no network request or filesystem change was made.'
  return
}

$TempRoot = Join-Path ([IO.Path]::GetTempPath()) ("install-dstack-" + [Guid]::NewGuid().ToString('N'))
$StagePath = $null
$BackupPath = $null
try {
  New-Item -ItemType Directory -Path $TempRoot | Out-Null
  $ArchivePath = Join-Path $TempRoot $Archive
  $ChecksumPath = "$ArchivePath.sha256"
  $ReleaseBase = "https://github.com/$Repository/releases/download/$Version"

  Invoke-WebRequest -Uri "$ReleaseBase/$Archive" -OutFile $ArchivePath
  try {
    Invoke-WebRequest -Uri "$ReleaseBase/$Archive.sha256" -OutFile $ChecksumPath
  } catch {
    throw "Could not download $Archive.sha256; installation requires its checksum. $($_.Exception.Message)"
  }

  $ChecksumText = [IO.File]::ReadAllText($ChecksumPath)
  $EscapedArchive = [Regex]::Escape($Archive)
  if ($ChecksumText -notmatch "^([0-9A-Fa-f]{64})  $EscapedArchive(?:`r?`n)?$") {
    throw 'Checksum sidecar is malformed or names the wrong release asset.'
  }
  $ExpectedChecksum = $Matches[1]
  $ActualChecksum = (Get-FileHash -LiteralPath $ArchivePath -Algorithm SHA256).Hash
  if (-not $ActualChecksum.Equals($ExpectedChecksum, [StringComparison]::OrdinalIgnoreCase)) {
    throw "SHA-256 checksum mismatch for $Archive."
  }

  Add-Type -AssemblyName System.IO.Compression.FileSystem
  $Zip = [IO.Compression.ZipFile]::OpenRead($ArchivePath)
  try {
    $Entries = @($Zip.Entries)
    if ($Entries.Count -ne 1 -or $Entries[0].FullName -cne 'dstack.exe' -or [string]::IsNullOrEmpty($Entries[0].Name)) {
      throw 'Release archive must contain only the dstack.exe executable.'
    }
  } finally {
    $Zip.Dispose()
  }

  $ExtractDir = Join-Path $TempRoot 'extracted'
  Expand-Archive -LiteralPath $ArchivePath -DestinationPath $ExtractDir
  $StagedBinary = Join-Path $ExtractDir 'dstack.exe'
  if (-not (Test-Path -LiteralPath $StagedBinary -PathType Leaf)) {
    throw 'Release archive does not contain the expected dstack.exe executable.'
  }

  $ExpectedOutput = "dstack $($Version.Substring(1))"
  $ActualOutput = & $StagedBinary --version
  if ($LASTEXITCODE -ne 0 -or $ActualOutput -cne $ExpectedOutput) {
    throw "Staged executable reported '$ActualOutput'; expected '$ExpectedOutput'."
  }

  New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
  $StagePath = Join-Path $InstallDir (".dstack.install." + [Guid]::NewGuid().ToString('N') + '.exe')
  Copy-Item -LiteralPath $StagedBinary -Destination $StagePath

  if (Test-Path -LiteralPath $Destination) {
    $BackupPath = Join-Path $InstallDir (".dstack.backup." + [Guid]::NewGuid().ToString('N') + '.exe')
    try {
      [IO.File]::Replace($StagePath, $Destination, $BackupPath)
      $StagePath = $null
      Remove-Item -LiteralPath $BackupPath -Force
      $BackupPath = $null
    } catch {
      if (-not (Test-Path -LiteralPath $Destination) -and (Test-Path -LiteralPath $BackupPath)) {
        [IO.File]::Move($BackupPath, $Destination)
        $BackupPath = $null
      }
      throw
    }
  } else {
    [IO.File]::Move($StagePath, $Destination)
    $StagePath = $null
  }

  Write-Output "Installed $ExpectedOutput to $Destination"
  $PathEntries = @(([Environment]::GetEnvironmentVariable('PATH', 'Process') -split ';') | Where-Object { $_ })
  $InstallFullPath = [IO.Path]::GetFullPath($InstallDir).TrimEnd('\')
  $OnPath = $PathEntries | Where-Object {
    [IO.Path]::GetFullPath($_).TrimEnd('\').Equals($InstallFullPath, [StringComparison]::OrdinalIgnoreCase)
  }
  if (-not $OnPath) {
    Write-Output "The install directory is not on PATH. Add this exact directory: $InstallDir"
  }
  Write-Output 'After dstack is on PATH, run: dstack shell install'

  if (-not [string]::IsNullOrWhiteSpace($HOME)) {
    $Legacy = Join-Path $HOME '.local/share/dstack/dstack.sh'
    if (Test-Path -LiteralPath $Legacy -PathType Leaf) {
      Write-Output "DStack v2 was installed to $Destination."
      Write-Output 'The legacy v1 Bash integration may still shadow the v2 binary.'
      Write-Output 'Follow MIGRATING_V1_TO_V2.md before expecting `dstack` to invoke v2:'
      Write-Output "  https://github.com/$Repository/blob/main/MIGRATING_V1_TO_V2.md"
      Write-Output 'Preview registry migration with the installed v2 executable:'
      Write-Output "  & `"$Destination`" migrate registry --dry-run"
      Write-Output 'Then migrate after reviewing the preview:'
      Write-Output "  & `"$Destination`" migrate registry"
      Write-Output 'Legacy v1 files and shell-profile lines were left unchanged.'
    }
  }
} finally {
  if ($StagePath -and (Test-Path -LiteralPath $StagePath)) {
    Remove-Item -LiteralPath $StagePath -Force
  }
  if ($BackupPath -and (Test-Path -LiteralPath $BackupPath) -and (Test-Path -LiteralPath $Destination)) {
    Remove-Item -LiteralPath $BackupPath -Force
  }
  if (Test-Path -LiteralPath $TempRoot) {
    Remove-Item -LiteralPath $TempRoot -Recurse -Force
  }
}

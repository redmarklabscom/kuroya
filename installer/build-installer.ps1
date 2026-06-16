[CmdletBinding()]
param(
    [switch]$SkipBuild,
    [string]$InnoCompilerPath
)

$ErrorActionPreference = 'Stop'

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$ReleaseExe = Join-Path $RepoRoot 'target\release\kuroya.exe'
$InnoScript = Join-Path $PSScriptRoot 'kuroya.iss'

function Resolve-InnoCompiler {
    param([string]$ExplicitPath)

    $candidatePaths = @()
    if ($ExplicitPath) {
        $candidatePaths += $ExplicitPath
    }
    if ($env:INNO_SETUP_COMPILER) {
        $candidatePaths += $env:INNO_SETUP_COMPILER
    }
    $candidatePaths += Join-Path $RepoRoot '.tools\InnoSetup6\ISCC.exe'
    if (${env:ProgramFiles(x86)}) {
        $candidatePaths += Join-Path ${env:ProgramFiles(x86)} 'Inno Setup 6\ISCC.exe'
    }
    if ($env:ProgramFiles) {
        $candidatePaths += Join-Path $env:ProgramFiles 'Inno Setup 6\ISCC.exe'
    }
    if ($env:LOCALAPPDATA) {
        $candidatePaths += Join-Path $env:LOCALAPPDATA 'Programs\Inno Setup 6\ISCC.exe'
    }

    foreach ($candidate in $candidatePaths) {
        if ($candidate -and (Test-Path -LiteralPath $candidate)) {
            return (Resolve-Path -LiteralPath $candidate).Path
        }
    }

    $command = Get-Command ISCC.exe -ErrorAction SilentlyContinue
    if ($command) {
        return $command.Source
    }

    throw @'
Inno Setup compiler not found.
Install Inno Setup 6, add ISCC.exe to PATH, set INNO_SETUP_COMPILER, or pass -InnoCompilerPath.
'@
}

Push-Location $RepoRoot
try {
    if (-not $SkipBuild) {
        cargo build -p kuroya-app --release
        if ($LASTEXITCODE -ne 0) {
            throw "cargo build failed with exit code $LASTEXITCODE"
        }
    }

    if (-not (Test-Path -LiteralPath $ReleaseExe)) {
        throw "Release binary not found: $ReleaseExe"
    }

    $InnoCompiler = Resolve-InnoCompiler -ExplicitPath $InnoCompilerPath

    $manifest = Get-Content -LiteralPath (Join-Path $RepoRoot 'crates\kuroya-app\Cargo.toml')
    $versionLine = $manifest | Where-Object { $_ -match '^version\s*=\s*"([^"]+)"' } | Select-Object -First 1
    if (-not $versionLine) {
        throw 'Could not read kuroya-app version from Cargo.toml'
    }
    $Version = [regex]::Match($versionLine, '^version\s*=\s*"([^"]+)"').Groups[1].Value

    New-Item -ItemType Directory -Force -Path (Join-Path $RepoRoot 'dist') | Out-Null
    Get-ChildItem -LiteralPath (Join-Path $RepoRoot 'dist') -Filter 'Kuroya-Setup-*.exe' -ErrorAction SilentlyContinue |
        Remove-Item -Force

    & $InnoCompiler "/DSourceRoot=$RepoRoot" "/DAppVersion=$Version" $InnoScript
    if ($LASTEXITCODE -ne 0) {
        throw "Inno Setup failed with exit code $LASTEXITCODE"
    }

    $installerPath = Join-Path $RepoRoot "dist\Kuroya-Setup-$Version.exe"
    if (-not (Test-Path -LiteralPath $installerPath)) {
        throw "Installer was not created: $installerPath"
    }

    Get-Item -LiteralPath $installerPath | Select-Object FullName, Length, LastWriteTime
} finally {
    Pop-Location
}

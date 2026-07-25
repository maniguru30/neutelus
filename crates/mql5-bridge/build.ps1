param(
    [switch]$InstallDeps,
    [switch]$Release
)

$ErrorActionPreference = "Stop"
$RootDir = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$Cargo = "$env:USERPROFILE\.cargo\bin\cargo.exe"

Write-Host "=== NautilusTrader MQL5 Bridge Build ===" -ForegroundColor Cyan
Write-Host "Root: $RootDir"
Write-Host ""

# Step 1: Verify Rust toolchain
Write-Host "[1/2] Checking Rust toolchain..." -ForegroundColor Yellow
$rustVer = & "$env:USERPROFILE\.cargo\bin\rustc.exe" --version 2>$null
if (-not $rustVer) {
    Write-Host "Rust not found. Install from: https://rustup.rs" -ForegroundColor Red
    exit 1
}
Write-Host "  Rust: $rustVer"

# Check that MSVC tools are available (link.exe from VS Build Tools)
$msvcAvailable = $false
try {
    $null = Get-Command "link.exe" -ErrorAction Stop
    $msvcAvailable = $true
} catch {
    # Try to find link.exe via vcvarsall
    $vsRoots = @(
        "${env:ProgramFiles}\Microsoft Visual Studio\2022\BuildTools",
        "${env:ProgramFiles(x86)}\Microsoft Visual Studio\2022\BuildTools",
        "${env:ProgramFiles}\Microsoft Visual Studio\2022\Community",
        "${env:ProgramFiles(x86)}\Microsoft Visual Studio\2022\Community",
        "${env:ProgramFiles}\Microsoft Visual Studio\2022\Professional",
        "${env:ProgramFiles(x86)}\Microsoft Visual Studio\2022\Professional"
    )
    foreach ($vs in $vsRoots) {
        $vcvars = Join-Path $vs "VC\Auxiliary\Build\vcvars64.bat"
        if (Test-Path $vcvars) {
            Write-Host "  Found VS at: $vs"
            # vcvars will set up the PATH for the current process
            cmd /c "`"$vcvars`" > nul && set" | ForEach-Object {
                if ($_ -match '^PATH=(.*)') {
                    $env:PATH = $matches[1]
                }
            }
            $msvcAvailable = $true
            break
        }
    }
}

if (-not $msvcAvailable) {
    Write-Host ""
    Write-Host "ERROR: Visual Studio Build Tools not found!" -ForegroundColor Red
    Write-Host ""
    Write-Host "Install them using one of these methods:" -ForegroundColor Yellow
    Write-Host ""
    Write-Host "  Method 1 - winget (fast):" -ForegroundColor Cyan
    Write-Host "    winget install Microsoft.VisualStudio.2022.BuildTools -e --source winget"
    Write-Host "    cd `"$env:TEMP`""
    Write-Host '    vs_buildtools.exe --quiet --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended'
    Write-Host ""
    Write-Host "  Method 2 - Manual download:" -ForegroundColor Cyan
    Write-Host "    https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2022"
    Write-Host "    Run: vs_BuildTools.exe --quiet --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
    Write-Host ""
    Write-Host "  Method 3 - Use an existing Visual Studio installation:" -ForegroundColor Cyan
    Write-Host "    Any edition (Community/Professional/Enterprise) with 'Desktop development with C++' workload"
    Write-Host ""
    Write-Host "After installation, re-run this script." -ForegroundColor Yellow
    exit 1
}
Write-Host "  MSVC linker: $(Get-Command link.exe | Select-Object -ExpandProperty Source)"

# Step 2: Build the bridge DLL
Write-Host ""
Write-Host "[2/2] Building bridge DLL..." -ForegroundColor Green

$buildArgs = @(
    "build"
    "--manifest-path", "$RootDir\Cargo.toml"
    "-p", "nautilus-mql5-bridge"
)
if ($Release) {
    $buildArgs += "--release"
}

Write-Host "> cargo $($buildArgs -join ' ')"
& $Cargo $buildArgs

if ($LASTEXITCODE -ne 0) {
    Write-Host "BUILD FAILED!" -ForegroundColor Red
    exit 1
}

# Step 3: Locate output
if ($Release) {
    $dllPath = "$RootDir\target\release\nautilus_mql5_bridge.dll"
} else {
    $dllPath = "$RootDir\target\debug\nautilus_mql5_bridge.dll"
}

Write-Host ""
Write-Host "=== BUILD SUCCESS ===" -ForegroundColor Green
Write-Host "DLL: $dllPath"
Write-Host ""
Write-Host "Deploy to MT5:" -ForegroundColor Cyan
Write-Host "  Copy DLL:   copy `"$dllPath`" `"%APPDATA%\MetaQuotes\Terminal\Common\Libraries\`""
Write-Host "  Copy EA:    copy `"$PSScriptRoot\mql5\NautilusEA.mq5`" `"%APPDATA%\MetaQuotes\Terminal\Common\MQL5\Experts\`""
Write-Host ""
Write-Host "In MT5: Tools > Options > Expert Advisors > check 'Allow DLL imports'" -ForegroundColor Yellow

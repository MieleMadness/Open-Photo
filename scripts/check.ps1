param([switch]$Package)
$ErrorActionPreference='Stop'
$projectRoot=Split-Path $PSScriptRoot
Push-Location $projectRoot
try {
    # Keep compiler scratch files and downloaded native dependencies in writable build folders.
    $buildTemp=Join-Path $projectRoot 'src-tauri/target/native-temp'
    New-Item -ItemType Directory -Force -Path $buildTemp | Out-Null
    $env:TEMP=$buildTemp
    $env:TMP=$buildTemp
    $env:ORT_CACHE_DIR=Join-Path $projectRoot 'src-tauri/target/ort-cache'
    if (!(Get-Command cl.exe -ErrorAction SilentlyContinue)) {
        $vswhere=Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
        if (!(Test-Path -LiteralPath $vswhere)) {throw 'Install Microsoft C++ Build Tools with Desktop development with C++ first.'}
        $instances=& $vswhere -all -products '*' -format json | ConvertFrom-Json
        $instance=$instances | Where-Object {Test-Path -LiteralPath (Join-Path $_.installationPath 'VC/Tools/MSVC')} | Select-Object -First 1
        if (!$instance) {throw 'The Desktop development with C++ workload is missing.'}
        & (Join-Path $instance.installationPath 'Common7/Tools/Launch-VsDevShell.ps1') -Arch amd64 -HostArch amd64 -SkipAutomaticLocation -NoLogo
    }
    function Invoke-Check([scriptblock]$Command) {
        & $Command
        if ($LASTEXITCODE -ne 0) {throw "Check failed with exit code $LASTEXITCODE"}
    }
    Invoke-Check {npm run prepare:video}
    Invoke-Check {npm run prepare:models}
    Invoke-Check {npm run tauri icon assets/app-icon.svg}
    Invoke-Check {cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check}
    Invoke-Check {npm test}
    Invoke-Check {npm run build}
    Invoke-Check {cargo clippy --manifest-path src-tauri/Cargo.toml --locked --all-targets -- -D warnings}
    Invoke-Check {cargo test --manifest-path src-tauri/Cargo.toml --locked}
    if ($Package) {Invoke-Check {npm run tauri build -- -- --locked}}
} finally {Pop-Location}

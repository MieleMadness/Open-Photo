param([Parameter(Mandatory=$true)][string]$Installer,[Parameter(Mandatory=$true)][string]$ReportDirectory)
$ErrorActionPreference='Stop'
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted' -or $env:RUNNER_OS -ne 'Windows') {throw 'Installer lifecycle acceptance runs only on disposable GitHub-hosted Windows runners, never a personal profile.'}
$projectRoot=Split-Path $PSScriptRoot
$installerPath=(Resolve-Path -LiteralPath $Installer).Path
$runnerRoot=[IO.Path]::GetFullPath($env:RUNNER_TEMP).TrimEnd('\')+'\'
$installDirectory=Join-Path $env:RUNNER_TEMP ('openphoto-installed-'+[guid]::NewGuid().ToString('N'))
$ReportDirectory=[IO.Path]::GetFullPath($ReportDirectory)
foreach ($ownedPath in @($installDirectory,$ReportDirectory)) {if (!$ownedPath.StartsWith($runnerRoot,[StringComparison]::OrdinalIgnoreCase)) {throw 'Installer test path is outside the disposable runner workspace'}}
New-Item -ItemType Directory -Path $ReportDirectory | Out-Null
function Run-Installer {
    $process=Start-Process -FilePath $installerPath -ArgumentList @('/S',"/D=$installDirectory") -WindowStyle Hidden -Wait -PassThru
    if ($process.ExitCode -ne 0) {throw "Installer failed: $($process.ExitCode)"}
    if (!(Test-Path -LiteralPath (Join-Path $installDirectory 'open-photo.exe'))) {throw 'Installed app executable is missing'}
}
function Run-Uninstaller([string[]]$Options=@()) {
    $uninstaller=Join-Path $installDirectory 'uninstall.exe'
    if (!(Test-Path -LiteralPath $uninstaller)) {throw 'Installed uninstaller is missing'}
    $arguments=@('/S')+$Options+@("_?=$installDirectory")
    $process=Start-Process -FilePath $uninstaller -ArgumentList $arguments -WindowStyle Hidden -Wait -PassThru
    if ($process.ExitCode -ne 0) {throw "Uninstaller failed: $($process.ExitCode)"}
    if (Test-Path -LiteralPath (Join-Path $installDirectory 'open-photo.exe')) {throw 'Uninstaller did not remove the application'}
}
Run-Installer
$manifest=Get-Content -LiteralPath (Join-Path $projectRoot 'config/ai-models.json') -Raw | ConvertFrom-Json
foreach ($model in $manifest.models) {
    $file=Join-Path $installDirectory ('bundled-ai/'+$model.name)
    if (!(Test-Path -LiteralPath $file) -or (Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash.ToLowerInvariant() -ne $model.sha256) {throw "Installed AI asset is missing or corrupt: $($model.kind)"}
}
foreach ($notice in @('NOTICE.md','LICENSE-OpenCLIP.txt','LICENSE-YuNet.txt','LICENSE-SFace.txt','LICENSE-PaddleOCR.txt','LICENSE-RapidOCR.txt','MODEL-CARD-OpenCLIP.md')) {if (!(Test-Path -LiteralPath (Join-Path $installDirectory ('bundled-ai/'+$notice)))) {throw "Installed AI notice missing: $notice"}}
$previousBundle=$env:OPENPHOTO_AI_BUNDLE_DIRECTORY
$previousReport=$env:OPENPHOTO_AI_ACCEPTANCE_OUTPUT
try {
    $env:OPENPHOTO_AI_BUNDLE_DIRECTORY=Join-Path $installDirectory 'bundled-ai'
    $env:OPENPHOTO_AI_ACCEPTANCE_OUTPUT=Join-Path $ReportDirectory 'ai-inference.json'
    Push-Location $projectRoot
    try {
        & cargo test --release --manifest-path src-tauri/Cargo.toml --locked ai_models::tests::bundled_ai_models_acceptance -- --exact --ignored --nocapture --test-threads=1
        if ($LASTEXITCODE -ne 0) {throw 'Installed AI inference acceptance failed'}
    } finally {Pop-Location}
} finally {$env:OPENPHOTO_AI_BUNDLE_DIRECTORY=$previousBundle;$env:OPENPHOTO_AI_ACCEPTANCE_OUTPUT=$previousReport}
$dataRoot=Join-Path ([Environment]::GetFolderPath('LocalApplicationData')) 'Open-Photo'
New-Item -ItemType Directory -Force -Path $dataRoot | Out-Null
$marker=[guid]::NewGuid().ToString('N')
$databaseFiles=@('user-data.sqlite','media-index.sqlite','video-projects.sqlite','ai-embeddings.sqlite','ai-faces.sqlite','ai-ocr.sqlite')
$ownedFiles=@()
foreach ($database in $databaseFiles) {
    foreach ($suffix in @('','-wal','-shm','-journal')) {
        $file=Join-Path $dataRoot ($database+$suffix)
        if (Test-Path -LiteralPath $file) {throw "Refusing to overwrite existing data during installer acceptance: $file"}
        [IO.File]::WriteAllText($file,$marker);$ownedFiles+=$file
    }
}
foreach ($directory in @('ai-models','previews-v1')) {
    $path=Join-Path $dataRoot $directory
    New-Item -ItemType Directory -Force -Path $path | Out-Null
    $file=Join-Path $path 'installer-acceptance.tmp';[IO.File]::WriteAllText($file,$marker);$ownedFiles+=$file
}
$browserFiles=@()
foreach ($base in @([Environment]::GetFolderPath('LocalApplicationData'),[Environment]::GetFolderPath('ApplicationData'))) {
    $path=Join-Path $base 'com.openphoto.desktop'
    New-Item -ItemType Directory -Force -Path $path | Out-Null
    $file=Join-Path $path 'installer-acceptance.tmp';[IO.File]::WriteAllText($file,$marker);$browserFiles+=$file
}
# This unknown media file inside the old app-data parent must survive every cleanup.
$media=Join-Path $dataRoot 'keep-original-installer-acceptance.mp4';[IO.File]::WriteAllText($media,$marker)
function Assert-Kept {
    foreach ($file in $ownedFiles+$browserFiles+@($media)) {if (!(Test-Path -LiteralPath $file) -or [IO.File]::ReadAllText($file) -ne $marker) {throw "Retained-data acceptance failed: $file"}}
}
Run-Uninstaller @('/UPDATE','/PURGEAPPDATA=1');Assert-Kept
Run-Installer
Run-Uninstaller @();Assert-Kept
Run-Installer
Run-Uninstaller @('/PURGEAPPDATA=1')
foreach ($file in $ownedFiles+$browserFiles) {if (Test-Path -LiteralPath $file) {throw "Explicit app-data cleanup left a managed file: $file"}}
if (!(Test-Path -LiteralPath $media) -or [IO.File]::ReadAllText($media) -ne $marker) {throw 'Original-media preservation acceptance failed'}
[ordered]@{app_version=(Get-Content (Join-Path $projectRoot 'package.json') -Raw|ConvertFrom-Json).version;models_installed_and_verified=$manifest.models.Count;update_retains_data=$true;default_uninstall_retains_data=$true;explicit_cleanup_removes_managed_data=$true;original_media_preserved=$true;scope='Disposable GitHub-hosted Windows installer lifecycle; no GUI playback or AI accuracy claim'} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $ReportDirectory 'installer-lifecycle.json') -Encoding utf8
Write-Output "Installed AI and uninstall lifecycle acceptance passed: $ReportDirectory"

$ErrorActionPreference='Stop'
$env:PSModulePath=(Join-Path $PSHOME 'Modules')+';'+[Environment]::GetEnvironmentVariable('PSModulePath','Machine')
$projectRoot=Split-Path $PSScriptRoot
$profile=if ($env:TAURI_ENV_DEBUG -eq 'true') {'debug'} else {'release'}
$targetRoot=if ($env:CARGO_TARGET_DIR) {[IO.Path]::GetFullPath($env:CARGO_TARGET_DIR)} else {Join-Path $projectRoot 'src-tauri/target'}
$binaryRoot=Join-Path $targetRoot $profile
if ($env:TAURI_ENV_TARGET_TRIPLE -and (Test-Path -LiteralPath (Join-Path $targetRoot "$env:TAURI_ENV_TARGET_TRIPLE/$profile/open-photo.exe"))) {$binaryRoot=Join-Path $targetRoot "$env:TAURI_ENV_TARGET_TRIPLE/$profile"}
$directml=Join-Path $binaryRoot 'DirectML.dll'
if (!(Test-Path -LiteralPath $directml)) {throw 'The compiled ONNX DirectML dependency is missing; build the Windows app before bundling.'}
# This is the x64 redistributable linked by the pinned ort-sys release.
if ((Get-FileHash -LiteralPath $directml -Algorithm SHA256).Hash -ne '9C9E6D822561C6C41B90E6994B3E8857CF1D66DBFB1E0C4C799C7C89B4E92DA1') {throw 'Unexpected DirectML runtime; review its identity and redistribution terms before updating the bundle.'}
$vswhere=Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
$installations=& $vswhere -all -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if ($LASTEXITCODE -ne 0) {throw 'Cannot locate the Microsoft C++ redistributable.'}
$crt=$null
foreach ($installation in $installations) {
    $candidates=Get-ChildItem -Path (Join-Path $installation 'VC/Redist/MSVC/*/x64/Microsoft.VC*.CRT') -Directory -ErrorAction SilentlyContinue | Where-Object {$_.FullName -notmatch '[\\/]onecore[\\/]'} | Sort-Object FullName -Descending
    if ($candidates) {$crt=$candidates[0].FullName;break}
}
if (!$crt) {throw 'Install the Microsoft C++ x64 redistributable build components before packaging.'}
$destination=Join-Path $projectRoot 'src-tauri/resources/native'
New-Item -ItemType Directory -Force -Path $destination | Out-Null
# Only this generated runtime directory is managed by this script.
Get-ChildItem -LiteralPath $destination -File -Filter '*.dll' | Remove-Item -Force
$sources=@(Get-Item -LiteralPath $directml)+@(Get-ChildItem -LiteralPath $crt -File -Filter '*.dll')
$entries=@()
foreach ($source in $sources) {
    Copy-Item -LiteralPath $source.FullName -Destination (Join-Path $destination $source.Name) -Force
    $entries+=@{name=$source.Name;sha256=(Get-FileHash -LiteralPath $source.FullName -Algorithm SHA256).Hash.ToLowerInvariant();version=$source.VersionInfo.FileVersion}
}
foreach ($required in @('DirectML.dll','msvcp140.dll','msvcp140_1.dll','vcruntime140.dll','vcruntime140_1.dll')) {if ($required -notin $entries.name) {throw "Missing runtime dependency: $required"}}
@{files=$entries} | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $destination 'native-runtime.json') -Encoding utf8
Copy-Item -LiteralPath (Join-Path $projectRoot 'third_party/ai/LICENSE-DirectML.txt') -Destination $destination -Force
Write-Output "Prepared application-local Windows runtime files: $($entries.Count)"

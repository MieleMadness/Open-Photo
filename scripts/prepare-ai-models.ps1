$ErrorActionPreference='Stop'
$env:PSModulePath=(Join-Path $PSHOME 'Modules')+';'+[Environment]::GetEnvironmentVariable('PSModulePath','Machine')
$ProgressPreference='SilentlyContinue'
$projectRoot=Split-Path $PSScriptRoot
$manifestPath=Join-Path $projectRoot 'config/ai-models.json'
$manifest=Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$destinationRoot=Join-Path $projectRoot 'src-tauri/resources/ai-models'
$cacheRoot=Join-Path $projectRoot 'src-tauri/target/ai-model-cache'
New-Item -ItemType Directory -Force -Path $destinationRoot,$cacheRoot | Out-Null
foreach ($model in $manifest.models) {
    if ($model.sha256 -notmatch '^[a-f0-9]{64}$' -or [IO.Path]::GetFileName($model.name) -ne $model.name) {throw 'Invalid AI bundle manifest'}
    $destination=Join-Path $destinationRoot $model.name
    if ((Test-Path -LiteralPath $destination) -and (Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash.ToLowerInvariant() -eq $model.sha256) {continue}
    $cached=Join-Path $cacheRoot $model.sha256
    if (!(Test-Path -LiteralPath $cached) -or (Get-FileHash -LiteralPath $cached -Algorithm SHA256).Hash.ToLowerInvariant() -ne $model.sha256) {
        $temporary=Join-Path $cacheRoot ($model.sha256+'.'+[guid]::NewGuid().ToString('N')+'.tmp')
        try {
            Write-Output "Preparing bundled AI asset: $($model.kind)"
            Invoke-WebRequest -Uri $model.url -OutFile $temporary -UseBasicParsing
            if ((Get-FileHash -LiteralPath $temporary -Algorithm SHA256).Hash.ToLowerInvariant() -ne $model.sha256) {throw "AI asset checksum mismatch: $($model.kind)"}
            Move-Item -LiteralPath $temporary -Destination $cached -Force
        } finally {if (Test-Path -LiteralPath $temporary) {Remove-Item -LiteralPath $temporary -Force}}
    }
    Copy-Item -LiteralPath $cached -Destination $destination -Force
    if ((Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash.ToLowerInvariant() -ne $model.sha256) {throw "Bundled AI copy failed verification: $($model.kind)"}
}
Copy-Item -LiteralPath $manifestPath -Destination (Join-Path $destinationRoot 'manifest.json') -Force
$notices=Join-Path $projectRoot 'third_party/ai'
if (!(Test-Path -LiteralPath $notices)) {throw 'AI redistribution notices are missing'}
foreach ($name in @('NOTICE.md','LICENSE-OpenCLIP.txt','LICENSE-YuNet.txt','LICENSE-SFace.txt','LICENSE-PaddleOCR.txt','LICENSE-RapidOCR.txt','MODEL-CARD-OpenCLIP.md')) {if (!(Test-Path -LiteralPath (Join-Path $notices $name))) {throw "Missing AI notice: $name"}}
Copy-Item -Path (Join-Path $notices '*') -Destination $destinationRoot -Force
Write-Output "Verified bundled AI assets: $($manifest.models.Count)"

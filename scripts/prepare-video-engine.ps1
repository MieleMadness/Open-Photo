$ErrorActionPreference = 'Stop'
# npm may inherit PowerShell 7's module paths when launching Windows PowerShell.
$env:PSModulePath = (Join-Path $PSHOME 'Modules') + ';' + [Environment]::GetEnvironmentVariable('PSModulePath','Machine')
$projectRoot = Split-Path $PSScriptRoot
$engineRoot = Join-Path $projectRoot 'src-tauri/resources/video-engine'
$cacheRoot = Join-Path $projectRoot 'src-tauri/target/ve'
$version = 'n8.1.3-24-g92611900bb'
$digest = '6c6167da57decb941a0466b60d95f7f51e4edd6b2d79c24dc4860c1c15701292'
$url = 'https://github.com/BtbN/FFmpeg-Builds/releases/download/autobuild-2026-10-27-18-38/ffmpeg-n8.1.3-24-g92611900bb-win64-lgpl-shared-8.1.zip'
New-Item -ItemType Directory -Force -Path $cacheRoot,$engineRoot | Out-Null
$archive = Join-Path $cacheRoot "$digest.zip"
if (!(Test-Path -LiteralPath $archive)) {
    Invoke-WebRequest -Uri $url -OutFile $archive
}
if ((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant() -ne $digest) {
    throw 'Video engine archive checksum mismatch. Remove the cached archive and retry.'
}
$expanded = Join-Path $cacheRoot $digest.Substring(0,12)
$complete = Join-Path $expanded '.complete'
if (!(Test-Path -LiteralPath $complete) -or (Get-Content -LiteralPath $complete -Raw).Trim() -ne $digest) {
    Expand-Archive -LiteralPath $archive -DestinationPath $expanded -Force
    [IO.File]::WriteAllText($complete, $digest, [Text.UTF8Encoding]::new($false))
}
$package = Get-ChildItem -LiteralPath $expanded -Directory | Select-Object -First 1
if (!$package) { throw 'Video engine package is empty' }
foreach ($name in @('ffmpeg.exe','ffprobe.exe')) {
    Copy-Item -LiteralPath (Join-Path $package.FullName "bin/$name") -Destination $engineRoot -Force
}
Get-ChildItem -LiteralPath (Join-Path $package.FullName 'bin') -Filter '*.dll' | Copy-Item -Destination $engineRoot -Force
Get-ChildItem -LiteralPath $package.FullName -Filter 'LICENSE*' | Copy-Item -Destination $engineRoot -Force
$files = Get-ChildItem -LiteralPath $engineRoot -File | Where-Object { $_.Extension -in @('.exe','.dll') } | ForEach-Object {
    @{name=$_.Name;sha256=(Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant()}
}
$manifest = @{version=$version;archive_sha256=$digest;source=$url;files=@($files)} | ConvertTo-Json -Depth 5 | Out-String
[IO.File]::WriteAllText((Join-Path $engineRoot 'engine.json'), $manifest, [Text.UTF8Encoding]::new($false))
$license = & (Join-Path $engineRoot 'ffmpeg.exe') -hide_banner -L 2>&1 | Out-String
if ($LASTEXITCODE -ne 0 -or $license -notmatch 'Lesser General Public License') { throw 'Expected LGPL video engine' }
[IO.File]::WriteAllText((Join-Path $engineRoot 'LICENSE-FFmpeg.txt'), $license, [Text.UTF8Encoding]::new($false))
Copy-Item -LiteralPath (Join-Path $projectRoot 'docs/VIDEO_ENGINE.md') -Destination (Join-Path $engineRoot 'SOURCE-AND-NOTICES.md') -Force
Write-Output "Verified video engine: $version"

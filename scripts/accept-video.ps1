param(
    [string]$OutputDirectory,
    [string]$MachineLabel='Local Windows build environment',
    [string]$Source,
    [switch]$LongRecording
)
$ErrorActionPreference='Stop'
$projectRoot=Split-Path $PSScriptRoot
if ($LongRecording -and !$Source) {throw 'Choose a source file for long-recording acceptance.'}
if ($Source) {
    $sourceFile=Get-Item -LiteralPath $Source
    if ($sourceFile.PSIsContainer) {throw 'Source must be a video file.'}
    $Source=$sourceFile.FullName
}
if (!$OutputDirectory) {$OutputDirectory=Join-Path $projectRoot ('src-tauri/target/video-acceptance-'+[guid]::NewGuid().ToString('N'))}
$OutputDirectory=[IO.Path]::GetFullPath($OutputDirectory)
# A fresh directory prevents accidentally presenting an older successful report.
New-Item -ItemType Directory -Path $OutputDirectory | Out-Null
$previousOutput=$env:OPENPHOTO_ACCEPTANCE_OUTPUT
$previousSource=$env:OPENPHOTO_REAL_SOURCE
Push-Location $projectRoot
try {
    $env:OPENPHOTO_ACCEPTANCE_OUTPUT=Join-Path $OutputDirectory 'video-export.json'
    $env:OPENPHOTO_REAL_SOURCE=$Source
    $testName=if ($LongRecording) {'video_integration_tests::video_long_recording_acceptance'} elseif ($Source) {'video_integration_tests::video_real_media_acceptance'} else {'video_integration_tests::video_release_acceptance'}
    & cargo test --release --manifest-path src-tauri/Cargo.toml --locked $testName -- --exact --ignored --nocapture --test-threads=1
    if ($LASTEXITCODE -ne 0) {throw 'Release video acceptance failed; no successful report is claimed.'}
    if (!(Test-Path -LiteralPath $env:OPENPHOTO_ACCEPTANCE_OUTPUT)) {throw 'Acceptance did not produce its report.'}
    # Read only the requested value; enumerating the key can encounter unsupported registry values.
    $cpu=([Microsoft.Win32.Registry]::GetValue('HKEY_LOCAL_MACHINE\HARDWARE\DESCRIPTION\System\CentralProcessor\0','ProcessorNameString',$null)).Trim()
    [ordered]@{
        schema_version=1
        machine_label=$MachineLabel
        cpu=$cpu
        logical_processors=[Environment]::ProcessorCount
        os=[Runtime.InteropServices.RuntimeInformation]::OSDescription
        architecture=[Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
        recorded_utc=[DateTime]::UtcNow.ToString('o')
        note=if ($Source) {'Real-footage native export check; excludes GUI and whole-app performance.'} else {'Synthetic native-pipeline baseline; no pass/fail speed or memory budget, no GUI measurements.'}
    } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $OutputDirectory 'environment.json') -Encoding utf8
    Write-Host "Release video acceptance reports: $OutputDirectory"
} finally {
    $env:OPENPHOTO_ACCEPTANCE_OUTPUT=$previousOutput
    $env:OPENPHOTO_REAL_SOURCE=$previousSource
    Pop-Location
}

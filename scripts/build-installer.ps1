# Copyright (c) 2026 GPT-6
# Licensed under the MIT License. See LICENSE file in the project root for full license information.

param(
    [Parameter(Mandatory)][string]$Payload,
    [Parameter(Mandatory)][string]$Version,
    [Parameter(Mandatory)][string]$Output
)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot
$tools = Join-Path $root 'target/inno-6.7.3'
$null = New-Item -ItemType Directory -Force $tools

function Get-PinnedFile($path, $hash, $urls) {
    if ((Test-Path -LiteralPath $path) -and (Get-FileHash -LiteralPath $path).Hash -eq $hash) { return }
    foreach ($url in $urls) {
        try {
            Invoke-WebRequest $url -OutFile $path -TimeoutSec 45
            if ((Get-FileHash -LiteralPath $path).Hash -eq $hash) { return }
            Write-Warning "Checksum mismatch for $url"
        } catch { Write-Warning $_ }
    }
    throw "Cannot obtain verified $path"
}

$compiler = Join-Path $tools 'compiler/ISCC.exe'
if (!(Test-Path -LiteralPath $compiler)) {
    $asset = 'jrsoftware/issrc/releases/download/is-6_7_3/innosetup-6.7.3.exe'
    $bootstrap = Join-Path $tools 'innosetup-6.7.3.exe'
    Get-PinnedFile $bootstrap '9c73c3bae7ed48d44112a0f48e66742c00090bdb5bef71d9d3c056c66e97b732' @(
        'https://mirrors.huaweicloud.com/github-release/jrsoftware/issrc/is-6_7_3/innosetup-6.7.3.exe',
        "https://github.com/$asset"
    )
    $dir = Split-Path $compiler
    $process = Start-Process -FilePath $bootstrap -ArgumentList (
        '/VERYSILENT /SUPPRESSMSGBOXES /NORESTART /CURRENTUSER /DIR="{0}"' -f $dir
    ) -WindowStyle Hidden -Wait -PassThru
    if ($process.ExitCode) { throw "Inno Setup bootstrap exited $($process.ExitCode)" }
}
$messages = Join-Path $tools 'ChineseSimplified.isl'
Get-PinnedFile $messages '7d544b9bb1d142cfa11f2e5d3cc8abe2e55f8e066c5124e3772675aa236e1278' @(
    'https://raw.githubusercontent.com/jrsoftware/issrc/is-6_7_3/Files/Languages/Unofficial/ChineseSimplified.isl'
)
$profileScript = Get-Content -Raw -LiteralPath (Join-Path $root 'installer/uninstall-profile.ps1')
function EncodedCommand($purge) {
    $body = '$purgeProfile = $' + $purge + [Environment]::NewLine + $profileScript
    [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($body))
}
$settings = Join-Path $tools 'settings.iss'
@(
    '#define Payload "' + (Resolve-Path -LiteralPath $Payload).Path + '"'
    '#define PackageVersion "' + $Version + '"'
    '#define ChineseMessages "' + $messages + '"'
    '#define KeepCommand "' + (EncodedCommand 'false') + '"'
    '#define PurgeCommand "' + (EncodedCommand 'true') + '"'
) | Set-Content -LiteralPath $settings -Encoding utf8BOM
$outputDir = Split-Path ([IO.Path]::GetFullPath($Output))
$null = New-Item -ItemType Directory -Force $outputDir
& $compiler "/DGeneratedSettings=$settings" "/O$outputDir" ("/F" + [IO.Path]::GetFileNameWithoutExtension($Output)) (Join-Path $root 'installer/ALTRunNext.iss')
if ($LASTEXITCODE) { throw "ISCC exited $LASTEXITCODE" }

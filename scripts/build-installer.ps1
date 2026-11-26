# Copyright (c) 2026 GPT-6
# Licensed under the MIT License. See LICENSE file in the project root for full license information.

param(
    [Parameter(Mandatory)][string]$Payload,
    [Parameter(Mandatory)][string]$Version,
    [Parameter(Mandatory)][string]$Output
)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot
$tools = Join-Path $root 'target/nsis-tools'
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

$release = 'nsis-3.13'
$archive = Join-Path $tools "$release.zip"
Get-PinnedFile $archive 'ba63dffc4410ee89193e1cb5a41989991bd77c61068da17e3156d136b7b0b3d8' @(
    "https://mirrors.nju.edu.cn/sourceforge/n/ns/nsis/NSIS%203/3.13/$release.zip",
    "https://downloads.sourceforge.net/project/nsis/NSIS%203/3.13/$release.zip",
    "https://github.com/tauri-apps/binary-releases/releases/download/$release/$release.zip"
)
$env:NSISDIR = Join-Path $tools $release
$compiler = Join-Path $env:NSISDIR 'Bin/makensis.exe'
if (!(Test-Path -LiteralPath $compiler)) { Expand-Archive -LiteralPath $archive -DestinationPath $tools -Force }
if ((& $compiler /VERSION) -notmatch '^v3\.13(?:\.0)?$') { throw 'Expected NSIS 3.13' }
$payloadDir = (Resolve-Path -LiteralPath $Payload).Path
$packageFiles = @('ALTRunNext.exe','CHANGELOG.md','LICENSE','Popup.wav','THIRD-PARTY-NOTICES.txt') + @(
    Get-ChildItem -LiteralPath (Join-Path $payloadDir 'languages') -Filter '*.json' -File |
        Sort-Object Name | ForEach-Object { 'languages\' + $_.Name }
)
if ($packageFiles.Count -le 5) { throw 'Missing packaged language resources' }
$packageFiles | Set-Content -LiteralPath "$tools/files.txt" -Encoding ascii
Add-Type -AssemblyName System.Drawing
$header = [Drawing.Bitmap]::new(150, 57)
$graphics = [Drawing.Graphics]::FromImage($header)
$icon = [Drawing.Icon]::new((Join-Path $root 'assets/app.ico'), 48, 48)
try {
    $graphics.Clear([Drawing.Color]::White)
    $graphics.DrawIcon($icon, [Drawing.Rectangle]::new(101, 8, 40, 40))
    $header.Save("$tools/header.bmp", [Drawing.Imaging.ImageFormat]::Bmp)
} finally { $icon.Dispose(); $graphics.Dispose(); $header.Dispose() }
$destination = [IO.Path]::GetFullPath($Output)
$null = New-Item -ItemType Directory -Force (Split-Path $destination)
& $compiler /V3 /INPUTCHARSET UTF8 "/DPAYLOAD=$payloadDir" "/DVERSION=$Version" "/DOUTPUT=$destination" "/DGENERATED=$tools" (Join-Path $root 'installer/ALTRunNext.nsi')
if ($LASTEXITCODE) { throw "makensis exited $LASTEXITCODE" }

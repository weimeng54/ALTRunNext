# Copyright (c) 2026 GPT-6
# Licensed under the MIT License. See LICENSE file in the project root for full license information.

param(
    [Parameter(Mandatory)][string]$Payload,
    [Parameter(Mandatory)][string]$Version,
    [Parameter(Mandatory)][string]$Output
)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot
$tools = Join-Path $root 'target/nsis-3.11'
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

$compiler = Join-Path $tools 'compiler/makensis.exe'
if (!(Test-Path -LiteralPath $compiler)) {
    $preinstalled = Join-Path ${env:ProgramFiles(x86)} 'NSIS/makensis.exe'
    if ((Test-Path -LiteralPath $preinstalled) -and (& $preinstalled /VERSION) -eq 'v3.11') {
        $compiler = $preinstalled
    } else {
        $asset = 'project/nsis/NSIS%203/3.11/nsis-3.11-setup.exe'
        $bootstrap = Join-Path $tools 'nsis-3.11-setup.exe'
        Get-PinnedFile $bootstrap '38d49f8fe09b1c332b01d0940e57b7258f4447733643273a01c59959ad9d3b0a' @(
            "https://mirrors.nju.edu.cn/sourceforge/n/ns/nsis/NSIS%203/3.11/nsis-3.11-setup.exe",
            "https://downloads.sourceforge.net/$asset"
        )
        # NSIS requires /D last, with its complete path unquoted.
        $process = Start-Process -FilePath $bootstrap -ArgumentList ("/S /D=" + (Split-Path $compiler)) -WindowStyle Hidden -Wait -PassThru
        if ($process.ExitCode) { throw "NSIS bootstrap exited $($process.ExitCode)" }
    }
}
if ((& $compiler /VERSION) -ne 'v3.11') { throw 'Expected NSIS 3.11' }
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

# Copyright (c) 2026 GPT-6
# Licensed under the MIT License. See LICENSE file in the project root for full license information.

param([bool]$PurgeProfile = $false)
$ErrorActionPreference = 'Stop'
function Remove-Profile {
    param([int]$Purge)
    $ErrorActionPreference = 'Stop'
    $local = [Environment]::GetFolderPath('LocalApplicationData')
    $profileDir = Join-Path $local 'ALTRunNext'
    $data = Join-Path $profileDir 'shortcuts.tsv'
    $registration = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\ALTRunNext_is1'
    $installDir = Get-ItemPropertyValue -LiteralPath $registration -Name InstallLocation
    $exe = Join-Path $installDir 'ALTRunNext.exe'

    $run = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey(
        'Software\Microsoft\Windows\CurrentVersion\Run', $true)
    if ($run) {
        try {
            foreach ($name in $run.GetValueNames()) {
                if ($name -match '^ALTRunNext-[0-9a-f]{16}$' -and
                    $run.GetValue($name) -eq ('"{0}" --enable-launch --data "{1}"' -f $exe, $data)) {
                    $run.DeleteValue($name)
                }
            }
        } finally { $run.Dispose() }
    }
    foreach ($name in 'ALTRunNext.lnk','ALTRunNext (installed).lnk') {
        $linkPath = Join-Path ([Environment]::GetFolderPath('SendTo')) $name
        if (![IO.File]::Exists($linkPath)) { continue }
        $link = (New-Object -ComObject WScript.Shell).CreateShortcut($linkPath)
        if ($link.TargetPath -eq $exe -and $link.Arguments -eq ('--enable-launch --data "{0}" --add' -f $data)) {
            [IO.File]::Delete($linkPath)
        }
    }
    if (!$Purge -or ![IO.Directory]::Exists($profileDir)) { return }

    function Remove-OwnedFiles([string]$directory, [string]$files) {
        if ([IO.File]::GetAttributes($directory) -band [IO.FileAttributes]::ReparsePoint) { return }
        foreach ($file in [IO.Directory]::EnumerateFiles($directory)) {
            if ([IO.Path]::GetFileName($file) -match $files) { [IO.File]::Delete($file) }
        }
        if (![IO.Directory]::GetFileSystemEntries($directory).Length) {
            [IO.Directory]::Delete($directory)
        }
    }

    if ([IO.File]::GetAttributes($profileDir) -band [IO.FileAttributes]::ReparsePoint) { return }
    $id = 'item-[0-9a-f]+-[0-9a-f]+-[0-9a-f]+'
    foreach ($directory in [IO.Directory]::GetDirectories($profileDir)) {
        $name = [IO.Path]::GetFileName($directory)
        if ($name -match "^altrun-media-$id$") {
            Remove-OwnedFiles $directory '^(background\.bin|notify\.wav)$'
        } elseif ($name -eq 'shortcuts.tsv.restore' -or $name -match "^\.altrun-restore-finished-$id$") {
            Remove-OwnedFiles $directory '^((old|absent|pending|next)-[0-2]|prepared|committed|media)$'
        }
    }
    $owned = "^shortcuts\.(tsv|settings\.ini|state\.tsv)(\.bak)?$|^ALTRunNext-before-restore-$id\.zip$"
    Remove-OwnedFiles $profileDir ($owned + "|^\.altrun-write-$id$|^shortcuts\.(stage-$id|altrun-tmp-\d+-\d+)$")
}

$result = Invoke-DesktopUser ${function:Remove-Profile} ([int]$PurgeProfile)
if ($result -ne 0) { throw "Profile cleanup failed: $result" }

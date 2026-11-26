# Copyright (c) 2026 GPT-6
# Licensed under the MIT License. See LICENSE file in the project root for full license information.

param([bool]$PurgeProfile = $false)
$ErrorActionPreference = 'Stop'
function Remove-Profile([bool]$Purge) {
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
    $linkPath = Join-Path ([Environment]::GetFolderPath('SendTo')) 'ALTRunNext.lnk'
    if ([IO.File]::Exists($linkPath)) {
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

# UAC may have been approved with another account. Run per-user work with the
# desktop user's limited token, before removing the machine registration.
$session = [Diagnostics.Process]::GetCurrentProcess().SessionId
if ($session -gt 0) {
    Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class DesktopSession {
    [DllImport("wtsapi32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    static extern bool WTSQuerySessionInformationW(IntPtr server, int session, int field, out IntPtr buffer, out int bytes);
    [DllImport("wtsapi32.dll")] static extern void WTSFreeMemory(IntPtr buffer);
    public static string Read(int session, int field) {
        IntPtr buffer; int bytes;
        if (!WTSQuerySessionInformationW(IntPtr.Zero, session, field, out buffer, out bytes))
            throw new System.ComponentModel.Win32Exception();
        try { return Marshal.PtrToStringUni(buffer); } finally { WTSFreeMemory(buffer); }
    }
}
'@
    $user = [DesktopSession]::Read($session, 5)
    $domain = [DesktopSession]::Read($session, 7)
    if (!$user -or !$domain) { throw 'Cannot identify the interactive user' }
    $account = ([Security.Principal.NTAccount]::new($domain, $user)).Translate(
        [Security.Principal.SecurityIdentifier]).Value
    $logon = 'Interactive'
} else {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    if ($identity.IsSystem -or $identity.User.Value -in 'S-1-5-19','S-1-5-20') { return }
    $account = $identity.User.Value
    $logon = 'S4U'
}
$worker = '& {' + (Get-Command Remove-Profile).Definition + '} $' + $purgeProfile.ToString()
$encoded = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($worker))
$action = New-ScheduledTaskAction -Execute "$PSHOME\powershell.exe" -Argument (
    '-NoProfile -NonInteractive -WindowStyle Hidden -EncodedCommand ' + $encoded) -WorkingDirectory $PSHOME
$principal = New-ScheduledTaskPrincipal -UserId $account -LogonType $logon -RunLevel Limited
$settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries
$name = 'ALTRunNext-Uninstall-' + [Guid]::NewGuid().ToString('N')
Register-ScheduledTask -TaskName $name -Action $action -Principal $principal -Settings $settings | Out-Null
try {
    Start-ScheduledTask -TaskName $name
    $timer = [Diagnostics.Stopwatch]::StartNew()
    do {
        Start-Sleep -Milliseconds 200
        $info = Get-ScheduledTaskInfo -TaskName $name
        $state = (Get-ScheduledTask -TaskName $name).State
        if ($timer.Elapsed.TotalSeconds -gt 45) { throw 'Profile cleanup timed out' }
    } until ($state -eq 'Ready' -and $info.LastRunTime.Year -gt 2000)
    if ($info.LastTaskResult -ne 0) { throw "Profile cleanup failed: $($info.LastTaskResult)" }
} finally {
    Unregister-ScheduledTask -TaskName $name -Confirm:$false
}

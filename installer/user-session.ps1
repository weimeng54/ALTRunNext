# Copyright (c) 2026 GPT-6
# Licensed under the MIT License. See LICENSE file in the project root for full license information.

function Invoke-DesktopUser([scriptblock]$Operation, [int]$Options) {
    # UAC may have been approved with another account. Run per-user work with the
    # desktop user's limited token, for setup and uninstall.
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
        if ($identity.IsSystem -or $identity.User.Value -in 'S-1-5-19','S-1-5-20') { return 0 }
        $account = $identity.User.Value
        $logon = 'S4U'
    }
    $worker = '& {' + $Operation.ToString() + '} ' + $Options
    $encoded = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($worker))
    $action = New-ScheduledTaskAction -Execute "$PSHOME\powershell.exe" -Argument (
        '-NoProfile -NonInteractive -WindowStyle Hidden -EncodedCommand ' + $encoded) -WorkingDirectory $PSHOME
    $principal = New-ScheduledTaskPrincipal -UserId $account -LogonType $logon -RunLevel Limited
    $settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries
    $name = 'ALTRunNext-Setup-' + [Guid]::NewGuid().ToString('N')
    Register-ScheduledTask -TaskName $name -Action $action -Principal $principal -Settings $settings | Out-Null
    try {
        Start-ScheduledTask -TaskName $name
        $timer = [Diagnostics.Stopwatch]::StartNew()
        do {
            Start-Sleep -Milliseconds 200
            $info = Get-ScheduledTaskInfo -TaskName $name
            $state = (Get-ScheduledTask -TaskName $name).State
            if ($timer.Elapsed.TotalSeconds -gt 45) { throw 'User operation timed out' }
        } until ($state -eq 'Ready' -and $info.LastRunTime.Year -gt 2000)
        return $info.LastTaskResult
    } finally {
        Unregister-ScheduledTask -TaskName $name -Confirm:$false
    }
}

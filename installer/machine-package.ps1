# Copyright (c) 2026 GPT-6
# Licensed under the MIT License. See LICENSE file in the project root for full license information.

param(
    [ValidateSet('Install','Remove','Open')][string]$Mode,
    [Parameter(Mandatory)][string]$Directory,
    [Parameter(Mandatory)][string]$Version,
    [ValidateSet(0,1)][int]$Purge = 0,
    [ValidateRange(0,31)][int]$Options = 14
)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'user-session.ps1')
$install = $Mode -eq 'Install'
$dir = [IO.Path]::GetFullPath($Directory).TrimEnd('\')
$arp = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\ALTRunNext_is1'
$exe = Join-Path $dir 'ALTRunNext.exe'
$marker = Join-Path $dir 'ALTRunNext.installed'
$uninstaller = Join-Path $dir 'Uninstall.exe'
$startLink = Join-Path ([Environment]::GetFolderPath('CommonPrograms')) 'ALTRunNext.lnk'
$desktopLink = Join-Path ([Environment]::GetFolderPath('CommonDesktopDirectory')) 'ALTRunNext.lnk'
$payload = Join-Path $PSScriptRoot 'payload'
$changes = [Collections.Generic.List[object]]::new()
$transaction = [Guid]::NewGuid().ToString('N')

function Assert-PlainPath([string]$path) {
    while ($path) {
        $item = Get-Item -LiteralPath $path -Force -ErrorAction SilentlyContinue
        if ($item -and ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
            throw "Cannot modify a redirected path: $path"
        }
        $path = [IO.Path]::GetDirectoryName($path)
    }
}

# Keep the previous file on its own volume until files, shortcut and registration agree.
function Update-File([string]$path, [string]$source) {
    Assert-PlainPath $path
    $exists = [IO.File]::Exists($path)
    $previous = $null
    if ($exists) { $previous = "$path.$transaction.old" }
    if ($source) {
        $stage = "$path.$transaction.new"
        try {
            [IO.File]::Copy($source, $stage)
            if ($exists) { [IO.File]::Replace($stage, $path, $previous) }
            else { [IO.File]::Move($stage, $path) }
        } finally { [IO.File]::Delete($stage) }
    } elseif ($exists) {
        [IO.File]::Move($path, $previous)
    } else { return }
    $changes.Add(@{Path=$path; Previous=$previous})
}

function Remove-EmptyDirectory([string]$path) {
    if ([IO.Directory]::Exists($path) -and ![IO.Directory]::GetFileSystemEntries($path).Length) {
        [IO.Directory]::Delete($path)
    }
}

function Update-Shortcut([string]$path, [bool]$create) {
    $shell = New-Object -ComObject WScript.Shell
    if ([IO.File]::Exists($path)) {
        if ($shell.CreateShortcut($path).TargetPath -ne $exe) {
            if ($create) { throw "Another shortcut already uses $path" }
            return
        }
    }
    if (!$create) { Update-File $path; return }
    $stage = Join-Path $PSScriptRoot 'ALTRunNext.lnk'
    $link = $shell.CreateShortcut($stage)
    $link.TargetPath = $exe
    $link.Arguments = '--show'
    $link.WorkingDirectory = $dir
    $link.Save()
    Update-File $path $stage
}

# The interactive user's token owns the Run value, Send to menu and launcher.
function Invoke-InstalledApp {
    param([int]$action)
    $ErrorActionPreference = 'Stop'
    $key = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\ALTRunNext_is1'
    $app = Join-Path (Get-ItemPropertyValue -LiteralPath $key -Name InstallLocation) 'ALTRunNext.exe'
    if ($action -eq 16) {
        Start-Process -FilePath $app -ArgumentList '--show'
    } else {
        $process = Start-Process -FilePath $app -ArgumentList "--configure-installed-integrations $action" -PassThru -Wait
        exit $process.ExitCode
    }
}

if ($Mode -eq 'Open') { exit (Invoke-DesktopUser ${function:Invoke-InstalledApp} 16) }

$mutex = [Threading.Mutex]::new($false, 'Global\ALTRunNext.Package')
try { $locked = $mutex.WaitOne(0) }
catch [Threading.AbandonedMutexException] { $locked = $true }
if (!$locked) { $mutex.Dispose(); exit 2 }
try {
    $registered = Get-ItemProperty -LiteralPath $arp -ErrorAction SilentlyContinue
    if ($dir.Length -le 3 -or ![IO.Path]::IsPathRooted($dir)) { exit 3 }
    Assert-PlainPath $dir
    if ($registered) {
        if (!$registered.InstallLocation -or $dir -ne $registered.InstallLocation.TrimEnd('\') -or
            ![IO.File]::Exists($marker)) { exit 3 }
    } elseif (!$install -or ([IO.Directory]::Exists($dir) -and [IO.Directory]::GetFileSystemEntries($dir).Length)) {
        exit 3
    }
    if ([IO.File]::Exists($exe)) {
        try { [IO.File]::Open($exe,'Open','ReadWrite','None').Dispose() }
        catch [IO.IOException] { exit 2 }
    }
    $files = @(Get-Content -LiteralPath (Join-Path $PSScriptRoot 'files.txt')) + @('ALTRunNext.installed','Uninstall.exe')
    $languageDir = Join-Path $dir 'languages'
    Assert-PlainPath $languageDir
    $legacy = $registered -and $registered.UninstallString -and
        $registered.UninstallString.Trim('"') -eq (Join-Path $dir 'unins000.exe')
    if ($legacy -and [IO.File]::Exists($uninstaller)) { exit 3 }
    try {
        if ($install) {
            $null = [IO.Directory]::CreateDirectory($languageDir)
            foreach ($file in $files) { Update-File (Join-Path $dir $file) (Join-Path $payload $file) }
            if (!$registered) {
                if ($Options -band 1) { Update-Shortcut $startLink $true }
                if ($Options -band 2) { Update-Shortcut $desktopLink $true }
            }
            if ($legacy) {
                foreach ($ext in 'exe','dat','msg') { Update-File (Join-Path $dir "unins000.$ext") }
            }
            $values = @{
                DisplayName='ALTRunNext'; DisplayVersion=$Version; Publisher='weimeng54'
                InstallLocation=$dir; DisplayIcon=$exe; UninstallString=('"'+$uninstaller+'"')
                QuietUninstallString=('"'+$uninstaller+'" /S'); URLInfoAbout='https://github.com/weimeng54/ALTRunNext'
                NoRepair=1; NoModify=1
            }
            if (!$registered) { $null = New-Item -Path $arp }
            foreach ($name in $values.Keys) { Set-ItemProperty -LiteralPath $arp -Name $name -Value $values[$name] }
            if (!$registered) {
                $integrations = ($Options -shr 2) -band 3
                $result = Invoke-DesktopUser ${function:Invoke-InstalledApp} $integrations
                if ($result -ne 0) { throw "Could not configure Windows integrations: $result" }
            }
        } else {
            foreach ($file in $files) { Update-File (Join-Path $dir $file) }
            Update-Shortcut $startLink $false
            Update-Shortcut $desktopLink $false
            & (Join-Path $PSScriptRoot 'uninstall-profile.ps1') -PurgeProfile ([bool]$Purge)
            Remove-Item -LiteralPath $arp
        }
    } catch {
        $failure = $_
        if ($install -and $values -and (Test-Path -LiteralPath $arp)) {
            if (!$registered) { Remove-Item -LiteralPath $arp }
            else {
                foreach ($name in $values.Keys) {
                    if ($registered.PSObject.Properties[$name]) {
                        Set-ItemProperty -LiteralPath $arp -Name $name -Value $registered.$name
                    } else { Remove-ItemProperty -LiteralPath $arp -Name $name -ErrorAction SilentlyContinue }
                }
            }
        }
        for ($i = $changes.Count - 1; $i -ge 0; $i--) {
            $change = $changes[$i]
            [IO.File]::Delete($change.Path)
            if ($change.Previous) { [IO.File]::Move($change.Previous, $change.Path) }
        }
        if (!$registered) { Remove-EmptyDirectory $languageDir; Remove-EmptyDirectory $dir }
        throw $failure
    }
    foreach ($change in $changes) { if ($change.Previous) { [IO.File]::Delete($change.Previous) } }
    if (!$install) { Remove-EmptyDirectory $languageDir; Remove-EmptyDirectory $dir }
} finally {
    $mutex.ReleaseMutex()
    $mutex.Dispose()
}

; Copyright (c) 2026 GPT-6
; Licensed under the MIT License. See LICENSE file in the project root for full license information.

#include GeneratedSettings

[Setup]
AppId=ALTRunNext
AppName=ALTRunNext
AppVersion={#PackageVersion}
AppPublisher=weimeng54
AppPublisherURL=https://github.com/weimeng54/ALTRunNext
DefaultDirName={autopf}\ALTRunNext
UsePreviousAppDir=yes
DisableDirPage=no
PrivilegesRequired=admin
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
DisableProgramGroupPage=yes
UninstallDisplayIcon={app}\ALTRunNext.exe
SetupIconFile=..\assets\app.ico
WizardStyle=modern
CloseApplications=no
RestartApplications=no
Compression=lzma2
SolidCompression=yes
OutputBaseFilename=ALTRunNext-Setup

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "chinesesimplified"; MessagesFile: "{#ChineseMessages}"

[Files]
Source: "{#Payload}\ALTRunNext.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#Payload}\languages\*.json"; DestDir: "{app}\languages"; Flags: ignoreversion
Source: "{#Payload}\Popup.wav"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#Payload}\CHANGELOG.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#Payload}\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#Payload}\THIRD-PARTY-NOTICES.txt"; DestDir: "{app}"; Flags: ignoreversion
Source: "ALTRunNext.installed"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{commonprograms}\ALTRunNext"; Filename: "{app}\ALTRunNext.exe"; Parameters: "--show"

[CustomMessages]
english.Occupied=Choose an empty folder for the first installation. To move your portable data, use Backup and restore in Settings.
chinesesimplified.Occupied=首次安装请选择空文件夹。如需迁移便携版数据，请使用设置中的“备份与恢复”。
english.Previous=Upgrade in the existing installation directory. To move the program, uninstall it first and keep your profile.
chinesesimplified.Previous=请在原安装目录升级。如需移动程序，请先卸载并保留个人数据。
english.CloseApp=Save your changes and exit ALTRunNext from the tray, then try again.
chinesesimplified.CloseApp=请保存修改，从托盘退出 ALTRunNext，然后重试。
english.Purge=Also delete this user's installed launch items, settings and history
chinesesimplified.Purge=同时删除当前用户安装版的启动项、设置和使用历史
english.Profile=Uninstall ALTRunNext
chinesesimplified.Profile=卸载 ALTRunNext
english.Remove=Uninstall
chinesesimplified.Remove=卸载
english.CleanupError=Could not clean up this user's ALTRunNext profile or shell shortcuts. Close the app and retry.
chinesesimplified.CleanupError=无法清理当前用户的安装版数据或系统快捷方式。请退出程序后重试。

[Code]
function OpenForUpdate(Name: String; Access, Share, Security, Disposition, Flags, Template: LongWord): Integer;
  external 'CreateFileW@kernel32.dll stdcall';
function CloseHandle(Handle: Integer): Boolean;
  external 'CloseHandle@kernel32.dll stdcall';

function Busy: Boolean;
var
  Handle: Integer;
  Exe: String;
begin
  Exe := ExpandConstant('{app}\ALTRunNext.exe');
  Result := FileExists(Exe);
  if Result then begin
    Handle := OpenForUpdate(Exe, $40000000, 0, 0, 3, 0, 0);
    Result := Handle < 0;
    if not Result then CloseHandle(Handle);
  end;
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
var
  Dir, Previous: String;
  Entry: TFindRec;
begin
  Dir := ExpandConstant('{app}');
  if RegQueryStringValue(HKLM64,
    'Software\Microsoft\Windows\CurrentVersion\Uninstall\ALTRunNext_is1',
    'InstallLocation', Previous) then begin
    if CompareText(RemoveBackslashUnlessRoot(Previous), RemoveBackslashUnlessRoot(Dir)) <> 0 then
      Result := CustomMessage('Previous')
    else if Busy then Result := CustomMessage('CloseApp');
  end else if FindFirst(AddBackslash(Dir) + '*', Entry) then begin
    try
      repeat
        if (Entry.Name <> '.') and (Entry.Name <> '..') then begin
          Result := CustomMessage('Occupied');
          Break;
        end;
      until not FindNext(Entry);
    finally
      FindClose(Entry);
    end;
  end;
end;

function ConfirmProfile(var Purge: Boolean): Boolean;
var
  Form: TSetupForm;
  Check: TNewCheckBox;
  Remove, Cancel: TNewButton;
begin
  Form := CreateCustomForm(416, 102, False, False);
  try
    Form.Caption := CustomMessage('Profile');
    Check := TNewCheckBox.Create(Form);
    Check.Parent := Form;
    Check.SetBounds(ScaleX(16), ScaleY(14), Form.ClientWidth - ScaleX(32), ScaleY(30));
    Check.WordWrap := True;
    Check.Caption := CustomMessage('Purge');
    Remove := TNewButton.Create(Form);
    Remove.Parent := Form;
    Remove.SetBounds(Form.ClientWidth - ScaleX(192), ScaleY(62), ScaleX(82), ScaleY(26));
    Remove.Caption := CustomMessage('Remove');
    Remove.ModalResult := mrOk;
    Remove.Default := True;
    Cancel := TNewButton.Create(Form);
    Cancel.Parent := Form;
    Cancel.SetBounds(Form.ClientWidth - ScaleX(98), Remove.Top, Remove.Width, Remove.Height);
    Cancel.Caption := SetupMessage(msgButtonCancel);
    Cancel.ModalResult := mrCancel;
    Cancel.Cancel := True;
    Result := Form.ShowModal = mrOk;
    Purge := Check.Checked;
  finally
    Form.Free;
  end;
end;

var
  PurgeProfile: Boolean;

function InitializeUninstall: Boolean;
var
  I: Integer;
begin
  Result := not Busy;
  if not Result then begin
    SuppressibleMsgBox(CustomMessage('CloseApp'), mbError, MB_OK, IDOK);
    Exit;
  end;
  if UninstallSilent then
    for I := 1 to ParamCount do
      if CompareText(ParamStr(I), '/PURGEUSERDATA') = 0 then PurgeProfile := True;
end;

procedure InitializeUninstallProgressForm;
begin
  if not UninstallSilent then
    if not ConfirmProfile(PurgeProfile) then Abort;
end;

procedure CurUninstallStepChanged(Step: TUninstallStep);
var
  Code: Integer;
  Args: String;
begin
  if Step <> usUninstall then Exit;
  Args := '-NoLogo -NoProfile -NonInteractive -EncodedCommand ';
  if PurgeProfile then Args := Args + '{#PurgeCommand}'
  else Args := Args + '{#KeepCommand}';
  if not Exec(ExpandConstant('{sys}\WindowsPowerShell\v1.0\powershell.exe'), Args,
    ExpandConstant('{sys}'), SW_HIDE, ewWaitUntilTerminated, Code) or (Code <> 0) then
    RaiseException(CustomMessage('CleanupError'));
end;

; Copyright (c) 2026 GPT-6
; Licensed under the MIT License. See LICENSE file in the project root for full license information.

Unicode true
ManifestDPIAware true
RequestExecutionLevel admin
SetCompressor /SOLID lzma
!include MUI2.nsh
!include nsDialogs.nsh
!include x64.nsh
!include FileFunc.nsh

!define ARP "Software\Microsoft\Windows\CurrentVersion\Uninstall\ALTRunNext_is1"
Name "ALTRunNext ${VERSION}"
OutFile "${OUTPUT}"
InstallDir ""
BrandingText "ALTRunNext"
VIProductVersion "${VERSION}.0"
VIAddVersionKey /LANG=1033 "ProductName" "ALTRunNext"
VIAddVersionKey /LANG=1033 "FileDescription" "ALTRunNext Setup"
VIAddVersionKey /LANG=1033 "FileVersion" "${VERSION}"
VIAddVersionKey /LANG=1033 "CompanyName" "weimeng54"
VIAddVersionKey /LANG=1033 "LegalCopyright" "Copyright (c) 2026 ALTRunNext contributors"
!define MUI_ICON "..\assets\app.ico"
!define MUI_UNICON "..\assets\app.ico"
!define MUI_HEADERIMAGE
!define MUI_HEADERIMAGE_RIGHT
!define MUI_HEADERIMAGE_BITMAP "${GENERATED}\header.bmp"
!define MUI_HEADERIMAGE_UNBITMAP "${GENERATED}\header.bmp"
!insertmacro MUI_PAGE_DIRECTORY
Page custom OptionsPage ReadOptions
!insertmacro MUI_PAGE_INSTFILES
UninstPage custom un.ProfilePage un.ReadProfile
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"
!insertmacro MUI_LANGUAGE "SimpChinese"

LangString ProfileTitle ${LANG_ENGLISH} "Uninstall ALTRunNext"
LangString ProfileTitle ${LANG_SIMPCHINESE} "卸载 ALTRunNext"
LangString ProfileText ${LANG_ENGLISH} "Your launch items, settings and history are kept by default."
LangString ProfileText ${LANG_SIMPCHINESE} "默认保留你的启动项、设置和使用历史。"
LangString PurgeText ${LANG_ENGLISH} "Also delete this user's installed launch items, settings and history"
LangString PurgeText ${LANG_SIMPCHINESE} "同时删除当前用户安装版的启动项、设置和使用历史"
LangString LocationText ${LANG_ENGLISH} "Installation folder"
LangString LocationText ${LANG_SIMPCHINESE} "安装位置"
LangString Busy ${LANG_ENGLISH} "Save your changes, exit ALTRunNext from the tray, and try again."
LangString Busy ${LANG_SIMPCHINESE} "请保存修改，从托盘退出 ALTRunNext 后重试。"
LangString Destination ${LANG_ENGLISH} "Choose an empty folder for a first install or use the previously registered installation folder to upgrade."
LangString Destination ${LANG_SIMPCHINESE} "首次安装请选择空文件夹。升级时请使用原安装目录。"
LangString Failed ${LANG_ENGLISH} "Could not complete the operation."
LangString Failed ${LANG_SIMPCHINESE} "操作未能完成。"

!include "install-options.nsh"
Var Purge
Var Check
Function .onInit
    StrCpy $Options 14
    ReadRegStr $PreviousInstall HKLM64 "${ARP}" "InstallLocation"
    ${If} $INSTDIR == ""
        StrCpy $INSTDIR $PreviousInstall
        ${If} $INSTDIR == ""
            StrCpy $INSTDIR "$PROGRAMFILES64\ALTRunNext"
        ${EndIf}
    ${EndIf}
FunctionEnd
Function un.ProfilePage
    !insertmacro MUI_HEADER_TEXT "$(ProfileTitle)" "$(ProfileText)"
    nsDialogs::Create 1018
    Pop $0
    ${If} $0 == error
        Abort
    ${EndIf}
    ${NSD_CreateLabel} 0 4u 100% 12u "$(LocationText)"
    Pop $0
    ${NSD_CreateLabel} 0 20u 100% 28u "$INSTDIR"
    Pop $0
    ${NSD_CreateCheckbox} 0 62u 100% 24u "$(PurgeText)"
    Pop $Check
    ${NSD_SetState} $Check $Purge
    nsDialogs::Show
FunctionEnd
Function un.ReadProfile
    ${NSD_GetState} $Check $Purge
FunctionEnd
Function un.onInit
    StrCpy $Options 0
    StrCpy $Purge 0
    ${If} ${Silent}
        ${GetParameters} $0
        ClearErrors
        ${GetOptions} $0 "/PURGEUSERDATA" $1
        IfErrors +2
        StrCpy $Purge 1
    ${EndIf}
FunctionEnd

!macro ApplyPackage Mode
    SetOutPath "$PLUGINSDIR"
    File "machine-package.ps1"
    File "user-session.ps1"
    File "${GENERATED}\files.txt"
    nsExec::ExecToStack '"$WINDIR\Sysnative\WindowsPowerShell\v1.0\powershell.exe" -NoLogo -NoProfile -NonInteractive -ExecutionPolicy RemoteSigned -File "$PLUGINSDIR\machine-package.ps1" -Mode ${Mode} -Directory "$INSTDIR\." -Version "${VERSION}" -Purge $Purge -Options $Options'
    Pop $0
    Pop $1
    ${If} $0 != 0
        DetailPrint "$1"
        ${If} $0 == 2
            StrCpy $1 "$(Busy)"
        ${ElseIf} $0 == 3
            StrCpy $1 "$(Destination)"
        ${Else}
            StrCpy $1 "$(Failed)$\r$\n$1"
        ${EndIf}
        MessageBox MB_OK|MB_ICONSTOP "$1" /SD IDOK
        SetErrorLevel 1
        Abort
    ${EndIf}
!macroend

Function .onInstSuccess
    IntOp $0 $Options & 16
    ${If} $0 != 0
    ${AndIfNot} ${Silent}
        nsExec::ExecToStack '"$WINDIR\Sysnative\WindowsPowerShell\v1.0\powershell.exe" -NoLogo -NoProfile -NonInteractive -ExecutionPolicy RemoteSigned -File "$PLUGINSDIR\machine-package.ps1" -Mode Open -Directory "$INSTDIR\." -Version "${VERSION}"'
        Pop $0
        Pop $1
        ${If} $0 != 0
            MessageBox MB_OK|MB_ICONEXCLAMATION "$(Failed)$\r$\n$1"
        ${EndIf}
    ${EndIf}
FunctionEnd

Section "Install"
    InitPluginsDir
    StrCpy $Purge 0
    SetOutPath "$PLUGINSDIR\payload"
    File "${PAYLOAD}\ALTRunNext.exe"
    File "${PAYLOAD}\Popup.wav"
    File "${PAYLOAD}\CHANGELOG.md"
    File "${PAYLOAD}\LICENSE"
    File "${PAYLOAD}\THIRD-PARTY-NOTICES.txt"
    File "ALTRunNext.installed"
    WriteUninstaller "$PLUGINSDIR\payload\Uninstall.exe"
    SetOutPath "$PLUGINSDIR\payload\languages"
    File "${PAYLOAD}\languages\*.json"
    !insertmacro ApplyPackage Install
SectionEnd

Section "Uninstall"
    InitPluginsDir
    SetOutPath "$PLUGINSDIR"
    File "uninstall-profile.ps1"
    !insertmacro ApplyPackage Remove
SectionEnd

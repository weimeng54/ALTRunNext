; Copyright (c) 2026 GPT-6
; Licensed under the MIT License. See LICENSE file in the project root for full license information.

LangString OptionsTitle ${LANG_ENGLISH} "Installation options"
LangString OptionsTitle ${LANG_SIMPCHINESE} "安装选项"
LangString OptionsText ${LANG_ENGLISH} "Choose shortcuts and startup options."
LangString OptionsText ${LANG_SIMPCHINESE} "选择快捷方式和启动选项。"
LangString UpgradeText ${LANG_ENGLISH} "This upgrade keeps your launch items, settings, history and existing Windows integrations."
LangString UpgradeText ${LANG_SIMPCHINESE} "升级将保留你的启动项、设置、使用历史，以及现有快捷方式、开机启动和“发送到”菜单设置。"
LangString StartShortcut ${LANG_ENGLISH} "Create a Start menu shortcut"
LangString StartShortcut ${LANG_SIMPCHINESE} "创建开始菜单快捷方式"
LangString DesktopShortcut ${LANG_ENGLISH} "Create a desktop shortcut"
LangString DesktopShortcut ${LANG_SIMPCHINESE} "创建桌面快捷方式"
LangString SignIn ${LANG_ENGLISH} "Start ALTRunNext when I sign in to Windows"
LangString SignIn ${LANG_SIMPCHINESE} "登录 Windows 时启动 ALTRunNext"
LangString SendTo ${LANG_ENGLISH} "Add ALTRunNext to the Send to menu"
LangString SendTo ${LANG_SIMPCHINESE} "在“发送到”菜单中添加 ALTRunNext"
LangString OpenAfter ${LANG_ENGLISH} "Open ALTRunNext after installation"
LangString OpenAfter ${LANG_SIMPCHINESE} "安装完成后打开 ALTRunNext"

Var Options
Var PreviousInstall
Var StartCheck
Var DesktopCheck
Var SignInCheck
Var SendToCheck
Var OpenCheck

!macro OptionRow Handle Text Y Bit
    ${NSD_CreateCheckbox} 0 ${Y} 100% 14u "$(${Text})"
    Pop $${Handle}
    IntOp $0 $Options & ${Bit}
    ${If} $0 != 0
        ${NSD_Check} $${Handle}
    ${EndIf}
!macroend

!macro ReadOption Handle Bit
    ${NSD_GetState} $${Handle} $0
    ${If} $0 == ${BST_CHECKED}
        IntOp $Options $Options | ${Bit}
    ${EndIf}
!macroend

Function OptionsPage
    !insertmacro MUI_HEADER_TEXT "$(OptionsTitle)" "$(OptionsText)"
    nsDialogs::Create 1018
    Pop $0
    ${If} $0 == error
        Abort
    ${EndIf}
    ${If} $PreviousInstall == ""
        !insertmacro OptionRow StartCheck StartShortcut 4u 1
        !insertmacro OptionRow DesktopCheck DesktopShortcut 26u 2
        !insertmacro OptionRow SignInCheck SignIn 48u 4
        !insertmacro OptionRow SendToCheck SendTo 70u 8
        !insertmacro OptionRow OpenCheck OpenAfter 92u 16
    ${Else}
        ${NSD_CreateLabel} 0 4u 100% 32u "$(UpgradeText)"
        Pop $0
        !insertmacro OptionRow OpenCheck OpenAfter 44u 16
    ${EndIf}
    GetFunctionAddress $0 ReadOptions
    nsDialogs::OnBack $0
    nsDialogs::Show
FunctionEnd

Function ReadOptions
    StrCpy $Options 0
    ${If} $PreviousInstall == ""
        !insertmacro ReadOption StartCheck 1
        !insertmacro ReadOption DesktopCheck 2
        !insertmacro ReadOption SignInCheck 4
        !insertmacro ReadOption SendToCheck 8
    ${EndIf}
    !insertmacro ReadOption OpenCheck 16
FunctionEnd

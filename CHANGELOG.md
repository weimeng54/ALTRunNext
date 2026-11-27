# 更新日志 / Changelog

记录 ALTRunNext 各版本的主要变更。 / Notable changes to ALTRunNext, organized by release.

格式参考 [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)。 / The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [未发布 / Unreleased]

### 新增 / Added

- 新增 Windows 安装版。可选择快捷方式、登录时启动和“发送到”菜单。覆盖升级沿用安装目录并保留启动项、设置和历史，卸载时可选择清除个人数据。\
  Added a Windows installer with options for shortcuts, launch at sign-in and Send to integration. Upgrades reuse the install location and preserve launch items, settings and history. Uninstall offers an option to remove personal data.
- 在「字体与颜色」中可单独设置菜单字体和字号。\
  Menu font and size can be set independently under Fonts and Colors.
- 新增繁體中文界面。\
  Added Traditional Chinese.

### 变更 / Changed

- 搜索时启动面板保持固定高度。没有匹配项时，按 Enter 可用当前输入新建快捷项；切换到其他应用时，启动面板自动收起。\
  The launcher stays the same height as you search. Press Enter when there are no matches to create an item with the typed alias. Switching to another app hides the launcher.
- 托盘、右键和按钮菜单统一使用主题配色和紧凑字号。深色弹窗使用略亮的底色和细边，在多层窗口中更易辨认。\
  Tray, context and button menus use the app theme and a compact text size. Dark dialogs use a lighter surface and a subtle outline so they stand apart from the window beneath them.
- 设置更改会自动写入配置文件，写入成功后生效。主题和显示语言会同步更新已打开的窗口；数值输入按 Enter 或离开输入框后生效。\
  Settings save automatically and take effect after a successful write. Open windows follow theme and display language changes. Press Enter or leave a numeric field to apply its value.
- 从托盘退出时会等待设置写入完成，并舍弃启动项管理窗口中未保存的更改。\
  Exiting from the tray waits for pending settings writes and discards unsaved changes in Manage Launch Items.

### 修复 / Fixed

- 导出的启动项统一保存为 `.tsv`。导入时可选择「所有文件」，直接打开以前导出的无扩展名或自定义扩展名文件。\
  Launch item exports now use the `.tsv` extension. Select All files when importing older exports with a custom extension or no extension.
- 修复重新呼出启动面板时，标题、输入框和结果列表短暂闪白的问题。\
  Fixed a white flash in the title, search box and results when reopening the launcher.
- 修复清空搜索框时启动结果列表短暂闪烁的问题。\
  Fixed a flicker when clearing the search box restored the full result list.

## [0.5.0] - 2026-11-24

### 新增 / Added

- 启动项可保存「以管理员身份运行」。启动面板右键菜单也可仅为本次启动请求管理员权限。\
  Save administrator mode per launch item or request it for one launch from the launcher context menu.

### 变更 / Changed

- 管理启动项和设置面板支持按 Esc 关闭。有未保存的修改时会先确认。\
  Press Esc to close Manage launch items or Settings, with confirmation when there are unsaved changes.

- 「发送到」添加启动项弹窗更紧凑，快捷项与名称并排，目标路径保留整行空间。\
  The Send to dialog places the alias and name side by side and gives the target path a full-width field.

- 参数弹窗使用统一的圆角输入框，程序的帮助、错误和确认弹框跟随当前主题。\
  Parameter prompts use the shared rounded input, and application help, error, and confirmation dialogs follow the active theme.

- 统一启动面板、管理窗口、导入和其他对话框的滚动条样式。鼠标移入滚动区域时，滑块加宽并显示淡色轨道。\
  Scrollbars throughout the app share the launcher’s style, with a wider thumb and a subtle track on hover.

- 「发送到」与启动项管理统一使用「测试打开」。\
  Send to and Manage launch items use the same label for testing a launch item.

### 修复 / Fixed

- 下拉列表使用主题底色标记当前选项，去掉选项内的虚线框。\
  Dropdowns highlight the current option with the theme selection color and remove the dotted outline inside the row.

## [0.4.7] - 2026-11-24

### 新增 / Added

- 外观设置可分别开关启动结果的竖线和横线，默认显示竖线。支持实时预览和自定义分隔线颜色。\
  Toggle vertical and horizontal result dividers independently, preview them live and customize their color. Vertical dividers are enabled by default.

### 变更 / Changed

- 打开外观设置时，启动面板预览直接显示多条示例结果。\
  The launcher preview in Appearance opens with several sample results.

- 启动面板的管理、设置和隐藏按钮使用线宽与视觉大小统一的图标。\
  The launcher’s Manage, Settings, and Hide buttons use icons with matching strokes and visual weight.

### 修复 / Fixed

- 鼠标移入启动面板右侧滚动区域时，滑块加宽并显示轨道。滑块设有最小高度，整条滚动区域支持点击跳转，可从滑块两侧留白处开始拖动。\
  The launcher widens the scrollbar thumb and shows the track when the pointer enters the scroll area. The thumb has a minimum height, and the full scroll area supports jumping to a position and grabbing the thumb from either side.

- 备份与恢复对话框的键盘焦点提示改为贴近复选框和单选按钮的细环。\
  Backup and restore dialogs show a compact keyboard focus ring around checkboxes and radio buttons.

## [0.4.6] - 2026-11-23

### 变更 / Changed

- 设置底栏统一提供「备份与恢复」。可分别选择启动项、程序设置和使用记录，恢复启动项默认追加并跳过冲突，也可替换当前清单。此前导出的 ALTRunNext 设置文件可从同一入口恢复。\
  Settings has a single Backup and restore menu. Choose launch items, program settings, or usage history. Restoring launch items appends and skips conflicts by default, with a replace option. Previously exported ALTRunNext settings files can be restored from the same menu.
- 启动面板左上角显示新的蓝色图标，托盘、程序文件和设置页也采用同一图标。\
  The launcher title shows the new blue icon, which also appears in the tray, executable, and settings.

## [0.4.5] - 2026-11-18

### 修复 / Fixed

- 缩短点击「保存并应用」后的等待时间。\
  Reduced the wait after clicking Save and Apply in Settings.
- 修复「运行」启动项无法执行输入的命令。需要输入参数的启动项使用独立弹窗，取消后回到原搜索结果。\
  Fixed the Run item failing to execute the entered command. Launch items that need input now open a separate prompt. Cancel returns to the original results.
- 修复在命令、目标路径等输入框中拖选长文本时，文字和选区短暂消失的问题。\
  Fixed text and selection briefly disappearing when dragging across long commands and paths in edit fields.

### 变更 / Changed

- 新生成的启动项列表预置「运行」（`r`）。参数弹窗沿用搜索框的英文输入设置。\
  New launch item lists include Run (`r`). The parameter prompt uses the same English input preference as the search field.
- Windows 可执行文件的详细信息显示产品名 ALTRunNext、发布者 weimeng54 和版权。\
  Windows executable details now show the ALTRunNext product name, publisher weimeng54, and copyright.

## [0.4.4] - 2026-11-15

### 新增 / Added

- 启动面板支持在结果列表中直接按 Del，或通过右键菜单「删除命令」确认删除所选启动项。\
  Delete the selected launch item directly from the launcher with Del or the Delete command context menu.

### 变更 / Changed

- 浏览器收藏夹导入预览按需显示表格行，减少大列表打开、筛选和清空筛选时的开销。\
  The browser bookmark import preview loads table rows on demand to speed up opening, filtering, and clearing large lists.

## [0.4.3] - 2026-11-14

### 变更 / Changed

- Windows 便携 ZIP 随附本文件。\
  The Windows portable ZIP includes this changelog.

## [0.4.2] - 2026-11-14

### 变更 / Changed

- 启动项较多时，搜索、清空输入和重新呼出面板更快，内存占用更低。\
  With large lists, searching, clearing the query and reopening the launcher are faster and use less memory.
- 启动命令后，面板可在使用记录保存期间继续响应。\
  The launcher stays responsive while saving usage history after a successful launch.
- 导入浏览器收藏夹时，筛选预览结果更快。\
  Filtering the browser bookmark import preview is faster.

### 修复 / Fixed

- 用 Delete 删除启动项时，确认或取消后键盘焦点回到列表，可以连续按 Delete。\
  The launch item list regains keyboard focus after confirming or canceling a deletion, so Delete can be used repeatedly.

## [0.4.1] - 2026-11-14

### 变更 / Changed

- 确认弹框支持 Enter 执行主操作、Esc 取消。导入预览、失效路径修复、「发送到」添加窗口和快捷键冲突提示也支持默认键盘操作。\
  Enter runs the primary action in confirmation dialogs and Esc cancels. Import previews, missing-path repair, the Send to dialog, and the hotkey conflict warning also support default keyboard actions.
- 启动面板右键菜单中的「隐藏」显示 Esc 提示。\
  The launcher context menu shows Esc as the shortcut for Hide.

### 移除 / Removed

- 从启动面板右键菜单移除「关于」。产品信息可在设置中查看。\
  Removed About from the launcher context menu. Product information is available in Settings.

### 修复 / Fixed

- 减少切换设置页时的闪烁，并固定底部按钮区域。\
  Reduced flicker when switching Settings pages by keeping the footer controls in place.

## [0.4.0] - 2026-11-14

### 新增 / Added

- 管理启动项支持按快捷项、名称和路径即时筛选，可在检查结果中直接修复失效路径，同时保留启动参数和使用历史。\
  Added live filtering by alias, name, or path and direct repair from the missing-path report, preserving launch arguments and usage history.
- 支持从浏览器导出的 HTML 导入收藏夹。预览可筛选、批量勾选和编辑快捷项，并标明重复或不支持的网址。\
  Added bookmark import from browser-exported HTML, with filtering, bulk selection, alias editing, and explanations for skipped URLs.
- 支持用本地 ZIP 备份与恢复启动项、设置、使用历史、自定义背景和声音。应用恢复前会自动备份当前数据，完成后重启。\
  Added local ZIP backup and restore for launch items, settings, usage history, custom backgrounds, and sounds. Restore backs up current data before applying changes and restarts the app afterward.

### 变更 / Changed

- 导入启动项时先预览，再选择追加或替换。追加会标出并跳过冲突项，点击「保存」后生效。\
  Launch item imports can be previewed, then appended or used to replace the list. Append mode identifies and skips conflicts. Changes take effect when saved.
- 精简设置页说明，并将保存提示与底部按钮对齐。\
  Simplified Settings copy and aligned save feedback with the footer buttons.

### 修复 / Fixed

- 修复浅色与深色主题下的启动面板滚动条，以及收藏夹搜索框和单元格编辑的渲染。\
  Fixed rendering of the launcher scrollbar, bookmark search field, and bookmark cell editor in both themes.

## [0.3.0] - 2026-11-14

### 新增 / Added

- 启动面板支持右键「编辑命令」或按 F2，打开管理窗口并定位到当前启动项。\
  Added Edit command to the launcher's context menu and F2 to open the selected item in the manager.
- 主呼出快捷键被占用时，启动时显示提示，可直接进入快捷键设置。\
  Added a startup warning when the primary hotkey is unavailable, with direct access to hotkey settings.
- 源码采用 MIT 许可证，便携 ZIP 随附许可证及第三方声明。\
  Adopted the MIT License for the source code and included the license and third-party notices in the portable ZIP.

### 变更 / Changed

- 可直接粘贴含空格的程序路径。程序解析目标和启动参数，并保留参数里的引号。导入原版清单时，会移除可安全省略的目标路径外层引号。\
  Program paths with spaces can be pasted directly. The app separates the target from its arguments and preserves quotes within arguments. Imports from the original ALTRun remove redundant quotes around target paths when safe.
- 统一启动器、设置和启动项管理窗口中的界面术语。\
  Standardized terminology across the launcher, Settings and Manage Launch Items.

### 修复 / Fixed

- 修复关闭设置后立即退出时偶发的退出受阻。\
  Fixed an intermittent failure to exit immediately after closing Settings.

## [0.2.1] - 2026-11-13

### 新增 / Added

- 管理列表支持拖动表头调整列宽、双击分隔线按内容适配，以及横向滚动。\
  Added draggable column dividers, double-click to fit, and horizontal scrolling to the manager table.
- 从资源管理器「发送到」添加 Windows 快捷方式时，读取 `.lnk` 的目标，从目标生成快捷项和名称，并保留启动参数。\
  When adding a Windows shortcut through File Explorer's Send to menu, read the `.lnk` target, suggest an alias and name, and preserve its launch arguments.

### 修复 / Fixed

- 「发送到」添加窗口和管理表格正确跟随浅色与深色主题。\
  The Send to dialog and manager table follow the app's light or dark theme.
- 手动新增时，表格和表单中的快捷项与名称均从空白开始，并随输入同步。\
  Manually added items start with a blank alias and name in both the table and form. The table updates as you type.

## [0.2.0] - 2026-11-13

### 新增 / Added

- 首个公开发布的 Windows 11 x64 便携版。默认按 Alt+R 呼出，通过快捷项打开程序、文件、文件夹或网址，支持精确、前缀和模糊匹配。\
  First public portable release for Windows 11 x64. Press Alt+R to open the launcher and use aliases to launch apps, files, folders, or websites with exact, prefix, and fuzzy matching.
- 管理启动项支持从资源管理器「发送到」添加目标，为文档指定打开程序，以及自动识别新版 TSV 和原版 ALTRun 的 `ShortCutList.txt`。设置使用新版 INI 导入和导出。\
  Manage launch items supports adding targets from File Explorer's Send to menu, choosing an app for documents, and detecting both ALTRunNext TSV and the original ALTRun `ShortCutList.txt`. Settings use the ALTRunNext INI format for import and export.
- 首次启动跟随 Windows 显示语言。内置英文和简体中文，便携包附带日语、韩语、法语、德语和西班牙语，并支持外置语言包。\
  The app follows the Windows display language on first launch. English and Simplified Chinese are built in. The portable ZIP includes Japanese, Korean, French, German, and Spanish, and the app supports external language packs.

[未发布 / Unreleased]: https://github.com/weimeng54/ALTRunNext/compare/v0.5.0...main
[0.5.0]: https://github.com/weimeng54/ALTRunNext/compare/v0.4.7...v0.5.0
[0.4.7]: https://github.com/weimeng54/ALTRunNext/compare/v0.4.6...v0.4.7
[0.4.6]: https://github.com/weimeng54/ALTRunNext/compare/v0.4.5...v0.4.6
[0.4.5]: https://github.com/weimeng54/ALTRunNext/compare/v0.4.4...v0.4.5
[0.4.4]: https://github.com/weimeng54/ALTRunNext/compare/v0.4.3...v0.4.4
[0.4.3]: https://github.com/weimeng54/ALTRunNext/compare/v0.4.2...v0.4.3
[0.4.2]: https://github.com/weimeng54/ALTRunNext/compare/v0.4.1...v0.4.2
[0.4.1]: https://github.com/weimeng54/ALTRunNext/compare/v0.4.0...v0.4.1
[0.4.0]: https://github.com/weimeng54/ALTRunNext/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/weimeng54/ALTRunNext/compare/v0.2.1...v0.3.0
[0.2.1]: https://github.com/weimeng54/ALTRunNext/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/weimeng54/ALTRunNext/releases/tag/v0.2.0

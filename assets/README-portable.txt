ALTRun 0.2 — Windows 11 x64 便携版

双击 ALTRun.exe 后按 Alt+R 呼出，输入快捷字，Enter 启动。
呼出时清空输入框并显示全部启用的命令，可以滚动浏览。
搜索优先完整快捷字、前缀和连续包含，再按字符顺序模糊匹配，例如 db 可匹配 doubao。
搜索框默认使用英文直接输入，参数输入和其他窗口仍用原输入法。可在设置的「搜索与执行」中关闭。
正式构建默认允许启动；--safe-mode 为不执行命令的预览模式。

Ctrl+Alt+L / Ctrl+L：最近启动
上下键 / Tab：选择；Esc：清空或隐藏
Ctrl/Alt + 数字：启动对应项
Ctrl+C：复制命令；Ctrl+D：打开目录
Alt+S：管理快捷项；F1：帮助
参数输入时 Ctrl+上下键 或 Alt+下键：参数历史
托盘右键：打开启动面板、管理快捷项、设置、退出
面板右键：复制命令、打开所在文件夹、最近启动

数据与设置均在快捷项文件旁：
shortcuts.tsv / shortcuts.settings.ini / shortcuts.state.tsv
保存会保留上一版 .bak，外部修改冲突时不会强制覆盖。
高级正则在受限进程运行；普通输入和呼出不等待磁盘、网络或图片。

管理窗口的工具菜单可以导出快捷项备份，或把备份载入为待保存草稿。
使用另一个数据文件：ALTRun.exe --data "目录\shortcuts.tsv"
默认呼出键为 Alt+R。可在设置的“快捷键”页按键录制。
在设置的“常规”页开启或取消“添加到‘发送到’”，再保存并应用。
「常规 → 显示语言」可选简体中文或 English。保存后重新打开设置即可看到变化。
「外观」支持浅色、深色和跟随系统。背景图可选择强度与裁切位置，预览可试输快捷字。
「字体与颜色」可以分别调整标题、输入框和结果列表的字体，保存后应用到启动面板。

项目：https://github.com/weimeng54/altrun
依赖许可证见 licenses。

English quick start
Run ALTRun.exe, then press Alt+R to show the launcher. Type a shortcut and press Enter.
Open Settings → General → Display language to select English or Simplified Chinese.
Appearance includes Light, Dark and Follow system themes with a live background preview.
Alt+S opens shortcut management. Open Settings → General to enable or disable File Explorer Send to.

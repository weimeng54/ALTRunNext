# ALTRunNext

[![Latest release](https://img.shields.io/github/v/release/weimeng54/ALTRunNext)](https://github.com/weimeng54/ALTRunNext/releases/latest)
[![Windows build](https://github.com/weimeng54/ALTRunNext/actions/workflows/windows.yml/badge.svg?branch=main)](https://github.com/weimeng54/ALTRunNext/actions/workflows/windows.yml)
[![MIT License](https://img.shields.io/github/license/weimeng54/ALTRunNext)](LICENSE)
[![Rust](https://img.shields.io/badge/language-Rust-dea584?logo=rust)](src/)

[简体中文](#简体中文) · [English](#english) · [更新日志 / Changelog](CHANGELOG.md)

## 简体中文

ALTRunNext 是一款面向 Windows 11（64 位）的轻量快捷启动器。按 **Alt+R** 呼出，输入快捷项，按 **Enter** 打开程序、文件、文件夹或网址。例如，输入 `cl` 可以找到快捷项为 `calc` 的 Windows 计算器。

### 快速开始

1. 从 [GitHub Releases](https://github.com/weimeng54/ALTRunNext/releases) 获取便携 ZIP，解压到有写入权限的文件夹，然后运行 `ALTRunNext.exe`。
2. 按 **Alt+R** 打开启动面板，使用 **↑ / ↓** 选择结果，按 **Enter** 启动，按 **Esc** 收起面板。
3. 右键单击托盘图标，选择「管理启动项」添加项目，或打开「设置」调整快捷键、语言、主题和呼出声音。

试装版也提供 NSIS 安装包。首次默认安装到 `C:\Program Files\ALTRunNext`，覆盖安装沿用此前目录。首次安装默认创建桌面快捷方式、启用登录 Windows 时启动，并添加到「发送到」菜单。开始菜单快捷方式和安装完成后打开可自行勾选。覆盖升级保留现有快捷方式和系统集成选择。更新前请先保存并从托盘退出。安装版将个人数据写入 `%LOCALAPPDATA%\ALTRunNext`，便携版写在 EXE 同级。可通过「备份与恢复」在两者之间迁移。在 Windows「已安装的应用」中卸载时，默认保留个人数据，勾选清除选项可移除当前用户的安装版数据。

### 添加和备份启动项

- 在「设置 → 常规」开启并保存「添加 ALTRunNext 到发送到菜单」。在资源管理器中右键单击目标，选择「显示更多选项 → 发送到 → ALTRunNext」，确认后保存。文件可以使用系统默认应用，也可以为该启动项指定打开程序。在设置里关闭此选项即可移除菜单入口。
- 在管理窗口左侧搜索框按快捷项、名称或路径筛选。「工具 → 检查失效路径」可定位并修复找不到的本地目标。
- 「工具 → 导入启动项」会识别 TSV 或原版 ALTRun 的 `ShortCutList.txt`，可追加或替换。「导入浏览器收藏夹」可从浏览器导出的 HTML 挑选网址。确认后点击「保存」应用。启动项导出为 TSV，设置的导入和导出使用 INI。
- 设置窗口底栏的「备份与恢复」可选择将已保存的启动项、程序设置（含自定义背景和提示音）或使用记录打包为本地 ZIP。恢复启动项默认追加并跳过冲突，也可选择替换当前清单。

### 界面语言

首次启动跟随 Windows 显示语言，也可在「设置 → 常规 → 显示语言」手动切换。英文和简体中文内置，便携包同时附带日语、韩语、法语、德语和西班牙语。将其他 UTF-8 JSON 语言包放在 EXE 同级的 `languages` 文件夹，再重新打开设置即可选用。[语言包格式与翻译说明](languages/README.md)

### 从源码构建

在 Windows 上安装 Rust（MSVC 工具链）、Visual Studio Build Tools 的 C++ 桌面开发组件和 Windows SDK。在仓库根目录运行以下命令，生成的程序位于 `target/x86_64-pc-windows-msvc/release/ALTRunNext.exe`。

```powershell
cargo build --release --locked --target x86_64-pc-windows-msvc --bin ALTRunNext
```

图标源文件是 `assets/app.svg`。修改后安装 `resvg-py==0.5.0`，运行 `python tools/build-app-icon.py` 重新生成多尺寸 `assets/app.ico`。可加 `--png app.png` 导出 256 像素 PNG。常规编译直接使用仓库中的 ICO。

作者 [weimeng54](https://github.com/weimeng54) · [联系邮箱](mailto:weimeng54@qq.com) · [反馈问题](https://github.com/weimeng54/ALTRunNext/issues)

项目代码使用 MIT 许可证。第三方资源和依赖的许可随便携包附带。[MIT 许可证](LICENSE)

## English

ALTRunNext is a lightweight launcher for Windows 11 (64-bit). Press **Alt+R**, type an alias, and press **Enter** to open an app, file, folder, or website. Type `cl` to find Windows Calculator, whose alias is `calc`.

### Getting started

1. Get the portable ZIP from [GitHub Releases](https://github.com/weimeng54/ALTRunNext/releases), extract it to a writable folder, and run `ALTRunNext.exe`.
2. Press **Alt+R** to open the launcher, **↑ / ↓** to select a result, **Enter** to launch it, and **Esc** to hide the panel.
3. Right-click the tray icon to manage launch items or open Settings. Settings include hotkeys, language, theme, and the popup sound.

Trial builds also include an NSIS installer. It defaults to `C:\Program Files\ALTRunNext` and remembers the installation directory for upgrades. On first install, the desktop shortcut, launch at Windows sign-in and Send to integration are selected by default. The Start menu shortcut and opening the app after installation are optional. Upgrades keep existing shortcuts and Windows integrations. Save your work and exit from the tray before updating. The installed app stores personal data in `%LOCALAPPDATA%\ALTRunNext`. Portable copies store data beside the executable. Use **Backup and restore** to transfer data. Uninstall from Windows **Installed apps** to keep your profile by default, or select the checkbox to remove the current user's installed data.

### Adding and backing up launch items

- In **Settings → General**, enable and save the Send To integration. In File Explorer, right-click a target and choose **Show more options → Send to → ALTRunNext**, then review and save the launch item. A file can open in the system default app or an app selected for that launch item. Turn off the setting to remove the menu entry.
- Use the search field in the manager to filter by alias, name, or path. **Tools → Check missing paths** lets you locate and repair missing local targets.
- **Tools → Import launch items** detects TSV or the original ALTRun `ShortCutList.txt` and offers to append or replace. **Import browser bookmarks** lets you select URLs from a browser-exported HTML file. Confirm the import and select **Save** to apply it. Exports use TSV for launch items and INI for settings.
- **Backup and restore** in the Settings footer creates a local ZIP with your choice of saved launch items, program settings (including custom backgrounds and sounds), and usage history. Restoring launch items appends and skips conflicts by default, with an option to replace the list.

### Display language

ALTRunNext follows the Windows display language on first launch. You can choose another language under **Settings → General → Display language**. English and Simplified Chinese are built in. The portable package also includes Japanese, Korean, French, German, and Spanish. Add other UTF-8 JSON packs to the `languages` folder next to the executable and reopen Settings. [Language pack format and translation guide](languages/README.md)

### Building from source

On Windows, install Rust with the MSVC toolchain, Visual Studio Build Tools with Desktop development with C++, and the Windows SDK. Run the following command from the repository root. The executable is written to `target/x86_64-pc-windows-msvc/release/ALTRunNext.exe`.

```powershell
cargo build --release --locked --target x86_64-pc-windows-msvc --bin ALTRunNext
```

The icon source is `assets/app.svg`. After editing it, install `resvg-py==0.5.0` and run `python tools/build-app-icon.py` to rebuild the multi-size `assets/app.ico`. Pass `--png app.png` to also export a 256 px PNG. Regular builds use the checked-in ICO.

Maintained by [weimeng54](https://github.com/weimeng54) · [Email](mailto:weimeng54@qq.com) · [Report an issue](https://github.com/weimeng54/ALTRunNext/issues)

The project code is licensed under MIT. The portable ZIP includes the notices for third-party resources and dependencies. [MIT License](LICENSE)

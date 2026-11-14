# ALTRunNext

[![Latest release](https://img.shields.io/github/v/release/weimeng54/ALTRunNext)](https://github.com/weimeng54/ALTRunNext/releases/latest)
[![Windows build](https://github.com/weimeng54/ALTRunNext/actions/workflows/windows.yml/badge.svg?branch=main)](https://github.com/weimeng54/ALTRunNext/actions/workflows/windows.yml)
[![MIT License](https://img.shields.io/github/license/weimeng54/ALTRunNext)](LICENSE)
[![Rust](https://img.shields.io/badge/language-Rust-dea584?logo=rust)](src/)

[简体中文](#简体中文) · [English](#english)

## 简体中文

ALTRunNext 是一款面向 Windows 11（64 位）的轻量快捷启动器。按 **Alt+R** 呼出，输入快捷项，按 **Enter** 打开程序、文件、文件夹或网址。例如，输入 `cl` 可以找到快捷项为 `calc` 的 Windows 计算器。

### 快速开始

1. 从 [GitHub Releases](https://github.com/weimeng54/ALTRunNext/releases) 获取便携 ZIP，解压到有写入权限的文件夹，然后运行 `ALTRunNext.exe`。
2. 按 **Alt+R** 打开启动面板，使用 **↑ / ↓** 选择结果，按 **Enter** 启动，按 **Esc** 收起面板。
3. 右键单击托盘图标，选择「管理启动项」添加项目，或打开「设置」调整快捷键、语言、主题和呼出声音。

### 添加和备份启动项

- 在「设置 → 常规」开启并保存「添加 ALTRunNext 到发送到菜单」。在资源管理器中右键单击目标，选择「显示更多选项 → 发送到 → ALTRunNext」，确认后保存。文件可以使用系统默认应用，也可以为该启动项指定打开程序。在设置里关闭此选项即可移除菜单入口。
- 在管理窗口左侧搜索框按快捷项、名称或路径筛选。「工具 → 检查失效路径」可定位并修复找不到的本地目标。
- 「工具 → 导入启动项」会识别 TSV 或原版 ALTRun 的 `ShortCutList.txt`，可追加或替换。「导入浏览器收藏夹」可从浏览器导出的 HTML 挑选网址。确认后点击「保存」应用。启动项导出为 TSV，设置的导入和导出使用 INI。
- 「设置 → 常规 → 备份与恢复」可把已保存的启动项、设置、使用历史和自定义背景及声音打包为本地 ZIP。

### 界面语言

首次启动跟随 Windows 显示语言，也可在「设置 → 常规 → 显示语言」手动切换。英文和简体中文内置，便携包同时附带日语、韩语、法语、德语和西班牙语。将其他 UTF-8 JSON 语言包放在 EXE 同级的 `languages` 文件夹，再重新打开设置即可选用。[语言包格式与翻译说明](languages/README.md)

### 从源码构建

在 Windows 上安装 Rust（MSVC 工具链）、Visual Studio Build Tools 的 C++ 桌面开发组件和 Windows SDK。在仓库根目录运行以下命令，生成的程序位于 `target/x86_64-pc-windows-msvc/release/ALTRunNext.exe`。

```powershell
cargo build --release --locked --target x86_64-pc-windows-msvc --bin ALTRunNext
```

修改图标后，可用安装了 Pillow 的 Python 运行 `python tools/build-app-icon.py`，重新生成 `assets/app.ico`。

作者 [weimeng54](https://github.com/weimeng54) · [联系邮箱](mailto:weimeng54@qq.com) · [反馈问题](https://github.com/weimeng54/ALTRunNext/issues)

项目代码使用 MIT 许可证。第三方资源和依赖的许可随便携包附带。[MIT 许可证](LICENSE)

## English

ALTRunNext is a lightweight launcher for Windows 11 (64-bit). Press **Alt+R**, type an alias, and press **Enter** to open an app, file, folder, or website. Type `cl` to find Windows Calculator, whose alias is `calc`.

### Getting started

1. Get the portable ZIP from [GitHub Releases](https://github.com/weimeng54/ALTRunNext/releases), extract it to a writable folder, and run `ALTRunNext.exe`.
2. Press **Alt+R** to open the launcher, **↑ / ↓** to select a result, **Enter** to launch it, and **Esc** to hide the panel.
3. Right-click the tray icon to manage launch items or open Settings. Settings include hotkeys, language, theme, and the popup sound.

### Adding and backing up launch items

- In **Settings → General**, enable and save the Send To integration. In File Explorer, right-click a target and choose **Show more options → Send to → ALTRunNext**, then review and save the launch item. A file can open in the system default app or an app selected for that launch item. Turn off the setting to remove the menu entry.
- Use the search field in the manager to filter by alias, name, or path. **Tools → Check missing paths** lets you locate and repair missing local targets.
- **Tools → Import launch items** detects TSV or the original ALTRun `ShortCutList.txt` and offers to append or replace. **Import browser bookmarks** lets you select URLs from a browser-exported HTML file. Confirm the import and select **Save** to apply it. Exports use TSV for launch items and INI for settings.
- **Settings → General → Backup and restore** creates a local ZIP with saved launch items, settings, usage history, and custom background and sound files.

### Display language

ALTRunNext follows the Windows display language on first launch. You can choose another language under **Settings → General → Display language**. English and Simplified Chinese are built in. The portable package also includes Japanese, Korean, French, German, and Spanish. Add other UTF-8 JSON packs to the `languages` folder next to the executable and reopen Settings. [Language pack format and translation guide](languages/README.md)

### Building from source

On Windows, install Rust with the MSVC toolchain, Visual Studio Build Tools with Desktop development with C++, and the Windows SDK. Run the following command from the repository root. The executable is written to `target/x86_64-pc-windows-msvc/release/ALTRunNext.exe`.

```powershell
cargo build --release --locked --target x86_64-pc-windows-msvc --bin ALTRunNext
```

After editing the icon, run `python tools/build-app-icon.py` with Pillow installed to regenerate `assets/app.ico`.

Maintained by [weimeng54](https://github.com/weimeng54) · [Email](mailto:weimeng54@qq.com) · [Report an issue](https://github.com/weimeng54/ALTRunNext/issues)

The project code is licensed under MIT. The portable ZIP includes the notices for third-party resources and dependencies. [MIT License](LICENSE)

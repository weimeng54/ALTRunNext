# ALTRunNext

[简体中文](#简体中文) · [English](#english)

## 简体中文

ALTRunNext 是一款面向 Windows 11（64 位）的轻量快捷启动器。按 **Alt+R** 呼出，输入快捷项，按 **Enter** 打开程序、文件、文件夹或网址。例如，可以用 `db` 找到快捷项为 `doubao` 的启动项。

### 快速开始

1. 从 [GitHub Releases](https://github.com/weimeng54/ALTRunNext/releases) 获取便携 ZIP，解压到有写入权限的文件夹，然后运行 `ALTRunNext.exe`。
2. 按 **Alt+R** 打开启动面板，使用 **↑ / ↓** 选择结果，按 **Enter** 启动，按 **Esc** 收起面板。
3. 右键单击托盘图标，选择「管理启动项」添加项目，或打开「设置」调整快捷键、语言、主题和呼出声音。

### 添加和备份启动项

- 在「设置 → 常规」开启并保存「添加 ALTRunNext 到发送到菜单」。在资源管理器中右键单击目标，选择「显示更多选项 → 发送到 → ALTRunNext」，确认后保存。文件可以使用系统默认应用，也可以为该启动项指定打开程序。在设置里关闭此选项即可移除菜单入口。
- 在「管理启动项 → 工具」导入启动项，程序会自动识别当前 TSV 或原版 ALTRun 的 `ShortCutList.txt`。确认导入后点击「保存」应用。启动项导出为 TSV，设置的导入和导出使用 ALTRunNext 的 INI。
- 便携版把设置、启动项和使用历史保存在数据目录。升级或迁移前，可备份整个解压目录。

### 界面语言

首次启动跟随 Windows 显示语言，也可在「设置 → 常规 → 显示语言」手动切换。英文和简体中文内置，便携包同时附带日语、韩语、法语、德语和西班牙语。将其他 UTF-8 JSON 语言包放在 EXE 同级的 `languages` 文件夹，再重新打开设置即可选用。[语言包格式与翻译说明](languages/README.md)

### 从源码构建

在 Windows 上安装 Rust（MSVC 工具链）、Visual Studio Build Tools 的 C++ 桌面开发组件和 Windows SDK。在仓库根目录运行以下命令，生成的程序位于 `target/x86_64-pc-windows-msvc/release/ALTRunNext.exe`。

```powershell
cargo build --release --locked --target x86_64-pc-windows-msvc --bin ALTRunNext
```

修改图标后，可用安装了 Pillow 的 Python 运行 `python tools/build-app-icon.py`，重新生成 `assets/app.ico`。

作者 [weimeng54](https://github.com/weimeng54) · [联系邮箱](mailto:weimeng54@qq.com) · [反馈问题](https://github.com/weimeng54/ALTRunNext/issues)

## English

ALTRunNext is a lightweight launcher for Windows 11 (64-bit). Press **Alt+R**, type an alias, and press **Enter** to open an app, file, folder, or website. Fuzzy matching lets you find an alias like `doubao` by typing `db`.

### Getting started

1. Get the portable ZIP from [GitHub Releases](https://github.com/weimeng54/ALTRunNext/releases), extract it to a writable folder, and run `ALTRunNext.exe`.
2. Press **Alt+R** to open the launcher, **↑ / ↓** to select a result, **Enter** to launch it, and **Esc** to hide the panel.
3. Right-click the tray icon to manage launch items or open Settings. Settings include hotkeys, language, theme, and the popup sound.

### Adding and backing up launch items

- In **Settings → General**, enable and save the Send To integration. In File Explorer, right-click a target and choose **Show more options → Send to → ALTRunNext**, then review and save the launch item. A file can open in the system default app or an app selected for that launch item. Turn off the setting to remove the menu entry.
- Open **Manage launch items → Tools** to import a list of launch items. ALTRunNext detects its TSV format and the original ALTRun `ShortCutList.txt` automatically. Confirm the import and select **Save** to apply it. Launch items are exported as TSV, and settings use the ALTRunNext INI format.
- The portable app stores settings, launch items, and usage history in its data directory. Back up the extracted folder before upgrading or moving it.

### Display language

ALTRunNext follows the Windows display language on first launch. You can choose another language under **Settings → General → Display language**. English and Simplified Chinese are built in. The portable package also includes Japanese, Korean, French, German, and Spanish. Add other UTF-8 JSON packs to the `languages` folder next to the executable and reopen Settings. [Language pack format and translation guide](languages/README.md)

### Building from source

On Windows, install Rust with the MSVC toolchain, Visual Studio Build Tools with Desktop development with C++, and the Windows SDK. Run the following command from the repository root. The executable is written to `target/x86_64-pc-windows-msvc/release/ALTRunNext.exe`.

```powershell
cargo build --release --locked --target x86_64-pc-windows-msvc --bin ALTRunNext
```

After editing the icon, run `python tools/build-app-icon.py` with Pillow installed to regenerate `assets/app.ico`.

Maintained by [weimeng54](https://github.com/weimeng54) · [Email](mailto:weimeng54@qq.com) · [Report an issue](https://github.com/weimeng54/ALTRunNext/issues)

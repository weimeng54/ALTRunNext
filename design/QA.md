# 原型检查记录

日期：2026-11-07。检查对象为设计原型，不是 Windows 原生成品。

## 已运行

- `node D:/altrun/design/check-prototype.cjs`：本次修订后 13 组交互/结构检查通过，包含 16 个页面与视口组合（1024、736、420、320px），无页面横向溢出，无浏览器脚本异常。新增验证浮窗宽 ≤420px、10 项高 ≤300px、单结果高 <130px、原版字段、单击选中和修饰数字执行。使用本机已有 Edge/Playwright，不安装浏览器或依赖。浏览器子进程的 TEMP/TMP 重定向至 D 盘 QA 目录。
- 人工查看启动面板、命令管理、参数弹层、深色与窄屏截图；发现并修复原生 table 在深色主题中正文未继承前景色的问题。
- `python D:/altrun/design/check-design.py`：离线核对 DESIGN.md 与原型的 9 对主题 token、字段 ID 唯一性、HTML fragment 边界、字体字段和 20 对基本文字/背景对比度。结果见 `qa/design-checks.json`。
- 按 brainstorming 的 builder check 进行独立只读审阅；修正“快捷字唯一”可能删减旧版重复快捷项的矛盾：稳定 ID 唯一，导入重复项保留并参与旧匹配。

## 静态审计的局限

运行了 frontend-design-premium 的 `audit_project.py --mode strict`，原始结果保留于 `premium-audit.json`。它仍返回 `affordance.actionless-button` 静态发现，**不能宣称该审计通过**。

逐项核对后，这些按钮均通过 `addEventListener` 的事件委托或脚本中的 `.onclick` 赋值连接动作。审计器在检查字面量 button 时只识别标签属性里的 onclick / @click，不识别本原型采用的绑定方式。页面导航、分类、清空、检查、新建、编辑、取消、删除、撤销、导入及两个对话框按钮均已映射到代码动作；主要流程已通过真实浏览器交互验证。没有为迎合检查器加入空的 onclick 或禁用可用按钮。

## 本次原版对照

只读检查了上游 `Form/frmALTRun.dfm` 和 `Form/frmALTRun.pas`：420px 宽主窗体；顶部 lblShortCut 是当前选中名称；edtHint 位于输入区；列表含编号、参数星号、快捷字、名称；edtCommandLine 显示 CMD=；单击选中、双击执行。按此修订原型并同步方案、DESIGN 和 UX 约定。默认工具按钮隐藏、CMD 与提示开启；示例命令取自原版默认命令，不表示已复原运行频率。

官方 `designmd lint` 未运行，因为当前未发现已安装的 CLI，且用户要求本次不安装工具。本次使用离线 token/结构检查，不能将其称为官方 lint 通过。

## 不代表已经验证的事项

真实中文输入法、Windows 原生控件/UI Automation、全局热键、Shell 命令、真实磁盘迁移、软件资源预算、旧版全部功能和 24 小时稳定性均留在原生实现阶段验证。浏览器测试中的 composition 事件是合成事件，不是真实 IME 验收。

预览无网络资源、真实文件读取、剪贴板读取和命令执行；迁移的数量和检查结果都是示例。编辑仅存在内存中，不保存到系统配置。

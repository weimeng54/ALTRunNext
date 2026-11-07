---
version: alpha
name: ALTRun
description: Windows 键盘启动器的拟议视觉系统；当前仅用于界面原型。
colors:
  background-light: '#f5f7fa'
  background-dark: '#191d24'
  surface-light: '#ffffff'
  surface-dark: '#222730'
  text-light: '#202a36'
  text-dark: '#ecf0f5'
  muted-light: '#596779'
  muted-dark: '#abb7c8'
  line-light: '#d8dfe8'
  line-dark: '#424c5a'
  blue-light: '#275fa8'
  blue-dark: '#94bfff'
  selected-light: '#e8f0fc'
  selected-dark: '#2c3d56'
  danger-light: '#aa3434'
  danger-dark: '#ffaaaa'
  ok-light: '#256347'
  ok-dark: '#9adbb6'
typography:
  sans:
    fontFamily: 'Segoe UI Variable, Microsoft YaHei UI, Segoe UI, sans-serif'
  mono:
    fontFamily: 'Consolas, monospace'
  brand:
    fontFamily: 'Bahnschrift, Segoe UI, sans-serif'
rounded:
  DEFAULT: 6px
  control: 6px
spacing:
  page: 24px
  row-standard: 24px
  row-compact: 20px
components:
  launcher: { width: 420px }
  editor: { width: 300px }
  native-input: { ownership: platform }
---

## Overview

根据用户反馈修订：平时不看 UI，以启动为主；显示的元素和内容参考原版。面向长期 ALTRun 用户，在 Windows 11 中用尽量少的按键打开目标。简体中文为首要原型语言，正式版本保留原版多语言能力。中性、紧凑、适合频繁使用的工具界面，不采用营销页布局。

主窗体按原版 `lblShortCut → edtShortCut/edtHint → lstShortCut → edtCommandLine` 的结构：顶部当前选中项名称；输入和操作提示；编号/参数星号、快捷字、名称三列；底部单行 `CMD=`。不新增图标列、逐行路径、类型标签、结果计数或操作页脚。原版三个工具按钮可选显示，默认按仓库示例 INI 隐藏。数字位置、参数标记和单击选中/双击执行沿用原版。

当前 token 真源为 `design/altrun-prototype.html` 的 `#altrun-design` 样式，此文镜像并解释其值（Model B）。`background/surface/text/muted/line/blue/selected/danger/ok` 的 light/dark 值分别映射 `--ar-bg/--ar-surface/--ar-text/--ar-muted/--ar-line/--ar-blue/--ar-select/--ar-danger/--ar-ok` 的 `light-dark()`；所有页面共用。正式 Rust UI 实现时再从同一 token 源生成原生主题值，不维护两套手工值。CSS 作用域只覆盖原型，不影响聊天外观。

## Colors

背景与表面以微小明度差分层；正文与次要信息保持清晰对比。信号蓝用于快捷字、焦点和选中轨；错误与成功同时用文字表达。自动跟随宿主主题，也提供浅/深显式切换。高对比使用系统色。

## Typography

管理页正文与控件 14px/1.5；启动框输入 14px、列表 13px、CMD 12px、输入提示 11px。中文回退到 Microsoft YaHei UI。启动面板不显示额外品牌标题或标语；顶部是选中项名称。避免外部字体下载。中文内容不使用大范围字距和装饰斜体。

## Layout

原型最大宽 1024px 以容纳管理页，启动面板宽 420px。默认紧凑行高 20px，可选 24px；最多显示 10 项，单条结果时高度自然收缩，无列表占位空白。示例选用原版默认命令中的 10 项，不以其顺序宣称复刻频率排名。顶部操作提示与底部 CMD 保留为可关闭选项；路径不在每一行重复。管理器为列表与 300px 编辑器，740px 以下纵向排列。预览页面切换与主题控件在产品浮窗外，不属于成品启动面板。正式桌面窗口用 DIP 和显示器工作区计算，原型响应式不等于承诺移动端产品。

## Elevation & Depth

只有启动浮窗与模态输入需要层次感；轻阴影，不使用持续模糊、渐变和大面积玻璃效果。表格与设置按细分隔线组织，不将每个设置项包装成卡片。

## Shapes

浮窗 6px，普通控件 6px，启动输入框 3px，列表选中轨 2px。主列表无应用图标。正式版本可选工具按钮使用经许可的系统/自绘符号；默认隐藏，不占用操作视线。

## Components

所有按钮为原生 button，字段有 label。主操作只有一个强强调按钮；删除与保存区分；所有焦点可见。按钮包含 hover、focus、disabled 状态。输入、命令行、表格、设置侧栏、状态提示跨页面复用相同词汇与 token。

原型对话框使用原生 dialog，背景 inert、Escape 取消、恢复焦点；命令编辑器是非模态面板。状态反馈使用统一 polite live region，字段错误用 alert。全局滚动条继承 token，高对比保持系统能力。原型不使用本地存储，不把演示信息写入配置。

原型只演示主题、密度、命令预览、内存中 CRUD、参数交互与示例迁移；生产级图标、完整键盘兼容、UIA 和高 DPI 由原生实现阶段验证。

## Do's and Don'ts

- 保留常用操作的位置；一眼识别快捷字、名称和目标。
- 同一操作跨页面使用相同文案、反馈和焦点行为。
- 不把原型的浏览器占用作为成品占用，不展示虚构性能读数。
- 不用外观现代化作为删掉原版功能的理由。

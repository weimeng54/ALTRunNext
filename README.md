# ALTRun native performance prototype

Windows 11 x64 原生启动器重写项目。优先保持原版 ALTRun 跟手、低占用的使用体验；性能以用户实际试用可接受为准，允许感知不到的轻微指标退步。

当前已有紧凑原生界面、快捷字搜索、基础命令启动、原样参数输入和托盘退出，尚不是完整替代品。截图采集实验已停止，不再以严格统计证明作为功能开发的前置门槛。

体验版通过 `--data <UTF-8 TSV>` 加载快捷项，`--show` 首次显示，`--enable-launch` 开放真实执行；默认模式继续只隐藏，供隔离测试使用。Ctrl+Alt+R 呼出，上下键/Tab 选择，Enter 或双击启动；参数命令先输入参数，Esc 返回。托盘右键退出。带空格且附带参数的程序路径必须加双引号，避免启动错误目标。

目前支持基本程序、文件、目录、http/https 网址、环境变量和 `%p` 原样替换。完整旧正则/排序、参数 URL 编码类型、`%c` 等其他旧模板、管理设置及完整迁移仍未完成；不支持的模板明确报错。当前为浅色主题，动态跨显示器 DPI 重建和高对比度主题待实现。

- [设计与兼容方案](docs/ALTRun-modernization-plan.md)
- [云构建任务清单](docs/superpowers/plans/2026-11-07-altrun-cloud-build.md)
- [性能门槛与当前进度](docs/validation/performance-gate.md)
- [界面参考原型](design/altrun-prototype.html)

源码在本地 D 盘编辑，GitHub Actions 标准 Windows runner 编译与测试；本机不安装 Rust、MSVC 或 SDK。原版真实配置、个人路径、剪贴板与诊断转储不进入仓库。

正式程序采用 Rust + Win32，保留必要资源常驻，无输入防抖、弹出动画、启动路径磁盘查询或网络访问。绘制后端以真实性能对照结果决定，未通过验证不承诺某框架必然流畅。

本仓库暂未选定新代码的开源许可证；上游实现与素材不直接复制。构建产物是试验包，不代表完整功能或发布版本。

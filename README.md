# ALTRun native performance prototype

Windows 11 x64 原生启动器重写项目。优先保持原版 ALTRun 跟手、低占用的使用体验；性能以用户实际试用可接受为准，允许感知不到的轻微指标退步。

当前已有紧凑界面设计、原生呼出/搜索原型和云端构建。正在推进可试用版本与旧功能兼容，尚不是完整替代品。截图采集实验已停止，不再以严格统计证明作为功能开发的前置门槛。

- [设计与兼容方案](docs/ALTRun-modernization-plan.md)
- [云构建任务清单](docs/superpowers/plans/2026-11-07-altrun-cloud-build.md)
- [性能门槛与当前进度](docs/validation/performance-gate.md)
- [界面参考原型](design/altrun-prototype.html)

源码在本地 D 盘编辑，GitHub Actions 标准 Windows runner 编译与测试；本机不安装 Rust、MSVC 或 SDK。原版真实配置、个人路径、剪贴板与诊断转储不进入仓库。

正式程序采用 Rust + Win32，保留必要资源常驻，无输入防抖、弹出动画、启动路径磁盘查询或网络访问。绘制后端以真实性能对照结果决定，未通过验证不承诺某框架必然流畅。

本仓库暂未选定新代码的开源许可证；上游实现与素材不直接复制。构建产物是试验包，不代表完整功能或发布版本。

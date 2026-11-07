# ALTRun native performance prototype

Windows 11 x64 原生启动器重写项目。第一门槛是与原版 ALTRun 同机对比的响应性能；未达标不进入完整功能开发。

当前已有紧凑界面设计与执行计划，尚无通过性能验收的替代品。

- [设计与兼容方案](docs/ALTRun-modernization-plan.md)
- [云构建任务清单](docs/superpowers/plans/2026-11-07-altrun-cloud-build.md)
- [性能门槛与当前进度](docs/validation/performance-gate.md)
- [界面参考原型](design/altrun-prototype.html)

源码在本地 D 盘编辑，GitHub Actions 标准 Windows runner 编译与测试；本机不安装 Rust、MSVC 或 SDK。原版真实配置、个人路径、剪贴板与诊断转储不进入仓库。

正式程序采用 Rust + Win32，保留必要资源常驻，无输入防抖、弹出动画、启动路径磁盘查询或网络访问。绘制后端以真实性能对照结果决定，未通过验证不承诺某框架必然流畅。

本仓库暂未选定新代码的开源许可证；上游实现与素材不直接复制。构建产物是试验包，不代表完整功能或发布版本。

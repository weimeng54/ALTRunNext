# 独立采样开销实验

当前观测区间约 15–51ms，其中包含截图、哈希和窗口检查，尚未分离，不能把全部耗时直接归因某个 API。先隔离采样开销，再决定是否改用 DXGI；不更改候选启动器或降低现有门槛。

实验对比同区域、交错顺序的两种实现：现有 compatible bitmap + GetDIBits；CreateDIBSection + GdiFlush 后读取直接像素缓冲区。均复用 DC/bitmap，分别记录复制、像素读取/同步、哈希及总耗时；运行期间只有内存中的像素，文件仅保存时间和合成正确性结论。

先在云端用不断变化的自建内存图案验证两种路径的 RGB 内容、上下方向及非标准宽度；随后独立隐藏运行本机采样，不创建可见窗口、不发送输入、不访问原版、不启动候选。默认无参启动必须有同目录明确的实验 profile，输出限 D 盘。每种模式 200 次，AB/BA 交错，排除预热；报告保留原始时间，固定 pending，无性能门槛样本。

这只能判断采样工具的部分开销，不能证明输入响应、正确字形呈现或硬件显示延迟。若开销仍太大，停止 GDI 方案的继续微调，转入基于系统桌面帧时间戳的独立实验。不会要求用户反复腾出桌面验证尚未校准的工具。

依据：[CreateDIBSection 同步要求](https://learn.microsoft.com/en-us/windows/win32/api/wingdi/nf-wingdi-createdibsection)、[GdiFlush 仅刷新调用线程批次](https://learn.microsoft.com/en-us/windows/win32/api/wingdi/nf-wingdi-gdiflush)。该同步不等于 DWM/显示器已呈现目标内容。

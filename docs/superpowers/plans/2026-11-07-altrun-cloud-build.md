# ALTRun 云端构建与原生重写执行计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. 本计划默认当前会话逐项执行；只有用户选择委派时才切换到 subagent-driven-development。

**Goal:** 在 GitHub 公开仓库中重写 Windows 11 原生 ALTRun，完整保留旧功能与紧凑交互，在云端编译、在用户电脑验证，以极低常驻占用交付便携版。

**Architecture:** Rust 核心与 Windows 平台层分离，Win32 消息驱动，Direct2D/DirectWrite 绘制配合原生 Unicode 输入控件。GitHub Actions 的标准 Windows runner 负责构建与自动测试；本机仅编辑源码、下载便携包和做真实 Windows 11 验收。本机不安装 Rust/MSVC/SDK，不配置本机 self-hosted runner。

**Tech Stack:** Rust stable 固定版本、Cargo workspace、windows-rs、serde/serde_json、必要的编码与匹配依赖；Windows x64 MSVC 目标；GitHub Actions `windows-2025`。

**Spec:** `docs/ALTRun-modernization-plan.md`；视觉与交互分别见 `DESIGN.md`、`UX-CONTRACT.md`。本计划替代之前“优先本机安装编译环境”的执行顺序，不改变完整功能目标。

日期：2026-11-07。状态：计划已生成，未开始实施。已存在的 HTML 为设计原型，不是正式产品实现。

## Global Constraints

- 用户仓库：`https://github.com/weimeng54/altrun.git`，用户声明为公开开源项目；上游行为基线是 `etworker/ALTRun` v1.46.7。
- 本地目录 `D:\altrun`；新增下载、运行数据和本项目临时输出位于 D 盘；不安装任何本机编译工具，也不改写全局 TEMP/TMP。
- 当前本地 `origin` 仍为 `https://gitee.com/weimeng54/altrun.git`，尚无首次提交；已有设计/原型文件待整理。GitHub 内容、默认分支与写权限尚未验证成功，不能假设远端为空。
- GitHub 新增远程名 `github`；保留 Gitee `origin`。新开发分支使用 `codex/` 前缀，不 force push，不自动镜像推送 Gitee，不重写现有远程历史。
- GitHub 远端为空时使用 `main`；不为空则尊重其默认分支并从该历史创建功能分支。首轮不擅自变更仓库可见性、许可证、保护规则或已有文件。
- 首发仅 Windows 11 x64。生产界面不采用 HTML/WebView、跨平台 GUI、全盘索引、云同步、持续网络轮询或常驻插件宿主。
- 启动窗口 420 DIP，默认行高 20 DIP，可选 24 DIP，最多可见 10 项，少结果时自然收缩。
- 主界面只呈现原版的选中项名称、输入框/提示、编号/参数星号/快捷字/名称、底部单行 CMD。CMD 和提示默认开启；工具按钮默认隐藏；导入以旧配置为准。
- 默认旧版匹配/排序；单击选择，双击/中键/Enter/兼容快捷键执行。中文 IME 组词期间不触发启动或唯一匹配自动执行。
- 原始旧配置只读；便携 Data 为新版唯一可写真源。数据目录不可写时提示，不能悄悄写入 C 盘 AppData。
- 所有兼容项都须有矩阵记录与验证证据；阶段性试用包不能宣传为全功能完成。网页测试、云端测试均不能替代本机 IME/焦点/DPI/占用验收。
- 优先使用可信国内 Rust 镜像，失败后记录并回退官方可信源；不禁用证书或校验，不上传真实私人配置、路径样本、剪贴板或诊断转储。

## Review Focus

1. **源仓库已有 README/许可/提交**：任务 1 验证保留远程历史，不覆盖现有许可，不误推 Gitee。
2. **重复快捷字及未知编码**：任务 3 保留多项、原序、频率与原文，编码歧义必须让用户选择，禁止静默丢项。
3. **IME Enter、热键冲突和焦点被抢**：任务 4/8 必测；不误执行、不留下旧热键失效状态、不操作错误的前台窗口。
4. **引号、占位符与不同执行类型**：任务 6 校验生成的执行计划，普通应用不隐式经过命令解释器；取消 UAC 不记成已执行成功。
5. **写入中断、只读目录与网络路径卡住**：任务 7/9 验证原子保存、备份恢复和超时状态；不把未确认路径当失效删除。

---

## 交付顺序与文件职责

正式代码尚未创建，以下是执行阶段的文件边界，不要求现在生成空壳。

```text
.github/workflows/ci.yml          标准 Windows 构建、测试、临时包
.github/workflows/release.yml     手动创建草稿 Release，不自动公开发布
rust-toolchain.toml               固定 Rust 工具链
Cargo.toml / Cargo.lock           workspace 与锁定依赖
crates/altrun-core/               命令模型、搜索排序、模板、执行计划
crates/altrun-legacy/             旧文件解码、解析、导入报告、导出
crates/altrun-windows/            Win32/Shell/COM/句柄/窗口/热键/绘制
crates/altrun-app/                状态机、原生 UI、持久化、参数输入与管理
tests/fixtures/legacy/           人工构造并脱敏的旧格式金样
scripts/                        云端检查、打包、D 盘下载/验收入口
docs/compatibility/              逐项矩阵、特殊语法、已知差异
docs/validation/                 本机性能、原生交互、稳定性与发布报告
design/                         现有参考原型，仅设计用途
```

依赖关系：1 → 2 → 3；4 可以在 2 后先做原生验证；5/6 依赖 3 的行为金样；7 依赖 3 的迁移模型；8/9 汇合核心与界面；10 必须在 4 的预算验证通过后全面完善；11/12 做真实验收和发布。

## 统一内部接口

接口属于项目内部 Rust crate，不增加 HTTP 服务、公共网络协议或 DLL ABI。

| 所属 | 接口 | 约束 |
|---|---|---|
| core/model.rs | `CommandId(u64)`、`Command`、`Settings`、`ParameterType` | ID 唯一且稳定，快捷字可重复；参数类型 None/Raw/LegacyUrl/Utf8Url；保留原始命令、频率与导入顺序 |
| legacy/import.rs | `preview_import(dir: &Path, encoding: EncodingChoice) -> Result<ImportPreview, ImportError>` | 只读，输出命令/设置/使用记录/问题和源文件指纹，不执行命令 |
| core/search.rs | `query(&self, input: &str, options: &SearchOptions, usage: &UsageSnapshot) -> Result<Vec<SearchHit>, SearchError>` | 排序稳定，返回 ID；不接触文件系统或 UI |
| core/template.rs | `resolve(command: &Command, input: &ExecutionInput) -> Result<ExecutionPlan, ResolveError>` | 输入包含参数与按需捕获的环境/剪贴板/前台窗口快照 |
| windows/executor.rs | `execute(plan: ExecutionPlan) -> Result<LaunchReceipt, LaunchError>` | 区分应用、文件/目录、URL、显式 Shell、内置窗口操作；回执只表示系统接受/进程创建 |
| app/store.rs | `load(dir: &Path) -> Result<AppData, StoreError>` / `save(&mut self, data: &AppData) -> Result<(), StoreError>` | 版本化 JSON、单写入者、同目录原子替换与 10 份配置备份 |
| app/controller.rs | `dispatch(&mut self, event: AppEvent) -> Vec<AppEffect>` | UI 纯状态变化与系统效果分离；查询序号取消陈旧结果 |

具体字段由模型模块统一定义，消费者不得自行创建同名但不兼容的模型。旧语法语义先由任务 3 形成金样，再实现；不能按新版喜好推测替换顺序。

## Task 1：接入 GitHub 并整理首批可公开内容

**Files:** 新增 `.gitignore`、`README.md`；必要时修订 `design/check-prototype.cjs` 的本机绝对路径；不上传生成缓存/私人配置。

**Interfaces:** 消费已有设计文件和用户 GitHub URL；产出可追踪的开发分支、公开内容清单、远程默认分支记录。

- [ ] 读取 GitHub refs/default branch、仓库已有 README/许可/提交；验证当前账号写权限。读不到或无写权限时只整理本地文件并报告，不假称已连接或已上传。
- [ ] 新增 `github` remote，保留 Gitee `origin`；按远端历史创建 `codex/bootstrap-cloud-build`，远端空仓库时以 `main` 建立基线。所有推送显式指定 `github`。
- [ ] 忽略 `/target/`、`/dist/`、`/downloads/`、`/temp/`、`/Data/`、`design/qa/` 生成输出、本机环境文件及真实用户配置。原型测试改为模块解析或环境变量指定 Playwright，不公开依赖当前用户名的硬编码路径；不为了测试在本机安装依赖。
- [ ] README 写清重写目标、设计文档入口、当前只有原型、原生开发状态和 GitHub 构建路线。若远端无许可证，只记录需要用户选择后补齐，不擅自替用户授予代码许可；未确认原版许可前不复制原版实现/素材。
- [ ] 检查 `git diff --check`、待提交清单及生成目录忽略规则；提交设计基线并推送开发分支。记录提交 SHA、远程 URL 和推送结果。没有授权凭据时不让用户在聊天粘贴 token。

**Acceptance:** GitHub 可看到与本地相同 SHA 的设计提交，Gitee 地址和历史未改变，临时数据未被追踪。此阶段不声称已有可运行程序。

## Task 2：建立真实可用的云端编译闭环

**Files:** `.github/workflows/ci.yml`、workspace manifests、`rust-toolchain.toml`、`crates/altrun-app/src/main.rs`、`scripts/package.ps1`、`crates/altrun-app/tests/build_smoke.rs`。

**Interfaces:** 产出一个可下载 x64 包和隐藏诊断入口 `altrun.exe --self-check <report-path>`；后续任务复用同一构建/打包流程。

- [ ] 固定 runner 为标准 `windows-2025`，目标 `x86_64-pc-windows-msvc`；从官方当前可用 stable 选取一个具体版本写入 `rust-toolchain.toml` 并记录 `rustc -Vv`，以后只通过专门升级提交改变版本。使用 edition 2024、最小工具链组件 rustfmt/clippy，提交 lockfile。
- [ ] 配置 `pull_request`、代码分支 push 和 `workflow_dispatch`。纯文档变更不重复 Rust 重编译；每分支 concurrency 取消过时运行，单作业超时 30 分钟。读取源码的默认权限仅 `contents: read`；不用付费 larger runner 或本机 runner。
- [ ] Rust 镜像默认 RsProxy/sparse。网络预检限时 20 秒，失败后日志记录并使用官方可信源，锁定版本不变；工具链可用 USTC 备用。GitHub 自身 checkout 和 Actions 使用官方地址。第三方 Actions 固定到官方仓库审核过的完整 commit SHA，在工作流注释记录版本。
- [ ] 先写 `build_smoke`：诊断入口应生成包含版本、目标架构和 `status=ok` 的 JSON，不创建窗口、不修改注册表、不执行配置命令。云端看到明确测试失败后，实现最小程序，再跑 fmt、clippy、test、release build 与打包。
- [ ] 生成 ZIP、SHA-256、构建元数据和测试报告；命名含短提交 SHA，artifact 保留 3 天。构建失败上传可用诊断记录，不生成成功包；缓存键包含 Windows/架构/工具链/lockfile，不上传 Data。
- [ ] 跟进云端运行至成功/失败，修复失败；下载同 SHA 包到 `D:\altrun\downloads\<sha>` 并校验，运行诊断入口写 D 盘报告。未跑通下载与本机诊断前，本任务未完成。

**Acceptance:** 本机无 Rust/MSVC/SDK，也能拿到并运行该提交的云编译 EXE。此时只是诊断骨架，不冒充启动器。

## Task 3：完成旧行为审计和可执行兼容金样

**Files:** `docs/compatibility/feature-matrix.md`、`docs/compatibility/legacy-syntax.md`、`tests/fixtures/legacy/`、`crates/altrun-legacy/src/{decode,parse,import}.rs`、`crates/altrun-legacy/tests/import_preview.rs`。

**Interfaces:** 定义 core 的统一模型和 `preview_import`；输出搜索、模板、文件格式金样供后续任务使用。

- [ ] 固定上游审计 commit，阅读全部窗体、事件、配置与工具单元；列出每个开关/按键/模板/启动参数/外部依赖及源位置。确认参数历史实际文件名与编码，不虚构文件名。功能矩阵每行至少有源证据、验收样例与所属任务。
- [ ] 为旧文件构造无私人路径的 UTF-8、BOM、GBK、系统代码页、空行、注释、旧频率前缀、含分隔符、未知项和坏数据样例。断言“两个同快捷字条目均保留且 ID 不同；未知项原文保留；歧义编码不自动替换；输入文件 SHA 不变”。先在云端看到失败，再实现解码/解析/预览。
- [ ] 用旧版源码明确 URL 与 UTF-8 编码差异、环境变量/占位符替换顺序、正则/通配符、排序公式与并列顺序。能安全运行旧版时在隔离目录对照；不能运行时标为源码推导，不能伪造运行证据。
- [ ] 对复杂正则建立独立兼容接口。根据审计语法集选定并锁定实现：常规匹配走线性路径；支持回溯的旧表达式交按需 worker，查询总超时 100ms 后终止该 worker 并报告超时；不能静默降级。无法表达的已发现旧语法必须先补齐或明确阻止“兼容完成”门槛。
- [ ] 执行 `cargo test --locked -p altrun-legacy` 和模型测试；提交编码/解析基线及兼容矩阵。用户真实配置只本机验证，不能进入公开 fixtures/artifacts。

**Acceptance:** 所有原功能均有条目和归属；迁移解析不会静默丢项。审计中新发现的必要行为回填对应任务，不通过删除需求关闭问题。

## Task 4：先交付紧凑原生面板，验证技术路线

**Files:** `crates/altrun-windows/src/{window,render,text_input,resources}.rs`、`crates/altrun-app/src/{controller,launcher,theme}.rs`、`crates/altrun-app/tests/controller.rs`、`crates/altrun-windows/tests/resources.rs`、`docs/validation/native-spike.md`。

**Interfaces:** 定义 `AppEvent/AppEffect`、`dispatch`，输出可真实呼出/隐藏的原生窗口；此阶段命令执行只使用诊断效果，不读取真实旧配置。

- [ ] 为控制器建立测试：IME composing + Enter 不产生 Execute；空查询 Esc 产生 Hide；单击只产生 Select；两个查询返回次序相反时仅较新结果生效。先云端失败，再实现状态机。
- [ ] 实现 Win32 消息循环、Unicode 输入框、Direct2D/DirectWrite 列表和单行只读 CMD。默认 420 DIP / 20 DIP / 10 项上限，少结果自然缩短；顶部显示选中名，参数项带星号；主题复用已确认 token。原型的导航栏、主题预览控件和演示说明不进入产品浮窗。
- [ ] 注册临时呼出热键并处理冲突；默认 Alt+R/Pause，不抢占已运行原版的热键。隐藏不销毁消息循环，不持续刷新；缺失显示器时将窗口移回有效工作区。
- [ ] 云端做状态/布局/资源生命周期测试，发布阶段试用 artifact；本机测试真实微软中文 IME、焦点、100/125/150/200% DPI、明暗/高对比、100 次呼出与隐藏。
- [ ] 用 Release 包记录初始 Private Bytes、工作集、CPU 和首帧时间。目标未达先修原生骨架，不一边扩大功能一边推迟证明路线。记录界面与数据规模，不能把这个小样本成绩当最终 1 万项预算通过。
- [ ] 提交验证结果和实现；未验证过的 Windows 行为保持未验收。

**Acceptance:** 原生紧凑交互基本可用；没有 IME 误执行、明显焦点故障或持续空闲绘制，资源开销有可核查初测。

## Task 5：实现旧搜索、排序和使用记录

**Files:** `crates/altrun-core/src/{search,ranking,usage}.rs`、`crates/altrun-app/src/search_tasks.rs`、`crates/altrun-core/tests/search_compat.rs`。

**Interfaces:** 实现 `query`；使用任务 3 的 golden cases，不与 UI 或文件系统耦合。

- [ ] 将金样写成失败测试，覆盖空查询、前缀/任意位置、大小写、`*`/`?`、旧正则、并列顺序、重复快捷字、记忆首选、频率及最近列表；断言完整有序 ID 列表，不仅结果数。
- [ ] 实现旧兼容引擎，默认只搜索旧定义字段；增强名称搜索是可选模式。复杂正则进入任务 3 定义的限时 worker；过时任务结果不能覆盖当前查询。
- [ ] 实现频率、最近记录和首选映射；无搜索任务不唤醒轮询。常用搜索只访问内存；显示前十项不丢弃完整结果集。
- [ ] 云端运行 `cargo test --locked -p altrun-core --test search_compat`，对 1 万条人工数据执行基准并记录；端到端 30ms 指标仍留待本机呈现测试。
- [ ] 接入原生面板，下载同 SHA 包核对熟悉的快捷字顺序；提交本阶段结果。

**Acceptance:** 搜索语义与排序金样通过；任何未覆盖的旧语法在矩阵中保留，不声称全面兼容。

## Task 6：实现参数、命令模板与 Windows 执行

**Files:** `crates/altrun-core/src/{template,execution}.rs`、`crates/altrun-windows/src/{executor,clipboard,foreground}.rs`、`crates/altrun-app/src/parameter_dialog.rs`、`crates/altrun-core/tests/execution_plan.rs`、`crates/altrun-windows/tests/executor.rs`。

**Interfaces:** 实现 `resolve` 与 `execute`，接入参数对话框和有界历史。

- [ ] 为引号/空格/中文/`%`/`&`、`%p`/`%c`、环境变量、工作目录和全部已发现前台窗口标记写执行计划失败测试；旧 URL 与 UTF-8 分别断言准确字节/字符串，不能共用 encodeURIComponent 代替。
- [ ] 按审计顺序实现替换与执行类型解析；普通应用保持程序与参数分离，显式 Shell 类型才调用解释器。剪贴板与前台窗口只在命令需要且呼出前捕获，不做常驻监听。
- [ ] 实现参数输入/取消/历史上限和 IME 保护。系统接受启动才更新启动记录；UAC 取消、Shell 拒绝、目标缺失都返回明确错误并保留输入。
- [ ] 云端用录制型 executor 检查执行计划，不运行关机/重启/注册表修改等危险真实命令；本机测试记事本、临时目录、测试网页和可取消 UAC，输出脱敏结果。
- [ ] 运行 `cargo test --locked -p altrun-core --test execution_plan` 及 Windows adapter 测试；确认应用退出后没有遗留常驻 worker，再提交。

**Acceptance:** 四类参数与旧模板可用，执行失败/取消不误报成功，普通启动不改变 Shell 语义。

## Task 7：实现可靠持久化和完整迁移

**Files:** `crates/altrun-app/src/{store,migration,backup}.rs`、`crates/altrun-app/tests/store_recovery.rs`、`crates/altrun-app/tests/import_roundtrip.rs`。

**Interfaces:** 实现 `load/save`，把任务 3 的 `ImportPreview` 写入全新便携 Data。

- [ ] 为原子替换前/后中断、坏 JSON、只读目录、外部编辑冲突和备份恢复编写失败测试；断言旧文件不变、原配置可恢复、不会回退写 C 盘。
- [ ] 定义 JSON schema 版本、稳定 ID 分配与显式升级路径；命令/设置/历史分别存储。命令与设置写入前备份，保留最近 10 份；使用记录仅在有脏数据时合并，执行后 1 秒内刷新，退出时尽力刷新。
- [ ] 迁移先预览后提交，提交前验证源指纹；编码不明确让用户选择。重复快捷字保留顺序、频率与原文；未解析/无法执行项保留并停用，不能删除。配置不可写时提供换目录与重试。
- [ ] 实现旧格式导入/导出及文本配置修改后校验重载；外部改动与未保存编辑冲突时保留副本并选择，不静默覆盖。
- [ ] 云端运行 `cargo test --locked -p altrun-app --test store_recovery --test import_roundtrip`；本机用副本进行真实旧配置导入与回退，私人报告只留 D 盘。提交通用实现和脱敏验收说明。

**Acceptance:** 原版文件不变，迁移可检查、可取消、可回退；坏数据不造成命令集被清空。

## Task 8：补齐原版快捷操作和 Windows 生命周期

**Files:** `crates/altrun-windows/src/{hotkey,tray,single_instance,startup,send_to}.rs`、`crates/altrun-app/src/input.rs`、`crates/altrun-app/tests/input_compat.rs`、`crates/altrun-windows/tests/instance_handoff.rs`。

**Interfaces:** 扩展 AppEvent/AppEffect，输入仍由同一控制器负责，系统集成统一返回 Result。

- [ ] 按矩阵编写数字两种顺序、Tab/Shift+Tab、翻页、Enter/Space、分号/单引号、Ctrl+D、复制、最近列表与最近执行热键的失败测试；IME 中所有这些键不得误启动。
- [ ] 实现热键替换事务：新注册成功才移除旧绑定；冲突时旧绑定仍可用。实现主/次热键、自启动、托盘菜单、延迟隐藏、启动显示/通知、执行后隐藏/退出、声音与唯一匹配自动执行。
- [ ] 用互斥体与仅当前用户可访问的 IPC 做单实例；第二次启动转交“呼出/发送到文件列表”，不重复打开主进程。恢复托盘 Explorer TaskbarCreated；旧前台窗口失效时不操作新占用该句柄的窗口。
- [ ] 云端测试状态转换/IPC 拒绝越权/参数传递；本机测试登录、Explorer 重启、休眠/解锁、多屏拔插和重复启动。注册自启动/发送到前保持设置开关明确可撤销。
- [ ] 全部已审计快捷键在矩阵对应测试记录后提交，不用网页快捷键测试充当本机证据。

**Acceptance:** 长期使用路径和边界场景均能工作，系统集成失败可见且不留下半配置状态。

## Task 9：完成命令管理、失效检查与窗口工具

**Files:** `crates/altrun-app/src/{command_manager,command_editor,path_tools}.rs`、`crates/altrun-windows/src/{path_validation,window_actions}.rs`、`crates/altrun-app/tests/manager_flow.rs`。

**Interfaces:** 复用 CommandStore 与 executor，不在编辑器重复解析命令。

- [ ] 为新建/编辑/取消、未保存离开、重复快捷字保留选择、删除撤销、失效检查与路径转换编写失败测试；同名不同 ID 不得删除错误对象。
- [ ] 按原版内容提供快捷字、名称、参数类型、命令行及必要高级项；大列表虚拟化，搜索/选择/滚动状态在保存后保持。只有保存通过验证后才更新数据。
- [ ] 实现拖入/发送到添加、绝对/相对路径转换。失效检查异步限时；网络超时标“未确认”，批量删除/转换先预览具体条目与数量，不直接清理。
- [ ] 用 Windows API 重写原版 ShowOnly/Hide/UnHide 等窗口工具；先保存可恢复的窗口集合，检查句柄与进程身份，受保护/提权窗口返回明确结果。不下载未知 WinCtl 二进制作为替代实现。
- [ ] 云端运行 `cargo test --locked -p altrun-app --test manager_flow` 及 fake-window adapter 测试；本机使用测试窗口验证恢复能力和非目标窗口不受影响，再提交。

**Acceptance:** 所有管理功能可用，检查不直接删除，窗口隐藏操作可恢复且不误操作。

## Task 10：补齐外观、语言和原生可访问性

**Files:** `crates/altrun-app/src/{preferences,theme,locale}.rs`、`crates/altrun-windows/src/accessibility.rs`、主题与语言资源、`crates/altrun-app/tests/preferences_compat.rs`。

**Interfaces:** UI 使用同一 Settings/主题 token；UIA 与控制器共用实际选中状态。

- [ ] 按原版配置建立映射测试，覆盖字体/列表格式/宽度/圆角/透明度/背景图、提示/命令行/工具按钮开关及语言包；不能把无法映射的旧值悄悄丢弃。
- [ ] 实现浅色/深色/系统主题、高对比和可关闭背景效果；默认保持紧凑、无应用图标列。资源按需加载，关闭设置释放不需要的对象，不使待机预算持续上升。
- [ ] 为自绘结果提供 UIA 名称、选中、执行、滚动和焦点语义；原生输入保留 IME/复制/选择；管理与设置不能被启动面板兼容 Tab 行为抢键。
- [ ] 原版语言能力和自定义设置逐项接通；语言与字体切换时检查长文本、候选框和对话框布局，透明/皮肤选项不能破坏可读性。
- [ ] 云端运行设置映射测试；本机用 Narrator、全键盘、主题/DPI、长名称与真实 IME 验收；在功能矩阵登记覆盖，再提交。

**Acceptance:** 外观现代化不删原功能，默认界面仍是启动工具；原生可访问性有实际证据。

## Task 11：完整功能、性能和长稳验收

**Files:** `scripts/measure.ps1`、`docs/validation/{windows11,performance,soak,compatibility}.md`、最终回归测试。

**Interfaces:** 验收报告以 commit SHA、包 hash、Windows 版本和测试数据规模关联，不把不同构建的结果混在一起。

- [ ] 清点矩阵：每个旧功能都必须是“通过＋证据”或明确未完成。未完成必要项不能宣布完整复刻；修复后复测对应场景。
- [ ] 在用户 Windows 11 对同 SHA Release 包，用 1 万条脱敏数据测：待机 Private Bytes ≤30 MiB、工作集 ≤60 MiB、显示面板 Private Bytes ≤50 MiB；全部自身进程计入，GPU/共享内存单列，不裁剪工作集。
- [ ] 测待机 5 分钟 CPU 均值 <0.1% 总 CPU；100 次热唤醒 P95 ≤100ms；200 次普通查询端到端 P95 ≤30ms；冷启动 20 次 P95 ≤500ms；首次扫描与外部噪声单列。
- [ ] 执行 1000 次以上呼出/隐藏、重复打开管理与设置、复杂正则超时/worker 恢复；运行 24 小时长稳，检查崩溃/死锁、Private Bytes、GDI/USER 句柄和线程趋势，预热后内存增长目标 ≤5 MiB 且无持续上升。
- [ ] 下载、真实桌面测试和 24 小时观察均在当前任务跟进到结果，不用未来提醒代替验收；用户尚未提供可运行桌面时保留待验收状态。
- [ ] 云端全量 fmt/clippy/test/build 通过，并确认本机验收报告匹配最终 SHA；必要改动之后只重跑受影响检查和最终发布构建，避免无意义重复。

**Acceptance:** 全功能和资源预算均有实际证据；云端 CI 绿灯不是本任务的唯一标准。

## Task 12：开源交付与便携发布

**Files:** `.github/workflows/release.yml`、`README.md`、`CHANGELOG.md`、`THIRD-PARTY-NOTICES.md`、许可文件（需先确认许可选择）、打包脚本。

**Interfaces:** ZIP、SHA-256、构建元数据、兼容报告和更新说明作为同一版本发布资产。

- [ ] 确认新代码的开源许可证；保留现有许可证，不擅自替换。梳理全部依赖和素材来源，未确认许可的旧代码/皮肤不纳入公开成品。
- [ ] 便携包不带真实 Data、下载缓存、调试转储和开发工具；首次运行在旁边创建 Data，程序目录不可写时提示。验证在未安装 Rust/VS 的本机可启动，必要运行库随包合法附带或使用经验证的静态 CRT 配置。
- [ ] 发布工作流仅手动触发并从通过验收的 SHA 构建；默认创建草稿 Release，检查文件、hash、说明和许可证后再公开。未完成验收的包标预览版，不能写成全功能完成版。
- [ ] 文档写明完整功能、已知差异、导入/备份/回退、最低系统、资源测试条件和当前是否签名；不将代码签名或 SmartScreen 信誉当成免费 CI 自带能力。
- [ ] 检查最终源码/产物 SHA、下载重验 hash、安装工具缺席环境的运行结果；公开发布后验证下载链接可用，保留回退版本。

**Acceptance:** GitHub 上的源码、发布包、报告可互相追溯，用户只下载便携包即可使用。

## 检查命令与完成标准

以下命令**在 GitHub Windows runner 上执行**；本机没有工具链时不尝试运行 Cargo：

```text
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --release --locked --target x86_64-pc-windows-msvc
```

本机只做不依赖编译环境的 hash 校验、运行成品与桌面验收；本机脚本通过进程级 TEMP/TMP 指向 D 盘。不得把任务 2 的 smoke 成功、任务 4 的骨架或任何网页测试结果当作整项目完成。

每个任务按“失败用例 → 明确失败 → 最小实现 → 云端通过 → 必要本机验证 → 提交/审查”闭环推进。修改共享接口时同步消费者与测试；阶段结束附 SHA、已通过项目、待验收项目和下一步。CI 的链接/日志是证据，不只口头报告成功。

## 计划自检

- 已覆盖主方案的旧功能、紧凑 UI、迁移、可靠性、外部窗口工具、可访问性、资源预算与便携发布。
- 五项 Review Focus 分别落在任务 1、3、4/8、6、7/9，拥有相应验收条件。
- 搜索、模板、持久化、导入和执行接口的输入/输出集中定义，后续任务不自行改名。
- 未验证的事实被分配给明确检查任务：远程历史/写权限 → 任务 1；旧编码/语法 → 任务 3；原生可用性/性能 → 任务 4/11；许可与运行库 → 任务 12。
- 本次没有创建 CI、安装工具、提交/推送或公开发布；这些都属于未来执行阶段。

## 权威参考

- 标准公开仓库 runner：https://docs.github.com/en/actions/reference/runners/github-hosted-runners
- Windows 镜像软件清单：https://github.com/actions/runner-images/blob/main/images/windows/Windows2025-Readme.md
- Actions 免费/计费边界：https://docs.github.com/en/billing/concepts/product-billing/github-actions
- 构建产物：https://docs.github.com/en/actions/tutorials/store-and-share-data
- Rust 国内镜像：https://rsproxy.cn/
- Rustup 路径配置：https://rust-lang.github.io/rustup/environment-variables.html

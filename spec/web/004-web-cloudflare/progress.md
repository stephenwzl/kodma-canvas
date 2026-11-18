# S004 Web / Cloudflare 迭代进度

更新日期：2026-11-18。规格：[v1.2](spec.md)。状态：**暂停，I1–I5 未开始。当前优先 macOS**。

## 当前确认门槛

用户选择 Cloudflare、每稿 Durable Object + 附属 SQLite 持久化文档、R2 仅图片等大资源、复用 FigCraft 增量同步协议。恢复 Web 排期后，先由用户确认 v1.2 架构图与规格，再启动 I1。

## 已确认的方向

- Cloudflare 为 Web 部署目标；本页单独跟踪 Web。
- 用户明确 Web 首版包含登录、云端保存和跨设备打开。
- UI 优先：设计核对 → 浏览器高保真运行与用户视觉确认 → 功能保障与测试。
- 保留完整 FigCraft Rust/WASM 运行时；Web 宿主独立定义，不沿用 AppKit / 本机路径 / 系统废纸篓。
- 文档 DO / WebSocket 增量会话是首版基础；Agent、成员 / 分享产品面板暂缓，后续云端 MCP。
- R2 不存日常整稿快照 / 日志；D1 仅元信息，文档 seq / 状态由文档 DO SQLite 持有。

## v1.2 迭代状态

| 阶段 | 状态 | 对应证据 |
| --- | --- | --- |
| I1 | 暂停，未开始 | 核稿；T01–T02 隔离公网带图保存、重启与双设备回读 |
| I2 | 未开始 | V01 实际浏览器两组截图与用户视觉确认 |
| I3 | 未开始 | F01–F02 真实登录、管理与权限 |
| I4 | 未开始 | F02–F04 失败恢复、隔离与收敛 |
| I5 | 未开始 | R01 公网亲验与发布准备 |

本次更新首发范围及四张 live 架构画板。尚无 I1 云端运行数据、产品 UI 截图或公开部署。以下为旧版历史记录。

## v1.1 图文更新（历史）

- 静态源码基线为 b13604e1，核对 FigCraft 属性级变更、client / batch / sequence、原持久化钩子及图片携带字节的定义；不新写实现。
- 经 FigCraft MCP 确认当前协作 live 后更新同一文件的架构页。当前页改为“Web 架构 v1.1 · 待确认”，四张画板覆盖拓扑、增量确认、资源外置、恢复与环境 / 门槛；原五张草案移入历史页，保留原生 CLI 合成样本而不冒充 DO 测试。定位见 [架构图索引](../../../docs/kodma/web-architecture.md#live-架构图定位)。
- 同步 S004 范围、规则、实施顺序、case 与产品入口，新增 D01–D08，撤销“R2 整稿保存”和“旧 revision 一律拒绝”的旧路线，明确图片桥接、D1 / DO / R2 非全局事务、PITR 与资源保留协调。
- 官方依据为 tldraw sync 和 Cloudflare DO SQLite / 生命周期文档；只参考公开职责分层，无第三方代码 / 资产或新依赖。
- 本轮未重新逐项核对高保真 UI、未构建 / 启动产品、未测试 DO 运行时、未创建 Cloudflare 资源、未部署。图文确认后再开始 I1。

本轮检查：四张画板逐张渲染观察通过，从另一个 live 客户端回读确认当前四张及历史五张画板；既有原型 / 高保真 / Design System / 原技术架构页保留。9 份变动均为 Markdown，本地链接目标、`cargo xtask docs` 和 `git diff --check` 通过。

提交前实际执行通用 `cargo xtask ci`：fmt、全工作区 Clippy 与桌面端 104 项测试通过；collab-net 的已知 `code_tests::a_share_on_every_interface_names_an_address_others_reach` 再次超过 60 秒未结束，终止本次对应测试进程，CI 以 SIGTERM 失败退出。没有到达后续 engine 测试及门禁，不宣称本轮 CI 全绿，也不把历史 3 项 engine 失败当作本次复跑结果。未修无关测试、未改 CI；这些通用检查不算 DO / Web 产品验收。

本次图文已形成可复核版本，状态保持待用户确认；没有开始 I1、添加依赖 / 应用代码 / 资产或进行任何 Cloudflare 资源操作。

## v1 初始规格准备与历史检查（保留）

源码基线 `63bcf296`。已读取产品路线图、设计 / 工作区 / 原生架构及 S003；核对 `figcraft-web`、同页 host bridge、页内 API、内存 saved_files、CanvasHost、Web 打包和 native-only server 文档。确认有可复用的完整浏览器运行时入口，但原生服务器不能直接当作 Worker；save 事件发送、最多 16 份的内存捕获均不是云端或浏览器持久化证明。

查阅 Cloudflare 官方 Static Assets、SPA 路由、Workers 限制、D1 / R2 binding、R2 一致性与 Secrets 文档，建立 Workers + D1 + R2 方案；链接见 [Web 架构](../../../docs/kodma/web-architecture.md)。未实际构建 Web 产物、测量本轮 WASM、连接 live 设计、操作浏览器或部署。

本次仅新增 / 更新 Markdown：Web 规格、case、架构、进度和入口；S003 标记暂停 P2，历史范围保留；S001 / S002 不变更验收结论。origin 未找到相关 Web / Cloudflare PR，issues 已禁用；不评论无关上游 issue，不创建开发分支。

本次检查：14 份变动文件均为 Markdown，本地链接目标、`cargo xtask docs` 和 `git diff --check` 通过，无应用代码、依赖、资产或 CI 配置变更。

按仓库提交前规则实际执行 `cargo xtask ci`：fmt、全工作区 Clippy 通过；桌面端 104 项、collab-net 112 项通过；engine 2059 通过、3 失败、3 忽略。失败与此前基线一致：

- `board_mindmap_tests::a_mind_map_is_a_first_node_with_three_children_on_branches`
- `board_mindmap_tests::more_lines_push_the_siblings_apart`
- `board_mindmap_tests::two_people_adding_at_once_end_with_one_layout`

CI 失败退出，后续检查未执行，不宣称全绿。本次未构建 / 启动 Kodma Web、运行浏览器或部署 Cloudflare；通用仓库检查不代表 Web 或登录 / 云端验收通过。未因本次文档调整修复无关引擎测试，也未继续 macOS 功能开发。

## 修订记录

| 日期 | 记录 |
| --- | --- |
| 2026-11-17 | 修订 S004 v1.1：参考 tldraw 分层，DO SQLite 持久化文档、R2 只存大资源，复用 FigCraft 增量协议；live 架构图与验收同步，等待用户确认后才开发。 |
| 2026-11-17 | 从头建立 S004 v1 Web / Cloudflare 规格，首版包含真实登录、云稿与跨设备；UI 优先，完整运行时，Agent 暂缓；macOS / S003 暂停 P2。仅规格，开发与部署未开始。 |

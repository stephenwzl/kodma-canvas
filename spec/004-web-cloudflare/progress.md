# S004 Web / Cloudflare 迭代进度

更新日期：2026-11-17。规格：[v1](spec.md)。优先级：**P1**。状态：**规格已建立，开发、运行验收与部署未开始**。

## 已确认的方向

- 用户将 Web 设为 P1，Cloudflare 为部署目标；macOS 暂停 P2，保留后续恢复，非取消 / 废弃。
- 用户明确 Web 首版包含登录、云端保存和跨设备打开。
- UI 优先：设计核对 → 浏览器高保真运行与用户视觉确认 → 功能保障与测试。
- 保留完整 FigCraft Rust/WASM 运行时；Web 宿主独立定义，不沿用 AppKit / 本机路径 / 系统废纸篓。
- Agent 暂缓，后续云端 MCP；首版不做本地 Agent 产品接入或实时协作服务。

## 迭代状态

| 迭代 | 状态 | 下一步 |
| --- | --- | --- |
| I1 · Web 设计与技术基线 | 未开始 | live 核对设计、浏览器基线 / 字体 / 产物大小；明确 Web 桥接、真实登录 provider、API / schema / 存储与部署配置 |
| I2 · Web 高保真 UI | 未开始 | 实际浏览器截图与设计对照，用户视觉确认 |
| I3 · 登录与云端稿件 | 未开始 | 服务端会话、用户隔离、云端新建 / 保存 / 列表、跨环境回读 |
| I4 · 文件管理与恢复 | 未开始 | 本地稿与名额、搜索 / 导入下载 / 软删除、离线 / 幂等 / 冲突 |
| I5 · Cloudflare 与亲验 | 未开始 | 隔离公网部署、双设备 / 双账户、浏览器矩阵、版本与回滚、最终亲验 |

AC01–AC12 / C01–C12 均未执行；暂无 Kodma Web 运行截图、Cloudflare 地址、登录 provider / 账户、绑定资源或云端保存证据。不能用 FigCraft 参考 Web 的已有功能填充本轮“通过”。

## 本次规格准备

源码基线 `63bcf296`。已读取产品路线图、设计 / 工作区 / 原生架构及 S003；核对 `figcraft-web`、同页 host bridge、页内 API、内存 saved_files、CanvasHost、Web 打包和 native-only server 文档。确认有可复用的完整浏览器运行时入口，但原生服务器不能直接当作 Worker；save 事件发送、最多 16 份的内存捕获均不是云端或浏览器持久化证明。

查阅 Cloudflare 官方 Static Assets、SPA 路由、Workers 限制、D1 / R2 binding、R2 一致性与 Secrets 文档，建立 Workers + D1 + R2 方案；链接见 [Web 架构](../../docs/kodma/web-architecture.md)。未实际构建 Web 产物、测量本轮 WASM、连接 live 设计、操作浏览器或部署。

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
| 2026-11-17 | 从头建立 S004 v1 Web / Cloudflare 规格，首版包含真实登录、云稿与跨设备；UI 优先，完整运行时，Agent 暂缓；macOS / S003 暂停 P2。仅规格，开发与部署未开始。 |

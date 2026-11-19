# Kodma AI · 通用会员与托管 Agent 服务最小规格

2026-11-19。待开发。与 [S005 Mac Agent](spec.md) 并行交付，作为 I3 托管模式的验收依据。

## 交付边界

购买的是 **Kodma AI 会员**。权益和额度归 Kodma 账号，所有接入 Kodma AI 的产品共用一个余额。用户可以在已接入购买的产品通过 App Store 或 Google Play 订阅，再到另一个已接入的产品登录同一账号使用。Canvas macOS 是首个权益消费端。Rust Pi 继续在 Mac 运行，FigCraft 负责画布操作、撤销与落盘。自定义端点沿用 S005 的直连路径。

首轮做一个 `kodma_ai` 权益、一个托管模型、一个商店沙盒的端到端验收。另一商店及后续有购买入口的 Kodma 产品沿用同一权益和接口。第二个产品的客户端实现、公开价格、正式额度与 Credits 单独排期。

## 服务归属

| 层 | 职责 |
| --- | --- |
| Kodma Account | 登录与令牌签发，以及通用会员、RevenueCat 身份映射、共享额度、原子预留和用量结算。 |
| RevenueCat | 统一处理 App Store 和 Google Play 验购及订阅状态。 |
| 各产品 API | 校验本产品的 Kodma 访问令牌，通过受信任的服务间调用传递已验证的账号身份及产品标识。Canvas 先复用 S006 的受众。 |
| 推理网关 | 持有模型密钥、转发 SSE 和调用模型。调用 Account 的权益和额度接口，在上游结束后回写实际用量。 |

Canvas API 对接推理网关。会员和额度由 Account 内的独立模块按账号统一管理，流水记录来源产品。内部 API、幂等结算和验收以 Account 仓库的[通用会员与共享额度接入规格](../../../../kodma-account/docs/kodma-ai-membership-spec.md)为准。

## RevenueCat 接入与身份

各销售 App 在各自商店独立建 SKU，统一映射到 Kodma AI 会员，同一账号共享权益和额度。参与会员的 App Store 版本须在同一开发者账号下分别配置等价的自动续订内购，适用于通过 Mac App Store 发布的 Canvas。未来单独售卖的 Credits 受购买商店规则约束，Google Play 的付费应用内虚拟货币限购买 App 内使用。详见 [Account 规格的商店商品与跨 App 权益](../../../../kodma-account/docs/kodma-ai-membership-spec.md#商店商品与跨-app-权益)。

1. Apple 和 Google Play 的 Kodma AI 订阅商品放在同一个 RevenueCat Project，映射到 `kodma_ai`。购买入口在 Kodma 登录后启用。Kodma Account 为 `(issuer, sub)` 分配并保存稳定的随机 UUID。所有购买端为同一个 Kodma 账号使用这个 RevenueCat App User ID。切换 Kodma 账号时，购买端切换 RevenueCat 身份后才允许购买或恢复。
2. 恢复购买采用 RevenueCat 的 `Keep with original App User ID`。首轮验证同一 Kodma 账号恢复成功、另一 Kodma 账号恢复同一购买时无法获得权益。首轮用隔离的测试配置和商店沙盒，正式权益仅接受生产购买。
3. 产品 API 校验 Kodma access token 的签名、`iss`、自身的 `aud`、有效期和 `kodma:access` scope。Canvas 测试 API 沿用 `https://canvas-api-test.kodma.app` 受众，其他产品使用各自登记的受众。Account 通过受信调用取得已验证的 `(issuer, sub)` 和来源产品，由 Account 查询 RevenueCat。Mac 如仅消费权益，只需使用 S006 的账号会话调用 Canvas API。
4. 推理网关发起托管任务时调用 Account 实时查询 `kodma_ai`。有效才允许启动，查不到或查验失败时在调用模型前返回可区分的错误。对话的后续模型轮次复用服务端记录的短时任务授权，授权绑定发起账号和来源产品，最长到权益有效期，且受任务最长时长限制。首次联调可给隔离测试账号发放带到期时间的测试权益，沙盒验收须使用实际商店购买产生的权益。缓存和 webhook 等到有延迟或调用量需求再接入。

[Kodma Account 接入契约](../006-kodma-account/spec.md#测试环境与接入契约) · [RevenueCat 用户身份](https://www.revenuecat.com/docs/customers/identifying-customers) · [恢复购买规则](https://www.revenuecat.com/docs/projects/restore-behavior) · [RevenueCat 服务端查询](https://www.revenuecat.com/docs/api-v2/customer/resources)

## 月额度与模型请求

- 首轮测试以 UTC 自然月为周期，每个 Kodma 账号每月一份预算，接入产品共用。同档多渠道订阅、从另一产品登录、重装、恢复购买及当月重新订阅都沿用当月余额。次月使用新预算，未用额度不结转。一次模型调用计入预留发生的月份。正式套餐的周期、额度与价格另行确定。
- Canvas API 提供 OpenAI 兼容的 `POST /v1/chat/completions` 和 SSE。Mac 通过 `Authorization: Bearer` 发送 Kodma access token，通过 `X-Kodma-Task-ID` 发送任务 UUID，通过 `Idempotency-Key` 发送逐轮请求 UUID。同一轮网络重试沿用请求 ID。首轮验证权益，后续轮次验证任务归属和有效期。服务端限制单次输入、输出和每任务调用轮数，上游密钥保存在服务端。
- 月度账户与逐轮调用记录持久化。每轮先根据服务端允许的请求上限计算最大用量，再在同一事务内创建调用记录和预留额度。余额不足时直接拒绝。各产品的并发请求共享同一余额，按账号、来源产品和逐轮请求 ID 去重。已在处理或结束的请求返回已有状态，避免重复发起上游请求或重复扣费，首版无需回放 SSE。
- 已知实际用量时原子结算并释放差额。确认上游尚未接收时释放全部预留。取消或断线后若收到可核对的用量，按该用量结算。上游是否接收或用量不明时保留预留，记录待核对状态并通过上游记录或测试运营核对后结算，不自动重发。以请求 ID 及上游请求标识对账。
- Mac 区分登录失效、无有效权益、额度不足、查验暂不可用和模型调用失败。仅在现有操作位置显示必要反馈。日志记录请求 ID、阶段、结果和用量，避免记录令牌、上游密钥、提示词或稿件内容。

## 并行开发与合流

| 工作 | 与 Mac I1 的关系 | 交付 |
| --- | --- | --- |
| Mac S005 I1 | 可独立启动 | 模拟 SSE 调进程内画布工具，验证原目标路由、单步撤销、保存重开、停止和重复回调。 |
| 会员、额度与网关联调 | 可同步启动 | Account 接 RevenueCat 查询与账号级额度，网关接预留、流式转发和用量回写。使用可控的模型与权益响应验证拒绝、并发及结算。 |
| 商店沙盒 | 可同步准备 | 创建首个沙盒商品并映射权益，实现登录后的购买和恢复。实际沙盒验收依赖测试账号、已配置商品及 Canvas API 的权益查询。 |
| Mac 联调 | 在 S005 I3 合流 | 对齐令牌、任务和请求 ID、错误返回与 SSE 契约。先验证托管改稿，再跑沙盒购买到 Mac 改稿。 |

## 首轮验收

| 用例 | 通过标准 |
| --- | --- |
| Mac I1 独立验收 | 使用模拟端点完成 [S005 I1](spec.md#设计与验收) 的进程内改稿用例，记录实际构建及结果。 |
| 购买后在 Mac 使用 | 在一个商店沙盒购买，同账号恢复购买成功。以同一 Kodma 账号登录 Mac，用托管 Agent 执行「改按钮颜色」和「生成可编辑卡片」。结果可编辑、可单步撤销、保存重开后可见。按请求 ID 核对预留和实际扣减。 |
| 权益失效及账号隔离 | 用另一账号恢复同一购买并尝试调用，验证无权益。真实沙盒购买到期且 RevenueCat 返回失效后，下一次任务在调用上游前被拒绝。用受控状态验证退款及查验暂不可用，核对实际商店退款通知能否传到 RevenueCat。 |
| 额度与重试 | 注入较低月额度，验证耗尽拦截、两设备并发不会超出预算、同请求重试只调用和结算一次。模拟两端同档有效权益及当月恢复，余额不增加。跨月进入新预算。 |
| 跨产品共用额度 | 用 Canvas 与另一个模拟的已登记产品代表同一测试账号发起任务。两边看到同一份权益并共同扣减同一余额，流水能按产品追踪。用错产品受众的令牌应在产品 API 被拒绝。 |
| 停止及断线 | 上游已确认与用量未知两种情形分别验证结算或待核对。Mac 停止后拒绝迟到的画布修改。 |

验收记录包含测试商店、Mac 构建、脱敏请求 ID、权益结果、上游调用次数及额度流水。首轮用受控数据覆盖双渠道合并；两个真实商店各自的完整沙盒联测作为后续交付。


## 第二阶段本地联验

2026-11-19 · 用户已验收第二阶段。使用 macOS 27.0.1 arm64 原生 Canvas、隔离稿件、Canvas API、推理网关、Account D1 和模拟模型 SSE。已通过 `--validate-kodma-hosted-edit` 发起工具调用，指定稿件填充改为 `#7C3AED`，前台另一份稿件保持原样。单次撤销、重做及保存重开均通过。模型工具调用与工具回执续轮分别结算 20、26 单位，预留归零。

联验复用 Canvas 的控制通道和引擎命令，使用测试访问令牌及模型响应。交互式 Agent 面板、Rust Pi 和真实模型或商店接入仍由各自阶段验收。 [Account 验证记录](../../../../kodma-account/docs/kodma-ai-membership-spec.md#第二阶段本地验证)

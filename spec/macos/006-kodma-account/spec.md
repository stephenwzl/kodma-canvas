# S006 · macOS 接入 Kodma Account

更新日期：2026-11-19。版本 v0.1，规格已由用户确认。以已验收的 [S003 本地工作区](../003-drafts-files-ui/spec.md)为基线，I1 先核对系统版本、回跳方式和客户端登记参数。[S005 Agent](../005-agent/spec.md)继续挂起，[Web S004](../../web/004-web-cloudflare/spec.md)继续暂停。

## 目标与范围

用户从 Kodma Canvas 打开系统浏览器，用已有 Kodma 账号登录，再回到发起登录的 Mac App。关闭并重开 App 可以恢复会话，用户可以从 App 退出，也可以在 Kodma Account 网站撤销这一台设备的会话。

本轮使用现有 Cloudflare 测试环境 `https://account-test.kodma.app`。账号网站负责邮箱验证码、浏览器会话与切换账号。Canvas 负责原生登录入口、回跳、凭据保存、恢复和退出。离线或未登录时仍可创建、编辑、保存和重开本地稿件。登录、切换账号和退出都不改变本地稿件、路径、列表或打开的标签。

本轮范围为 Mac 测试构建的账号接入。生产发布、Frame 原生接入、云稿、业务权限、计费和 Agent 留给各自阶段。

[账号服务接入指南](https://git.oldiron.work/old_iron_works/kodma-account/src/branch/main/docs/integration.md) · [测试环境 OIDC Discovery](https://account-test.kodma.app/.well-known/openid-configuration)

## 用户流程

| 状态 | App 行为 |
| --- | --- |
| 未登录 | 从一个原生账号入口打开「登录」。点击后由系统浏览器打开 Kodma Account。 |
| 等待浏览器 | App 显示等待状态并允许取消。同一次尝试只接受一次有效回跳。失败或超时后可重试，重试使用一组新参数。 |
| 已登录 | 账号入口显示账号身份和「退出登录」。复用浏览器已有会话时，由账号网站提供继续使用或切换账号。 |
| 恢复会话 | 重启后读取钥匙串并验证会话。无网络或暂时服务不可用时保留可用的本地会话记录并允许重试。凭据被明确拒绝时清理会话并回到登录入口。 |
| 退出 | 点击后清除本机账号状态与钥匙串凭据。在线时请求撤销本次 App 会话。离线或撤销失败时仍可本地退出，并明确服务端会话可在账号网站撤销。 |

评审时从 [FigCraft「Kodma · Mac Agent 与统一账号」](https://figma.oldiron.work/files/1061fa703c391f22b02f623c)取 Mac 登录入口、等待回跳和已登录状态为参考。以测试账号网站的实际操作为准。沿用原生工作区，界面只保留账号身份、操作和必要状态反馈。

## 测试环境与接入契约

1. **客户端登记**。Canvas macOS 测试构建使用独立公共客户端 `kodma-canvas-macos-test`，显示名为 Kodma Canvas，平台为 macOS，`resource` 为 `https://canvas-api-test.kodma.app`。客户端认证方式为 `none`，允许授权码和刷新，要求 PKCE。网页客户端 `kodma-canvas-test` 和 `kodma-frame-test` 可作为对照。
2. **应用身份与回跳**。[Kodma 验证包](../../../script/build_and_run.sh)固定 Bundle ID 为 `app.kodma.canvas`，最低版本为 macOS 14.0。回跳使用 `ASWebAuthenticationSession` 和精确 URI `app.kodma.canvas:/oauth/callback`。Mac 原生回跳和覆盖安装后的钥匙串会话列入后续实测。
3. **浏览器授权**。构建固定测试 issuer，从 Discovery 获取授权、令牌、JWKS 与撤销端点，校验 Discovery 返回的 issuer。采用授权码和 PKCE S256，每次独立生成 `state`、`nonce` 和 verifier。scope 为 `openid offline_access kodma:access`，使用登记的 `resource`。Discovery 当前未声明客户端认证方式 `none`，接入需明确按公开客户端换码与刷新，实测兼容性。
4. **回跳和身份**。仅处理匹配当前登录流程、登记 URI、`state` 与 `iss` 的回跳。校验 ID Token 的 ES256 签名、`iss`、`aud`、有效期与本次 `nonce`，并按服务契约核对访问令牌的受众和 `sub`。以可信的 `sub` 作为测试环境的 Kodma 用户 ID。邮箱仅用于登录和显示。取消、超时、退出或重试后，使旧回调和迟到响应失效。
5. **会话生命周期**。刷新凭据按 issuer、客户端与当前会话存入 macOS 钥匙串。每个会话串行刷新，成功后保存轮换凭据。刷新结果不明时避免反复发送旧凭据。退出与刷新协调，防止旧请求恢复本机已退出的会话。日志只记录产品、环境、阶段、请求 ID 和错误类别。截图与日志都避开邮箱验证码、授权码、回调完整 URL、Cookie 和令牌。

2026-11-19 已读取公网测试 Discovery，确认 issuer、四个端点和 PKCE S256。服务方已用两个网页测试客户端验证真实邮箱收码、登录复用与会话撤销。Canvas macOS 客户端已登记到测试环境，公网授权请求可打开账号登录页。账号服务的隔离测试已验证自定义 scheme 的授权码兑换和刷新。Mac 原生回跳及钥匙串仍待实施和验收。

## 实施顺序与验收

| 阶段 | 完成标准 |
| --- | --- |
| I1 · 接入准备与原生回跳 | 核对系统版本，确定回跳方式、测试 bundle 与登记值。用隔离 Mac 测试构建验证系统浏览器登录、真实邮箱收码、回到正确 App、兑换授权码并验证身份。验证错误 `state`、错误 issuer 和非当前尝试的回调被拒绝。 |
| I2 · 原生账号界面与会话 | 提供实际 App 的未登录、等待、已登录、取消和失败重试截图供视觉验收。验证重复点击、超时、迟到回跳，App 重启和刷新凭据轮换，钥匙串不可用时显示失败并可重试。 |
| I3 · 测试环境联验 | Mac 和现有 Frame 网页测试客户端用同一浏览器登录，核对同一个 Kodma 用户 ID、登录复用和独立会话。账号网站撤销 Mac 会话后，Mac 无法继续刷新，Frame 仍能刷新。验证 Mac 在线退出、离线退出、退出与刷新重叠。用隔离本地稿件验证未登录、退出和换账号后仍可打开、编辑、保存和重启恢复。 |

用户亲验主线为「打开 Mac 测试构建 → 登录 → 浏览器收码或复用会话 → 返回 Canvas → 重启恢复 → 退出」。交付实际原生窗口截图、测试环境和构建版本、已脱敏的用例结果。Frame 网页对照结果记录为协议与会话隔离验证，双原生验证留待 Frame 接入后。

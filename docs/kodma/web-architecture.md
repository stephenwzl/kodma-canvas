# Kodma Web 与 Cloudflare 架构

更新日期：2026-11-17。状态：**S004 v1 实施方案，尚未实现 / 部署**。Web P1；macOS 暂停 P2、保留后续恢复。范围与顺序以 [S004](../../spec/004-web-cloudflare/spec.md)为准；macOS 历史承载方案见[原生承载](canvas-editor-extraction.md)。

## 已有能力与新增工作

| 已核对的源码 | 当前事实 | Web 产品还需补齐 |
| --- | --- | --- |
| [Web 入口](../../apps/figcraft-web/src/web.rs)、[Trunk 配置](../../apps/figcraft-web/Trunk.toml) | 已有 eframe WebRunner、WASM、WebGPU / WebGL2 路径和浏览器文件服务 | Kodma 产品外壳、正确画布区域、字体与实际浏览器验收 |
| [host bridge](../../apps/figcraft-web/src/host.rs) | 支持同页 `data-host="page"` 和同源 iframe；文件 / dirty 事件桥接 | 采用同页模式；补显式稿件身份、保存请求 / revision 与持久化确认，不能按文件名串联多稿 |
| [页内 API](../../apps/figcraft-web/src/api.rs)、[工作区快照](../../crates/ui-egui/src/workspace.rs) | opt-in 页内请求进入原控制队列，有命令 / 观察能力 | 产品级适配与生命周期；同一运行时，明确目标，不把调试 API 当云端鉴权 |
| [saved_files](../../apps/figcraft-web/src/saved_files.rs) | 按文件名保存最多 16 份内存捕获结果 | 不是持久稿件仓库；需本地 IndexedDB 和云端稿件存储 |
| [CanvasHost](../../crates/ui-egui/src/lib.rs)、[UI 帧](../../crates/ui-egui/src/app.rs) | 已有保留运行时的外围隐藏 / 区域策略 | Web 特定入口、DOM 焦点与坐标适配，不复用 AppKit 视图 |
| [原生服务器](../server.md) | `figcraft-server` 是 native-only 常驻服务，已有共享链接 / 协议 | 不能原样部署 Workers；S004 单独实现身份与快照 API，不先移植完整实时服务器 |
| [打包脚本](../../packaging/web/package.sh) | 可构建静态 Web 包并按未压缩 WASM 文件检查上限，当前默认 25 MiB | 产品独立输出、实测尺寸、Cloudflare 路由 / 缓存 / 环境与发布回读 |

以上是静态源码事实，不代表新宿主、存储、登录或 Cloudflare 已验证。打包说明中的旧体积 / 功能描述不代替当前产物测量。

## 部署与状态归属

浏览器运行 DOM 外壳与完整 Rust/WASM 编辑器；同一 Session 内原命令修改文档。外壳管理路由、登录状态、列表、字段草稿与保存请求，不另起可编辑 JSON 文档模型。序列化快照由运行时提供，浏览器与服务端只保存快照 / 元信息。

首版部署选择 **Cloudflare Workers Static Assets + Worker API + D1 + 私有 R2**。官方支持静态文件与 Worker API 同部署，D1 / R2 通过服务端 binding 使用；这为本方案提供平台基础，不能代替应用验证。[静态托管](https://developers.cloudflare.com/workers/static-assets/)、[D1 binding](https://developers.cloudflare.com/d1/worker-api/d1-database/)、[R2 binding](https://developers.cloudflare.com/r2/api/workers/workers-api-usage/)

- 静态资源：HTML / CSS、WASM glue / `.wasm`、合法素材；页面无服务端渲染硬依赖，真实编辑在浏览器执行。
- Worker：登录回调与会话、同源 `/api/*`、输入限额、用户 / 稿件鉴权、云名额、保存幂等与 revision 校验。
- D1：内部用户 / provider 身份映射、会话（如自持会话）、稿件 ID、owner、名称、当前 revision / 对象引用、删除状态和幂等记录。具体 schema / 索引 / 迁移由 I1 给出；UI 列表读目录，不把完整文件塞进列表接口。
- R2：私有不可变稿件版本，以不可变对象键保存 `.figcraft` 内容；首版可将图片等保留在原生文件内，不另扩素材管理系统。对象读取经过 Worker 所有权检查，不公开 bucket。
- IndexedDB：匿名 / 超名额本地稿、已登录用户独立命名空间的离线检查点及待提交状态；不按名字覆盖，不跨账户暴露。

持久 ID 与 URL 不包含凭据；编辑器 UID 只在当前运行时有效。`/drafts/:id` 必须重新验证访问资格并加载已保存版本，不能靠猜到 ID 获得访问。首版不实现实时多写合并；未来云端 MCP 与协作必须复用稿件 ID、鉴权和版本协议，不能直接绕过当前 revision 更新 R2 文件。

## 云端保存提交边界

1. 浏览器从原运行时获得目标稿件的序列化快照，捕获持久 ID、UID、编辑代次、预期服务器 revision、内容摘要和幂等请求 ID；先完成所需本地检查点，再发起云端保存。
2. Worker 验证会话 / owner / 删除状态、允许大小和请求形状。幂等重试先核对此前提交；不信任前端 owner 或用户提供的任意对象键。
3. 将完整内容写入新的不可变 R2 对象。对象键不能只有客户端基准 revision，否则并发写会覆盖；每次候选保存独立。写失败不修改目录指针。
4. 在 D1 内用预期 revision 条件更新稿件指针及幂等记录；创建 / 恢复还需原子校验名额。并发只有一个请求可推进相同基准版本；失败返回冲突或明确错误。
5. 仅指针提交成功才返回新 revision / 摘要，客户端仅确认对应序列化版本；之后的新修改继续保持待保存。R2 已写但 D1 未提交会留下无引用对象，后续安全回收；旧有效快照不提前删除。
6. 重连或响应丢失时以请求 ID / 服务器状态确认，不盲目重复创建。读取通过 D1 当前已提交指针获取完整 R2 对象；指针不存在 / 对象不可读时返回明确错误，不退成空稿。

**D1 与 R2 没有本方案可依赖的跨服务原子事务。** R2 对象操作的一致性不意味着上述两个服务可原子提交；因此必须通过不可变内容 + 条件指针推进 + 幂等记录处理失败。[R2 一致性说明](https://developers.cloudflare.com/r2/reference/consistency/)

I1 给出实际 API 请求 / 响应、revision 作用范围、事务 SQL、幂等记录生命周期、本地检查点频率、上传上限、故障补偿与安全清理时序；I3 / I4 用失败与并发 case 验证。不得以这一文字方案声称已经具备原子保存或零丢失保证。

## 身份、缓存与运行环境

真实登录 provider 尚未选定，I1 核对现有基础设施后落定最小登录方式；不自行假设本仓库已经有账户系统。会话使用服务端验证的安全方案，覆盖 callback state / 重放、过期 / 撤销、CSRF / 同源写入检查和退出后的本地隐私边界。provider 主体映射内部稳定 user ID，名称 / 邮箱展示值不是授权依据。

Cloudflare / provider 密钥放服务端 Secrets / bindings，不能注入浏览器 bundle、页面配置或日志。[官方 Secrets](https://developers.cloudflare.com/workers/configuration/secrets/)

- HTML 重新验证，内容哈希资源可长缓存；会话和私有稿件响应禁止公开共享缓存。
- 明确 `/api/*`、`/auth/*` 和缺失资源的处理，页面路由才回 SPA；浏览器直达 / 回调验证不能只测首页。[官方 SPA 路由](https://developers.cloudflare.com/workers/static-assets/routing/single-page-application/)
- Workers Static Assets 当前单文件 25 MiB，上限针对原文件；浏览器 WASM 与 Worker 可执行包的限制不同，分别验证。不要引用旧 Web 打包文档的约数作为本轮证据。[官方限制](https://developers.cloudflare.com/workers/platform/limits/)
- 本地 / 测试 / 生产使用不同数据资源、会话配置和回调地址；记录构建 SHA、资源 hash、Worker 版本与迁移版本。生产发布与资源付费需按授权执行；本次只写方案，不创建资源、不部署。

实时协作与云端 MCP 暂缓；未来可评估 Durable Objects 的会话协调，但先做登录、云稿与跨设备保存闭环，不将 DO 当作首版必需依赖。

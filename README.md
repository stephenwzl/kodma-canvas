# Kodma

Kodma 是一个以稿件为单位的 AI 画布工作区，当前优先推进 macOS 版，Web S004 暂停。用户描述想做的界面或流程，在画布上查看结果，再用对话修改。设计支持两种入口：在稿件右侧使用内置 Agent，或让自己的 Agent 通过 MCP 加入同一稿件。

## 当前进展

macOS 的 S001 / S002 已验收关闭，[S003](spec/macos/003-drafts-files-ui/progress.md)本地稿件与原生工作区已由用户验收通过并结项，macOS 下一步优先接入现有 Kodma Account，[Mac Agent S005](spec/macos/005-agent/spec.md)挂起。[Web S004 v1.2](spec/web/004-web-cloudflare/spec.md)继续暂停。

| 想了解什么 | 从这里开始 |
| --- | --- |
| 产品决策、设计稿位置和交互进度 | [Kodma 产品与设计](docs/kodma/README.md) |
| 下一阶段做什么、怎样验收 | [Kodma 路线图](ROADMAP.md) |
| 一句话如何变成画布修改 | [工作区架构与 Agent 链路](docs/kodma/workspace-architecture.md) |
| Web 如何运行及部署到 Cloudflare | [Web 架构](docs/kodma/web-architecture.md) |
| macOS 历史界面如何接入现有画布 | [画布显示与原生承载](docs/kodma/canvas-editor-extraction.md) |

## 运行画布基线

当前可运行 FigCraft 参考编辑器及 [S001 验证宿主](spec/macos/001-canvas-editor/implementation.md)。Kodma 复用其 Rust 文档模型、命令与撤销、画布渲染、协作协议和 MCP 接口。开发时可在仓库根目录启动参考宿主，验证一次编辑的完整行为：

```sh
cargo run -p figcraft
```

组件的构建条件、协议、测试与维护资料见[开发文档索引](docs/README.md)。FigCraft 原有规则见 [apps/figcraft/AGENTS.md](apps/figcraft/AGENTS.md)。源码采用 [MIT](LICENSE-MIT) 或 [Apache-2.0](LICENSE-APACHE) 许可，素材与品牌标识的来源及许可见 [ATTRIBUTION.md](ATTRIBUTION.md)。

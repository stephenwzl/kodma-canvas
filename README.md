# Kodma

Kodma 是一个以稿件为单位的 AI 画布工作区，优先面向 macOS。用户描述想做的界面或流程，在画布上查看结果，再用对话修改。设计支持两种入口：在稿件右侧使用内置 Agent，或让自己的 Agent 通过 MCP 加入同一稿件。

## 当前进展

已完成四页高保真原型、设计系统和架构图；S001 仅画布显示及 AppKit 同窗承载已由用户验收通过。下一阶段 [S002：macOS 原生工作区最小闭环](spec/002-native-workspace/spec.md)已建立规格，按原生首页、稿件标签、固定设计右栏的顺序接入完整 FigCraft 运行时，验收切稿、属性修改、撤销、保存重开和 MCP。S002 应用开发及运行验收未开始；账户、云端名额与模型服务在后续阶段实施。

| 想了解什么 | 从这里开始 |
| --- | --- |
| 产品决策、设计稿位置和交互进度 | [Kodma 产品与设计](docs/kodma/README.md) |
| 下一阶段做什么、怎样验收 | [Kodma 路线图](ROADMAP.md) |
| 一句话如何变成画布修改 | [工作区架构与 Agent 链路](docs/kodma/workspace-architecture.md) |
| macOS 界面如何接入现有画布 | [画布显示与原生承载](docs/kodma/canvas-editor-extraction.md) |

## 运行画布基线

当前可运行 FigCraft 参考编辑器及 [S001 验证宿主](spec/001-canvas-editor/implementation.md)。Kodma 复用其 Rust 文档模型、命令与撤销、画布渲染、协作协议和 MCP 接口。开发时可在仓库根目录启动参考宿主，验证一次编辑的完整行为：

```sh
cargo run -p figcraft
```

组件的构建条件、协议、测试与维护资料见[开发文档索引](docs/README.md)。贡献规则见 [AGENTS.md](AGENTS.md)。源码采用 [MIT](LICENSE-MIT) 或 [Apache-2.0](LICENSE-APACHE) 许可，素材与品牌标识的来源及许可见 [ATTRIBUTION.md](ATTRIBUTION.md)。

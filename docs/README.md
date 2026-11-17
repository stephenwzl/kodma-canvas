# Kodma 开发文档

从[产品设计与进度](kodma/README.md)了解稿件、画布和 Agent 如何交互。当前优先阅读 [Web / Cloudflare 架构](kodma/web-architecture.md)；macOS 暂停 P2，原[工作区架构](kodma/workspace-architecture.md)和[原生承载方案](kodma/canvas-editor-extraction.md)保留供恢复参考，当前里程碑见[产品路线图](../ROADMAP.md)。

分阶段的实施规格、验收标准和迭代记录见 [spec 目录](../spec/README.md)。当前阶段为 **P1 [S004：Web 工作区与 Cloudflare 云端稿件](../spec/004-web-cloudflare/spec.md)**，当前 v1.1 图文待用户确认，确认前不开始 I1 或部署；macOS [S003](../spec/003-drafts-files-ui/spec.md)暂停 P2，S001 / S002 已验收关闭。

以下资料描述 Kodma 使用的 FigCraft 技术组件。命令、crate、包名和文件后缀与现有实现一致；兼容性数据对应上游编辑器的评估日期。

## 引擎、协议与工具

- [FigCraft architecture](architecture.md)
- [figcraft-cli](cli.md)
- [FigCraft MCP server](mcp.md)
- [FigCraft control protocol](control-protocol.md)
- [Real-time collaboration protocol (draft)](collab-protocol.md)
- [figcraft-server](server.md)
- [Penpot import and export](penpot.md)
- [FigCraft plugins](plugins.md)

## 编辑器能力与历史评估

- [FigCraft reference editor](engine-guide.md)
- [FigCraft engine baseline](engine-status.md)
- [FigCraft roadmap: milestones and what's next](roadmap.md)
- [Where FigCraft falls short of Figma](gaps.md)
- [FigCraft parity with Figma](target-app-parity.md)
- [Parity checklist](parity-checklist.md)
- [Auto layout and constraints parity](auto-layout-parity.md)
- [Boards parity](boards-parity.md)
- [Collaboration parity](collaboration-parity.md)
- [Components, variants, styles and variables parity](components-parity.md)
- [Dev Mode parity](dev-mode-parity.md)
- [File format parity](file-format-parity.md)
- [Hardware and platform parity](hardware-parity.md)
- [Localization parity](localization-parity.md)
- [Prototyping parity](prototyping-parity.md)
- [UI and interaction parity](ui-parity.md)
- [Interaction measurements](interaction-measurements.md)

## 维护与发布组件

- [Contributors in the About window](contributors.md)
- [Translating FigCraft](translating.md)
- [Feature naming](feature-names.md)
- [FigCraft egui 0.37 migration plan](egui-037-migration-plan.md)
- [egui 0.37 local implementation — stage 1 notes and follow-ups](egui-037-local-followups.md)
- [Release playbook](release-playbook.md)
- [Releasing FigCraft](releasing.md)
- [Online sharing through the default server: terms and privacy](online-sharing-terms.md)

部署参考宿主时可查阅 [Web 托管](../packaging/web/README.md)及 [Windows 安装](../packaging/windows/INSTALLATION.md)。组件资产来源记录在[署名清单](../ATTRIBUTION.md)。

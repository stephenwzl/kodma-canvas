# Kodma 实施规格与迭代进度

这里保存每个开发阶段的范围、验收标准和实际进度。产品方向见[路线图](../ROADMAP.md)，整体接入方案见[原生承载方案](../docs/kodma/canvas-editor-extraction.md)。

| 编号 | 规格 | 当前状态 | 进度 |
| --- | --- | --- | --- |
| S001 | [保留 FigCraft 运行时的画布显示与原生承载](001-canvas-editor/spec.md) | v2 已完成；用户验收通过（2026-11-16），跨显示比例测试取消 | [迭代记录](001-canvas-editor/progress.md) |
| S002 | [macOS 原生工作区最小闭环](002-native-workspace/spec.md) | v1.2 已完成；用户验收通过并关闭（2026-11-17） | [迭代记录](002-native-workspace/progress.md) |
| S003 | [稿件创建、文件管理与 UI 还原](003-drafts-files-ui/spec.md) | v1 规格已建立；UI 优先，开发与运行验收未开始；Agent 暂缓 | [迭代记录](003-drafts-files-ui/progress.md) |

每个规格使用独立编号目录，`spec.md` 保存目标、范围和验收条件，`progress.md` 保存实施步骤、验证结果、遗留问题及下一步。范围调整记入变更记录，验证结果包含对应提交或代码版本；未执行的检查保持“未验证”。后续完整产品工作区等阶段另建规格，避免把当前阶段无限扩大。

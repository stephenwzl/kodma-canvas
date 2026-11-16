# S002 I1：宿主接口与工作区框架

状态：I1 实现与验证记录；S002 的完整 AC 仍按[进度](progress.md)逐项验收。沿用完整 `Session / FigApp / UiState`，不新增引擎实例或持久化文档模型。

I2 后续实现、文件操作证据与亲验步骤见[首页与本地文件](i2-home.md)。I3 标签 / 关闭与状态隔离见[实施与亲验步骤](i3-tabs.md)。下文保留 I1 当时的接口和边界记录。

## 入口与所有权

`./script/build_and_run.sh --workspace --verify --control 17982` 构建并启动 macOS 工作区；直接运行桌面二进制的参数为 `--native-workspace [FILE]`。未给文件且无恢复稿件时显示原生首页框架，文件参数继续进入原有 `file.open`。默认入口、`--canvas-only` 和 `--native-canvas` 保留；两种 native host 同时指定时拒绝启动。

窗口仍由 eframe / winit 持有，`NSWindow.contentView` 仍是原 WinitView。`native_workspace::Host` 仅持有四个原生区域及其只读控件，退出前移除这些子视图；不替换视图、不接管 GPU layer、不引入 unsafe。I1 使用已验证的 100 ms 安全轮询桥，稳定状态不重建控件，尺寸或首页状态未改变时不重新布局。完整原生编辑菜单与输入上下文随 I4 的可编辑字段接入，I1 没有可编辑原生字段。

`./script/accept_s002_i1.sh` 一键启动框架并运行 I1 专用合成文件 / MCP 验证；输出文件和回读位于忽略目录 `dist/s002-i1/`，每次生成新文件。它不替代完整 S002 A/B 人工验收。

## 快照与刷新

| 观察字段 | 唯一来源 | I1 用途 / 后续约束 |
| --- | --- | --- |
| `documents[].id/name/dirty` | `Session.docs`、`DocState.uid`、`doc.name`、`is_dirty()`；按 `FigApp::shows` 过滤当前窗口 | 原生列表 / 标签摘要；不存文件路径，不复制稿件内容。I1 列表只显示前八份的只读摘要，不是 I2 最近文件列表或 I3 可滚动导航 |
| `active` | 原运行时活动文档 UID | 对照 `doc.list`，索引变化不作为身份变化；重开后取得新 UID |
| `home` | `UiState.home` 或当前窗口无稿件 | 隐藏右栏和画布命中区域，不销毁稿件；S001 的强制画布模式保持原语义 |
| `page / revision` | `DocState.page / revision` | 后续回调目标和刷新依据；首页不暴露旧页面选区 |
| `selection[].id/name/kind/width/height` | 当前 `DocState.selection` 与同一 `Document` 的节点 | I1 只读名称、类型、W/H；MCP、撤销、切稿和选区变化后同步。不是可写数据模型或属性草稿 |

`FigApp::workspace_snapshot()` 在运行时线程读取；宿主在 `background_tick` 之后同步。帧内控制请求若在同步之后改变状态，下一个逻辑 tick 刷新，I1 不承诺零延迟。只读 `ui.workspace {}` 提供同一快照、egui 布局单位的 host insets 和最后已布局画布区域；首页返回 `canvas: null`。它不能通过 `document` 偷切到后台稿件，读回不修改 dirty / 历史。原生实际呈现仍须截图观察，快照不是原生控件已绘制的证明。

## 设计、命令和接入轮次

设计定位见[高保真对照](design-reference.md)。下表区别 I1 已接通与后续 UI 接入。

| 设计区域 / 控件 | 读写接口 | I1 与后续实施 |
| --- | --- | --- |
| 242 左栏、84 顶栏、408 右栏 | `CanvasHost.left_inset/top_inset/right_inset` | I1 原生区域与原画布布局均生效；1440 / 1200 宽和界面缩放需核对；非法值有限化并保留至少一个布局点 |
| 首页标签 / 稿件标签、稿件列表 | 快照 UID；提交时从 UID 重新找 `doc.switch {index}` / `file.close {index}` | I1 是只读摘要；I3 才加入可点击切稿、关闭与 dirty 确认，不用文本标签冒充按钮 |
| 首页新建 / 打开 / 最近 | `file.new {name?}`、`file.open {path}`、`file.openRecent {path? / index?}`；原 `FigApp` 文件对话框路径 | I2 接入。原程序化调用不打开对话框；UI 完成打开后退出首页，`doc.switch` 引擎命令本身不改变 `UiState.home` |
| 返回首页 | `doc.home {}` 请求原 UI home 状态 | I1 经原控制通道可验证；I2–I3 接原生入口 |
| 选区名称 / 类型、W/H | 同一选区快照；写入 `node.set {ids, props:{name / width / height}}` | I1 只读；I4 逐字段提交。名称使用真实值，类型本轮沿用引擎标识，尚未接中文本地化 |
| X/Y / 旋转 / 不透明度 | `cmd::props::describe` 的 frame-relative X/Y；`node.set {ids, props:{x,y,absoluteRotation,opacity}}` | I4 接入；组内坐标不能直接拿原始 transform.translation；设计栏世界旋转与相对旋转必须区分，数值 / 自动布局约束交给引擎 |
| 单项纯色 / 文字 | `node.set {ids, props:{fills:[color]}}`、`{characters,fontSize}` | I4 先检查单项纯色及文字样式适用性，不简化复杂数据。一次确认一次事务 |
| 撤销 / 保存 | `edit.undo/redo {}`、`file.save {path?}`、`file.saveAs {path}`；UI 走既有保存请求 | I1 验证原命令 / MCP / 保存重开；原生菜单与用户确认交互继续在 I2–I4 验收 |
| attached MCP | 原 backend → `engine.execute`，明确 `document` 为 UID 或文件路径 | I1 验证首页期间定向修改与恢复观察；不另起 Session，不增加 Agent 成员身份 |

## 回调、草稿与焦点接入约定

I1 不产生属性草稿或原生写入回调。I3 的导航回调捕获 UID，执行前确认当前窗口仍拥有目标并重新解析索引；找不到就丢弃，不能使用旧索引。I4 的草稿额外捕获节点集合、页面和编辑代次：切稿 / 回首页 / 关稿件 / 换选区主动失效，A→B→A 也不能复用旧代次；目标删除或外部字段变化取消该字段草稿。`revision` 可触发重读，但不能单凭 UID 与 revision 代替这些交互代次。实际写入仍受引擎的只读 / 版本预览 / 锁定等规则校验。

I4 覆盖原生 Enter / Tab / Escape、同稿件结束编辑、系统中文上下文与原生编辑菜单；画布失去键盘时使用既有 `CanvasHost.keyboard` 围栏。对应确定性负向场景见[验收计划](acceptance.md)，尚未实现的回调与属性提交不记为通过。

## 区域与视觉差异

AppKit 常规面板内容以左下为原点，但实际 WinitView 的 `isFlipped()` 为 true。宿主按父视图方向换算区域：flipped 时顶部 y=0、右栏与首页 y=84；非 flipped 时顶部 y=height−84、右栏与首页 y=0。左栏高占满内容区，顶部从 x=242 起。egui 左上为原点；布局区域为 `(242,84,width−242−408,height−84)`。三项 inset 均除以 `ctx.zoom_factor()`，使原生逻辑尺寸在界面缩放后保持对齐；显示像素比仍由原 winit / egui 路径处理。

I1 交付区域、真实数据及生命周期框架，尚未完成视觉还原：字标暂用系统字形；首页插画 / 文件入口、组件样式、分隔与字段栅格、完整导航及可编辑设计栏分轮接入。右栏类型和尺寸目前为只读文本。不得将本轮框架截图或协议回读当成 AC01 全部通过。首页 Agent 输入 / 快捷入口、页面导航、悬浮工具条和完整属性栏等范围边界继续按 v1.1。

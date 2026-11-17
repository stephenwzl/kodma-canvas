# S001 v2 实现与验证说明

日期：2026-11-16。实现基线：`0f277f3f`。这是保留完整运行时的验证实现，S001 总体验收状态见[进度表](progress.md)。

## 实现选择

不拆分 Session、FigApp、UiState，不另写画布编辑循环。新增可选的 `FigApp::canvas_host`，只保存宿主的显示策略、原生区域预留宽度与键盘归属；不写入稿件或改变原来的隐藏 UI 偏好。

[原帧循环](../../../crates/ui-egui/src/app.rs)继续执行初始化、对话框结果、控制请求、指针输入、快捷键、IME、渲染、协作同步与保存。仅画布模式跳过首页及标签条绘制，并复用原有隐藏 UI 的布局路径。原生区域由同一布局预留；不创建第二套 Session，不复制 frame 方法。

| 适配位置 | 目的 | 回归与升级核对点 |
| --- | --- | --- |
| [FigApp 与 CanvasHost](../../../crates/ui-egui/src/lib.rs) | 增加宿主显示策略，默认 None 保留原行为 | 状态不持久化到文档；默认编辑器仍可用 |
| [帧布局](../../../crates/ui-egui/src/app.rs) | 条件绘制导航、首页与侧栏，预留原生区域 | 文档切换或隐藏 UI 快捷键不能重新露出外围界面；尺寸与命中坐标一致 |
| [画布输入](../../../crates/ui-egui/src/canvas/input.rs) | 原生输入拥有键盘时禁用临时平移及缩放工具键 | 焦点切换时不误改画布；松开前补齐同帧遗漏的末次移动，保持原有事务；IME 路径不替换 |
| [控制状态](../../../crates/ui-egui/src/app/control.rs) | ui.inspect 返回 canvasHost 和实际画布范围 | 外部 Agent 能分辨宿主状态；命令、鉴权及目标文档处理沿用原实现 |
| [参数与桌面入口](../../../apps/figcraft/src/main.rs) | 仅画布与原生验证模式，保留默认启动方式 | 原模式的标题栏、窗口管理与后台逻辑保留；原生区域按 egui 界面缩放换算 |
| [AppKit 验证宿主](../../../apps/figcraft/src/native_canvas.rs) | 同窗原生输入框、宽度滑块与输入源选择，显式焦点和区域同步 | 保持原 contentView、图层与事件循环，升级时重查 winit 的窗口假定 |

## 原生承载方式及限制

固定版本 winit 的 macOS WindowDelegate 把 NSWindow.contentView 当作自己的 WinitView。直接将它替换为任意父容器，会破坏这一内部假定。因此当前采用同窗组合：保留原 WinitView 与 NSWindow，加入真实 AppKit 子控件，并由 egui 为其预留区域。

这已用于验证原生控件与 egui 在同一窗口中共同工作，不是独立窗口拼接或截图预览。它也不等同于验证了把 WinitView 搬进 SwiftUI 的 NSViewRepresentable；未来若改由 SwiftUI 持有整个窗口，仍需单独确认承载方式。

宿主通过现有 objc2 的安全接口实现，无新增 unsafe 或外部 FFI。objc2 已在依赖图中，本次显式依赖为 MIT 许可；AppKit/Foundation 仅增加已有依赖的功能选项。没有新增资产、字体或渲染依赖。

验证宿主每 100 ms 同步原生区域与焦点，避免改写 winit delegate 或添加不安全回调；这一轮询仅在 `--native-canvas` 启用，不能作为最终产品的空闲性能结论。

现有 ui.screenshot 返回 egui 渲染结果，不包含 AppKit 输入框和滑块。整窗原生组合已通过实际窗口截图观察，但截图接口的原生控件覆盖尚需后续处理。当前 AccessKit 树仍没有列出原生控件，不能宣称无障碍已通过。原生字段的 Edit 菜单、全选 / 剪贴板及撤销快捷键已在续验中补齐并实测；它们只在字段编辑时接管快捷键，回到画布后恢复原输入路径。

## 运行与复测

macOS 默认启动原生验证窗口：

```sh
./script/build_and_run.sh --verify --control 17981
ruby script/verify_canvas_host.rb 17981
```

仅画布和默认参考编辑器分别启动：

```sh
./script/build_and_run.sh --canvas-only --verify --control 17983
./script/build_and_run.sh --reference --verify --control 17982
```

脚本只停止名为 KodmaCanvas 的验证进程，使用独立的临时配置目录（默认系统临时目录下的 `kodma-canvas-validation`，可通过 `KODMA_CANVAS_CONFIG_DIR` 指定），构建应用束并运行；不修改其他 FigCraft 安装或使用其稿件。生成的截图、状态与 MCP 证据保存在 gitignored 的 `dist/s001-evidence`。界面中的输入框、滑块和输入源选择是验证控件，不是 Kodma 产品设计。

宿主回归使用：

```sh
cargo test -p figcraft-ui-egui --lib canvas_host_tests
cargo test -p figcraft --bin figcraft args_tests
```

验证脚本在确认 canvasHost 存在后，创建合成文档，检查控制通道读写、撤销重做、无效目标拒绝、真实 attached MCP、共享运行时和画布截图。最小化后使用 `ruby script/verify_canvas_host.rb 17981 --background` 仅检查后台控制与 MCP；普通脚本需要可绘制的窗口。它不代替鼠标、系统输入法、跨屏、性能或生命周期验收。

## 升级演练

演练目标为已获取的 `upstream/main` 提交 `64132ad9`，相对本仓库原 FigCraft 基线 `7b34dbab` 前进 266 个提交。演练在临时源码副本和独立构建目录进行，不将上游升级合入本地主分支。

适配补丁可直接应用，加入相同新增文件后，`cargo check -p figcraft -p figcraft-ui-egui --all-targets` 已通过。最初共享构建目录下出现过缺少函数的编译报错，未在独立目录复现；演练结论以隔离构建结果为准。随后隔离构建 figcraft / figcraft-cli 并启动真实原生窗口，控制通道、attached MCP、ui.input 拖动、单次手势撤销及截图均通过；真实最小化期间的后台控制与 MCP 也通过。本轮已补真实系统中文输入源选择、原生全选和组词中焦点往返；跨屏、候选窗和 Finder 拖放尚未覆盖，不能据此宣称全部升级兼容。详细结果见进度表。

## 同帧指针释放修复

真实鼠标续验发现，UI 在同帧收到移动和松开时，只把松开发给引擎，丢失最后一次预览更新；整个手势同帧时可能完全不移动。释放路径现在在坐标变化时先发送 Move，再发送 Up，保持一次事务。默认参考编辑器同样受益，未替换输入循环或引擎工具。`canvas_fixes_tests::a_batched_drag_keeps_its_release_position_and_one_undo_step` 覆盖两种模式和两类事件批次，实际窗口拖动及回读见[进度记录](progress.md)。

## 系统输入法与原生焦点

验证宿主的输入源选择使用 [NSTextInputContext](https://developer.apple.com/documentation/appkit/nstextinputcontext) 的可用源、当前源与本地化名称 API，只操作已经启用的源。原生字段接手或输入源菜单返回字段后，等待 eframe 关闭原上下文的 IME，再在后续 tick 激活原生文本上下文；明确选择输入源也重新安排一次激活。跟踪实际字段编辑状态，连续组词时不重复激活；不拦截、翻译或制造中文输入事件。窗口非 key 或字段没有编辑器时不激活。

FigApp 在宿主键盘归属为原生时清除未提交的画布预编辑，防止输入围栏跳过取消事件后留下旧预览；原编辑会话和光标保留。该适配有先失败后通过的回归。真实系统逐键输入、取消、替换及焦点往返结果见[迭代进度](progress.md)，候选浮窗位置尚未直接观察，不能据输入成功推定其定位已通过。默认 FigCraft 未启用宿主策略时沿用原路径。

## 原生编辑菜单与退出清理

原生 NSTextField 依赖 AppKit responder 的编辑菜单处理 Cmd+A / C / X / V / Z。宿主用已有 muda 0.21.2（MIT OR Apache-2.0）构造标准动作，并保留 craft-menu 的应用菜单及 Quit 保护。以同步、安全 AppKit API 将 Edit 子项附加到原菜单，不转换原生裸指针，也不替换文档命令路由。字段失焦时除了隐藏菜单，还清除其子项 key equivalent，防止隐藏菜单截获画布快捷键；获得原生字段焦点后恢复。

macOS 原有 on_exit 在保存状态后直接 process::exit，以避开已有的 AppKit / AccessKit 窗口销毁异常。新宿主必须在该保护之前显式 drop，移除控件、临时菜单并执行输入源恢复条件；不能依赖进程结束时自动析构。五次实际启动 / 正常退出的探测检查安装和移除各一次，不改变原退出保护。

## 亲自验收与探测入口

[人工 case](acceptance.md)包含启动、编辑、中文、跨屏、系统服务、attached MCP、保存重开和结果记录。

```sh
./script/accept_s001.sh
ruby script/s001_agent_edit.rb 17981
```

第一条构建并打开新的合成卡片，第二条通过真实 attached MCP 只修改最后生成且当前打开的卡片。文件保存在 `dist/s001-acceptance/`，每次生成新文件，保留前次验收结果。棋盘格图片由程序生成，不引入外部资产。

两个开发探测分别检查文档重开和实际宿主安装 / 移除；它们不是用户人工 case 的前置要求：

```sh
ruby script/verify_s001_lifecycle.rb 17981 <KodmaCanvas进程ID>
ruby script/verify_s001_host_cycles.rb 17981
```

宿主循环会关闭并重启验证应用五次，结束时应用已退出；先保存需要保留的编辑。文档循环使用另一份合成文件；默认编辑器对照时需显式传 `--reference`。短程探测不能替代长期大稿件性能，结果与限制见[进度](progress.md)。

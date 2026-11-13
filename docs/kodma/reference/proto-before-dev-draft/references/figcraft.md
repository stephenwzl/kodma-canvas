# FigCraft 桌面端启动与安装

1. 检查桌面 App、CLI/MCP、运行进程和本地控制端口。macOS 检查 /Applications、~/Applications 和 PATH，也检查宿主已配置的 MCP 或显式提供的 CLI 路径。分别验证两部分；一个不在 PATH 并不表示桌面 App 没装。复用可用版本。
2. 缺少桌面端时，Agent 自动查询官方最新稳定发布 https://api.github.com/repos/storytold/figcraft/releases/latest 。从返回的 assets 选择适合当前 OS、架构的桌面安装包及所需 CLI，使用返回的 browser_download_url；版本和下载地址都不要硬编码。桌面已存在但缺 CLI 时先查最新稳定版，再优先使用与桌面版本匹配的官方 CLI。若发行包已内置 CLI，按该版本说明使用。版本对应的安装与 MCP 用法见 https://github.com/storytold/figcraft 。
3. 自动完成适用的下载、校验（发布信息提供 digest 时核对）、解包和用户级安装，复查版本。macOS 的 DMG 可挂载后复制到可写的 Applications 目录；CLI 可放入用户可写的 bin 并用绝对路径启动。只有操作系统要求管理员或交互授权时，指出用户需完成的具体一步，随后继续验证。保留已有安装和打开的未保存文档。若网络或架构阻塞，报告具体缺失项和官方资产入口。
4. **先识别用户的工作稿和 session。** 先盘点正在运行的 FigCraft 进程、窗口、已配置的 MCP 和各进程实际监听的本地控制端口；用只读文档检查匹配目标稿的完整路径。优先复用用户当前打开该稿的实例，即使它使用了不同的端口。预设或上次使用的端口无响应，只说明该地址不可用；不要因此用 open -n 再开用户端。若目标窗口已打开但没有可用控制端口，使用该窗口现有的协作邀请，或引导用户在该窗口开启本地协作；保持当前窗口和文档。只有确认没有运行的用户端且本次需要桌面宿主时才启动用户端。已有 live session 就复用；未入会才在目标文档上执行 collab.host（bind=local，port=0）。该命令能替换原会话，不能对已有会话重跑。若要创建新稿，在用户确认的任务文档位置建立唯一稿，保留其他打开的文档。邀请码仅在本机进程之间传递，不写进交付物、日志或回复。
5. **Agent 使用无窗口 guest。** 优先复用仍可访问的 headless MCP 会话。若已经加入目标 session，直接沿用；若没有可复用的 guest，后台运行 figcraft-cli mcp（不带 --attach）并保持同一 stdio 会话，在其中通过 run_command 调用 collab.join 加入用户当前稿。不要为 guest 使用 open -n、open -gn 或启动 FigCraft.app。headless 初始文档是空白的，加入并完成同步后才是目标共享稿；核对角色 guest、状态 live、可写、文档身份以及双方 peer ID 后再编辑。若版本不支持 headless 协作或加入失败，报告原因并恢复 CLI 连接，不另开桌面实例。
6. **区分两个人的状态。** 以 peer ID 锁定用户（本地双端时可从用户实例的 me 核对），读取其 presence.selection；Agent 自己的 inspect_document.selection 属于 Agent。用户选区为空、已切页、多人身份不清时，只就本次目标核对。写入指明节点 ID，避开用户正在输入的图层与同时修改的字段。从 guest 校验共享变更；保存规范路径时仅对已核实的宿主目标文档执行保存。不要操纵用户窗口的选择、跳页、缩放、聚焦、跟随或演示模式。原型回放和导出在 Agent 自己一端做。
7. **连接异常先恢复协作。** 查原因、重连并回读已生效的修改，再决定是否重试。若只能直连用户窗口或拿到只读邀请，说明限制，等可编辑会话恢复再写入。结束时仅按需退出 Agent 自己的会话，不停止用户的房间。

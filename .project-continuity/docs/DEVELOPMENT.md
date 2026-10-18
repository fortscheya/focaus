# Development Control

日期：2026-10-18。当前阶段G2（四路已启动）。D167启动实施，D168允许Git初始化；工程七文档作为只读基线，不把旧“本轮只写文档”当现阶段禁令。

## G0 Scope
- 初始commit：根.gitignore、docs/engineering七文件、.project-continuity/POLICY.json/.gitignore/GOAL.md/PLAN.md/HANDOFF.md/status/*.md/docs/*.md，显式逐路径暂存；不使用git add .。
- 排除development-guide及复制原文、BAILIAN_API_INTEGRATION.md原件、test-recording、原课件、音视频/派生大文件、凭据、本机状态、数据库、缓存/构建与大报告；不导入参考原文。
- main保存初始基线；codex/focaus-integration实施G1。G1提交SHA作为四路共同起点，分支codex/focaus-frontend、codex/focaus-native、codex/focaus-ai、codex/focaus-materials。
- D170已授权本地merge/cherry-pick整合和公共契约向四路同步；不包含rebase、push、发布、删除分支/worktree。

## Controller / Creation Evidence
- Controller thread 01a14cae-d8ce-7053-bd12-737ea1bef020，host local，主路径/Users/pitr/Desktop/Project/hackthon2026skd/focaus。
- list_projects确认项目Focaus，projectId f2e92149-ba53-44f1-87ed-848b34650309，host local；开始时isGitRepository=false，与git检查一致。
- create_thread工具声明本host支持gpt-6.1-sol与thinking=max；四路已显式请求该组合并由工具接受；总控保持原设置。运行配置字段未独立读回（详见后文）。
- 不创建临时同目录开发对话，不使用subagent或fork替代。真实threadId由host返回/列表解析，clientThreadId只记准备中。

## G1 / Acceptance
- 单源Schema生成TS/Rust及引擎契约，C01–C09、capabilities/errors、旧1.x资料适配和legacy_summary、未知major拒写及未知模板只读降级。
- 最小SQLite迁移固定实体/版本/时间映射/转写/Fact/Note/SourceEdge/UserPatch/任务outbox/材料解析/组件/搜索屏障/focaus结果。
- 公共traits、composition stub与可消费fixture；2D/conv2d数学正反例、Unicode/版本删除、focaus路径/前置/双截止/代次及C01–C06冲突样例。
- 验证通过后记录实际命令和SHA；G1不是产品验收。V01–V26/U01–U14仍待实际验证。

## G0/G1 Observed Results
- G0初始main提交3312e4ee99099fad2b963fd25c11bfc697a1263b；当前分支codex/focaus-integration。list_projects已读回isGitRepository=true。
- G1单源schemas/source.py生成JSON Schema/TS/Rust；机器契约、命令目录、capability/四槽位绑定、显式旧包适配与unknown拒写/降级、SQLite schema 1迁移、HostServices/StubHost、fixture adapter已实现。公共Schema带结构/跨字段约束；Python另做语义/闭包检查，N仍须实现运行期事务/actor/数学/删除门禁，不能把类型检查当这些运行期验证。
- 自制45个正反例（37结构、8语义）、12个运行场景说明、数学/Unicode/hash独立预期；运行场景说明不算实现或测试通过。数字hash固定RFC8785，Unicode profile 15.0.0。
- 实际验证：generator --check；Python unittest 11项；Rust workspace test（5个契约测试）；TS类型检查与Ajv 37例及RFC8785向量；桌面占位入口Vite生产构建均通过。没有真实麦克风/UI行为/模型/课程验收。
- 相关修复：生成Rust枚举大小写；pnpm安装明确允许esbuild必要构建脚本。首次Rust编译和pnpm脚本策略失败均已修复重跑，不隐藏失败历史。
- Node使用本机bundled v24路径，pnpm实际11.25.0并记录packageManager；Rust stable1.99.1隔离安装在主目录.local内，不改全局shell。Python验证在.local/venv。
- 桌面基础依赖锁定React19/HeroUI3/Tailwind4，已核对HeroUI官方quick-start；仅构建占位，不称界面完成。Three/R3F/D3未锁依赖，待F的U12。
- workspace_hygiene已在Git状态下运行，clean。初始既有ASR-CANDIDATES末尾空行被git diff --check提示，原只读资料保留；本次代码diff另检查。

## G1 Baseline / Persistent Chats
共同基线：d9a8cdf68617cd73c25cfbd335502764c6f40d76（G1已提交）。host均local。

| 路 | App标题 | 真实thread ID | worktree | 分支 |
| --- | --- | --- | --- | --- |
| F | Focaus 前端与编辑器 | 01a14cc5-23a7-7291-bee4-4a6fa6c527fd | /Users/pitr/.codex/worktrees/e7e6/focaus | codex/focaus-frontend |
| N | Focaus 原生后端与录音 | 01a14cc5-74e5-7440-967d-a845fec15a0a | /Users/pitr/.codex/worktrees/65ea/focaus | codex/focaus-native |
| A | Focaus AI流程 | 01a14cc5-be76-7362-aa8c-aec06808e0c4 | /Users/pitr/.codex/worktrees/a34d/focaus | codex/focaus-ai |
| E | Focaus 资料转换引擎 | 01a14cc6-0f0a-7cc3-b5c1-9e7ceae1482c | /Users/pitr/.codex/worktrees/54e7/focaus | codex/focaus-materials |

四次create_thread均显式请求model=gpt-6.1-sol、thinking=max，调用成功且未报配置不支持。create_thread只返回clientThreadId/local；真实ID和路径已由list_threads/read_thread确认，git worktree list/各副本HEAD确认共同基线。App初建detached HEAD，总控逐一确认干净/相同SHA后切到上述既存角色分支，再下发替换实际路径/基线/总控ID的§7.2–7.5完整任务。没有同目录替代、subagent或fork。
运行配置独立读回：当前create_thread/list_threads/read_thread未提供model/thinking运行配置字段，只能确认请求被接受而非独立核验运行值；不以worker自报冒充host证据。总控设置未更改。
本地整合授权已于本轮用户答复明确获得，允许总控审查后merge/cherry-pick四路及同步公共契约；不含rebase/push/发布/删除分支或worktree。覆盖上文待答历史状态。
C向F/N下发物理接线：focaus_bootstrap返回宿主window_id/contract_version；focaus_command接收Command，事件focaus://events承载EventBatch。N按实际WebView校验actor/window/scope并拒host-only命令；C负责composition/config。工程约定待真实联调。

## G2 in progress
- C提交b3692f2锁定SQLite/CPAL/Unicode15规范化与共用transport依赖，默认workspace测试/TS检查通过；已核查公共路径无修改后cherry-pick到F 861e9ef / N d28ef10 / A 7855617 / E 796c265。移除无效的跨包./schema export，实际Schema仍由公共文件消费。
- Tauri入口/菜单栏/后台保活/真实WebView绑定正在接线，尚未编译运行验收。N提供NativeHost::open/bind_window/dispatch_webview/take_event_batches/exit；原生麦克风由显式操作启动。
- evaluation/assets.json登记两份qta原件SHA256、字节数及ffprobe轨道元数据；未解码/试听/选轨/关联L8或L9/托管复制。实际内容与完整可解码性未知，不把元数据当验证。
- C初审在途N IPC发现非变更ok响应缺committed_revisions；已请求修正并按CommandResult验证真实响应。A时间映射需覆盖gap与begin/end端点，已交独立边界检查。
- U02局部真实证据：ffmpeg9.0.1分别对两份qta轨0 AAC完整decode-to-null，输出时间1275371667us/6140310000us且退出0；轨1 APAC均因无decoder退出234。未保存音频、未混轨，尚无试听/主轨选择/课堂内容关联。evaluation/asset-decode.json记录精确源hash和结果。
- D171已收到真实课件路径；L08/L09原始PDF元数据登记evaluation/courseware-assets.json，E已获限定本地提取任务，派生只入其忽略.local。

## G2 Integration Evidence (2026-10-18)
- 整合N 05e4e97→c34fcef、F a6df009→6db6188、A e8ef477→ee5f50b、E 6442697→fde9518；逐路限定目录复核。C公共接线4e8452d/native_flow149b116及A依赖锁5b30910。
- C使用相同NativeHost/SQLite/wire建立双窗口、ACK/事件读取、幂等、身份拒绝与重启恢复集成；cargo workspace test及native-shell check通过，未开启真实设备。Python11/TS37+hash/generator一致性通过，E4项通过。
- F正在收紧native渲染和宿主真实ID/补读后ACK；N真实搜索/两阶段视图在途；A transport安全边界加固及真实协议核对；E路径/恢复/完整性边界。各阶段提交不等于G2或产品验收完成。
- E真实课件deep：L08 82页，67 complete/15 partial，text63/table4/formula13/image2，92 search units，23.39s；L09 86页，76 complete/10 partial，text76/formula8/image2，84 units，22.71s。两份2页图像回退，搜索覆盖partial；light分别80/84 basic_ready和2 partial。manifest通过公共语义校验，原件/派生未上传或纳管。
- 真实课程质量、课件与qta内容对齐、真实云、macOS设备和长录音未验证。Rustfmt未安装，未宣称格式检查通过。

# Development Control

日期：2026-10-18。当前阶段G1（验证已通过，提交待生成）。D167启动实施，D168允许Git初始化；工程七文档作为只读基线，不把旧“本轮只写文档”当现阶段禁令。

## G0 Scope
- 初始commit：根.gitignore、docs/engineering七文件、.project-continuity/POLICY.json/.gitignore/GOAL.md/PLAN.md/HANDOFF.md/status/*.md/docs/*.md，显式逐路径暂存；不使用git add .。
- 排除development-guide及复制原文、BAILIAN_API_INTEGRATION.md原件、test-recording、原课件、音视频/派生大文件、凭据、本机状态、数据库、缓存/构建与大报告；不导入参考原文。
- main保存初始基线；codex/focaus-integration实施G1。G1提交SHA作为四路共同起点，分支codex/focaus-frontend、codex/focaus-native、codex/focaus-ai、codex/focaus-materials。
- 本地merge/cherry-pick整合和公共契约向四路同步授权已询问，等待用户；不包含rebase、push、发布、删除分支/worktree。

## Controller / Creation Evidence
- Controller thread 01a14cae-d8ce-7053-bd12-737ea1bef020，host local，主路径/Users/pitr/Desktop/Project/hackthon2026skd/focaus。
- list_projects确认项目Focaus，projectId f2e92149-ba53-44f1-87ed-848b34650309，host local；开始时isGitRepository=false，与git检查一致。
- create_thread工具声明本host支持gpt-6.1-sol与thinking=max；四路将显式请求该组合，总控保持原设置。现在尚未创建，不记作已接受/已读回运行配置。
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

# Development Control

日期：2026-10-18。当前阶段G0。D167启动实施，D168允许Git初始化；工程七文档作为只读基线，不把旧“本轮只写文档”当现阶段禁令。

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

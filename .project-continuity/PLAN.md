# Plan

## Completed: Engineering Documentation (D160)
- Q1–Q127已回应，D157确认产品共同理解；D160本轮文档授权已完成。项目docs/engineering/含README及六份正文、C01–C06文字契约、V01–V17验收矩阵、U01–U10待验证项、五份启动提示词。
- 完成文档链接/结构/字段状态/职责/时序/只读文件保护检查；不是机器类型/产品运行验收。无产品/测试代码、依赖安装、服务、密钥读取、模型调用、上传、Git初始化或线程创建。
- 工程推荐（Rust宿主/AI crate、独立Python引擎、SQLite单写等）是设计未实测；视觉参数、具体依赖、机器Schema、预处理/走神算法、五档示例与未定质量门槛在对应实施阶段验证。
- D161文档补充完成：开发对话统一gpt-6.1-sol/max；INTERFACES §9.1明确四模型专用凭据槽位、filetrans笔误更正及不串用规则，U04补错配/缺失/重试绑定验证计划。仅保存非敏感映射，实际值未配置/验证。

## Completed: Engineering Amendment (D165/D166)
- 2026-10-18七文档与五prompt已纳入D162–D164；版本2.0.0及兼容策略、C07–C09、V18–V26/U11–U14已补齐。文档设计完成，不代表功能实现/测试通过。
- 后续优先验证受控模板/数学（N负责确定性、A负责语义）、F的WKWebView资源/无障碍、N索引/查询与E文字锚点、focaus必要衔接与双证据截止；具体库未锁定，默认范围/算法是工程推荐。

## Active: G2 Integration with Four Persistent Development Chats
- G0 done：2026-10-18用户允许Git初始化。初始main仅纳管工程七文档、非敏感continuity及.gitignore；总控codex/focaus-integration，四路frontend/native/ai/materials从G1精确提交各自建worktree。详见docs/DEVELOPMENT.md；D170已授权本地整合。
- G1 done（d9a8cdf6）：C01–C09单源跨语言类型/Schema、错误与capability、旧包适配/拒写/降级、最小SQL迁移、公共traits与composition stub、脱敏正反例及可重复检查；通过后本地提交共同基线。
- G2 active：四路首批已整合到fde9518，真实NativeHost/SQLite双窗口/ACK/重启集成与native-shell编译通过；F/N/A/E继续下一批边界与真实宿主接线。G3–G5 planned：先可靠本地链与早期U11–U13，再获得云授权后的真实ASR/进度/focaus，随后三层/编辑/引用/对象与搜索，最后真实课程1倍速/课后全量/长录音恢复/课堂包。各模块commit或fixture通过不能关闭全体验收。
- 四路已通过create_thread创建，实际路径/分支/基线已核验，完整§7.2–7.5任务与原样消息授权已下发；真实ID/host/path和配置证据见docs/DEVELOPMENT.md。总控唯一维护公共文件与continuity，持续read_thread/wait_threads复核并整合。
- 产品模型/专用凭据继续INTERFACES §9.1，真实云调用/上传尚未获本阶段明确授权。qta先本地轨道核验与内容对齐，不按编号配课、不混轨、旧转写非真值。

## Later / Explicitly Deferred
- 系统声音在麦克风/已有录音闭环后；Web/iOS联动、按实际课次自动组织、终端用户服务配置、长期隐私留存策略后续。
- 首版不规划录音以外离线能力，不处理课件音视频/外链内容；不实现转场/逐步播放。
- 跨课程综合问答/主动联网另议。Jev新原音频方案仅历史探索，当前不是首版实施依赖；旧20秒/P+1/每分钟进度方案已退役。

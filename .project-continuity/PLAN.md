# Plan

## Completed: New L03 Presentation (2026-10-18)
- 已按正在运行的第二版 L03 Demo 重制 8 页路演，接入现有实时 ASR 客户端，增强翻页与页内动效；HTML/PDF/PPTX 已更新并通过本地验证和渲染 QA。实现仅修改 `presentation/`，保留原宿主 Origin/凭据/额度约束，本轮未上传麦克风音频。详见 `status/PRESENTATION.md`；本条取代下方旧 6 页交付状态。

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
- G2 active：四路成果已整合至0ced003；真实NativeHost/SQLite、WKWebView空库/课程创建/重启、第二轮麦克风开始→水位→停止→重启恢复、stable standalone/六格 view注册、空索引Command+F和F真实content.get投影代码已通过对应验证。N R0 writer/SQLite隔离已通过28项原生回归；当前继续C的真实资料导入UI、逐页不可变parse版本接线和有资料搜索/三层内容端到端验收。G3–G5 planned：先可靠本地链与早期U11–U13，再获得云授权后的真实ASR/进度/focaus，随后三层/编辑/引用/对象与搜索，最后真实课程1倍速/课后全量/长录音恢复/课堂包。各模块commit或fixture通过不能关闭全体验收。
- 四路已通过create_thread创建，实际路径/分支/基线已核验，完整§7.2–7.5任务与原样消息授权已下发；真实ID/host/path和配置证据见docs/DEVELOPMENT.md。总控唯一维护公共文件与continuity，持续read_thread/wait_threads复核并整合。
- 产品模型/专用凭据继续INTERFACES §9.1，真实云调用/上传尚未获本阶段明确授权。qta先本地轨道核验与内容对齐，不按编号配课、不混轨、旧转写非真值。

## Active: One-hour Local Demo (D174)
- Demo route `?demo=1` is implemented in the desktop app for the user's immediate walkthrough: local microphone capture with local sentence replay, source selection and courseware insertion, transcript/courseware navigation, L08 convolution reconstruction, 2D object interaction, and secondary transcript correction.
- The route uses only local curated DeepLearning-derived sample text. It visibly states local Demo data, does not upload audio, and does not invoke the four cloud models. This is a reviewable UI/demo path, not real ASR or full product acceptance.
- Demo 已在 `18251de` 提交；typecheck、12项桌面测试、build 和独立端口 `?demo=1` 可见路由检查均通过。真实 streaming ASR 继续受此前云/数据/费用授权边界约束。

## Completed: Submission Documents (2026-10-18)
- 已完成两份申报 Word：项目说明 3 页、技术及第三方资源说明 6 页；全部 9 页渲染与视觉检查、DOCX 完整性和必需章节检查通过。成员、第三方资源及验证边界见 `docs/SUBMISSION.md`。
- 文档交付已完成；原创声明签署与外部课件／录音／图标授权确认仍由团队处理，未代为上传或提交。

## Later / Explicitly Deferred
- 系统声音在麦克风/已有录音闭环后；Web/iOS联动、按实际课次自动组织、终端用户服务配置、长期隐私留存策略后续。
- 首版不规划录音以外离线能力，不处理课件音视频/外链内容；不实现转场/逐步播放。
- 跨课程综合问答/主动联网另议。Jev新原音频方案仅历史探索，当前不是首版实施依赖；旧20秒/P+1/每分钟进度方案已退役。

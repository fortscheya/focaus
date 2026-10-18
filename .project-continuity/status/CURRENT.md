# Current State

## Active Development (D167/D168)
- 2026-10-18 G0已完成初始化与白名单提交3312e4e；list_projects读回Git=true，总控在codex/focaus-integration。
- G1已实现单源C01–C09类型/Schema、迁移schema1、host traits/stub、旧包适配/能力拒写/降级、45脱敏正反例和12运行场景说明；实际验证通过，共同基线已提交d9a8cdf6。
- generator一致性、11项Python/SQLite、5项Rust契约、37例TS/Ajv和RFC8785、前端占位生产构建均通过；产品V/U未执行。
- Rust1.99.1位于忽略.local；Node用bundled runtime，pnpm11.25.0。根依赖/锁文件仅C维护，具体3D库未锁。
- 四开发对话已创建、真实worktree/分支核验并派发实施；总控01a14cae-d8ce-7053-bd12-737ea1bef020 / local。工具声明支持gpt-6.1-sol/max；运行设置未读回。
- D170已授权本地整合merge/cherry-pick；云调用/上传/费用未获得本阶段明确授权，不读取实际密钥。
- 下列文档交付观察为此前阶段记录；非Git/只写文档等阶段限制由D167/D168替代。执行登记docs/DEVELOPMENT.md。

## What Exists
- 2026-10-18完成D165七文档修订（README/ARCHITECTURE/DATA_MODEL_AND_FORMAT/INTERFACES/WORKSPACE_AND_FLOWS/VALIDATION/PARALLEL_DELIVERY），五份开发启动提示词同步。D160原工程基线保留有效部分，D162–D164新增需求已落实设计。
- 工程契约2.0.0、资料格式focaus.material/2.0.0、交互子协议focaus.visualization/1.0.0；保C01–C06/V01–V17/U01–U10，追加C07–C09/V18–V26/U11–U14。尚无机器Schema、数据库迁移或产品实现。
- 工作区再次核实非Git；未初始化/提交/建分支worktree或开发对话。

## Confirmed Product / Engineering Distinction
- P：原生自动2D/3D可视化、三类内容（含两层转写）搜索/Command+F、正式小写focaus的自适应总结与理解衔接；原录音优先、三层内容/用户保护/五档事实/引用/窗口/整次转移继续。
- P：focaus完整≤15秒，进度约10秒、音频证据落后>30秒待更新；不恢复20秒预测/按时间加页。四模型北京与INTERFACES §9.1专用凭据映射继续；第4项为filetrans，实际值不保存/配置/验证。
- E（D166）：受控注册表/结构参数，N数学验证与索引权威，F渲染与搜索UI，A语义候选/有据衔接，E提取/定位；默认Command+F只搜当前聚焦view，NFC/casefold映射、版本/删除屏障。focaus保内部catchup，正文/动态指针双证据时间。
- E：升major有必填/联合类型兼容理由，旧包显式适配、未知能力明确降级，旧catchup结果仅legacy_summary；迁移未执行。具体库、运行隔离/资源预算、检索/数学/教学效果均属U，不能记已验证。
- 总控唯一拥有公共类型/注册表/迁移/根配置/continuity；四路独立worktree和互斥目录，gpt-6.1-sol/max。

## Validation Actually Performed
- 读取本次附件、七文档、直接相关已确认基线/continuity及visualize参考；保原产品/专项资料只读。
- 公开官方核对：Three.js MIT（r186发布2026-09-24）、R3F MIT（v9.8.2发布2026-10-15，v9配React19）、D3 ISC（v7.9.0发布2024-03-12）；均未归档且有提交记录。Tauri macOS为系统WKWebView，Three WebGLRenderer需WebGL2，R3F有按需渲染接口。未安装/运行库，不把维护迹象当兼容通过。
- 文档检查：本地链接/锚点、Markdown表格/围栏、JSON节选语法、C/V/U编号及五提示词；人工交叉检查职责、模型/凭据槽位、版本/状态、引用/删改/时间与任务顺序。最终复核通过：7份工程文档、21个本地链接/锚点、7个围栏块（含5份启动提示词和1个JSON节选）、398行表格列数、C01–C09/V01–V26/U01–U14完整连续且无越界引用；未检出旧契约/旧thinking设置或凭据样式值。首轮提示词词项检查后补明中文职责，再复核0项错误。
- 11份只读参考/音频的size/mtime与编辑前一致，无音频内容读取；工程目录无新增代码/配置文件。workspace_hygiene.py退出2，原因非Git，未称通过。

## Pending / Unverified
- V01–V26和U01–U14全部为未来产品验证计划。尤其待测：注册表触发/数学正确性、WKWebView多窗GPU/无障碍、Unicode/OCR全文覆盖与删除竞态、focaus真实理解衔接与15秒完整时延。
- G0/G1先完成授权/纳管清单、机器类型/样例/迁移和共同基线，再四路实施G2–G5。不以本次文档授权执行产品开发/资料上传/模型调用。
- 两份qta先前元数据1275.3717秒与6140.31秒，合计7415.6817秒；AAC双声道/APAC四声道，L8/L9多对多覆盖未知。未试听/解码/转换/转写，旧DeepLearning转写非真值。
- 本轮没有产品代码、测试程序、依赖安装、服务启动、模型委派/API调用、用户资料上传、读取本机密钥或保存验证凭据。development-guide不读取照搬/纳管，原资料保留。

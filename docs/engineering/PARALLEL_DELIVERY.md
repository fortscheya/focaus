# 五对话并行交付与启动提示词

工程契约 2.0.0｜2026-10-18。安排为一个总控和四个真正独立、持久化、在Codex App可见的开发对话。本轮只写启动方案，不创建对话或Git。

## 1. 所有权与目录写入边界

以下路径是**未来工程的首选目录契约**，当前不创建骨架。总控在G1生成最小可消费文件、锁文件和类型后再交各路；任何改名须同一契约变更同步所有消费者。

| 对话/代号 | 拥有模块与可写路径 | 数据写权限 | 不可自行修改 |
| --- | --- | --- | --- |
| 总控整合 C | `docs/engineering/`、权威`.project-continuity/`；`packages/contracts/`、`crates/focaus-contracts/`、`schemas/`（含组件注册表定义/公式规范、搜索profile/锚点、focaus结果Schema）；`apps/desktop/src-tauri/migrations/`；根workspace manifests/lockfiles、桌面package配置、Tauri配置与Rust composition root；`tests/integration/`、`fixtures/contracts/`、`evaluation/` | 制定模型/迁移/验证；应用运行时仍经宿主 | 不代替用户改产品目标，不擅自发布/上传 |
| 前端编辑器 F | `apps/desktop/src/`下UI/编辑器/样式/前端适配器与就近单元验证；含`visualizations/`渲染交互、`search/`UI/高亮、`focaus/`结果呈现 | 只发命令/patch/提案，读snapshot/event；浏览器缓存可丢 | 数据库/迁移、采集、云协议、公共契约、宿主窗口实现、其他路文件 |
| 原生后端录音 N | `apps/desktop/src-tauri/src/{capture,audio,storage,tasks,ipc,windows,lifecycle,library,search,visualizations}/`；`tests/native/` | 唯一权威DB/资源写入执行者、任务/事件事务、采集与窗口仲裁；搜索投影/查询/可见性屏障、组件确定性计算/校验/状态存储 | 公共Schema/迁移版本、模型语义、引擎算法、UI；`main.rs/lib.rs`接线由C |
| AI流程 A | `crates/focaus-ai/`（实现/提示模板/协议适配/范围内测试，含可视化发现/候选/语义校验和focaus上下文组织） | 通过宿主trait读快照、报checkpoint/候选；无SQL权威写 | 宿主store实现、迁移、产品模型名单、UI、引擎解析器、continuity |
| 资料引擎 E | `engines/material/`（独立包、解析/重构/暂存输出/范围内测试、页区域对象文字/搜索定位与结构化依据） | 只写指定任务暂存目录，经宿主接收版本包 | 应用DB/笔记正文、公共格式、云凭据、宿主/其他路目录 |

子目录自身manifest由该路提出，根workspace、共享lockfile和跨模块依赖仍由C合并维护；不得四路各改根lockfile。N负责运行迁移，C拥有迁移内容和编号；A/E负责提议新数据需求，不能夹带私有迁移。F/E共享对象能力来自C05/C07，不共享一份可随意变更的编辑器JSON。C维护模板白名单/字段/算法语义与独立样例；N实现确定性计算和数学检查，F实现渲染器而不自产权威数值，A的语义检查不能替代数学检查。搜索索引唯一归N，E只产提取数据，F只读命中定位；focaus检索由N执行、A取舍生成、F展示，不增加第五个开发对话。

runtime中的单一写入口与开发期文件所有权是不同概念：N实现事务并不获得任意改变产品语义的权利；C拥有公共Schema也不要求所有业务实现都堆在总控。

## 2. 开发启动门与集成顺序

### G0：由后续总控准备可隔离仓库

当前非Git，必须先检查实际状态。总控若尚无Git初始化/分支授权，先完成只读纳管清单和明确分支方案，再一次性请求必要授权；这不阻止只读契约核对。本文件和当前文档任务不自动授予执行Git初始化的权限。

建议起点与分支：首次仓库 `main` 为受控初始基线，总控 `codex/focaus-integration`；四路 `codex/focaus-frontend`、`codex/focaus-native`、`codex/focaus-ai`、`codex/focaus-materials`，共同起点为G1通过后的精确commit。若已有合适分支就复用，不重复建议。初始化前按VALIDATION §7排除development-guide、录音/派生大文件、凭据和本机状态，不用git add .盲目纳管。不自动推送。

### G1：总控先交最小契约底座

本套2.0.0文字契约可用于分工；开始写四路产品代码前C必须完成以下可被各路导入的交付：

1. 生成/维护同一来源的TS/Rust/引擎JSON Schema类型：C01–C09，以及版本协商、错误枚举；避免手抄三份相似类型。选择一种Schema源并给出生成过程与可重复检查。明确contract 2.0.0、focaus.material/2.0.0、组件子协议1.0.0及capability握手；旧1.x适配与unknown拒写/降级语义先有样例，不能写端各自猜。
2. 建立最小数据库迁移：课程课堂/会话/资源与时间映射/两层转写/Fact与Note/SourceEdge/UserPatch/任务与outbox/Material和parse version；加入组件/参数/默认交互状态/语义去重键、SearchUnit/锚点/覆盖/visibility_epoch和focaus新结果字段；允许按阶段细化加速索引，但ID、版本、归属和删除屏障事务边界先固定。
3. 脱敏契约样例：未归属录音、缺口与重叠、partial→final/重复云句、迟到生成、用户删改/输入中、五档stale、旧来源/删除墓碑、表格/图表/结构图/partial页、六格满和拆出回滚；受控2D/conv2d_spatial与独立数学正反例、演示值、重复/用户删组件、未知模板；Unicode映射/同源双视图去重/未OCR/版本删除搜索；focaus桥接/窗口外前置/未知焦点/正文指针双截止与迟到代次。
4. 公共宿主trait与composition stub，明确消费者接口；前端fixture adapter和引擎输入输出fixture可消费同一套数据。样例中不包含真实音频片段或用户密钥。新traits至少覆盖visualization.evaluate/validate、search.open/query/page/locate、context.retrieve与catchup.display_ack；实现可空，但接口/错误/fixture可消费。
5. 运行适合公共契约的格式/类型/样例验证，记录精确基线SHA、版本、允许写路径。以一个有意义的本地commit交付，不需要先做完整产品。

G1通过后四路同时开始，不必等待N的全部实现或E的完整重构。F可用fixture开发，A可用宿主trait替身，E可用格式fixture；这些替身必须能被真实adapter替换，并显式标注非验收模式。

### G2–G5：四路并行，按纵向闭环集成

| 阶段 | 必须接通的路径 | 进入条件与交付证据 |
| --- | --- | --- |
| G2 可靠本地链 | F录音控制→N采集/文件/事件；F编辑→N版本/patch；E原页/基础文字→N搜索投影→F查找；F组件样例→N确定性计算 | 唯一会话、关窗继续、停止保存、初始同步；录音事实不依赖云。A并行完成协议/候选校验；提前做U11–U13小样例暴露数学/GPU/Unicode风险，非最终质量验收 |
| G3 实时文字与进度 | N音频/映射→A streaming→N raw→F；断网后filetrans填洞；A progress/focaus上下文检索与生成→F双时间显示 | 获得明确云调用/资料传输授权后真实接入；V03/V06/V07/V25/V26的首轮真实切片，不把接口替身算通过 |
| G4 派生与对象编辑 | raw→secondary→Fact/note/五档；E深对象→可编辑副本；自动组件/保护/恢复；全范围搜索/失效；引用/纠正/删除/转移 | 共用同一DB、时间轴、ID与SourceEdge；V04/V05/V08–V13/V15–V24 |
| G5 真实课程验收 | 两份qta内容对齐后1倍速实时与课后全量；长录音/恢复；完整课堂包往返 | V01–V26的实际结果、focaus理解衔接/3D数学/全文搜索与固定要求报告；未定门槛先基线后共同锁定 |

每阶段完成可独立交付部分及时本地commit，阶段不是停工等待“继续”的节点。只要下一步已获授权且输入明确，继续推进。需要跨路依赖时继续本路可独立验证工作，不能私自改公共接口解除阻塞。

## 3. 四路首批交付与验收责任

| 对话 | 首批可审查交付 | 必交证据 | 后续依赖 |
| --- | --- | --- | --- |
| F | HeroUI默认壳、开始页/课程工作区、六格布局、内容绑定、fixture驱动录音状态；编辑器细粒度patch/IME保护与来源对照原型；C07受控2D/3D渲染fixture、Command+F/搜索结果、focaus双截止/衔接样例 | 类型/相关构建、实际UI深浅色/键盘/焦点/布局；标明fixture，无真实采集声明 | N窗口/快照/事件/查询/计算；A提案/五档/focaus；E标准对象/定位 |
| N | 宿主唯一会话、原声独立落盘/缺口/恢复、菜单栏；单写DB/outbox、命令/资源接口、view移交骨架；search投影/屏障/query与组件数学验证接口 | 真macOS采集/关窗/停止/失败恢复；事务/幂等/事件重放验证，不以短录音替长测 | C迁移/契约；A服务adapter；F view.ready |
| A | 双ASR适配状态机/时间映射/去重、快照→候选管线、progress/catchup分离、raw→secondary/Fact提案；focaus路径/前置与可视化识别/参数候选 | 格式/乱序/取消/迟到校验；获授权后实际协议与时效报告，未测不记通过 | N音频/任务/持久化trait；E带页文字/公式/定义；N检索/数学检查；F交互结果 |
| E | 独立可运行转换入口协议、PDF/PPTX原件→页/基础文字→标准对象包；身份/资源/局部失败与纠正格式；ExtractedSearchUnit/原页对象映射及覆盖 | manifest/资源闭包与样本转换；可编辑表/图/结构图证据与未完成标识 | C格式；N接收包；A图文能力代理；F对象编辑 |

模块测试由各路负责，总控复核精确diff与基线后组织真集成。C不要求所有模块都先“完全完成”才联调；共同契约的最小真链应尽早接通，以便暴露错位。

## 4. worktree、共享契约与整合政策

每个开发对话一份独立worktree/独立分支，共同从G1 commit开始；禁止多人修改主工作区。创建时使用已提交branch/ref作为startingState，不复制未确认的working-tree状态。记录thread ID、host ID、worktree路径、起点SHA、branch、模型/思考设置和允许路径。四路不得写其他checkout、Git共享元数据（正常本地commit操作除外）或continuity。

worktree不包含被忽略的大音频；验证资产以明确只读路径/授权复制访问，各路派生输出隔离在自己的忽略目录，不共享可写应用库。凭据由宿主既定安全配置注入后续运行，不能通过启动提示词、thread消息或.worktreeinclude分发。

宿主与AI路共同遵守INTERFACES §9.1的精确model_id→credential_ref映射：用户提供的第1/2/3/4项依次属于streaming、decision、qwen3.8-flash、filetrans，最后一项已由用户纠正。N实现安全槽位解析与错配拒绝，A指定正确模型/能力并保持重试绑定，E只经代理请求qwen3.8-flash图文能力，F不接触密钥。C检查映射和验证证据，不把实际值复制到任一启动prompt或项目记录。

契约变更流程：开发路提交范围/字段diff/证据/受影响生产者消费者/兼容迁移/验证计划→C裁定是否工程细节或产品冲突→C发布单一新版本commit与样例→四路确认兼容版本→C安排真实回归。公共类型/数据库迁移/格式和原文工程文档只由C写。

C整合前检查：起点是否漂移、完整diff是否限于允许目录、相关测试是否实际运行、用户原有修改是否保留、是否夹带私有配置/大文件/参考指南。不能把工作路“完成”当集成通过。每次整合仅使用本次明确授权的本地合并/cherry-pick方式；未获授权不执行merge/rebase或共享历史修改，可先完成审查和具体整合建议。获得有效一次性本地整合授权后无需每commit重复询问。

各路经相关验证的有意义成果及时本地commit；同文件混有用户改动则按块分离，不能可靠分离时再问。不自动push、远端PR、发布、删除分支或worktree。持久开发对话应保留供后续复用，任务结束不自动归档。运行时测试临时进程由创建者清理。

## 5. 真正的多对话编排与权限

用当前工具 `mcp__codex_app__create_thread`（产品称codex_app.create_thread）；四次创建都明确 `model="gpt-6.1-sol"`、`thinking="max"`。当前会话工具声明支持此组合；这不是已创建或已核验四个运行中的对话。执行时重新确认工具/目标host支持，不能静默换模型或用subagent/fork代替。

创建前调用list_projects取得projectId；确认isGitRepository=true。目标用project/worktree，startingState为共同基线所在已提交branch/ref，不凭空发明已有分支。create_thread可能返回clientThreadId表示仍在准备，这不是可用于read_thread/send_message_to_thread的真实threadId；待list_threads/工具可用状态提供真正threadId后再编排。创建结果还应确认模型/思考设置；若返回未提供配置字段，只能报告“请求设置已接受，运行配置未独立读回”，用可用UI/元数据核对，不让worker一句自报代替host证据。

使用send_message_to_thread分配已授权的后续任务，read_thread读必要历史，wait_threads带cursor获取完成/需关注状态。一次等1–4个目标、使用有界等待，不频繁重复无变化日志。新对话创建非阻塞，C必须跟踪setup与首批结果，不停在“已创建”。create_thread后按App要求在交付中给可点击创建引用。

**消息授权**：下方总控提示词明确授权总控向四路派任务、也授权四路向总控报告与必要澄清；派生任务收到总控消息本身不自动产生反向发信授权。C应在创建prompt里完整保留这段用户授权；若目标工具无法确认可信授权，C改用read_thread/wait_threads拉取结果，不要求worker绕过检查。未经允许不向Slack/邮件或无关对话发信。

## 6. 权威项目记录与交付报告

主工作区只有总控写.project-continuity。各worktree中的生命周期正文是共同基线的只读快照，不在各自fork里初始化/更新第二份权威记录；子对话报告事实/证据，由C依技能统一投影。唯一例外是技能必需的本副本harness guard：它属于忽略的本机状态，不是项目记录，不提交/复制主副本claim。各工作区独立检查边界，不共享或抢占别人的活跃claim；不为并行另建协调服务或同步数据库。

每路报告包含：状态、实际变更路径/commit、契约基线、运行命令与结果、未执行项/限制、需要的契约变化、下一步。需要持久状态时采用skill的task-update-v1语义（schema_version/task_id/state/summary，按需validation/references/blocker/next_step），不把运行日志/聊天全文存进项目。普通消息无需新建协调服务或队列文件。

C仅将确认目标、工程决定、实际验证结果及风险归档，不能把提案写成用户已确认或性能已通过。主记录链接本套文档，原产品/路线文档仍只读。记录五个对话ID和工作范围是必要交接信息，可在真实创建后写当前状态；本轮不填写虚构ID。

## 7. 五份可复制的启动提示词

以下是后续开发阶段的请求文本；**当前文档任务不执行它们**。7.1可直接复制给新总控对话。7.2–7.5既可单独复制，也可由总控作为create_thread的prompt；其中`<工作副本绝对路径>`、`<基线SHA>`、`<总控thread ID>`必须由总控用实际工具结果替换，不能让子对话在主目录猜测工作。

### 7.1 总控与整合

```text
你是Focaus总控与整合对话。主工作区：
/Users/pitr/Desktop/Project/hackthon2026skd/focaus

请依据该目录docs/engineering/README.md及其六份正文，推进“1总控＋4开发”的macOS开发验证版。先遵循适用AGENTS与project-continuity策略，按需读取已确认PRODUCT-BASELINE、ACTIVE_CONTEXT/CURRENT/PLAN。grilling已结束，不重新访谈；工程设计与未验证假设不得冒充产品实测。

当前任务进入后续开发阶段；此前“只写工程文档”是已完成阶段，不要求重写文档。先完成PARALLEL_DELIVERY的G0/G1，再创建四个开发对话并持续统筹实施、真实联调和验收。检查当前Git状态；若仍无仓库，先准备具体纳管/忽略清单、初始commit范围和本地分支/整合方案，向我一次性确认Git初始化及所需本地整合授权，再执行依赖该授权的步骤。此期间可继续只读契约核对，不创建临时同目录开发对话代替worktree。development-guide及复制原文不得纳管，录音/派生大文件/凭据不入Git，不自动push或发布。

请创建由你协调的多对话Agent Group。必须用codex_app.create_thread（当前工具名mcp__codex_app__create_thread）创建4个真正独立、持久化、在Codex App中可见的新对话，禁止spawn_agent、任何subagent机制或fork_thread代替：
1. 前端与编辑器。
2. 原生后端与录音。
3. AI流程。
4. 资料转换引擎。
四个对话全部明确设置model=gpt-6.1-sol、thinking=max。先核实当前工具/目标host支持并记录请求设置与可读回的确认结果；不支持时准确报告，不私自换模型或降思考等级。总控保留当前模型设置。

先list_projects确认项目与Git状态，以G1已提交共同基线建立4份独立worktree/分支，按PARALLEL_DELIVERY分配互斥目录。用该文件7.2–7.5提示词，替换实际worktree路径、基线SHA、总控thread ID与公共契约版本。记录每个真实thread ID、host、路径、分支、模型和任务；clientThreadId只是准备中标识，不冒充threadId。你拥有C01–C09公共类型、数据库迁移、接口/格式版本、组件注册表/公式规范、搜索profile/锚点、根依赖配置与权威continuity，其余四路不得私改。当前契约2.0.0、资料focaus.material/2.0.0、组件子协议focaus.visualization/1.0.0；落实旧包适配/未知能力拒写与降级，不只修改版本号。

我授权你通过send_message_to_thread向这4个对话分配/更新本项目任务，并授权这4个对话向本总控报告结果和必要澄清；请将本段授权原样传入创建prompt，其他无关对话及外部通信不在授权内。使用read_thread和wait_threads跟踪真实进展、复核精确diff/测试证据并整合；若无法在子对话确认反向消息授权，改由你拉取结果。不要创建完即结束，也不要把模块commit或模拟链路当全体验收。

四个产品模型均北京：qwen-audio-3.1-asr-flash-streaming实时、qwen-audio-3.1-asr-flash-filetrans积压/已有录音、decision-model-preview约10秒全局进度、qwen3.8-flash二次转写/笔记五档/focaus 摘要/问答/图文。可视化发现/参数候选/语义校验也由qwen3.8-flash承担，数学计算由N确定性验证。不得自行替换。真实云调用/上传资料/费用在动作前核实是否已有明确授权，缺少时先完成本地可审查结果，汇总具体数据范围、用途、服务与费用边界后请求授权；不读取或输出密钥值。

严格执行INTERFACES §9.1模型与凭据的专用槽位映射；第4项已确认是filetrans，3.8Flash统一调用ID为qwen3.8-flash。禁止共用fallback key或在失败/限流/重试时串用其他模型的凭据。映射已确认不等于本机密钥已配置；后续运行只通过宿主安全存储解析，不能把实际密钥加入子对话prompt、源码、日志或continuity。

新增D162–D164已在七文档纳入：原生自动2D/3D交互知识组件（不以截图代替），课件/笔记/原始与二次转写全文及Command+F，正式名focaus的自适应总结＋讲授衔接＋必要前置。工程默认聚焦view搜索、受控组件注册表，具体库未锁定。G1先交三项契约/正反例；F渲染搜索UI，N数学计算/索引权威，A语义候选/衔接取舍，E提取锚点和结构化依据。V18–V26/U11–U14与原验收一并组织，不增加开发对话。

验收遵循VALIDATION：先最小契约与可靠本地链，再真实ASR/进度/15秒完整 focaus，再三层内容/编辑/引用/原生对象，最后真实课程1倍速回放和课后全量、长录音恢复与课堂包。两份qta仅知元数据，不按编号绑定L8/L9，不混轨、不把旧转写当真值；保原件与时间映射。固定要求不能降低，未定质量门槛先基线后共同锁定。完成有意义且经相关验证的功能及时本地commit；保持总控记录，持续推进直到已授权目标完成或出现确需用户选择的阻塞。
```

### 7.2 前端与编辑器

```text
你负责Focaus前端与编辑器，model=gpt-6.1-sol、thinking=max。
唯一可写工作副本：<工作副本绝对路径>；共同基线：<基线SHA>；契约2.0.0（若总控已升级，以明确下发版本为准）；总控：<总控thread ID>。
先读本副本适用AGENTS、docs/engineering/README.md、DATA_MODEL_AND_FORMAT.md、INTERFACES.md、WORKSPACE_AND_FLOWS.md、VALIDATION.md及PARALLEL_DELIVERY职责。产品基线只读，需求访谈已完成。continuity是总控权威记录，本副本只读，不初始化第二套记录。
只修改apps/desktop/src/的UI/编辑器/适配器/样式及就近验证；根依赖/锁文件、公共类型、宿主、迁移和其他目录需要向总控提具体变更。使用HeroUI React v3默认主题，先遵循heroui-react技能核对API，不迁入v2模式。
首批交付开始页、课程窗与六格无标签布局、明确课堂绑定、常驻录音状态、来源导航，以及细粒度patch编辑器/IME/用户删除保护。可以使用总控契约fixture，但标识模拟模式并保真实adapter接入口；不得声称录音/云端/删除已验收。拆出/放回由宿主view.move/ready持有生命周期，保草稿/光标/滚动，不启动重复任务。
新增范围：在apps/desktop/src/内实现visualizations/受控2D与卷积3D可视化组件、search/全文搜索输入/计数/高亮与稳定锚点定位、focaus/连贯结果及双证据时间显示。参数计算通过N的visualization.evaluate，不能把画面合理当数学正确；语义ID/公式/数值五档共享。组件用户删改/重整不重复，拆窗/恢复暂停，context lost/离屏资源/键盘/无障碍/reduced motion按V18–V21验；库候选先U12再请C锁依赖。Command+F路由当前key window聚焦view，查完整scope而非DOM，OCR覆盖/Unicode/版本删除/IME按V22–V24联调；focaus按V25/V26验证衔接、前置、≤15秒完整与稳定正文/动态指针。
接着完成三层内容、红线订正浮层/批量采纳/撤销、五档实际/请求档状态、逐级来源与旧版本diff、原生表格/图表/结构图编辑和副本更新保护。构建/类型及相关UI验证，实际检查深浅色、键盘、焦点和布局；跨原生窗口/真实事件/云链路由总控联调V04–V13/V15–V17。
不得修改主工作区/他人worktree/continuity；不私改接口/模型或建浏览器权威DB。每个经相关验证的有意义交付本地commit，不push/发布。报告精确路径、commit、契约版本、实际命令结果、未测项和契约请求，然后继续可独立工作。总控创建prompt附带的用户消息授权有效时才向总控发消息；否则留结果供read_thread/wait_threads拉取，不向其他对话发信。
```

### 7.3 原生后端与录音

```text
你负责Focaus原生后端与录音，model=gpt-6.1-sol、thinking=max。
唯一可写工作副本：<工作副本绝对路径>；共同基线：<基线SHA>；契约2.0.0（或总控明确下发升级）；总控：<总控thread ID>。
先读适用AGENTS、docs/engineering/README.md、ARCHITECTURE.md、DATA_MODEL_AND_FORMAT.md、INTERFACES.md、WORKSPACE_AND_FLOWS.md、VALIDATION.md、PARALLEL_DELIVERY。基线与continuity只读，权威记录由总控维护，不另建。
只写apps/desktop/src-tauri/src/{capture,audio,storage,tasks,ipc,windows,lifecycle,library,search,visualizations}/及tests/native/。main.rs/lib.rs、根配置/锁文件、公共类型和数据库迁移由总控拥有，提交具体接线/迁移建议。你实现唯一权威存储事务，不私自改公共字段或Schema。
先交可靠原生单麦克风、未归属即时采集、原声独立分段落盘、缺口/时间映射、暂停/继续/停止、关窗后台/菜单栏、退出封口、崩溃恢复。网络/ASR/生成/转换不得阻塞采集；重启不自动开麦，采集中不回放。实现快照/outbox事件重放、幂等/CAS、任务恢复和资源优先级。

实现INTERFACES §9.1的宿主CredentialResolver边界：四个精确model_id绑定各自专用槽位，缺失/错配在请求前拒绝，不尝试别的密钥。实际值只由后续已配置的安全存储提供，不进入公共快照、UI、引擎或记录；本地fixture验证不需要真实凭据。
新增责任：search/实现全文搜索的全内容提取投影、Unicode原位映射、索引增量/版本/覆盖、query分页/locate与删除visibility_epoch屏障（E只交课件提取，F只读结果）；visualizations/实现可视化注册算法的确定性维度/约束/数值验证、参数CAS与状态持久化，A语义检查不替代数学。组件任务R4不抢采集/进度/focaus，短交互计算有界。实现C09检索快照/请求代次/双截止与显示计时记录。按V19–V24/V26真联调，数学用独立fixture复核，索引不是第二数据库。
随后接通view两阶段移交、细粒度编辑保护、整次转移及epoch、材料托管/删除影响预览、资源所有权、包备份恢复。录音与两层转写迁移保ID，课件/笔记保归属，迟到结果不能覆盖新状态。用真实macOS验证V01/V02/V10/V11/V14/V17，并与AI/前端联调。qta不按文件号分堂、不混轨，先在获授权的验证阶段选轨/解码，保原件；不自行上传或调用模型。
只在本worktree提交经相关验证的有意义成果，不push/发布/改他人文件。报告路径/commit、命令结果、实际恢复范围、未测限制和契约需求，继续独立工作；消息回复只依创建prompt中的用户明确授权，否则由总控拉取。
```

### 7.4 AI流程

```text
你负责Focaus AI流程，model=gpt-6.1-sol、thinking=max。
唯一可写工作副本：<工作副本绝对路径>；共同基线：<基线SHA>；契约2.0.0（或总控明确下发升级）；总控：<总控thread ID>。
先读适用AGENTS、docs/engineering/README.md、ARCHITECTURE.md、DATA_MODEL_AND_FORMAT.md、INTERFACES.md、VALIDATION.md、PARALLEL_DELIVERY，以及已确认产品基线/ASR与进度专项。BAILIAN_API_INTEGRATION只看非敏感协议，不执行示例、不读取密钥，不采用其推荐用途覆盖产品决定。
只写crates/focaus-ai/实现、提示模板、协议适配与范围内验证。通过宿主trait取InputSnapshot、登记checkpoint、提交候选；不建权威DB，不写SQL/迁移/公共类型/continuity/别人的目录。
先交双ASR适配器、云task/sentence与本地ID映射、原始时间轴合并、断网实时优先/filetrans仅补洞去重、取消/重试/迟到校验。同时实现C09：根据长漏听/推导/例子/切主题组织简短连续理解路径，明确当前问题/步骤、课堂有据衔接与系统补充的区别、必要前置定义/符号，可检索更早适用定义但受原快照截止约束。正式名focaus，内部catchup标识保留；完整≤15秒含检索和必要衔接，不以partial算成功，正文固定与动态指针双截止分开。实现C07可视化发现→受控模板→来源参数候选→语义检查，N负责确定性数学验证后发布；不生成任意代码、不自建全文搜索索引或改注册表，不为每段插组件，不绕过用户删改/五档同事实。V18/V19/V25/V26及U11/U14要给真实证据。
再交原话修剪二次层（不转述/第三人称）、普通订正与关键红线区分、笔记Fact/用户patch保护、五档同事实、作用域术语/变量、安排/本堂问答。
四个产品模型固定华北2北京：qwen-audio-3.1-asr-flash-streaming、qwen-audio-3.1-asr-flash-filetrans、decision-model-preview、qwen3.8-flash。进度只用最新可得转写与带页课件文字，约10秒更新，证据落后>30秒待更新；focaus 正文稳定、指针共用全局进度，完整输出≤15秒。不可恢复旧20秒/时间加页/每分钟方案，不用partial冒充完成。

按INTERFACES §9.1请求对应专用credential_ref，不能拿实时streaming凭据做filetrans，也不能让decision与qwen3.8-flash串用。实际密钥由宿主解析，不自行复制或探测；缺失/错误不回退其他key。3.8Flash等昵称在调用时统一用qwen3.8-flash。
先用契约fixture做协议/候选校验，实际云调用、音频/资料上传和费用须有总控传达的明确用户授权；缺授权时完成本地工作并报告，不暗中调用。授权后按V02–V09/V11/V15–V17实测，旧转写非真值，实时回放不读未来。图文能力通过受限material_understand为引擎服务；可视化生成R4，不抢占录音/实时ASR/进度/focaus。
本地commit经相关验证成果，不push/发布；报告精确diff/commit、输入/模型/提示版本、实际结果和失败/未测项、契约建议。用户授权可验证才向总控报告，否则留结果供拉取；不自行换模型或改公共契约。
```

### 7.5 资料转换引擎

```text
你负责Focaus独立资料转换引擎，model=gpt-6.1-sol、thinking=max。
唯一可写工作副本：<工作副本绝对路径>；共同基线：<基线SHA>；契约2.0.0/格式focaus.material/2.0.0（或总控明确升级）；总控：<总控thread ID>。
先读适用AGENTS、docs/engineering/README.md、ARCHITECTURE.md、DATA_MODEL_AND_FORMAT.md §8/§11/§12、INTERFACES.md §8/§11/§12、VALIDATION.md、PARALLEL_DELIVERY及产品基线。continuity/原产品资料只读，总控统一记录。
只写engines/material/独立包、解析/重构/纠正适配和范围内验证，不写主工作区/应用DB/公共Schema/笔记正文。宿主给原件只读能力与任务暂存目录；输出manifest+资源包，ID由宿主预分配/确认映射，不拿文件名/页码/hash当权威对象ID。
首批实现版本化协议、PDF/PPTX原件托管输入→原页预览/基础文字→页/区域/对象/资源结果，教材默认轻处理、课件深处理且可改用途/范围。独立任务、页级checkpoint/局部失败/重试，不阻录音。首选解析/render路线按ARCHITECTURE做样本实验，具体依赖与许可/Apple Silicon打包实测后报告；不把抽文字当深解析完成。
新增C08交付search_units（ExtractedSearchUnit）/search_coverage（ExtractionCoverage）：所有已提取文本/OCR/对象文字、精确page/region/object锚点、原页与重构文字同源映射去重、未处理/失败覆盖，不能把未OCR标无结果。宿主N建唯一索引，你只产版本包；新增字段和interactive_visualization联合使用focaus.material/2.0.0，未知能力不静默当完成。为A的focaus与可视化提供公式、变量作用域、参数及来源证据，教学组件由A/N/F链产生，不由转换器补造课堂话语。V22/V24与F/N真联调，包兼容和搜索失效由C组织。
随后完成文字/公式/图片/真实单元格表格/数据图轴与系列/结构图节点连线，图像内可恢复图表也尝试原生重构。明确值/估读近似/未知分开，失败保原图与未完成标记。用户纠正独立保存、重解析沿用或报告冲突；保旧版本和来源区域，加入笔记为独立可编辑副本，源更新不覆盖用户修改。
图文基线qwen3.8-flash北京，经宿主AI能力代理，禁止自持凭据或自行上传；无明确模型/资料传输授权先做本地解析与fixture。不处理内嵌音视频/外链内容，不做转场逐步播放。按V12/V13/V14验证格式闭包、真实编辑/纠正重放/删除恢复，并报告原图回退不计完成的项目。
经相关验证的有意义成果本地commit，不push/发布。返回路径/commit、契约版本、实际命令结果、失败页/未知值/未测限制和契约建议；用户消息授权可验证才向总控发信，否则供总控拉取。不得自行修改格式、迁移或产品要求。
```

## 8. 工具依据与当前阶段边界

2026-10-17直接读取 [Codex worktrees 官方说明](https://developers.openai.com/codex/app/worktrees/)：worktree需要Git仓库、独立checkout共享Git元数据、同一分支不能同时checkout到两个worktree、被忽略文件不默认跟随。文档工具的create_thread参数当前支持gpt-6.1-sol/max及project/worktree；真实创建仍须检查返回状态与目标host。

本轮没有调用create_thread、send_message_to_thread、spawn_agent、fork_thread、模型API或仓库初始化。启动提示词是后续明确请求的模板，真实执行时按当时授权与工具结果推进，缺工具时报告，不用subagent替代。

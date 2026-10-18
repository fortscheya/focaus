# 公共接口与任务协议

工程契约 2.0.0｜2026-10-18。命令名、状态和字段语义是跨模块契约；本轮不生成实现、Schema文件或测试程序。公共实体详见 [DATA](DATA_MODEL_AND_FORMAT.md)，窗口事务详见 [WORKSPACE](WORKSPACE_AND_FLOWS.md)。

## 1. C04：传输封套、初始同步与事件

Tauri IPC 只暴露白名单命令，以版本化 JSON 数据传输；不允许任意 SQL、shell、路径或 HTTP proxy 命令。前端无文件系统/云服务直写权。引擎 JSON Lines 使用相同命令关联原则，但只暴露引擎能力子集。

### 1.1 Command / Result

每条命令必须有 `contract_version, command_id, name, actor, window_id, payload`；无窗口的内部调用window_id显式为null。变更命令额外有 `idempotency_key, expected_revisions`。actor 类型为 user/host/ai/engine，宿主依据调用通道验证，不能信任调用方自报权限。expected_revisions 为 `{entity_id: revision_id}`，不可省略目标已有版本。

成功返回 `{command_id, status:"ok", result, committed_revisions, event_seq}`；只排队成功返回 `status:"accepted", task_id, current_state`，不表示任务完成。失败返回 `{command_id, status:"error", error:{code,retryable,scope,message,details}}`，details 只允许安全字段，不回传密钥、服务URL或完整供应商响应。所有响应包含 contract_version；accepted可附不含正文的result标识（如search_id），仍以task_id跟踪执行。

幂等作用域为 `(actor identity, command name, idempotency_key)`；相同键与相同规范化请求返回同一结果，相同键不同请求报 `IDEMPOTENCY_MISMATCH`。command_id 是本次传输追踪号，重发可变；idempotency_key 代表同一用户意图，不因网络重试变化。一次点击恢复与用户主动再次生成是不同意图。

### 1.2 事件与无缝初始同步

权威事件必须有 `event_id, contract_version, global_seq, scope, event_type, entity_id, entity_seq, revision_id, cause_command_id/task_id, committed_at, payload`。事件只能在事务提交后发出；outbox 与数据同事务持久化。事件是变更通知/小patch，音频字节与大资源不经事件广播。

1. UI先安装事件接收器，再调用 `state.open`。宿主注册订阅并原子读取一致性快照及 `snapshot_seq`，订阅从该序号之后缓冲。
2. UI用快照替换本地投影（保存独立未提交草稿），调用 `state.ack` 确认 snapshot_seq，宿主重放其后事件。权威事件按批交付：`subscription_id, after_seq, through_seq, events`，through_seq表示服务端已经扫描至该全局水位（含被scope过滤的事件）。UI按event_id去重、按global_seq排序，确认through_seq；合法scope过滤可造成事件序号跳号，不能误判丢包。只有批次after_seq不接当前已确认水位或明确传输缺口才请求replay。
3. scope 是课程/课堂/view/全局录音等白名单集合。重连传 last_seq；日志已截断返回 `CURSOR_EXPIRED`，重新快照，不假定漏掉的事件无关。
4. `recording.live_meter`、模型 token preview 等可丢的临时事件使用独立 `ephemeral_seq`，不占权威 global_seq，不写数据库事实。final事件才更新权威版本。

| 命令 | 调用方 → 拥有方 | 输入 | 输出/状态变化 | 失败语义 |
| --- | --- | --- | --- | --- |
| `state.open` | UI → 宿主 | scopes、last_seq可空、支持contract/format major、capabilities | subscription_id、快照、snapshot_seq；或可重放增量 | 未支持版本拒写；过旧游标要求快照 |
| `state.ack` | UI → 宿主 | subscription_id、snapshot_seq/upto_seq | 开始重放/推进确认水位 | 不接受倒退或越过已交付批次through_seq |
| `state.replay` | UI → 宿主 | subscription_id、after_seq | 有序事件/快照要求 | CURSOR_EXPIRED，不静默跳过 |
| `state.close` | UI → 宿主 | subscription_id | 释放订阅，不影响业务任务 | 重复关闭成功 |

Scope为判别联合：`{kind:global}`、`{kind:course,course_id}`、`{kind:lesson,lesson_id}`、`{kind:entity,entity_id}`、`{kind:view,view_id}`；宿主校验窗口/worker的可读边界。AppSnapshot固定包含contract_version、host_run_id、snapshot_seq、scopes、global（唯一录音状态、设置revision、窗口注册表）、records（courses/lessons/material attachments/content heads/task summaries/progress/view states的按ID映射）和continuations。records另含所请求目标的visualization内容状态、search index/coverage水位和focaus当前result head；不包含GPU上下文/完整结果缓存。records只含所请求作用域、每项带revision；正文/大对象用content.get按head revision分段读取，continuation固定snapshot_seq与读取版本，不能混读两次状态。空集合显式为空，未请求集合省略；不能将省略解读为已删除。

权威事件基础目录（新增组件/搜索事件见§11/§12）：`recording.state_changed / recording.coverage_changed`、`transcript.raw_changed / transcript.secondary_changed`、`content.committed / content.proposal_ready / content.source_status_changed`、`task.changed`、`progress.changed`、`material.page_ready / material.parse_changed`、`workspace.view_changed / workspace.layout_changed`、`library.deleted / library.restored`。事件 payload 至少含相应实体 ID、revision 和变更范围；消费方必要时重新查询实体，不从事件重新推导第二套事实。

## 2. 任务、取消与失败的统一状态机

`Task` 必须含 task_id（本地UUID）、kind、target、snapshot_id、state、priority、attempt、deadline_at（可空）、idempotency_key、checkpoint、error（可空）、provider_bindings、created_at、updated_at。模型任务还含 model_id/region/prompt_version/parameters_hash；本地任务不能伪填 model_id。

冻结 kind：`asr_stream / asr_file / secondary / note_incremental / note_reorganize / language_render / catchup / progress / qa / material_convert / material_reparse / material_understand / export / restore / visualization_discover / visualization_build / visualization_validate / visualization_evaluate / search_index / search_query`。新增kind继承相同状态机，不另建权威任务库。

| state | 进入/离开规则 |
| --- | --- |
| `queued` | 持久排队→running；可在开跑前取消/被新版superseded |
| `running` | 正在执行→succeeded/failed/retry_wait/waiting_network/waiting_input/paused/cancel_requested/interrupted/superseded |
| `waiting_network` | 网络不可用，checkpoint保留；恢复后queued |
| `waiting_input` | 缺音轨选择、来源或必要用户选择；满足后queued，不自动猜 |
| `paused` | 宿主因资源预算/用户暂停任务，保checkpoint；与录音paused独立 |
| `retry_wait` | 退避等待→queued；达到预算→failed |
| `cancel_requested` | 已记录取消屏障，尚在释放资源；拒绝其后发布内容→cancelled |
| `interrupted` | 崩溃恢复观察到在途任务；按类型查询/重放/放弃 |
| `succeeded / failed / cancelled / superseded` | 终态；人工重试生成新task/attempt关系，不能偷偷改终态正文 |

远端服务 task_id 记为 `provider_task_id`，远端 status 保在 provider binding，不复用本地任务枚举。任务完成≠候选全部被采纳：succeeded 可产出 stale/review proposal，result 必须报告 `publication=committed / review_required / rejected_stale`，UI不得显示已更新正文。

错误码冻结：`INVALID_ARGUMENT, UNSUPPORTED_VERSION, NOT_FOUND, SOURCE_UNAVAILABLE, SOURCE_DELETED, CONFLICT, STALE_INPUT, IDEMPOTENCY_MISMATCH, CURSOR_EXPIRED, RECORDING_BUSY, LANGUAGE_LOCKED, PLAYBACK_BLOCKED, GRID_FULL, EDIT_PROTECTED, PERMISSION_DENIED, DEVICE_LOST, DISK_FULL, CORRUPT_ASSET, NETWORK_UNAVAILABLE, AUTH_REQUIRED, RATE_LIMITED, PROVIDER_ERROR, DEADLINE_EXCEEDED, CANCELLED, INVALID_RESULT, UNSUPPORTED_CAPABILITY, SEARCH_INVALIDATED, INDEX_NOT_READY`。

所有错误须有 scope（command/task/asset/page/recording）、retryable、用户可见短因、可选 safe diagnostics。鉴权错误不无限重试；schema错误不让原始模型JSON入库；资料单页错误不失败整次录音。

网络重试按 Retry-After 或有上限指数退避＋抖动；具体次数/时间为 U04 参数，必须记录总预算。快照已过期时先 supersede，不能重试旧输入直到成功。只取消本地不能保证云任务停止计费/执行；保 provider binding，屏蔽晚结果并尽力取消/清理，不宣称远端已取消。

catchup 从用户按下时起15秒截止，预算含排队、快照、网络、生成、校验与显示。超时返回 DEADLINE_EXCEEDED，未完整结果标 partial/failed；不得把首字、缩短输入却不告知、或超时后到达结果计作达标。progress 周期不是重试预算，过期请求不能堆积。

端到端时长用同一UI单调时钟测量“输入发生→完整结果可见”，UI的15秒deadline负责最终状态判定，系统日期调整不能刷新预算。triggered_at仅为审计墙钟；宿主给排队/网络/校验保守子预算，不能拿服务计时替代UI结果。后台超时后返回可供用户明确查看的迟到候选，但本次仍计超时，不能回填为成功。

## 3. 宿主内部服务与唯一写入口

| 接口 | 调用方 → 拥有方 | 输入 | 输出/状态变化 | 失败与保护 |
| --- | --- | --- | --- | --- |
| `identity.reserve` | AI适配器/引擎代理 → 宿主 | 类型、作用域、操作幂等键、候选匹配线索 | 稳定 ID 映射，不写业务正文 | 键复用冲突拒绝；不允许模型自报权威UUID |
| `snapshot.build` | 任务管理器 → 宿主存储 | purpose、target、证据截止、范围、档位 | 不可变InputSnapshot及允许访问的资源句柄 | 缺失/删除来源明确返回，不读取未来材料 |
| `task.enqueue` | UI命令处理/业务模块 → 宿主 | kind、snapshot、priority、deadline、键 | accepted + 本地task_id | 同输入意图去重；资源不足排队可见 |
| `task.checkpoint` | worker → 宿主 | task_id、attempt、cursor、safe state | 持久checkpoint/task.changed | 旧attempt或终态写入拒绝 |
| `result.submit` | AI/引擎 → 宿主 | task/attempt、snapshot_id、候选、引用、暂存资源 | 事务校验→commit/review/stale；task结果 | 过期/取消/保护范围不能直接写正文 |
| `resource.read` | UI/worker → 宿主 | resource_id、授权范围 | 有限字节/宿主资源句柄 | 不接受任意绝对路径或远程URL |
| `proposal.apply` | 用户命令/允许自动整理 → 宿主 | proposal_id、base revisions、采纳范围 | 新revision/edges/undo组 | 冲突返回diff，不能last-write-wins |

这组接口是 Rust traits/受限引擎代理语义，不要求把所有内部方法开放给 WebView。公共类型由总控生成跨语言映射，AI/引擎不得另建“临时权威库”。可重建缓存必须标来源版本且可丢弃。

CAS失败默认不写；宿主可对局部Proposal执行显式rebase：逐个验证selector/before_hash仍匹配、touched范围不与用户patch/删除意图/lease相交、引用内容未改变，再在新base上事务提交并返回实际committed_revisions。全文重整、来源删除/归属epoch变化、关键内容已改不能自动rebase。来源仅追加新尾部不必作废已覆盖的稳定范围，提交仍保原snapshot_id与真实evidence_cutoff；不得借rebase假造新的输入快照。

## 4. 前端命令目录

以下所有持久变更都适用 §1 的幂等与 CAS。公共约定：查询返回精确版本；变更返回新版本及事件；长任务只 accepted。所有者均为宿主对应协调器，前端仅请求。

| 命令 | 调用方 → 拥有方 | payload / result | 状态变化、失败语义 |
| --- | --- | --- | --- |
| `course.create` / `lesson.create` | UI → Library | 名称/course_id/可空日期 → 新实体 | 默认课堂档3或全局默认；不启动录音 |
| `lesson.update` | UI → Library | metadata patch/material_scope → revision | 已有内容窗不随选中变化；无效范围拒绝 |
| `schedule.upsert` | UI → Library | 周期/时区/例外/提醒 → rule | 校验时间/冲突；到下课时间不停止录音 |
| `settings.update` | UI → Settings | 白名单偏好/默认语言 → revision | 不回填旧课堂、不接受模型密钥作为UI参数 |
| `content.get` | UI → ContentStore | entity/revision/selector → 内容与来源状态 | deleted仅墓碑；无此版本明确NOT_FOUND |
| `content.patch` | 编辑器 → ContentStore | UserPatch、lease_epoch → revision/undo_group | 相交编辑冲突；用户删除写DeleteIntent |
| `edit.begin` / `edit.end` | 编辑器 → EditCoordinator | target/selectors/view/composition、draft revision → lease | begin前AI不能碰活动区域；end不丢未提交草稿 |
| `content.undo` | UI → ContentStore | undo_group_id、当前revision → 逆事务 | 后续编辑冲突要预览，不覆盖新改动 |
| `correction.draft` | UI → ContentStore | suggestion_id、浮层草稿/IME状态 → draft_id | 不是采纳，不改正文 |
| `correction.accept` / `correction.reject` | UI → ContentStore | suggestion_id、base、最终文本 → revision | 接受/明确拒绝；过期建议不自动迁移范围 |
| `correction.accept_batch` | UI → ContentStore | 当前正文revision、pending suggestion IDs → applied/skipped/undo_group | 原子采纳适用独立建议；跳过rejected/stale，逐项报告 |
| `note.reorganize` | UI → TaskManager/AI | note_id、范围 → task/proposal | 全局结构改变必须预览后proposal.apply |
| `language.select` | UI → ContentStore/AI | lesson_id、tier → requested/actual/status | recording/paused报LANGUAGE_LOCKED；缺档排队保原档 |
| `source.resolve` | UI → SourceResolver | edge_id、返回锚点 → 实际目标/跳层/旧新版本 | 跳层仅导航；删除不返回已删原文 |
| `source.diff` | UI → SourceResolver | SourceRef、新revision → 结构/文本diff | 无新版本/已删返回明确状态 |
| `definition.resolve` | UI → SemanticIndex | definition_id、可见档/作用域 → 短定义和跳转锚点 | 不同定义不合并；跨课程仅建议 |
| `catchup.request` | UI/快捷键 → TaskManager/AI | lesson/recording、triggered_at、范围/起点 → task | 默认max(估计漏听,5分钟)，未知估计回看5分钟且注明；15秒deadline |
| `progress.calibrate` | UI → ProgressCoordinator | recording/lesson、material/page/region、evidence anchor → calibration_seq | 只校准位置，不改转写/证明讲过；晚旧结果拒绝 |
| `qa.ask` | UI → TaskManager/AI | 本堂问题、选段/范围 → answer task | 本堂有效材料；区分背景，不自动写笔记 |
| `note.add_selection` | UI → ContentStore | answer/object/source selection、目标note → 副本/proposal | 显式入笔记；逐级证据未齐为pending_evidence |
| `task.cancel` / `task.retry` | UI → TaskManager | task_id、理由/新的输入选择 → 状态/新task | 不暗中取消录音；超期输入重建快照 |
| `package.export` / `package.restore` | UI → Library | lesson、格式/受限文件选择、策略 → task/预览 | 课堂包闭包/hash校验；恢复冲突先预览，不含凭据 |
| `app.exit` | UI/菜单 → Lifecycle | 明确退出意图 → 封口与checkpoint结果 | 录音结束保存后退出；未保存数据/封口失败必须显著说明 |

## 5. 录音、回放、归属与 ASR

### 5.1 录音命令

录音会话状态：`starting → recording`、`recording ↔ paused`，starting/recording/paused/interrupted均可明确stop进入 `stopping → stopped`。starting失败为 `failed`；设备/崩溃/落盘中断为 `interrupted`，保已采数据。恢复旧数据不恢复采集；interrupted只有在用户明确resume后才经starting建立同会话新采集段，保真实缺口及不确定边界。`stopped` 后重新开始形成新会话，使用课堂 last_recording_tier，不能resume终态。

| 命令 | 调用方 → 拥有方 | 输入 → 输出 | 状态/失败 |
| --- | --- | --- | --- |
| `recording.start` | UI/菜单 → RecordingCoordinator | 可空lesson、device选择 → recording_id、starting | 真正收到采集帧后发recording；已有active报RECORDING_BUSY并给当前归属 |
| `recording.pause` / `recording.resume` | UI/菜单 → 同上 | recording_id、expected state → 状态 | pause封存当前范围；resume接受paused或用户明确恢复的interrupted，先停回放再开采集，失败保原非采集状态 |
| `recording.stop` | UI/菜单 → 同上 | recording_id → stopping/stopped | 原声封口后stopped；ASR/整理继续独立任务 |
| `recording.transfer` | UI → AssignmentCoordinator | recording_id、目标lesson、expected_epoch → 新epoch/影响摘要 | 整次音频及两层转写归属原子改变、不断采；笔记/资料不搬 |
| `recording.import` | UI → ImportCoordinator | 文件选择句柄、可空lesson → 托管asset/task | 先托管副本校验；未选轨/未知对齐waiting_input，不凭文件名归属 |
| `recording.align` | UI → AlignmentCoordinator | 原始区间→课堂/未知、证据、确认状态 → alignment revision | 支持多对多；重叠需显式，不能伪连续 |
| `audio.play` / `audio.seek` / `audio.stop` | UI → PlaybackCoordinator | SourceRef/range → 播放状态 | recording时PLAYBACK_BLOCKED；paused/stopped可播；seek不改采集时间轴 |

transfer事务同写：session assignment_epoch、转写归属投影、旧/新课堂受影响源状态、录音档锁、旧上下文生成任务失效。历史 ASR 音频事实仍可入当前录音；旧课件上下文的派生输出不得写入新课堂，需新快照。正在网络连接若不支持安全更新热词/context，在保采集的前提下切新 ASR 连接/映射，旧连接尾部按时间和coverage合并，不能假装已即时换上下文。

### 5.2 AI适配输入输出

| 接口/任务 | 调用方 → 拥有方 | 输入 | 输出 | 状态变化/失败语义 |
| --- | --- | --- | --- | --- |
| `asr.stream.open` | 宿主TaskManager → AI适配器 | task/snapshot、model/region、格式/采样率、热词/context、映射原点 | provider binding、ready | 等远端started后按协议发帧；连接失败不影响原声 |
| `asr.stream.feed` | 音频支路 → AI适配器 | 有序chunk_seq、AudioSpan、编码字节、mapping | 发送水位、可选背压 | 超出预算丢弃待发实时积压并登记历史缺口，不阻writer |
| `asr.stream.finish` | 录音协调器 → AI适配器 | end watermark | 服务finish/尾句或超时 | 缺尾句登记coverage待补，stop不无限等云端 |
| `asr.raw_candidate` | AI适配器 → 宿主result.submit | provider task/sentence、connection_epoch、partial/final、相对时间、词/说话人、映射 | provisional/final本地utterance revisions | 云ID去重；final不可被新partial倒退；未知speaker不标教师 |
| `asr.file.submit` | 宿主 → AI适配器 | 未覆盖原始范围、派生文件句柄、mapping、受限上传能力、snapshot | 本地task＋provider_task_id | 本地路径不能作file_urls；远端提交不确定先查已有binding再重试 |
| `asr.file.collect` | 恢复/轮询调度 → AI适配器 | binding/checkpoint | 字/句时间、候选、覆盖报告 | 只填历史洞，保稳定实时；下载/URL过期可重试，不改时间基准 |

stream partial 更新只能覆盖同一远端句的 provisional revision；final化后内容冻结。ASR提交以该音频范围的coverage/utterance revision为CAS范围，不因其他句已提交、整个转写集revision增长而一律拒绝正常尾句；仍须检查原件删除、任务取消与映射有效性。remote句重分段需要新的候选映射，不把 remote sentence_id 顺序当全局顺序。filetrans跨切片词边界须依据时间/词对齐，不能单凭相同句文去重，也不能把正常重复讲述删掉。

## 6. 二次转写、笔记、语言、问答

全部由 TaskManager 调 AI crate，输出通过 `result.submit` 提交。输出必须回显 task/attempt/snapshot_id 与 contract_version，不接受模型自行指定任意文件或数据库动作。

| kind | 输入契约 | 输出契约与发布政策 | 失败语义 |
| --- | --- | --- | --- |
| `secondary` | 有序稳定Raw refs、上下文、当前Secondary revision、用户订正/拒绝、保护范围 | 原话修剪operations＋逐级边＋普通修正审计＋关键建议；顺序不乱，学生猜测不当教师确认 | 转述/第三人称/无来源/关键语义自动改判INVALID_RESULT，不能“宽松解析”入正文 |
| `note_incremental` | Secondary/Material refs、共同Fact集、既有对象、删除意图、完整段落/安排触发 | Fact delta＋局部object ops＋source edges；无新信息no-op；用户区域提案 | 未完成推导不全重写；来源过期stale，用户相交review |
| `note_reorganize` | 全局有效来源快照与全部用户patch/删除意图 | 重排/合并/新增差异预览、稳定身份映射 | 必须人工采纳；无法映射身份不得以新建整份掩盖 |
| `language_render` | 精确fact_set_revision、目标档、已有呈现patch/定义映射 | 同Fact IDs呈现、alignment与保真检查 | 漏否定/数值/条件或事实增加为INVALID_RESULT；旧fact集结果stale |
| `catchup` | §13的范围/输入快照、进度版本、必要前置检索 | DATA §13的focaus_context_v2结果、单一理解路径与双证据时间；候选经校验提交 | 不等下次进度/补齐，缺必要衔接为partial；正文稳定，迟到不得替换新请求 |
| `qa` | 本堂问题/选段、有效资料/转写、检索范围 | answer分段标classroom/material/ai_background及来源 | 无课堂证据就说明，禁止用背景解释冒充教师原话；非自动入笔记 |

安排与术语更新是上述已采纳 Fact delta 的派生投影，仍经宿主事务，不能建立另一份随意被生成覆盖的“真值”。相对DDL使用 Lesson.actual_date，未知则保原句未定。教师更正保前后证据并更新明确值；“老师明确考点/课堂强调/AI推测”单独字段。

## 7. 全局课堂进度

`progress` 调用方为宿主周期调度器，拥有方为 AI 的 decision 适配器；模型固定 `decision-model-preview`，北京。输入只用**最新可得转写与带页课件文字**作为语义证据；候选ID、最近校准/进度和证据截止只是控制元数据，不能添加未来转写、音频推理或“时间到了所以翻页”的证据。

输入含本堂课件范围、材料/页/区域稳定ID、文字描述、未定位/脱离课件选项、calibration_seq、decision_seq。长课件可分层检索候选，但必须可跳页/回页，不限定当前页邻页；候选总数遵守官方限额。输出position_status（located/unlocated/off_material）、page_id/region_id（可空）、confidence、source_refs、evidence_cutoff_ms（可空）、calibration_seq、decision_seq；模型不得返回范围外页码。它只决定位置，宿主负责录音状态与时效标签。

宿主单一 ProgressState 为课件标记、对齐与focaus 指针共用。提交条件：recording/assignment_epoch、材料版本、校准序号匹配；证据与decision_seq不比当前旧。手动校准先升序号，旧在途结果拒绝。后续有新证据的决策可移动位置，手动校准不是永久锁页；用户阅读页/可选跟随状态与模型位置分别存储。

ProgressState分三个轴：position_status=`located / unlocated / off_material`；recording_state=`recording / paused / stopped / interrupted`（采集starting暂沿用前态，未开录音为stopped）；freshness=`current / pending_update`。录音中 `current_recording_ms - evidence_cutoff_ms > 30000` 或尚无证据截止时，freshness强制pending_update，保last_position入口。这里current_recording_ms是同一recording原始时间坐标的当前采集时点，不能混用其他录音或服务时间。返回时刻不刷新证据时点。暂停/停止保最后证据、不继续倒计时推进位置，恢复后按新采集时点重新算freshness。没有课件/匹配失败为unlocated，既能表达“新鲜但未匹配”也能表达“未匹配且陈旧”，不推断教师未讲。

## 8. 资料引擎与窗口接口

### 8.1 资料

| 接口 | 调用方 → 拥有方 | 输入 → 输出 | 状态/失败 |
| --- | --- | --- | --- |
| `material.import` | UI → Library | 文件句柄、用途/页范围 → Material/Attachment、基础task | 先托管原件；基础文字/页可先用，深解析独立 |
| `material.convert` | TaskManager → 引擎 | source version/受限path、format_version、ID池、profile、页范围、纠正集、budget → 页checkpoint、暂存manifest/resources | 引擎只能写指定暂存目录；局部failed不冒充completed |
| `material.page_result` | 引擎 → 宿主 | task/snapshot/page、对象包、资源hash/诊断 → 原子页版本＋page_ready事件 | 格式/引用/hash错误拒收，不显示半写资源 |
| `material.reparse` | UI/TaskManager → 引擎 | 指定页/区域、base parse、纠正集 → 新候选parse＋映射/冲突 | 保旧版本，匹配失败保纠正待review |
| `material.correct` | UI → Library | 对象/单元格/关系patch与base → Correction revision | 独立用户层，重解析不覆盖 |
| `material.detach` | UI → Library | attachment_id/base → removed revision/影响 | 后续快照排除；历史引用仍可读 |
| `material.delete_preview` | UI → Library | material/source范围 → impact_revision＋计数/副本清单 | 只读；包含间接边与共享资源 |
| `material.delete_confirm` | UI → Library | impact_revision、明确范围、保/删副本选择 → 删除事务/回收任务 | impact变化返回CONFLICT；独立用户修改保护 |
| `material_understand` | 引擎经宿主 → AI适配器 | 受限页/区域图像resource＋文本/结构要求、snapshot → 对象候选/来源区域/unknown/estimated | qwen3.8-flash北京；不返回数据库动作，不自行上传任意文件 |

### 8.2 窗口

| 命令 | 调用方 → 拥有方 | 输入 → 输出 | 状态/失败 |
| --- | --- | --- | --- |
| `workspace.open_course` | UI → WindowCoordinator | course_id → course_window_id | 已开则聚焦，未开创建并进工作区；不改录音 |
| `view.open` | UI → WindowCoordinator | lesson_id、target、grid/standalone → view_id | grid满报GRID_FULL并给两种入口，不自动替换 |
| `view.save_state` | UI → ViewStore | view_id、draft/selection/scroll/layout、组件视角/步骤、搜索UI状态、revision → persisted view revision | 文档编辑先走content.patch，视图保存不写正文 |
| `view.move` | UI → WindowCoordinator | view_id、target host、base view revision → transfer_id、preparing | 预留格位/创建新窗；draft flush失败保原承载 |
| `view.ready` | 新承载UI → WindowCoordinator | transfer_id、view_revision → committed/released | 单一lease移交后销毁旧承载；失败rollback无重复任务 |
| `view.maximize` / `view.restore` | UI → LayoutCoordinator | view_id → 布局revision | 保存/恢复原网格；非删除其他内容窗 |
| `view.minimize` / `view.close` | UI → WindowCoordinator | view_id → closed/minimized view状态 | 仅视图变化，存草稿；后台任务持续 |
| `layout.resize` | UI → LayoutCoordinator | 合法column/row比例、base → layout revision | 最多3列/列2窗，非空填满，不制造标签格 |

## 9. 北京服务适配事实与本项目政策

| model_id（精确拼写） | 职责 | 协议与必须隔离的身份 |
| --- | --- | --- |
| `qwen-audio-3.1-asr-flash-streaming` | 实时原始ASR | WebSocket run-task/finish-task；provider_task_id＋sentence_id＋connection_epoch |
| `qwen-audio-3.1-asr-flash-filetrans` | 历史洞/已有录音非实时ASR | HTTP异步提交/查询/下载；本地task_id≠provider_task_id |
| `decision-model-preview` | 全局课堂讲解进度 | System One结构化choice；不生成focaus 正文 |
| `qwen3.8-flash` | 二次转写、笔记/五档、focaus理解衔接、问答、图文理解、可视化识别/候选/语义校验 | 首选Chat Completions JSON/SSE适配；生成文本与ASR WebSocket完全不同 |

全部 `region=cn-beijing`。端点选择由宿主服务配置提供，北京模型权限在未来获授权的接入阶段验证，本轮不读取配置。禁止文档/日志/前端保存真实凭据或私有服务地址。

### 9.1 模型与凭据严格绑定（2026-10-17补充）

用户已确认此前第4项及积压用途的streaming标注为笔误，正确模型为filetrans。用户所称“3.8Flash / qwen-3.8Flash”在本项目统一指精确调用ID `qwen3.8-flash`，不得将昵称原样作为model参数。按此次用户提供凭据的顺序建立以下非敏感映射；表中K1–K4只是顺序标记，不是密钥值、前后缀或可用凭据。

| 顺序标记 | 精确model_id | 必须使用的credential_ref | 允许职责 |
| --- | --- | --- | --- |
| K1（第1项） | `qwen-audio-3.1-asr-flash-streaming` | `bailian_beijing_asr_streaming` | 实时语音转写 |
| K2（第2项） | `decision-model-preview` | `bailian_beijing_progress` | 课堂讲解进度决策 |
| K3（第3项） | `qwen3.8-flash` | `bailian_beijing_multimodal` | 多模态处理、二次转写、笔记五档、课件理解、focaus理解衔接、本堂问答及可视化语义候选/已定图文任务 |
| K4（第4项，已纠正） | `qwen-audio-3.1-asr-flash-filetrans` | `bailian_beijing_asr_filetrans` | 积压、已有录音及非流式语音转写 |

宿主内部CredentialResolver按 `(provider=bailian, region=cn-beijing, exact model_id)` 查唯一credential_ref，再从安全凭据存储取得该槽位的值用于实际请求。不得使用一个通用API Key覆盖四模型，不轮询/试用其他槽位，也不在鉴权失败、限流或重试时回退到另一模型的密钥。filetrans的提交、查询、结果收集及必要上传能力须沿用本任务所属的授权配置，不借用streaming凭据。

模型/槽位/地域绑定不符时在发起网络请求前以INVALID_ARGUMENT拒绝，safe details可标credential_binding_mismatch；缺少对应槽位或鉴权不可用返回AUTH_REQUIRED。不得通过向云端试探多把密钥来猜正确映射。提供方额外要求独立上传凭据时，应明确建立该能力绑定，不自行把另一模型密钥填进去。

凭据解析只发生在宿主受控适配层；前端、引擎、InputSnapshot、公共类型样例、日志、备份和开发对话prompt中不携带实际值。文档只保存上述映射，不能用文字记录假装已完成本机密钥配置。后续由用户在安全存储中配置各槽位，再由获授权的宿主请求使用；本轮不创建凭据文件/运行配置，不验证密钥，也不调用模型API。

### 9.2 公开协议依据与限制

公开依据为本项目2026-10-17已核对记录及 [ASR总览](https://help.aliyun.com/zh/model-studio/asr-model)、[实时客户端事件](https://help.aliyun.com/zh/model-studio/qwen-audio-asr-streaming-client-events)、[实时服务端事件](https://help.aliyun.com/zh/model-studio/qwen-audio-asr-streaming-server-events)、[filetrans HTTP](https://help.aliyun.com/zh/model-studio/fun-asr-recorded-speech-recognition-http-api)、[decision API](https://help.aliyun.com/zh/model-studio/decision-model-api)、[qwen3.8-flash](https://help.aliyun.com/zh/model-studio/qwen3-8-flash)。该轮只读接入参考的过滤视图；2026-10-18增量修订未重读凭据/接入配置，仍未实际调用。

- filetrans 当前单次一个 URL，≤12小时/2GB；必带异步请求头及 parameters 对象。本地文件须经受控托管/上传才能供服务读取。开发期首评官方临时上传，REST/SDK差异、有效期与清理均需 U04 实测；48小时URL到期不等于对象已确认删除。上传材料必须先有明确授权。
- 热词最多2000且超额可能被截取；context 受每轮400字符与各类型最近5条/轮次规则限制。由AI模块有意筛选本堂术语，不传整份课件。SDK对filetrans context支持有限，首选直接HTTP。stream中途更新上下文尚未验证，不照搬其他型号continue-task。
- decision最多16问、choice最多255选项，必须保未定位兜底；官方latency不是产品端到端耗时。
- qwen3.8-flash可结构化输出仍需本地Schema/来源/语义检查。SSE未收到完整完成信号、finish_reason=length、无有效正文都不能标完成；usage-only包不是内容。focaus首评关闭额外思考以控制预算，参数为可逆工程方案，效果和≤15秒均待U05验证。

## 10. 迟到、重试与冲突处理矩阵

| 场景 | 必须行为 |
| --- | --- |
| 同一provider final重复/重连重复 | remote binding＋本地范围去重，同结果no-op；不重复插句 |
| filetrans覆盖已有稳定streaming范围 | 稳定区不改；补洞，边界歧义保候选 |
| note生成中用户改单元格/打字 | 精确相交部分review/defer，其余可合并；光标/选区不跳 |
| 录音转移后旧派生结果到达 | assignment_epoch不符→STALE_INPUT；ASR仅按原始音频规则特判 |
| 新来源或删除发生后生成完成 | 不发布过期事实；来源删除清候选正文，旧版本可追仅限未永久删除 |
| 新进度/手动校准后旧决策返回 | 拒绝回滚；不以返回时刻更新“最新” |
| 五档新事实产生后旧档生成完成 | 标stale保当前可见档，不混入另一Fact集 |
| 引擎重试重复页结果 | 同task/snapshot/page/hash幂等；同身份不同内容须新revision |
| 崩溃发生在资源落盘与DB提交之间 | 检查manifest/事务日志，复用完整暂存或清孤儿；不产生悬空引用 |
| 取消后服务仍返回 | 接收最小状态便于清理，不发布正文，不改变cancelled终态 |


## 11. C07：交互组件协议

### 11.1 能力协商与候选提交

`state.open`/引擎启动握手增加capabilities：supported_contract_majors、supported_material_majors、visualization_protocol_majors、模板ID及版本范围、search_projection_versions、focaus_result_semantics。响应给chosen版本与unsupported能力；宿主与所有写端须同一major。不能握手支持2却忽略新增对象字段；未知模板仅可无损保留并只读fallback，禁止编辑/执行/宣称ready。旧1.x导入必须走README规定的显式适配器，不能静默丢字段。

本地计算/索引任务使用相同InputSnapshot身份、来源与版本语义，但 `model_id/region/prompt_version` 显式null，`executor_id/executor_version`必填；模型任务这些模型字段必填且executor标适配器版本。不能为本地算法伪填qwen。N保权威state，F/A/E只提交调用和候选。

| 接口/命令 | 调用方 → 拥有方 | 输入 → 输出 | 状态变化与失败语义 |
| --- | --- | --- | --- |
| `visualization.request` | 笔记提交调度器/用户 → N TaskManager | note_id、changed fact IDs/显式选段、base revisions、trigger=`automatic / explicit` → task_id | 排R4 discover；合并未运行范围，相同semantic_key不重复插入；没有适合知识点成功no-op |
| `visualization.discover`（kind visualization_discover） | N → A | InputSnapshot、已有组件/删除键/支持模板/定义作用域 → candidates（purpose、模板族、fact/definition/参数依据、needs_evidence理由） | A语义识别；不受支持不能生成任意代码，证据不足needs_evidence，无权直接写Note |
| `visualization.build`（kind visualization_build） | N → A | 经宿主identity映射的候选、注册表版本、值来源、SourceRefs → 结构化描述/语义检查/插入或更新Proposal | 模型只产schema允许字段，来源/定义冲突review；旧snapshot结果STALE_INPUT |
| `visualization.validate`（kind visualization_validate） | N任务管理器 → N DeterministicValidator | 候选hash/公式ID/参数/资源、注册表版本、A语义检查 → validation report/确定性结果hash | schema/source/math检查，所有必需检查pass才可result.submit；数值/维度错INVALID_RESULT；未知算法UNSUPPORTED_CAPABILITY |
| `visualization.evaluate`（kind visualization_evaluate） | F → N计算池 | object/revision、参数草稿、interaction_seq、算法版本 → accepted/task，完整结果含参数hash/shape/steps/局部计算 | 不改正文；R2有界可取消。旧seq结果丢弃，不覆盖新调参；非法参数INVALID_ARGUMENT且保最后合法结果 |
| `visualization.patch` | F → N ContentStore | UserPatch、base revision/lease、参数或默认交互状态、已验证参数hash（可空） → revision/undo_group/计算状态 | N必须核对自己签发的校验报告/算法/参数hash及当前revision，不能信任F缓存。改数学参数须先evaluate通过，未就绪拒INVALID_RESULT（validation_required）；仅视角修改可不计算。非法拒绝不写，有效改动user_owned |
| `visualization.retry` | F → N TaskManager | object/candidate、base、失败阶段 → 新task与当前快照 | 复用语义身份，不能通过retry覆盖用户patch/DeleteIntent；永久删除源SOURCE_DELETED |

所有数值计算/资源加载均在短写事务外完成，提交时只做版本/报告hash核查；不能在SQLite写锁内算大矩阵。自动插入/更新仍通过result.submit/proposal.apply，不新增绕过用户保护的insert API。移动/缩放/复制/删除/撤销走content.patch/content.undo；复制时宿主分配新对象/子元素ID并转资源所有权，生成记录不复制成新付费任务。自动任务只有在未相交用户/IME区域且全部校验通过才提交；否则返回review_required，用户未采纳前正文不变。

事件：`visualization.changed`（对象revision、generation_status、validation摘要、publication）与`task.changed`为权威事件，服从§1 outbox。F的`visualization.runtime_report`是受限临时状态上报（view_id/object_id、runtime_state、资源租约、safe error），N协调资源预算后返回allowed/suspend；走ephemeral_seq，不保存模型/内容真值。持续无可见承载时暂停渲染；生成仍可后台完成。context lost/渲染异常只影响该组件承载，不触发重新调用模型。

`result.submit`同时校验registry/算法/参数hash、fact/definition/scope、归属epoch、源码已删状态和模板能力。用户删除/改参数后迟到结果不可覆盖；来源新增可保原候选供diff。公式/示例值/模板升级造成行为变化须新revision和迁移检查，不能在运行时从CDN获取“最新模板”。

## 12. C08：搜索命令、事件与失效

提取拥有者E（资料）/N（笔记、两层转写）；规范化/索引/查询/可见性门禁拥有者N；焦点/查询输入/高亮拥有者F；C维护公共profile/anchor/schema/fixture。E不直接操作索引，F不以DOM搜索替代全文。任务 `search_index` 是可重建投影，无模型调用；查询取消不取消录音或其他生成。

| 接口/命令 | 调用方 → 拥有方 | 输入 → 输出 | 状态变化与失败语义 |
| --- | --- | --- | --- |
| `search.open` | F当前焦点view → N SearchCoordinator | view_id、content target、version_mode、actual_visible_tier（适用）、历史revision（仅显式） → search_id、解析后的scope/coverage/index_generation | 创建session，不跨课堂；无聚焦内容NOT_FOUND；unsupported内容说明；打开即显示当前范围 |
| `search.query` | F → N | search_id、递增query_seq、字面query、case_sensitive → accepted/task_id及search_id | kind search_query，session updating；只执行最新未运行query；空串清空命中并ready，不生成全篇“空命中” |
| `search.page` | F → N | search_id、query_seq、scope_snapshot_id、cursor（首批null） → hits、total_count、count_status=`exact / provisional`、coverage、index_generation、next_cursor | 当前generation固定分页、按文档阅读序；缺索引INDEX_NOT_READY而非0结果；版本/删除屏障变化SEARCH_INVALIDATED，不拼旧新页 |
| `search.locate` | F → N SourceResolver | search_id、hit_id、view_id、expected revision → validated anchor、nav_kind、所需页/对象、可用性 | 再验版本/归属/墓碑；已变SEARCH_INVALIDATED、已删SOURCE_DELETED，均不返回旧snippet；只导航，不调用audio.play |
| `search.refresh` | F → N | search_id、新query_seq、selected_hit_id、current heads → 新查询task/保留选中映射（可空） | current跟新head、historical固定版本但仍检查删除；保同身份有效选中，否则提示失效不抢滚动 |
| `search.close` | F → N | search_id → closed | 幂等关闭与取消此session查询；不取消全局索引任务；view保存query/范围/选中锚点便于恢复 |
| `search.projection_submit` | E通过material.page_result / N提取器 → N IndexStore | E的ExtractedSearchUnits/ExtractionCoverage或N的SearchUnits、owner revisions/source_hash → 新index_generation与SearchCoverage | 不是WebView公共写API；资料包原子接收后由N统一规范化/映射再生效，旧提取版本拒绝；错误不改权威正文 |
| `search.rebuild` | N恢复/升级调度器 → N IndexStore | scope、projection/profile版本、head snapshot → search_index task | R4、checkpoint/可重试，重建完成CAS切换generation；本地来源已删不得重新引入 |

`search.index_changed`为权威事件，含scope、index_generation、coverage、dirty_ranges；`search.results_changed`含search_id/query_seq/snapshot/generation/status/count，消费者再page，不广播全文。`search.invalidated`含受影响scope/query、visibility_epoch、reason（head_changed/removed/deleted/transferred/tier_changed/profile_changed）、must_clear。三者均用§1有序事件/重放。删除的must_clear=true，F立即清高亮/片段/选中缓存；UI断连或睡眠恢复后先同步屏障再显示缓存，不能在重连前露出已删原文。即使漏通知，page/locate门禁也拒绝。

更新期间保仍有效的选中锚点和阅读位置，实时新增以“有新结果/更新中”提示，不自动跳首项或末项。计数exact只针对该快照已完成且覆盖完整的范围；coverage非complete一律标provisional，并说明未处理页/区域。无结果显示“已索引部分0处，仍有未处理内容”。历史快照scope也需展示版本与来源状态；普通detach可显式历史查，永久删除不可查。重试原query_seq同内容幂等；相同seq不同内容拒IDEMPOTENCY_MISMATCH；更旧seq只允许取消/丢弃，不能覆盖新结果。

## 13. C09：focaus上下文生成与展示提交

保留 `catchup.request`/kind `catchup`，对用户只叫focaus，不能另外创建一个同时运行的focaus任务来兼容更名。2.0.0的输入在§4基础上扩展如下：

| 阶段/接口 | 调用方 → 拥有方 | 输入 → 输出 | 发布/失败 |
| --- | --- | --- | --- |
| `catchup.request` | F按钮/快捷键 → N TaskManager | view_id、lesson/recording/epoch、request_generation、triggered_at、UI剩余deadline、requested window/起点/估计依据、actual tier → task_id/snapshot | N校验范围，15秒从最初输入计；新请求代替同view旧请求显示，旧任务尽力取消；无录音仍明确可得范围，不推断未讲 |
| `context.retrieve`（内部） | A编排 → N SemanticIndex | 已冻结snapshot、当前问题候选/definition/binding IDs、允许课程范围/证据截止、剩余预算 → 版本化前置候选与来源/缺口 | 只读，检索本课堂及先前适用定义；新选入来源由N生成派生InputSnapshot并保parent_snapshot_id，仍受原证据截止/权限/时间预算，不事后篡改旧snapshot |
| `catchup.generate`（kind catchup的生成阶段） | N → A/qwen3.8-flash | 最终InputSnapshot、progress_at_trigger、窗口/活动/前置候选/限制 → DATA §13完整候选 | 输入已包含所需来源版本；teacher_path需证据，系统补充独立标记，不等进度下一轮或可视化生成 |
| `catchup.validate`（内部本地） | A编排 → N提交门禁＋A语义检查 | 候选/输入hash/来源/定义/覆盖与剩余budget → complete/partial/failed与原因 | N校验结构/版本/CAS、A校验衔接/必要前提/来源角色；完成信号不等于语义完整，缺必要节点或截断不是complete |
| `catchup.display_ack` | F → N结果元数据 | result_id、request_generation、body_revision、UI单调elapsed、实际显示完整标记 → 计时记录 | ack只记录显示，不改body/来源；过15秒标超时，本次不得回填通过；旧代结果只能明确查看历史 |

context.retrieve不能把用户未来转写/课后笔记带入实时快照；检索早于回看窗口但同课程有效定义需outside_window标签。检索时间和补充内容计入同一15秒预算，不能用一份15秒内历史概述和更晚的衔接补丁合称完整结果。系统解释可帮助理解但不得伪造教师实际因果路径；证据不足/未知focus/缺必要前提在coverage与limitations说明，并按不完整规则统计。

发布复用 `content.committed` / `task.changed`，payload带result_id/request_generation、result_semantics、completion_status、body_evidence_cutoff、progress_binding；partial也必须明确标记。`progress.changed`只更新指针，禁止写body_revision或body_generated_at。`catchup.display_ack`用当前UI计时；拆出时转移未结束请求的原单调计时预算，若无法可靠迁移计时不能重置15秒，只能报告计时未知且不计达标。

若触发后出现更晚转写，已生成正文仍只说明自己的依据；新指针可能较新。若来源/定义内容改变、录音转移、永久删除或请求代次已更新，宿主拒绝覆盖；资料仅新增且旧引用仍合法时可保旧快照显示真实截止。超时的完整候选可供明确查看，保deadline失败；不能以任务succeeded或新指针掩盖UI完整显示超过15秒。

# 数据模型与资料中间格式

工程契约 2.0.0｜资料格式 `focaus.material/2.0.0`｜2026-10-18。本文是逻辑模型，不规定每个实体必须单独建表；实际 SQL 迁移由总控唯一维护。

## 1. C01：身份与版本的通用规则

| 字段/类型 | 冻结语义 |
| --- | --- |
| `entity_id` / 各类 `*_id` | 宿主分配的 UUID 字符串，永久稳定；不得用标题、页码、路径、文字偏移、云 ID 或 hash 代替 |
| `revision_id` | 每次持久内容修改生成的 UUID；revision 不可变，当前指针可变，父版本由 `parent_revision_ids` 记录 |
| `entity_seq` | 宿主为实体维护的递增整数，用于排序/CAS；不能按 UUID 或 wall clock 比较新旧 |
| `content_hash` | 明确编码的内容 SHA-256；结构数据采用 canonical JSON 后 hash，资源按字节 hash；排除易变诊断/生成时间 |
| `contract_version` | IPC/任务语义版本，当前 `2.0.0` |
| `created_at` / `committed_at` | RFC3339 UTC 时间，显示按 Asia/Shanghai 或用户设置；不代替音频坐标 |
| `lifecycle` | 内容/资源的 `active / removed / deleted`；removed 为作用域关联移除，deleted 为不可取回原文的墓碑；录音和任务使用各自state |
| `extensions` | 可选命名空间对象，不承载冻结必填语义；未知扩展可保留而不能擅自解释 |

UUID 在一次创建操作的重试中复用。AI 模型可返回 `candidate_key`，不能自行创建权威 ID；引擎通过宿主事先分配的 ID 池或 `identity.reserve` 取得稳定身份。页面重排、标题改变、语言变换、整次录音转移不得换 ID。确实拆分/合并实体时生成新 ID 并保留 `derived_from_ids`，旧 ID 仍可定位其旧版本或墓碑，不猜测一对一关系。

字段表写“必须”者必须存在；允许空值与缺字段不同。除明确可扩展字段外，写端拒绝未知 enum 值；读端可只读呈现 unsupported 状态。所有数值坐标必须声明单位，禁止默认混用秒和毫秒。

## 2. 核心实体、拥有者与写入权

所有权威写入均经宿主事务 API。下表“内容生产者”不是数据库写权限。

| 实体 | 必须字段与关系 | 内容生产者 / 生命周期 |
| --- | --- | --- |
| `Course` | course_id、name、revision_id | 用户命令；删除要遍历关联影响 |
| `Lesson` | lesson_id、course_id、title、actual_date（可空）、timezone、requested_language_tier、last_recording_tier（可空）、material_scope | 用户；actual_date 未知不按文件创建日推断 |
| `ScheduleRule` | schedule_id、course_id、时区、时段、每N周、生效范围、系列/单次例外、可选提醒 | 用户；默认时段45分钟可改，不自动启停录音 |
| `RecordingSession` | recording_id、lesson_id（可空）、assignment_epoch、state、capture_language_tier、timeline_origin、durable_watermark_ms | 采集器或音频导入；同堂可多次，整次转移只改归属与 epoch |
| `AudioAsset` | audio_asset_id、recording_id、resource_id、container、tracks、original/derived、decode_status | 后端；原文件托管副本与派生资源分开 |
| `AudioSpan` | audio_span_id、recording_id、asset/track、frame_range、timeline_range、mapping_revision_id、coverage_status | 后端；缺口可无 asset，不能用补零假装有效音频 |
| `RawUtterance` | utterance_id、recording_id、audio_spans、ordinal、speaker_ref（可空）、provisional/final、text revision、remote_binding | AI ASR 候选，宿主采纳；final 原文不可被派生覆盖 |
| `SecondarySegment` | secondary_id、recording_id、raw SourceRefs、ordered spans、text、suggestions、generation_revision | AI 原话修剪＋用户 patch；按录音归属解析课堂，正文不存窗口标签 |
| `Fact` / `Note` | fact_id、typed proposition、qualifiers、source_edges；note_id、lesson_id、fact_set_revision、对象图 | AI 提案＋用户；事实包含数值/单位/否定/条件/范围/模态，不能仅是一段摘要字符串 |
| `LanguageVariant` | variant_id、note_id、tier 1..5、fact_set_revision、presentation_revision、sync_status | 同一事实集的呈现，非五套独立事实库 |
| `ContentPresentationMeta` | target_id、generation_completed_at（可空）、evidence_cutoff、requested_tier/actual_tier（适用时）、sync_status | 宿主在成功提交时更新；“上次总结于”和证据截止是UI元数据，不进入正文 |
| `Material` | material_id、source_version_id、托管原件 resource_id、文件类型、display_name | 用户导入；源文件更新新建版本，不覆盖旧字节 |
| `MaterialAttachment` | attachment_id、material_id、lesson/course scope、用途 textbook/courseware/reference、页范围、active/removed | 用户；移除课堂关联不等于永久删源 |
| `ParseVersion` | parse_version_id、material/source version、format_version、engine_build、解析配置、page/object IDs、纠正基线 | 引擎候选包，宿主验证落库；旧解析保留供旧引用 |
| `SourceEdge` / `UserPatch` | 见 §5/§6 | 宿主统一维护，不能各模块私建来源标识 |
| `ProgressState` | lesson_id、recording_id、assignment_epoch、snapshot_id、position_status、page_id/region_id（可空）、evidence_cutoff_ms（可空）、calibration_seq、decision_seq、recording_state、freshness | AI 候选；宿主依据新旧证据/校准提交；未定位与证据陈旧分别表达 |
| `CatchupResult` / `Answer` | result_id、snapshot_id、body revision、source_refs、coverage、completion_status；focaus新增字段见§13 | AI 候选；focaus正文固定，问答非自动入笔记 |
| `Arrangement` | arrangement_id、kind、最新明确值、原话 SourceRefs、历史更正、时间解析状态 | 从课堂事实派生；明确考点/强调/AI推测分别标记 |
| `Task` / `RemoteBinding` | 见 INTERFACES §2；本地 task_id 与 provider_task_id 分离 | 宿主拥有任务状态；远端只作映射 |
| `ContentView` / `EditLease` | view_id、lesson_id、target、host_window_id、view_revision；field/element locks | 宿主协调，UI呈现；不改变内容归属 |

不得只靠 `lesson_id` 存两层转写归属：录音转移时它们通过 recording_id 及 assignment_epoch 原子更新归属投影，稳定引用仍指原 entity_id。已有 Note、MaterialAttachment 不随录音搬走。

ContentView.target为判别联合：`lesson_collection`含lesson_id/content_kind；`entity`含entity_id、可选revision_id与来源selector。视图的lesson_id记录打开时上下文，entity目标还显示宿主解析得到的current_owner_lesson_id。转移后课堂集合按新归属重新投影，实体来源视图保目标和返回锚点、显示新归属，不随浏览选择重绑。

## 3. 来源快照与服务输入

`SourceRef` 为精确版本引用，必须包含 `entity_id`、`revision_id`、`content_hash`、`selector`。selector 是以下判别联合：

| `selector.kind` | 必须的定位内容 |
| --- | --- |
| `audio` | recording_id、audio_span_id、track_id、start_frame/end_frame、start_ms/end_ms、mapping_revision_id |
| `text` | block_id、element_ids 或稳定 range anchor；字符偏移只作校验/显示备用 |
| `material_region` | material_id、source_version_id、parse_version_id、page_id、region_id、bbox、coordinate_space |
| `object` | object_id、子对象/单元格/节点 ID 列表；不能只给渲染后屏幕坐标 |
| `whole` | 仅确实引用整个实体的场景；不能滥用于精确订正/证据定位 |

`InputSnapshot` 必须含：snapshot_id、parent_snapshot_id（首个快照null）、contract_version、purpose、target_id、lesson_id（可空）、recording_id（可空）、assignment_epoch（适用时）、sources[]、attachment_set_revision、fact_set_revision（适用时）、target_base_revision、edit_revision、language_tier（适用时）、evidence_cutoff（按 recording_id 的坐标集合）、available_at、executor_id/executor_version、model_id、region=`cn-beijing`、prompt_version、parameters_hash；本地任务的模型字段显式null并带executor_id/executor_version，模型任务不可为空（INTERFACES §11.1）。对进度另带 calibration_seq、decision_seq；对转换带原件与纠正集版本。

快照只纳入请求时已可得的证据；多录音不能取各录音最大毫秒数冒充同一时钟。无权威时间对齐时返回各自证据范围＋unknown。snapshot hash 是审计/幂等材料，不能让 provider URL、凭据或服务敏感响应进入 hash 日志正文。模型收到的是经白名单组装的证据内容，不是整个数据库或文件目录。

下例为**InputSnapshot字段节选**，不是完整合法请求；UUID 和 hash 用占位标签，完整机器样例由 G1 生成，不能据此断言已完成 Schema 校验：

```json
{
  "contract_version": "2.0.0",
  "snapshot_id": "<host-uuid>",
  "purpose": "note_incremental",
  "target_id": "<note-uuid>",
  "recording_id": "<recording-uuid>",
  "assignment_epoch": 3,
  "target_base_revision": "<note-revision>",
  "edit_revision": "<edit-revision>",
  "evidence_cutoff": [{"recording_id": "<recording-uuid>", "end_ms": 312000}],
  "sources": [{
    "entity_id": "<secondary-uuid>",
    "revision_id": "<secondary-revision>",
    "content_hash": "<sha256>",
    "selector": {"kind": "text", "block_id": "<block-uuid>", "element_ids": ["<element-uuid>"]}
  }],
  "region": "cn-beijing",
  "model_id": "qwen3.8-flash"
}
```

## 4. C02：原始时间轴、音轨与对齐

### 4.1 两个坐标系

- 录音相对时间：`start_ms/end_ms` 为整数毫秒，半开区间 `[start_ms,end_ms)`；从本会话 timeline_origin 起算，暂停/设备中断保留实际时间缺口。原生单调时钟锚点负责运行期时间；重启/睡眠无法可靠还原部分标 uncertain，不由服务器到达时间填补。
- 文件帧坐标：选定音轨的 `start_frame/end_frame` 为整数帧，另存 sample_rate、stream_time_base、PTS 原点。JSON 整数均须在安全整数范围内。帧→毫秒按有理比例计算，在边界统一取整，保帧级精度与误差范围；不能累计逐片浮点舍入漂移。
- 授课日历时间：可空的绝对锚点与 uncertainty；导入起点由用户校准或内容证据确定。绝不从文件创建时刻推出授课时刻。

`TimeMapping` 版本记录每段派生音频 `derived_frame_range → original_asset_id/track_id/original_frame_range → recording timeline` 的仿射比例/偏移、延迟补偿和最大误差。拼接、去静音、切片边缘、重采样须分段记录，可组合追溯；跨缺口不可用一个比例映射。ASR 的 begin_time/end_time 按该请求实际发送音频的坐标转换，不以提交时刻/下载时刻排序。

`CoverageInterval` 区分 `captured / silence_observed / capture_gap / decode_unknown`；ASR coverage 独立为 `unsubmitted / pending / provisional / final / failed / uncertain`。捕获成功不等于内容完整，静音不能自动判未讲，ASR缺口不等于缺录。恢复有重叠时保原文件重叠关系，不伪造单连续轨。

### 4.2 多文件、多课堂、多轨

导入录音保存一个原文件托管副本，独立登记各音轨，推荐先试听/解码 AAC 双声道与 apple_apac 候选的可用性，再明确选择识别轨。不能直接混合多轨或把每轨当一堂课。未选轨前任务状态 `waiting_input`，不把未验证轨自动宣布最优。

外部文件与课堂使用 `RecordingAlignment` 多对多关系：alignment_id、asset/session、原始区间、lesson_id（可空）、课堂相对位置（可空）、confidence、confirmed_by、evidence_refs。一个文件跨堂时为确定的课堂区间建立独立 RecordingSession/AudioSpan 视图，共享原件资源；未知区间保未归属。重复引用同一段不会复制音频事实。这里是导入内容对齐，不能用来把正在进行的“整次转移”改成自动切段。

`RawUtterance.remote_binding` 至少含 provider、provider_task_id、connection_epoch、provider_sentence_id（可空）、local_request_segment_id。provider_sentence_id 只在一个服务任务内有意义；同名云句号不合并本地句子。稳定句以宿主 ID＋原始 AudioSpan 定位。filetrans 只填 coverage ledger 中仍无稳定底稿的区间；边缘重叠匹配使用时间、词序和来源综合判断，低可信候选待校对，不插重复句、不覆盖稳定实时句。

## 5. C03：逐级引用、可用性与版本变化

证据图必须保持 `Note/Fact → SecondarySegment → RawUtterance → AudioSpan`。笔记也可直接引用 `Material source version/page/region`；课件证据的角色必须为 `material_supplement`，不能标为 `teacher_said`。AI 背景解释单独标 `ai_background`，不是课堂证据。

`SourceEdge` 必须含 edge_id、from entity/revision/selector、to SourceRef、role、created_by、snapshot_id、availability。每条边精确到实际支持的事实/文本/单元格，混合段落可拆出多条边。`availability=available / source_removed / source_deleted / intermediate_unavailable / unresolved` 与 `freshness=current / update_available / new_source_available` 分离。删除状态不能靠前端隐藏文字代替。

导航解析器可沿已有图跳过缺失中间层，返回 `resolved_target`、`traversed_edges`、`skipped_layers`、原因；**不改写引用图**，也不伪建“笔记直接引用原始转写”的旁路边。永久删除的中间节点可留无正文的墓碑边以导航仍可用下级；已删除的最终来源不能缓存显示原文。

focaus/问答可在二次转写尚未就绪时使用可得原始转写并直接展示其来源，不能因此阻塞15秒摘要。当问答/片段被明确加入笔记时，宿主规范化为逐级证据链；二次层未就绪则生成 `pending_evidence` 用户草稿，等真实二次层建立后关联，不能补造一段二次讲话来凑链。资料补充可直接形成笔记材料引用。

新来源出现为 `new_source_available`；已引用来源的新版本为 `update_available`。未采纳更新时打开旧版本、原位置并提供新旧 diff；源更新不自动把旧 SourceRef 指向新页。重整采纳后创建新内容 revision 与新边，旧链仍在允许保留历史内可追。

原始 ASR 从provisional追加为final版本后冻结该句正文；不得再用新revision“更新”原始定稿。校对/订正保存在独立层，不改final原文。二次转写允许客观少字/多字/错字修正，但必须保存原词、判断和来源审计；UI无需额外标记普通错字。关键数值、否定、概念替换、条件、公式/关系改变必须是独立 `CorrectionSuggestion`，不可自动应用。教师明确自纠要保错误表达和自纠两部分。

## 6. 编辑、订正与删除意图

`UserPatch`：patch_id、target_id、base_revision、operation、selector、before_hash、after_value、author=`user`、intent、undo_group_id、created_at。operation 包括文本 span 插删改、单元格修改、节点/边修改、移动/缩放、复制、删除；文本由稳定 element ID＋左右锚点定位，offset 不能作为唯一身份。宿主在 editor adapter 和 canonical object graph 间转换，拒绝无法精确定位的 patch。

用户修改范围标 `user_owned`；未改区域可继续生成。`DeleteIntent` 记录被删 fact/object/element IDs、语义范围、基线来源版本和用户动作，不仅保存“这个位置空了”。后续同事实生成不自动补回；新实质来源只给恢复建议或手动重整 diff。删除意图可随撤销复原，不能被模型自行清空。

`EditLease` 按 view_id、target、selectors、lease_epoch、composition_active、草稿 revision 保存。活动输入和 IME 组合区域禁止 AI 提交；过期 lease 不授权覆盖未提交草稿，必须先恢复/隔离草稿。其他窗口编辑同一范围要显式冲突，不能 last-write-wins。

`Proposal` 必须含 proposal_id、snapshot_id、base_revision、operations、source_edges、touched_selectors、protected_intersections。宿主判定：未受影响可自动合并；与用户范围相交进 review；目标/来源版本失效进 stale；重复 proposal 幂等返回。结构重排也要验证子节点保护，不能借移动父块绕过文本锁。

`CorrectionSuggestion`：suggestion_id、secondary_id、source_revision、range anchor、original_text、suggested_text、reason、semantic_risk、state。state=`pending / accepted / rejected / obsolete`；打开浮层草稿不是 accepted；Esc仅撤本次草稿，明确保留原句才 rejected。采纳后保存用户最终文字与 provenance，后续整理沿用。

批量采纳是一组独立 suggestion 的原子事务，记录适用数量/跳过 rejected 与 stale 数量和 undo_group_id。Cmd+Z 撤整批；若之后已有用户编辑，撤销也先检测 revision 冲突，不能逆操作覆盖新编辑。AI 判断普通/关键订正，用户仅选择结果；不让用户承担风险分类任务。

## 7. 五档、定义与局部变量

### 7.1 共同事实与呈现

Fact 集是五档唯一事实基础：source edges、数值单位、肯否定、条件/约束、动作和关键修饰均固定。LanguageVariant 仅调整表达比例/术语括注，保 fact_id、definition_id、object_id。中文主导至接近课堂中英混用共五档；每个正文同时只展示一个档。不能每档独立总结后用字符串相似度冒充事实一致。

`sync_status=ready / generating / stale / failed`，UI区分 requested tier 与 actual visible tier。切档无内容预览，但未生成/不同步时保当前档并显示实际档。事实改动使其他档 stale；用户改动若仅表达留在该呈现的 patch；涉及事实时生成共同事实修改，不能静默“翻译回写”其他用户文本，冲突档待确认。术语定义导航始终解析到目标笔记当前可见档。

新课堂默认3，全局默认只影响新建课堂。`RecordingSession.capture_language_tier` 在 recording/paused 均锁定；停止后可切显示档，后台尚在收尾不扩大锁定。再次录制同课堂恢复 `last_recording_tier`，不会把停止后临时浏览档误用为录音档。整次转移时 session 的录音档不变，目标课堂录音期间锁此档；已有笔记不强行即时改档，按 requested/actual 与待同步机制显示。原课堂若无活动录音则解锁。此为满足锁档与不覆盖的工程合并政策，需 V08 联调。

### 7.2 概念、定义、变量

`Concept` 表示概念身份，`Definition` 表示具体解释：definition_id、concept_id、lesson_id、scope_id、valid_from_anchor、valid_to_anchor（可空）、source_refs、definition_fact_ids。Alias 绑定 definition/concept＋语言＋适用作用域，不以字符串相同强行并同名定义。

解析顺序：本课堂当前作用域→本课堂先前仍有效定义→同课程先前仍适用定义；跨课程仅建议，需用户明确采纳。链接数量按必要专业词/难点/变量控制，不对每个词自动连线。

`SymbolBinding`：binding_id、symbol、scope_id（推导/段落）、definition_id、valid_from_anchor、valid_to_anchor、meaning、expression、constraints、value（可空）。重定义创建新 binding，向后生效，不能改写前面公式。有效顺序基于推导作用域的稳定语义锚点及来源顺序，不以当前页面像素位置或拖排后的屏幕位置重新推断。没有值就不填精确值；来源记号与课堂记号通过 explicit mapping 关联，不等价记录差异。外部引文原样保留。

## 8. C05：资料引擎中间格式

### 8.1 包结构与完整性

资料包由 `manifest.json` 的概念结构＋资源集合构成（本轮不创建 JSON 文件）。必须字段：format=`focaus.material`、format_version=`2.0.0`、material_id、source_version_id、parse_version_id、source_hash、engine_build、parse_profile（light/deep）、requested_pages、pages、objects、resources、corrections_applied、unresolved_conflicts、provenance、completion、search_units、search_coverage。文中的 `focaus.material/2.0.0` 是这两个字段的合称。

包内路径必须相对资源根、不越界、不含私有地址。资源描述：resource_id、media_type、byte_length、sha256、relative_path、role（original_page/image/crop/preview等）、owner_refs。宿主验证文件存在、尺寸/hash/类型、引用闭包、无重复 ID、版本兼容后原子接收；单页失败允许包 `partial`，不等于整份完成。

页：page_id、revision_id、source_page_index（0起）、display_label、width/height、unit=`pt`、rotation、source_transform、preview_resource_id、reading_order、region_ids、status、errors。坐标归一化为页旋转后的可视空间，左上原点、x右y下；region bbox 用 `[x,y,width,height]` 的0..1比值，关键形状可另带 polygon。pt 用于布局重建，归一化 bbox 用于引用定位，须保存到原 PDF/PPTX 坐标的 transform，不能混淆。

区域：region_id、page_id、bbox、object_ids、semantic_role、parent_region_id（可空）。对象关系采用 contains / reading_next / labels / connects / overlays 等显式边，不仅一张平面截图。PPTX native shape ID、PDF 对象位置是匹配线索，不能直接成为全局 ID。

对象公共字段必须有 object_id、revision_id、type、source_refs、region_ids、layout、content、extraction、editability、status、children。`layout` 保 relative bounds、z-order、group、constraints（邻接/对齐/标注等）；不承诺像素级字体复刻。`extraction` 为 native/text/OCR/model/mixed，另存质量证据、模型/引擎配置、estimated 标记，不把任意置信分数当正确率。

manifest.completion=`partial / complete / failed`；页status=`basic_ready / partial / complete / failed / unsupported`；对象status=`complete / partial / unresolved / unsupported`，editability=`native / image_only / read_only`。complete按所请求profile/range判定：light完成仅表示基础任务完成；deep中应恢复的表/图只有图片回退时必须partial/unresolved，不能以image_only宣称重构完成。纠正replay_status=`applied / conflict / unmatched`，冲突不丢记录。

### 8.2 原生对象判别联合

| type | 必须内容 | 未完成的显式状态 |
| --- | --- | --- |
| `text` | 稳定 block/element IDs、文本 runs、层次与阅读序 | OCR不清保 source crop，uncertain spans |
| `formula` | 数学表达、display mode、符号 bindings、原区域 | 无法还原为 `unresolved`＋原图，不能伪造 LaTeX |
| `image` | resource_id、alt/说明、裁剪、原始尺寸 | 图像本身可作为完整图片对象；不能把应重构图表称完成 |
| `table` | 稳定 row/column/cell IDs、二维 grid、rowspan/colspan、值/单位/公式/注释 | 缺单元格值=null＋unknown，保原区域 |
| `data_chart` | chart kind、axes/scales/units、series/point IDs、x/y 值或区间、legend、labels | 每点 value_origin=`explicit / estimated / unknown`，estimated带误差/估读依据，unknown不补造 |
| `structure_diagram` | node IDs、标签、edge IDs、端点/方向/类型、分组与局部位置 | 不确定连线/箭头显式 unresolved，不用近邻猜成确定依赖 |
| `group` | children、布局/对齐约束、组内坐标、reading_order | 可部分完成，每个子对象有状态 |
| `interactive_visualization` | §11的组件描述/参数/计算定义/来源/说明、受控模板键 | 未支持模板为unsupported，校验失败为unresolved；静态预览不代表交互完成 |

数据图坐标必须区分数值/分类/日期、线性/对数，点坐标与像素坐标分别存储。表格必须真实单元格，结构图必须节点连线，公式必须可编辑数学结构或明确未完成。原图始终可对照；截图仅为回退证据，不计入原生重构通过数。

2.0.0新增search_units/search_coverage（见§12），即使没有可提取文字也必须显式为空并声明哪些页/对象未处理。原页文字与重构对象文字同源映射去重。交互对象进入共享原生对象联合，但普通PDF/PPTX不要求引擎凭空生成教学组件：E产出结构化事实与搜索锚点，A的课堂教学演绎在笔记生成链另建对象；如引擎无法可靠恢复受支持交互语义，则保原件并标unsupported，不凭截图推断动画算法。

### 8.3 解析、纠正和笔记副本

原件版本不可变；解析版本是对原件的一次解释；`MaterialCorrection` 为独立用户层：correction_id、source/parse base、object/field selector、before_hash、replacement、scope、replay_status。重解析先生成新候选，再按稳定身份/原区域/语义映射尝试重放纠正；明确匹配才 apply，歧义留 conflict 并对照，不覆盖用户纠正。

页级发布也遵守不可变性：同一转换task可先后产出多个parse_version_id，每次已发布manifest都是不可变快照，可用parent_parse_version_id关联；未变页复用其page_id/revision和资源。基础文字升级为深对象或partial变完整时发布新解析版本，不能原地修改旧manifest的内容/completion。活动转换进度在Task中更新，不写回已经被引用的ParseVersion。

对象匹配由引擎提出、宿主确认身份映射；不允许在重排/拆分后静默把旧 ID 复用给不同知识。旧解析引用仍指旧包。`lineage` 保存 split/merge/supersede 映射，不能只保最新页号。

加入笔记生成新的 note object IDs 与可编辑副本，保存 `copied_from` 精确 SourceRef、copy_group_id、user_owned patches 和独立资源所有权。引擎对象与笔记共享结构能力，不共享可变内容实例。源更新仅产生副本更新提案；用户独立修改保留。单对象/组插入须复制关系闭包，外部依赖以引用保留，不遗漏图例/表头/公式约束。

教材默认 light（文字/必要OCR），课件默认 deep；用途推荐可改，支持选页/章节。先暴露原页和基础文字，深处理按页提交和重试。Word先转PDF作为显式外部准备步骤；图片/TXT/Markdown经同一来源/对象契约导入。内嵌音视频及外链内容标 unsupported，不抓取；PPT转场/逐步显现不建播放模型。

## 9. 移除、删除、迁移与恢复

从课堂移除只把 MaterialAttachment 改 removed、增加 attachment_set_revision，未来快照过滤；历史源/旧引用仍可访问。下一次整理按边分析：纯失依据 AI 内容移除；混合块只去对应 AI 子片段；用户文本保留并标失效引用。不能直接级联 DELETE note。

永久删除先由宿主生成带 `impact_revision` 的预览：源/版本/缓存、直接引用数、间接受影响笔记、手动副本、独立用户修改、共享资源所有者。确认事务再次比对 revision；变化则返回 conflict 要重看预览。默认保手动副本，明确列出“仍保留的内容”；可预览同时删除未独立修改部分，独立用户内容继续保护。保留副本必须转为独立资源所有者，删源不能释放它仍需要的资源。

删除处理清除允许删除的原件、版本正文、解析缓存、搜索索引、快照中的内容副本、日志中的正文缓存、暂存/生成候选与受影响任务；保留最少无正文墓碑 ID/关系以解释“来源已删除”。受影响的在途结果拒绝发布并清理。资源按所有者引用计数回收，不把 content_hash 相同当相同生命周期。普通文件删除不宣称 SSD 法证级抹除；外部原件/用户保存的旧备份不自动删除。

数据库迁移由总控提供顺序号、前/后版本、前置校验、备份及回退方案；后端事务执行。破坏性迁移先导出可恢复包，不在 UI 打开前半迁移状态中接写入。未知未来 major 只读或拒绝，不尝试猜字段。

完整课堂包：包含清单、schema/format版本、课程课堂、原生对象、允许保留 revisions、逐级引用闭包、原声/必要资源、纠正/用户patch/删除意图/时间映射；不含凭据、上传URL、浏览器数据。跨课堂边须清点依赖：允许携带的最小源闭包明确列出，其余导为 external-unresolved stub，不能静默丢引用或把整门课暗含进备份。

恢复先暂存校验所有 hash/ID/边闭包再提交：相同ID且相同hash可复用；同ID内容冲突需导入新命名空间并用显式映射重写包内全部引用，或用户确认合并，不能覆盖本地较新内容。活库删除墓碑优先于旧包，恢复已删除源需明确预览/确认；不因导包自动复活。全新库恢复旧备份将恢复当时包含的数据，界面明示其备份时间与删除状态。PDF为阅读导出，Markdown为简化导出，只有课堂包承担完整编辑/关系恢复。交互组件纳入协议/模板版本、参数、来源、用户patch/删除意图、资源、已提交的步骤和必要视角；计算缓存可重建，索引只保重建所需源数据与版本，不以缓存为备份真值。恢复先协商模板能力/验证资源与数学约束，未知模板保数据和明确unsupported，不执行外部代码。PDF/Markdown导出当前已验证状态的静态图或表格、公式/参数/近似或示例标签、来源及“交互已静态化”说明；没有可用渲染时导出文字/数据表及失败说明，不能称可恢复交互。

## 10. 冻结与后续扩展清单

先冻结C01–C06：UUID身份、revision/CAS、时间单位与映射、snapshot字段、逐级边、删除墓碑、用户保护/undo、语言共同事实、对象核心类型与坐标、任务/窗口状态、原子提交边界。总控在 G1 必须为每项准备正例/冲突例。新增C07–C09的冻结项见下文，旧编号不重排。

允许后续扩展：布局提示、更多图表子类型、抽取诊断、非关键统计、性能指标和编辑器私有呈现字段。新增类型仍走版本协商；影响来源、恢复或用户保护的字段不能以“实现细节”由单路绕过总控。


## 11. C07：交互组件、注册表与持久状态

### 11.1 身份与字段

组件是Note对象图中的 `type=interactive_visualization`，同一object_id跨语言/布局/来源更新保持；不另造一套脱离C01的“widget事实库”。可以从材料对象/Fact生成，保存准确SourceEdge。副本新建object_id并保derived_from_ids/来源，不共享用户参数可写状态。资料包使用同一描述结构，包的format_version与组件protocol_version独立。

| 必须字段/结构 | 语义与冻结边界 |
| --- | --- |
| `object_id, revision_id, parent_revision_ids` | C01身份/内容版本；revision同时覆盖已提交参数与交互状态。无独立可漂移content_version整数 |
| `protocol, protocol_version` | `focaus.visualization` / `1.0.0`；未知major拒执行 |
| `template_id, template_version, registry_revision` | C维护的受控模板键；如 `function_curve_2d`、`conv2d_spatial`，后者是二维算子三维呈现 |
| `concept_fact_ids, definition_ids, symbol_binding_ids, scope_id` | 课堂含义/局部变量关联；重定义不能反改已存公式 |
| `semantic_key, purpose, placement_anchor` | 宿主按知识身份/作用域/教学目的/模板族规范化计算去重键，插入锚点稳定；键不含语言档/显示文字/本轮时间/来源revision/参数滑动值 |
| `parameters, parameter_schema_version` | 结构化类型化参数与单位/合法区间、轴顺序、维度；不允许任意表达式代码 |
| `value_bindings[]` | 每个参数/数据项有binding_id、value_origin=`classroom / derived / demonstration`、值/单位、SourceRefs或推导依赖、formula_id/作用域；未知值显式null＋unknown原因 |
| `computation` | algorithm_id/version、operator_semantics、封闭公式AST/标识、input/output shape、数值精度与容差策略ID、依赖binding IDs；模型预报结果不是计算真值 |
| `resources[]` | resource_id/hash/用途/所有权，不接受任意网络路径；只读素材与手动副本分别所有 |
| `source_refs, snapshot_id, fact_set_revision` | 精确证据/生成输入，笔记对象遵守C03逐级来源；来源不可用状态单独保存 |
| `interaction_state` | 选中输入/输出单元ID、step_index、必要camera（位置/目标/缩放）、user_parameter_overrides；持久的参数值以parameters为准，overrides记录用户意图而非第二份值 |
| `presentation_by_tier` | 1..5标签/说明/可搜索描述与实际ready/stale状态；共用事实、公式、参数、步骤和object_id，不每档重新采样数据 |
| `generation_status, validation, fallback` | 状态/失败见下；fallback含可选preview_resource_id、等价说明与数据表，非隐藏失败 |

`generation_status=proposed / validating / ready / needs_evidence / failed / stale / unsupported`。它不是Task.state，也不是C05对象status。只有math与semantic及schema/source检查均pass才ready；缺证据needs_evidence，来源新版本stale并保旧已验证版本可读（永久删除按墓碑）。`validation`含checks（schema/source/semantic/math，各为pending/pass/fail/unknown）、validator_version、parameters_hash、validated_revision、diagnostics；检查记录针对被验证内容hash，不将“模型说正确”当math pass。验证过程中的候选不覆盖现有ready对象。

generation/validation是内容候选或新revision的状态，后台任务进度在Task中，不原地修改已引用revision；现有ready对象的待更新标记放ContentPresentationMeta。运行状态按 `(view_id,object_id)` 保存为临时 `unmounted / ready / playing / suspended / context_lost / failed`；不能把一个视图的GPU失败写成其他视图内容失败。播放时钟不进内容revision，不逐帧落库；离开交互/暂停/移交时提交必要step/camera，重开均暂停。共享参数是内容编辑，经EditLease/CAS；普通相机拖动为view私有状态，用户保存默认视角时才写对象interaction_state。view.save_state保存该窗相机/搜索状态，备份同时包含对象默认及允许的view状态，恢复可选择采用。

### 11.2 增量、用户保护与语言

semantic_key由宿主和C的规范规则产生，A的候选key只作匹配线索。相同Key重复发现返回已有对象或no-op；来源升级生成同object_id的新proposal，不再插一个。概念拆合遵守derived_from_ids规则。用户主动复制新建ID与manual_copy标识，自动发现不把副本当需要删除的重复项。

参数修改/移动/尺寸/子标签由UserPatch精确记录；AI与受保护字段相交转review，重整只重排允许范围。DeleteIntent包含object_id与semantic_key/知识范围，重复识别、不同语言或模板小版本不能绕过删除；撤销恢复原ID/参数/来源。实质新知识需明确恢复提案，不静默补回。示例数据不会变成Fact或teacher_said；课堂数值被用户调成演示值后，该字段标demonstration/user_override并保原证据可对照。

### 11.3 卷积3D教学样例与可计算规范

首个3D样例推荐 `conv2d_spatial`：用不同高度/空间分离输入矩阵、卷积核、当前滑动窗口和输出平面，**算子维度仍为2D，场景维度为3D**。`operator_dimension=2, scene_dimension=3`必须分别存储。真正三维卷积需要D/H/W三个轴和三个kernel/stride/padding，另用模板及算法版本，不能把镜头旋转或多输入通道称3D卷积。

课堂定义先决定 `operator_semantics=cross_correlation / convolution`；前者直接点乘滑窗与K，后者在空间轴翻转K。多通道的通道求和、N/C/H/W轴序与bias是否包含都显式声明；定义缺失时不默选并标老师采用。示例可明确选择一种演示定义并与课堂未知状态区分。

对于每个空间轴，输出尺寸 `O=floor((I+P_before+P_after-D*(K-1)-1)/S)+1`，I/K/S/D为正整数、padding为非负整数；O必须正，张量真实shape与声明一致，元素有限、单位/类型一致；不得生成NaN、越界或以丢单元格掩盖维度错误。dilation首个模板可仅支持1，但字段必须显式，不支持其他值返回UNSUPPORTED_CAPABILITY。kernel size改变需用户明确选择新核值或标演示填充，不能保原课堂标签却自动生成核。

教学fixture（纯自制示例，非课堂事实）：I为3×3矩阵 `[1,2,3;4,5,6;7,8,9]`，K为2×2矩阵 `[1,0;0,-1]`，stride=1、padding=0、dilation=1、单通道、无bias。cross-correlation输出2×2且每格−4；convolution翻核后每格+4。stride=2时形状1×1；四侧padding=1、stride=1时3×3输入输出4×4。此为待实现验证的数学预期，本轮没有运行算法测试；V19须独立手算/参考计算复核而非两遍调用同一实现。

单步按稳定输出坐标/ID选择窗口：坐标 `(oh,ow)` 的输入起点为 `(oh*S_h-P_top,ow*S_w-P_left)`，越界使用明确padding值；局部计算逐项展示输入、对应核值、乘积、求和及bias。播放/暂停只推进step_index，不改变数据；调参重算后保仍合法的选中格，不合法则回到首个有效格并解释。旋转/缩放/重置只改视角，不改数学结果。二维替代视图显示同一矩阵、步骤和局部算式；无需识别颜色也能建立输入→窗口→输出对应。

## 12. C08：搜索文本、锚点、版本与覆盖

### 12.1 派生搜索结构

C05包的 `search_units` 是 **ExtractedSearchUnit**，不是已建好的宿主索引行。E只交：unit_key（包内匹配线索）、owner entity/revision、display_text、field_path、原文SearchAnchor、equivalence_group_id/双视图锚点、source_role、提取版本与hash。`search_coverage` 是 **ExtractionCoverage**：requested/processed/unprocessed/failed页及区域、原因、state=complete/partial/unavailable。两者为必填，空数组不代表处理完成。宿主接收包后校验源ID与位置，分配/复用search_unit_id，统一规范化/建offset_map，再补索引水位形成以下SearchUnit/SearchCoverage；E不得自行决定index_generation、visibility_epoch或查询命中数。原文字符到页区域/对象文字的映射由E提供，规范化字符到原文的映射由N负责。

| 结构 | 必须字段及职责 |
| --- | --- |
| `SearchUnit` | search_unit_id（宿主稳定投影ID）、owner_entity_id/revision_id、content_hash、kind=`material / note / raw / secondary`、visible_tier（非语言内容null）、field_path、display_text、normalized_text、normalization_profile、offset_map、anchor、equivalence_group_id（可空）、source_role；仅派生投影，非新的SourceRef身份 |
| `SearchAnchor` | owner_entity_id/revision_id、selector（沿C01）、field_path、原文半开scalar范围；课件另带page/region/object及bbox，转写带utterance/secondary ID和原始audio spans/time，图形标签含object/label ID与定位fallback |
| `SearchCoverage` | scope snapshot、required_units/pages、indexed_units/pages、unprocessed/failed/stale区间与原因、state=`complete / partial / indexing / unavailable`、source_extraction_revision；complete只说明已知可搜文字覆盖，不证明OCR识别准确 |
| `SearchIndexState` | projection_version=`1`、index_generation（递增整数）、scope/head manifest hash、dirty_ranges、coverage、visibility_epoch、last_committed_seq；N唯一维护，可由内容重新构建 |
| `SearchSession` | search_id、view_id、query_seq、query/options、scope、scope_snapshot_id、index_generation、visibility_epoch、selected_hit_id、cursor、status=`ready / updating / stale / closed`；可恢复的UI视图状态不等于索引真值 |
| `SearchHit` | hit_id、search_unit_id/revision、matched_anchor/ranges、display_snippet、equivalence_group_id、nav_kind=`text / page_region / object_focus`；hit_id按同一版本/规范化命中范围稳定产生，不作为跨revision来源身份 |

`scope`为当前view解析的单一内容范围：material及其parse revision/页范围；note及actual_visible_tier/presentation revision；raw或secondary的recording/lesson集合。`version_mode=current / historical`；historical必须指定不可变revision清单并显示历史标签，current只接受当前有效head和归属/attachment集合。不同语言、原始与二次两种范围不混为一组结果；跨课堂/跨课程UI搜索不在本次范围。

提取规则：E提取课件文本层、OCR及重构对象文字，建立同源 `equivalence_group_id` 与原页/对象双导航锚点。确证同一来源内容只计一次，以可编辑对象文字为优先、原页为备用；不能单凭同字符串合并不同位置的重复出现。无法建立对应者标覆盖/映射诊断，不默称去重完成。N从已提交笔记/转写模型提取，不读当前DOM：标题/正文、表格cell、图表labels/series名、结构图nodes、可视化说明/标签/显式参数文本、公式规范文本均有field_path。公式保可读数学文本与规范LaTeX索引别名，同一个位置命中别名不重复计数；不承诺符号代数等价检索。图形命中定位对象/标签及说明栏，不能伪造Canvas像素中文字范围。

### 12.2 规范化与精确映射

工程默认基础查找是字面子串，大小写不敏感、无正则/模糊/语义扩展；提供大小写敏感选项。规范化profile `nfc_casefold_ws_v1`：按Unicode规范组合NFC，非case-sensitive时全casefold后再NFC，将连续空白规范化为单空格；不去变音、不自动简繁/全半角同义化。固定Unicode数据版本于G1公共样例，升级profile须重建索引。中文不依赖分词，跨runs同一正文逻辑单元可匹配，不能跨无关单元格/句段拼出假命中。

每个规范化scalar映射回原文scalar区间与稳定element IDs；casefold扩展/组合字符/空白折叠可多对一或一对多，高亮覆盖完整原始字素而不拆代理对/组合字。F通过对应revision文本映射UTF-16 DOM offset，不能直接把UTF-8字节或规范化offset当DOM位置。Query也用同profile，偏移只是定位辅助，跨revision不复用裸偏移；文档变化重新验证selector/before_hash，否则命中失效。

### 12.3 更新、移除与删除

append/raw provisional→final/secondary/重整/用户patch/资料纠正重解析/语言变化均脏标精准unit并发布索引事件。原始final不修改，仅新句追加/投影更新；新的parse revision保旧搜索投影供显式历史，current切新head。actual_visible_tier尚未切换时继续搜实际档，切换提交后重开scope，不混计未就绪档。

查询固定scope_snapshot/index_generation和query_seq，分页不混版本。实时追加标有新结果，选中命中/滚动不跳；刷新保持仍存活的selected_hit_id，旧命中消失先提示不自动跳到下一行。当前head变化但新索引未完成时旧命中不标当前：重新匹配合法部分，未完成区显示partial；历史搜索可保旧head，永久删除例外。

移除attachment/录音归属转移即时提升visibility_epoch并过滤当前scope；显式历史仍按保留规则可查。永久删除同事务建立tombstone/失效屏障，禁止query/page/locate返回旧原文，主动撤回跨窗口缓存命中/片段并清持久索引；后台清理失败仍不能穿透屏障。保留的手动副本独立所有，其自己的正文可查，但不得借副本缓存显示已删来源全文。查询原始/二次命中只导航文本/时间，回放仍受PLAYBACK_BLOCKED约束。

## 13. C09：focaus结果与证据时间

内部 `CatchupResult` / kind `catchup` 保留，UI标签统一 `focaus`。新结果 `result_semantics=focaus_context_v2`，旧1.x适配为 `legacy_summary` 且无新验收资格，不凭历史摘要补造字段。

| 字段 | 2.0.0语义 |
| --- | --- |
| `result_id, request_generation, snapshot_id, body_revision_id` | 宿主ID、同view递增触发代次及不可变正文；迟到旧代只可明确查看历史 |
| `requested_window, effective_window, estimate_basis` | 起止原始音频锚点/用户起点、max(估计漏听,5分钟)默认及估计不确定性；范围变化必须说明 |
| `current_focus` | topic、problem、activity=`derivation / example / transition / explanation / unknown`、step（可空）、source_refs、status=`supported / uncertain / unknown`；位置陈旧时不得写“现在确定在讲” |
| `path_nodes[], path_edges[]` | node_id（结果内稳定）、role=`missed_key / bridge / current / prerequisite`、fact/definition/binding IDs、body_block_ids、evidence_role=`teacher_path / system_explanation / material_supplement / unknown`、SourceRefs；edge声明时序/依赖/主题衔接、证据与确定性，不能仅凭邻接推因果 |
| `prerequisites[]` | definition/binding/fact身份、为何当前需要、source_refs、scope_id、outside_window；可空但必须说明none_needed/insufficient_evidence，不假设用户已懂 |
| `body, presentation_tier` | 简短连贯正文，块与上述节点对应，不要求固定四栏；事实/公式/参数遵循共同Fact，不用语言变换新增知识 |
| `body_evidence_cutoff, body_generated_at` | 正文各recording的证据截止集合与生成时间，生成后固定；无录音证据显式null/unknown |
| `progress_at_trigger, progress_binding` | 输入时ProgressState完整版本/截止/新鲜度快照（尚无进度则null＋原因）；动态绑定仅含lesson/recording/assignment_epoch，无活动录音时recording_id/epoch可空并显式无当前指针，不把后续进度序号写回正文 |
| `coverage, limitations, completion_status` | coverage按音频缺录/识别缺口/资料补充及范围；completion_status=`complete / partial / failed`，缺必要路径/前置、截断或未过校验为partial/failed，不计完整成功 |
| `triggered_at, completed_at, displayed_complete_elapsed_ms` | 墙钟审计与UI单调时钟的完整显示耗时；未完整显示elapsed为null；超过15秒保超时事实 |

系统补充的解释可引用课件/定义，但不能变为老师原话；teacher_path必须有课堂转写依据，其他来源遵循C03。focaus可直接引用此刻可得raw，后续选入笔记仍需逐级证据或pending_evidence。跨旧课堂前置按§7.2规则带明确来源，不使ProgressState扩大输入种类。progress_binding最新指针有自己的evidence_cutoff_ms/freshness；UI同时显示正文依据和指针依据，无法对齐录音坐标时分别声明unknown。

C07–C09在G1必须冻结：组件键/协议/版本、来源角色与值来源、确定性算法语义/数值校验、参数patch/恢复、SearchAnchor/映射/profile/coverage/可见性屏障、搜索scope与query_seq、focaus路径角色/双截止/完整语义。后续可扩展模板、布局、诊断、加速实现、生成策略，但需capability与版本协商；不得以extensions绕过这些必填语义。

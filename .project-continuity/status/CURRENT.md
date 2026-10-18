# Current State

## Active Development
- 2026-10-18 D167–D171：后续开发获授权；G0初始3312e4e、G1共同d9a8cdf6已提交。四个独立Codex App会话和worktree已建立，均请求gpt-6.1-sol/max且工具接受，运行配置字段未独立读回。详见docs/DEVELOPMENT.md。
- 当前codex/focaus-integration代码整合至0ced003，已整合F的真实录音事件/CAS、Unicode/数学、coverage包装、ContentView和content.get投影，N的资料原件托管/原子搜索投影及R0原声登记隔离，A runtime以及E sidecar；未执行push/rebase/发布。
- D170授权本地merge/cherry-pick和契约同步，不含rebase/push/发布/删除分支或worktree；四路任务与报告消息已获授权。
- N首批实现SQLite单写/事务outbox/CAS/幂等、CPAL独立采集线程、分段封口恢复、任务/凭据绑定/数学；第二轮真实设备采集和恢复已通过关键本地门。搜索/窗口协调与投影已实现，材料接收已进入宿主纵向链。
- F实现HeroUI工作区/编辑/搜索/组件/focaus样例和adapter；native隔离、bootstrap、录音事件分流、ContentView列表及content.get投影已整合；投影保留partial/unsupported并按revision丢弃旧响应。构建通过不替代真实内容验收。
- A实现双ASR/进度/focaus/五档/候选门禁和transport；官方形状协议及本地回放通过，真实云运行未验证。
- E独立Python JSONL sidecar、PDF/PPTX/文本/图片、断点/资源/对象/双锚点/覆盖与纠正；复杂图表/OCR仍有partial/unsupported。

## Verified Evidence
- G1 C01–C09单源Schema/TS/Rust、schema1迁移、旧1.x适配与未知拒写/降级；45正反例中37结构、8语义，12运行场景说明不当测试通过。
- C native_flow使用真实NativeHost/SQLite及wire：双窗口共享状态、初始ACK前无事件、幂等课程、content.get、伪造window拒绝、重启恢复、不自动开麦通过。
- cargo test --workspace --locked通过（A29、契约6、wire2/native_flow2）；native-shell cargo check通过；Python13契约/SQLite/子进程、TS37例/RFC8785、generator --check通过。E8项含真实子进程取消/重启续传通过。N后来找到rustfmt driver并执行模块格式化。
- E真实PDF本地抽取：L08 82页，light80 basic_ready/2 partial；deep67 complete/15 partial（63 text/4 table/13 formula/2 image），92 search units。L09 86页，light84 basic_ready/2 partial；deep76 complete/10 partial（76 text/8 formula/2 image），84 search units。两份搜索覆盖partial，公式为候选证据；manifest共享校验通过。结果仅.local，无云。
- qta原件hash/轨道元数据见evaluation/assets.json；AAC两轨分别完整解码至null 1275371667us/6140310000us；APAC工具缺decoder，见evaluation/asset-decode.json。没有混轨/试听/选轨/课件绑定。
- 工程七文档为只读设计基线；其“非Git/只写文档/未实现”是已结束阶段状态，本记录给出有证据的后续变化。产品目标/固定门槛不随代码更改。

- 真实macOS WKWebView：空库bootstrap、课程/课堂创建、真实ContentView注册、standalone窗口bootstrap、六格view注册、Command+F空索引状态、正常退出/重启恢复与不自动开麦通过。
- D173第二轮默认麦克风采集：ACK持续前进，UI水位增长，停止后state=stopped；SQLite水位46730ms、47个WAV、数据库完整性通过。第一轮coverage误解析缺陷已通过第二轮交互复验关闭；没有云/上传/转写。
- 整合后验证：cargo workspace 42项相关测试、native harness19项、资料10项、TS契约37项、桌面Vitest7项及构建均通过；warning仅为未使用的内部require_receipt方法。
- 最新整合后验证：cargo workspace 42项、native harness28项、资料引擎10项、TS契约37项、桌面Vitest9项、typecheck/build均通过；N R0 的真实生产 write_loop/SQLite持锁/journal恢复测试包含在28项内。
- 最终签名App复验：同一库的真实课程窗显示3个宿主ContentView和CONTENT.GET PROJECTION；因无已提交正文，面板显示`empty`，未填充fixture。最终App启动、恢复、签名和workspace_hygiene均通过。

## Pending / Unverified
- G2仍未完成关窗继续录音、长录音恢复、真实资料导入UI、有资料后的真实Command+F命中以及编辑/引用/对象的端到端验收；content.get投影代码已接线，但不能由模块测试或空索引关闭完整内容验收。
- G3–G5待真实北京四模型、≤15秒完整focaus/约10秒进度、三层内容/保护/引用/对象/Command+F、真实课程1倍速与课后全量/课堂包。fixture、模块commit、构建不能关闭这些验收。
- 用户提供真实课件目录/Users/pitr/Desktop/SKD/DeepLearning，L08/L09原始PDF清单在evaluation/courseware-assets.json；未根据文件编号绑定qta，不把旧转写作为真值。
- 云调用/上传/费用未获本阶段具体授权；密钥未读取，后续只宿主安全存储解析四槽位。公开endpoint/服务可用性与实际配置需分别确认。
- development-guide及原文复制、原课件/录音/派生大文件/凭据/本机状态不入Git；真实L08/L09只读派生仍在.local；grilling完成不重开。

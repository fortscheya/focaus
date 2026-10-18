# Current State

## Active Development
- 2026-10-18 D167–D171：后续开发获授权；G0初始3312e4e、G1共同d9a8cdf6已提交。四个独立Codex App会话和worktree已建立，均请求gpt-6.1-sol/max且工具接受，运行配置字段未独立读回。详见docs/DEVELOPMENT.md。
- 当前codex/focaus-integration已整合N c34fcef、C shell/wire 4e8452d、F 6db6188、C native_flow 149b116、A ee5f50b/锁5b30910、E fde9518。
- D170授权本地merge/cherry-pick和契约同步，不含rebase/push/发布/删除分支或worktree；四路任务与报告消息已获授权。
- N首批实现SQLite单写/事务outbox/CAS/幂等、CPAL独立采集线程、分段封口恢复、任务/凭据绑定/数学；真实设备未启动。搜索/窗口后续在途。
- F实现HeroUI工作区/编辑/搜索/组件/focaus样例和adapter；native无fixture污染修复在途。构建通过不是macOS交互验收。
- A实现双ASR/进度/focaus/五档/候选门禁和transport；真实协议与云运行未验证。
- E独立Python JSONL sidecar、PDF/PPTX/文本/图片、断点/资源/对象/双锚点/覆盖与纠正；复杂图表/OCR仍有partial/unsupported。

## Verified Evidence
- G1 C01–C09单源Schema/TS/Rust、schema1迁移、旧1.x适配与未知拒写/降级；45正反例中37结构、8语义，12运行场景说明不当测试通过。
- C native_flow使用真实NativeHost/SQLite及wire：双窗口共享状态、初始ACK前无事件、幂等课程、content.get、伪造window拒绝、重启恢复、不自动开麦通过。
- cargo test --workspace --locked通过（含A25、契约5、wire/native_flow）；native-shell cargo check通过；Python11契约/SQLite、TS37例/RFC8785、generator --check通过。E4项本地单测通过。Rustfmt当前toolchain缺失，格式检查未执行。
- E真实PDF本地抽取：L08 82页，light80 basic_ready/2 partial；deep67 complete/15 partial（63 text/4 table/13 formula/2 image），92 search units。L09 86页，light84 basic_ready/2 partial；deep76 complete/10 partial（76 text/8 formula/2 image），84 search units。两份搜索覆盖partial，公式为候选证据；manifest共享校验通过。结果仅.local，无云。
- qta原件hash/轨道元数据见evaluation/assets.json；AAC两轨分别完整解码至null 1275371667us/6140310000us；APAC工具缺decoder，见evaluation/asset-decode.json。没有混轨/试听/选轨/课件绑定。
- 工程七文档为只读设计基线；其“非Git/只写文档/未实现”是已结束阶段状态，本记录给出有证据的后续变化。产品目标/固定门槛不随代码更改。

## Pending / Unverified
- G2尚未完成真实macOS麦克风/权限/关窗采集/设备恢复/长录音；F native联调、N搜索/移交/托管/资源删除/备份，C sidecar组合继续。
- G3–G5待真实北京四模型、≤15秒完整focaus/约10秒进度、三层内容/保护/引用/对象/Command+F、真实课程1倍速与课后全量/课堂包。fixture、模块commit、构建不能关闭这些验收。
- 用户提供真实课件目录/Users/pitr/Desktop/SKD/DeepLearning，L08/L09原始PDF清单在evaluation/courseware-assets.json；未根据文件编号绑定qta，不把旧转写作为真值。
- 云调用/上传/费用未获本阶段具体授权；密钥未读取，后续只宿主安全存储解析四槽位。公开endpoint/服务可用性与实际配置需分别确认。
- development-guide及原文复制、原课件/录音/派生大文件/凭据/本机状态不入Git；grilling完成不重开。

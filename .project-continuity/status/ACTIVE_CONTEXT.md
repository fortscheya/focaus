# Active Context

## Current Task
- 2026-10-18：短期 Demo 优先：完成本地流式识别入口、选段入课件、原话↔课件跳转、2D 卷积对象、课件重构示例和二次转写订正；决策模型、filetrans 与真实云链路暂缓。
- 2026-10-18：C推进G2真实本地链；G0 3312e4e/G1 d9a8cdf6完成；四个持久对话继续，不重建。
- 总控01a14cae-d8ce-7053-bd12-737ea1bef020/local，codex/focaus-integration；代码整合至0ced003，后续continuity证据另有本地提交；四路ID/path/模型证据见docs/DEVELOPMENT.md。主continuity仅C写。
- D170允许本地cherry-pick/merge/契约同步，不含push/rebase/发布/删除。

## Current Evidence
- 已整合F录音/CAS、Unicode/数学、coverage包装和真实ContentView展示（d3507f0/c6a9ac1/52c7221/b4c9042），N资料托管/原子搜索投影（d2eb1ba），A runtime（150f3cd），E sidecar（1886d71）。
- 实际嵌入WKWebView：空库bootstrap、键盘输入课程课堂、ACK2、CmdQ正常退出、重启snapshot2恢复及不自动开麦通过。最初旧版启动失败已由F修复；验收时需确保退出旧进程。
- D173用户已允许约20秒本机默认麦克风+权限、仅.local/validation-library、不云/费用。第二轮最新App真实采集：ACK 4→121→147，UI水位约39秒、停止后状态stopped/时长约46秒；SQLite state=stopped、durable_watermark_ms=46730、integrity_check=ok，47个单声道48kHz float32 WAV、总字节8974356。未评ASR质量。
- 最新整合验证：cargo workspace AI32/contracts6/native_flow2/wire2、native harness19/19、资料引擎10/10、TypeScript契约37、桌面Vitest7、typecheck/build、schema生成检查均通过；最新App ad-hoc签名通过。
- 真实L08/L09 PDF仅本地抽取，82/86页，搜索partial；qta AAC解码通过/APAC缺decoder，未选轨/绑定课件。
- Demo 工作树新增 `apps/desktop/src/demo/DemoWorkspace.tsx`，并接入 `?demo=1` 路由和演示样式：本地麦克风 chunk 采集、定制 sentence 增量回放、选段入课件、来源跳转、L08 卷积课件重构、2D 节点/卷积核交互和二次转写采纳/拒绝。

## Next
- F已提交并整合真实content.get投影（5e6186c/0ced003）：note/raw/secondary/material/content_view/unsupported/partial分流，material collection view按lesson scope取投影，旧revision异步响应丢弃；继续做有资料真实命中和编辑/引用接线。N R0已提交907a56a：原声封口与SQLite登记解耦、journal/缺口恢复、删除屏障；C继续真实资料导入UI与逐页不可变parse版本接线。
- 真实窗口已完成稳定身份/恢复/standalone bootstrap和六格注册观察；Command+F在空索引显示真实0态，真实命中待有内容投影后复验。
- 真实云/上传/费用仍待具体授权；四专用凭据槽位不读取、不输出、不串用。
- Demo 已明确显示“本地 Demo 数据 / 不调用云模型 / 不上传音频”；麦克风可实际采集，但当前文字来自本地 Demo stream adapter，不能称为真实 ASR。真实 qwen-audio streaming 仍待单独云调用/数据/费用授权。
- Demo 阶段已提交 `18251de`（`feat: add local streaming classroom demo`）；提交后工作树仅有外部未跟踪 `presentation/`，未纳入本次提交。
- 契约2.0.0/material2.0.0/visualization1.0.0；四专用槽位，不读/输出密钥，指定模型不换，grilling不重开。

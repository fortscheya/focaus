# 技术路线收敛

## 当前授权与状态
- 2026-10-17：D157确认Q127，grilling完成；D159当前仅提供工程文档提示词，另一模型编写文档，不在本轮实现。无工程、依赖安装、产品测试或推理API调用。
- D137/D138已接受技术方向与引擎边界，D144接受Q117基线后定门槛，D149接受预处理验证路线。D139要求推荐grilling后多对话并行，当前不创建对话/工程；默认UI规范来源为用户AGENTS。

## 已确认路线
- D137（Q114）：Tauri 2 + React + TypeScript，UI用HeroUI React v3默认设计；录音/持久化由原生后台持有，具体录音绑定/编辑库须测试。界面已由D135/D140–D142改为课程多窗/吸附布局，见WORKSPACE-UI.md。
- D138（Q115）：转换核心独立于界面，可单独转换/测试；复用成熟底层解析/OCR，自研对象重构/来源关联/局部布局。版本化结构数据与资源保持对象身份、页/区域、布局、来源版本；本地管理可用云端AI，不承诺离线。
- D144（Q117）：先建立真实样本基线，再共同确定未定数值/固定回归集；15秒等已定约束不放宽。
- D149（Q122）：原声可靠落盘与ASR预处理支路分开，格式/电平/降噪去混响对照后定默认，保时间映射/异常可旁路、不阻采集；增益与延迟待测。
- D150：全局课堂进度指定decision-model-preview，输入转写/带页PPT文字，约10秒更新；快速跟上指针复用，取消20秒/P+1时间外推。接口事实与故障边界见CATCHUP-MODELS.md。
- D151–D154：转移默认纠正整次归属/跨堂结束新开；满6格腾位或独立窗；进度证据超过30秒陈旧；实时ASR选qwen-audio-3.1-asr-flash-streaming，积压选qwen-audio-3.1-asr-flash-filetrans，见ASR-CANDIDATES.md。
- D155：用户明确四模型统一华北2（北京），根目录BAILIAN_API_INTEGRATION.md仅协议/参数参考，推荐用途无决定效力。用户称凭据备好，未索取/验证/保存值/调用服务；主agent仅读取过滤凭据行与私有主机后的文档视图，不改原文或运行示例。
- D156（Q126）：两层转写/录音迁移保引用身份；课件/已有笔记保原归属，双方受影响汇总标可更新/保护用户内容，新识别用目标上下文、已定稿原始不重写。D157最终确认，当前完整基线见PRODUCT-BASELINE.md。

## GPT建议（待确认）
- 进度任务按课堂/来源版本隔离，转移后旧响应不写入新课堂；D156材料处理已确认，任务/语言/引用状态迁移需整合测试。
- 约10秒定位调用建议有新转写才更新，快照带证据截止时点；单路合并最新输入，旧请求晚到不覆盖新状态。停录/暂停与恢复行为依录音状态，实际接口/吞吐/输入窗口待核对与测试。
- D157接受qwen3.8-flash作为首版二次转写/笔记五档/快速跟上摘要/问答基线并为引擎提供图文提取；独立任务有各自提示词/结构/时延配置。未经课堂测试，不承诺15秒或重构质量；转换/渲染仍由引擎/编辑器负责。

## 已确认并行分工（D157接受，尚未启动）
- 5个对话：1总控整合+4开发；D159本轮只交付工程文档编写提示词。
- 总控：公共契约/对象身份/来源版本与时间轴/数据库迁移、阶段集成、真实课程验收、权威continuity同步。
- 前端：开始页、课程工作区、网格/内部窗口、编辑器与对象渲染、来源导航；通过宿主接口请求拆出/放回，不独立实现采集或解析。
- 原生后端：Tauri宿主/原生窗生命周期、录音/本地预处理、文件资料库/数据库、任务执行与故障恢复、命令事件接口；公共迁移由总控管理。
- AI流程：ASR服务适配与流恢复、二次转写/订正、笔记/五档、快速跟上/对齐/术语/问答等生成检索逻辑；调用宿主存储与任务接口，不再另建权威数据库。
- 资料引擎：PDF/PPTX轻深处理/对象重构/OCR/来源区域/转换重试与纠正应用，产出标准对象与资源；编辑展示归前端，落库归宿主。
- 先由总控定义最小公共类型/错误状态/IPC与样例，再四路各用独立worktree/分支和明确写入范围并行，用样例替身解除等待，最终用真实链路验收。共享契约/迁移/权威continuity单一负责人，各路经相关验证后本地commit；development-guide不提交。当前未初始化Git/建分支/创建线程/开发代码。

## 官方事实与限制（2026-10-17）
- Tauri core与WebView分开，core负责平台资源/全局状态；macOS使用WKWebView。https://v2.tauri.app/concept/process-model/ （The Core Process / The WebView Process）。
- 系统托盘需tray-icon功能；全局快捷键有官方插件且须配置capabilities。https://v2.tauri.app/learn/system-tray/ 、https://v2.tauri.app/plugin/global-shortcut/ 。
- 支持打包外部二进制/由宿主启动sidecar（含Apple Silicon目标命名）。https://v2.tauri.app/develop/sidecar/ 。
- ExitRequested有prevent_exit机制。https://docs.rs/tauri/latest/tauri/struct.ExitRequestApi.html 。这不代表录音会自动跨窗口关闭保持，必须显式设计进程/窗口/退出生命周期并实测。
- 主agent直接读取上述官方文档；限定Gemini worker仅核对公开能力，已收取复核并关闭。其将sidecar/宿主支持推为录音必然连续的表述未采纳；麦克风权限、设备中断/休眠/崩溃恢复及编辑器性能均未运行验证。
- 新多窗核对：WebviewWindow可动态创建宿主Window+Webview，event有emitTo/listen。https://v2.tauri.app/reference/javascript/api/namespacewebviewwindow/ 、https://v2.tauri.app/reference/javascript/api/namespaceevent/ 。主agent核对构造器与事件小节；这些原语不构成已完成/已验证的拖出停靠交互。页面unstable提示针对new Webview子视图，不能无差别套到new WebviewWindow。
- 音频核对：Azure MAS的DSP说明列出增益/去混响/降噪等，并提示失真会损害ASR、其单麦克风/低SNR降噪效果有限。https://learn.microsoft.com/en-us/azure/ai-services/speech-service/audio-processing-speech-sdk （DSP enhancements）。这是MAS相关证据，不推定Qwen等必然提升；该页列Windows/Linux，不作为已选macOS预处理实现。主agent已直接读取；新限定Gemini事实worker已收取关闭，无用户音频处理或外传。
- qwen3.8-flash官方模型卡北京能力表列Image/Text/Video输入、Text输出及结构化输出支持。主agent直接读取：https://help.aliyun.com/zh/model-studio/qwen3-8-flash 。不据宣传速度/长上下文推定课堂时延或生成质量；视频不是首版自动处理范围。

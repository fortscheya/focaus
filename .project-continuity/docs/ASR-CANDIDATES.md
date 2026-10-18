# ASR 选型、官方事实与待验证项

<!-- continuity-document authority: continuity exported: false -->

- 核对日期：2026-10-16及2026-10-17；仅公开官方资料，未调用识别API、上传课堂材料或产生识别费用。
- 用户需求与早期候选见D13–D14；2026-10-17用户以D154明确双链路型号，替代早期未定状态。公开能力与厂商描述不证明真实课堂性能。

## 当前已选（2026-10-17，D154）
- D155补充：所有接入使用华北2（北京）；用户的BAILIAN_API_INTEGRATION.md只作接口参考，未验证账号/密钥或执行请求。
- 实时采集转写：qwen-audio-3.1-asr-flash-streaming。
- 积压音频补转写：qwen-audio-3.1-asr-flash-filetrans；上传/任务/时间戳/上下文公开限制已由主agent核对如下，未调用API或处理录音。
- 继续D66：恢复先追最新，历史文件链路后台补齐；两者按本地原始音频时间轴插入，不把历史识别结果冒充实时最新。应用采集/原声落盘独立于两条云端链路。
- GPT实施解释：导入已有录音属于非实时处理，默认使用filetrans；已稳定实时ASR不因补处理而覆写，边界重叠用于定位/去重，不生成重复原始句段。补齐后派生材料按既有来源更新规则处理，具体衔接待测试。

## 双链路官方核对（2026-10-17）
- 总览明确列streaming为WebSocket/热词+Prompt上下文，音频时长“无限制”；filetrans为非实时HTTP/热词+Prompt上下文/支持说话人分离，≤12小时/2GB。此为音频规格，不推定单连接无超时或真实课堂性能达标。https://help.aliyun.com/zh/model-studio/asr-model 。
- filetrans使用异步提交/查询task_id/下载结果；input.file_urls单次1个HTTP(S) URL。REST接口也支持官方oss://临时URL（文档有效期48小时/不供生产高并发），SDK不支持该临时URL；URL到期不等同已确认云端对象即时删除。首版开发可评估官方临时上传，原始本机副本继续保留，实际上传实现/清理能力未测。
- 请求必须带X-DashScope-Async: enable及parameters（无配置也传{}），否则可能提交后任务失败。文件HTTP接口支持即时/预编译热词与context；SDK暂不支持上下文。上下文每轮≤400字符、最近5轮，合并热词超过2000会随机取2000，需本地有意筛选而非传整份课件。
- 返回包含句与词的begin_time/end_time（毫秒），可用于映射原始录音时间轴；speaker_id仅启用说话人分离时显示，启用分离建议≤2小时。对应HTTP API：https://help.aliyun.com/zh/model-studio/fun-asr-recorded-speech-recognition-http-api （流程/输入/识别结果）。
- 这些为精确列出3.1型号的官方事实，不沿用3.0秒计费/其他型号限制；延迟、断线续接、混合语言保真、切片边界与时间戳准确性仍需实际验证。
- 限定Gemini辅助检索在一次等待及一次状态检查后仍未完成，已关闭；主agent直接检索并读取上述官方总览/HTTP API、实时用户指南完成必要核对。未运行示例/安装SDK/读取凭据/上传音频。

## 历史候选核对（2026-10-16，供对照）

- 首测建议：阿里云 `qwen-audio-3.1-asr-flash-streaming`。官方明确持续课堂/会议转写、中英自由切换、热词与上下文增强；客户端有远场/近场 VAD。服务端区分中间与最终识别、句/字时间戳，并回累计Token用量。北京原价输入6元/百万Token、输出4.5元/百万Token；不得套用3.0的按秒价格，也未据此估算本项目每小时账单。
  - https://help.aliyun.com/zh/model-studio/asr-model
  - https://help.aliyun.com/zh/model-studio/qwen-audio-3-1-asr-flash-streaming
  - https://help.aliyun.com/zh/model-studio/qwen-audio-asr-streaming-client-events
  - https://help.aliyun.com/zh/model-studio/qwen-audio-asr-streaming-server-events
- Qwen 增强限制：即时热词与预编译热词合计最多2000个；上下文每轮最多400字符、最近5轮，主要需含实际术语；不能把整份课件直接作为无限上下文。任务中更新上下文的文档按支持模型限定，未实测确切版本的会话内热词/上下文更新。
  - https://help.aliyun.com/zh/model-studio/improve-asr-accuracy
- 对照建议：腾讯云大模型2.0 `16k_zh_en_2.0`，V2 WebSocket 有中间/稳态句子、时间信息及预建/临时热词；临时热词最多128个。公开接口未列同Qwen式对话上下文输入；自学习/替换表与自然语言上下文应区分。国内后付费基础识别1元/小时，附加功能另计。文档4008为超过15秒未发送音频数据，不能误写成静音15秒。新会话需新voice_id；不能用其当应用录音稳定身份。
  - https://cloud.tencent.com/document/product/1093/131127
  - https://cloud.tencent.com/document/product/1093/35686
- 对照建议：火山豆包流式2.0。官方区分双向流式、双向流式优化/二遍及流式输入；后者不能冒充逐字实时输出。公开API含中英、热词、上下文及2.0图片字段，热词最佳实践表又将上下文/图片列于非流式/二遍，实际链路适用性未完全消歧，不宣称所有实时模式同等支持。双向流式热词100 tokens，非流式/二遍5000词；后付费基础1元/小时。会话内动态更新当前文档未见，不能据此断言全平台绝不支持。
  - https://docs.volcengine.com/docs/DoubaoVoice/LargemodelstreamingautomaticspeechrecognitionAPI?lang=zh
  - https://docs.volcengine.com/docs/DoubaoVoice/hot-words-and-context?lang=zh
  - https://docs.volcengine.com/docs/DoubaoVoice/model-list?lang=zh
  - https://docs.volcengine.com/docs/DoubaoVoice/Billinginstructions-21?lang=zh
- 本地评估候选：开源 Qwen3-ASR 0.6B/1.7B，官方提供流式/离线能力与参考框架。它们不是云端3.1同一模型；Mac部署运行时、速度、准确度、耗电尚未验证，不保证现成可用。
  - https://github.com/QwenLM/Qwen3-ASR

## 待验证

- 用同一段深度学习课堂，比较纯音频、课件提炼热词、热词+上下文；热词过强可能误植其他词。以人工复核材料衡量术语、中英切换、否定、数值/日期及漏句，不把模型输出或课件当唯一真值。
- 流式按真实1倍速逐步输入；在选定触发点只使用当时已可见音频/转写/课件。记录首个有效文字、稳定结果的延迟分布、修订幅度、缺口、断线恢复与实际费用；完整90+30分钟连续压力测试另验。
- 快速跟上的15秒从按键到全部输出完成，连同上下文准备与最后一段音频处理一起计时。确认低置信度、长回看窗口及服务不可用行为后再定可行性测试。


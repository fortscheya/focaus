# Focaus material engine

这是 `focaus.material/2.0.0` 的独立资料转换 sidecar。它只读取宿主托管的
PDF、PPTX、纯文本、Markdown 或图片原件，只在宿主提供的 staging 目录写入
不可变候选包和页级 checkpoint，不访问数据库、不读取凭据、不调用云模型，也
不写笔记或搜索索引。

开发运行方式：

```sh
PYTHONPATH=engines/material python3 -m material_engine.cli
```

标准输入和标准输出都是 JSON Lines。每条输入必须是契约 2.0.0 的
`EngineRequest`；当前支持 `handshake`、`convert`、`resume`、`cancel`。转换请求
的 payload 至少包含 `material_id`、`source_version_id`、`parse_version_id`、
`source_handle`、`staging_handle`、`identity_pool`、`profile` 和 `pages`。页码从
0 开始，空 `pages` 表示全部页。宿主应预分配或确认 identity pool 中的 ID；当
开发 fixture 没有提供某个 ID 时，sidecar 会生成 request-scoped candidate，并在
manifest provenance 中标为 `engine_candidate_requires_host_confirmation`，绝不会
把文件名、hash 或页码本身当作权威对象 ID。

典型请求：

```json
{"contract_version":"2.0.0","request_id":"...","name":"convert","task_id":"...","payload":{"material_id":"...","source_version_id":"...","parse_version_id":"...","source_handle":"/host/managed/read-only.pdf","staging_handle":"/host/task-staging","identity_pool":{},"profile":"deep","pages":[],"corrections":[],"budget_ms":600000}}
```

结果顺序为多个 `checkpoint`/`page_result`，最后是 `completed`。每个
`page_result` 都带一个新的 `snapshot_parse_version_id`、累计 `manifest` 和
`manifest_path`，对应 `staging_handle/snapshots/<parse_version_id>.json`；文件只
能创建一次，`parent_parse_version_id` 必须先存在。请求中的 `parse_version_id`
保留给最终快照，`manifest.json` 只是可替换的恢复/head 副本，不能作为历史快照
身份。`page_result` 顶层同时携带 `material_id`、`source_version_id`、`source_hash`，
以及当前页的 `page`、`regions`、`objects`、`resources` 和 `search_units` 闭包；
这些页级字段与其 `snapshot_parse_version_id` 使用同一组锚点，并带有与磁盘
checkpoint 相同的 `progress` identity。`completed` 也重复提供最终 manifest 的
identity 与 `progress`，便于宿主在不解析内部路径的情况下核对任务结果。page checkpoint
之间的父链只用于 sidecar 恢复；`completed` 的最终 manifest 始终保留宿主已经
确认的 `parent_parse_version_id`，不会把内部 checkpoint 当成宿主父版本。单页解析、渲染
或对象重构失败会保留该页的 `failed` 或 `partial` 状态并继续其他页；只有输入
不可读、版本不兼容、staging 越界等全局错误才返回 task-level error。`resume` 使用
同一 staging 目录里的 checkpoint 跳过已完成页；`retry_pages` 可在 payload 中
指定局部重试页。

重解析若父快照不在当前 staging 目录，应在 payload 中提供宿主托管的本地
`parent_manifest_handle`。sidecar 会校验 `material_id`、`parse_version_id`、格式、
资源 hash 和闭包后才开始发布；不存在的父版本不会被静默当作根。资源字节按
`resource_id`/SHA-256 复用并拒绝不同字节覆盖，旧 snapshot JSON 始终保留。

## 解析和覆盖边界

- PDF 先读取原生文字流，并用 `pdftoppm`（若宿主提供）生成原页 PNG 预览。
  没有文字层的页面保留原页/预览 image object，OCR 覆盖写为
  `unprocessed`，不会报告“无命中”。
- PPTX 直接读取 PresentationML 的文本、表格、图片、图表缓存和基础连接图，
  不把幻灯片截图当成唯一输入；`soffice` 与 `pdftoppm` 只负责页预览。嵌入音视
  频、外链内容和转场被标记为 unsupported/diagnostic，不抓取或展开。
- 教材的 `light` profile 保证基础页和文字优先；课件的 `deep` profile 尝试真
  实表格、图表轴/系列/点、公式候选和结构图节点/连线。无法可靠重构时保留原图
  并标记 `partial`/`unresolved`，不把 image-only 回退计为深解析完成。
- 每个包都产出 `search_units` 和 `search_coverage`。unit 是交给宿主 N 规范化
  和分配 `search_unit_id` 的 `ExtractedSearchUnit`，sidecar 不生成
  `index_generation`、命中数或可见性 epoch。原页/对象的同源文本使用双锚点和
  `equivalence_group_id`，不同位置的相同字符串不会合并。
- 公式/变量信息只作为 `provenance.formula_evidence` 的来源候选，带页、区域、
  对象和原文范围；sidecar 不生成课堂话语、`interactive_visualization` 或
  focaus 路径。A/N/F 仍需按来源证据和共同事实链决定是否使用。

运行时不需要第三方 Python 包。若 host 镜像中有 `pdftoppm`、`soffice`，会在
受限 staging 目录内调用它们；路径可用 `FOCAUS_PDF_RENDERER` 和
`FOCAUS_OFFICE_RENDERER` 明确指定。没有这些工具也不会伪称深解析成功。

# Focaus 路演演示工具

这个目录按 `FRAME_Interactive_Presentation_Spec_v1.0.md` 建立独立的 Slidev 工具链。HTML 是正式演示媒介，PDF/PPTX 只作为静态备份。

## 当前状态

当前已根据用户提供的 `STORYBOARD.md` 实现 6 页、5 分钟路演演示。Slide 2 使用本地麦克风可选采集与离线预置流，Slide 3–5 使用本地深度学习示例数据，Slide 6 汇总跨页持续的三层路演材料。

## 本地命令

```bash
pnpm install
pnpm dev
pnpm build
pnpm serve
pnpm test
pnpm export:pdf
pnpm export:pptx
```

正式演示使用浏览器打开构建后的 HTML，并通过本地 HTTP 服务提供资源；不要依赖 `file://` 或临场外网字体、图片和数据。使用 `pnpm serve` 或双击 `open-demo.command` 启动本地服务；它会把 Slidev 的 `/1` 到 `/6` 页面地址回退到 `index.html`，因此直接打开某一页或刷新深链接都能正常工作。

浏览器地址使用 `http://127.0.0.1:3032/`，也可以使用 `http://127.0.0.1:3032/1` 直接打开第 1 页。不要用 `python3 -m http.server` 代替 `serve-demo.py`，因为标准静态服务器会把 `/1` 当成文件并返回 404。

当前实时流是本地彩排实现：点击 Focaus 后尝试打开浏览器麦克风，文字、二次转写和笔记使用预置路演流逐段累积，不上传音频，也不调用云模型。接入真实 ASR 时只替换 `composables/useLivePitchStream.ts` 的 adapter。

Slidev 默认使用 `localhost` 监听。本机 Playwright 检查通过 IPv6 回环地址访问开发服务，属于本机环境差异，不影响正式浏览器打开方式。

## 目录约定

- `slides.md`：Slidev 幻灯片入口和叙事顺序
- `components/`：Vue 交互组件和图表
- `styles/`：FRAME 语义 Token、排版和通用样式
- `docs/STORYBOARD.md`：收到剧本后维护的页面故事板
- `tests/`：Playwright 导航、交互和截图检查
- `pnpm-lock.yaml`：独立锁定演示工具依赖，不与根目录产品工作区共享

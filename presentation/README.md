# Focaus 路演演示工具

这个目录按 `FRAME_Interactive_Presentation_Spec_v1.0.md` 建立独立的 Slidev 工具链。HTML 是正式演示媒介，PDF/PPTX 只作为静态备份。

## 当前状态

当前只有工具链烟雾页和故事板模板，没有使用未提供的剧本编造路演内容。收到剧本后，先更新 `docs/STORYBOARD.md`，再实现完整页面。

## 本地命令

```bash
pnpm install
pnpm dev
pnpm build
pnpm test
pnpm export:pdf
pnpm export:pptx
```

正式演示使用浏览器打开构建后的 HTML，并通过本地 HTTP 服务提供资源；不要依赖 `file://` 或临场外网字体、图片和数据。静态服务需要把未知路径回退到 `index.html`，这样直接打开某一页的深链接也能正常工作。

Slidev 默认使用 `localhost` 监听。本机 Playwright 检查通过 IPv6 回环地址访问开发服务，属于本机环境差异，不影响正式浏览器打开方式。

## 目录约定

- `slides.md`：Slidev 幻灯片入口和叙事顺序
- `components/`：Vue 交互组件和图表
- `styles/`：FRAME 语义 Token、排版和通用样式
- `docs/STORYBOARD.md`：收到剧本后维护的页面故事板
- `tests/`：Playwright 导航、交互和截图检查
- `pnpm-lock.yaml`：独立锁定演示工具依赖，不与根目录产品工作区共享

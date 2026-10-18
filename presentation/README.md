# Focaus 新版互动路演

基于第二版真实 L03 课堂 Demo 重制，共 8 页，约 5 分钟。HTML 保留交互与动效，PDF/PPTX 是静态备份。演讲顺序与讲者动作见 [故事板](docs/STORYBOARD.md)。

## 打开演示

在 Finder 双击 `open-demo.command`，保持终端运行，然后在 Chrome 打开：

http://127.0.0.1:1421/

也可手动启动：

```bash
cd /Users/pitr/Desktop/Project/hackthon2026skd/focaus/presentation
python3 serve-demo.py
```

左右方向键翻页；鼠标移到左下角可打开 Slidev 全屏、概览和演讲者控制。直接刷新 `/1` 到 `/8` 正常；不要改用普通 `python3 -m http.server`，它无法处理幻灯片深链接。`?handout=1` 用于展开静态讲义状态。

## 现场识别与彩排

第 2 页默认是“现场识别”。点击开始后，浏览器连接现有本地语音宿主 `http://127.0.0.1:4319`，请求麦克风权限；服务的 partial/final 事件决定显示的文字。支持停止等待尾句、断线提示、跨页保留、手动摘录和结束页停止。离开页面关闭麦克风。

配套启动器使用 **1421**，这是既有语音宿主已允许的本机来源；不修改宿主的 Origin、凭据或音频额度设置。真实语音识别仍需要原本的宿主运行、专用凭据可用、网络可用，并遵循宿主已有音频额度。若额度不足，页面报告错误，不伪装成功。完整产品入口仍为 `http://127.0.0.1:1422/?demo=1`。

选择“彩排回放”可离线播放已准备的 L03 片段，明确标记为回放，不打开麦克风，不上传音频。加载幻灯片与导出备份都不会自动开始识别。

## 本轮内容

1. 封面
2. 真实 ASR：现场输入、临时句、定稿和摘录
3. 可信边界：Sigmoid 漏“不”字的人工情境，原始句保持只读
4. 六窗课堂工作区分组展开
5. focaus：固定 85:46 时点、p.79、Xavier 初始化的回顾与跟上
6. 来源链：原始句 148 与课件 p.35 对照
7. 可操作的 2D 卷积计算
8. 本场识别、课堂订正示例与手动摘录收束

focaus、课程笔记与二次转写均沿用当前产品里已准备的课堂场景，不冒充现场云模型生成。卷积页是明确的演示参数计算；实际产品支持更多参数与数学宿主校验。来源说明见故事板。

## 开发与验证

```bash
pnpm dev
pnpm build
pnpm serve
pnpm test
pnpm exec tsc --noEmit
```

`pnpm dev` 是源码预览，正式现场演示使用 1421 启动器。若终端找不到 Node.js，使用 Codex `load_workspace_dependencies` 提供的 runtime，不改全局 shell 配置。

测试覆盖 1280×720 / 1920×1080 鼠标交互、现场 ASR 协议事件与停止尾句（隔离模拟）、宿主不可用错误。协议测试不上传音频、不调用云模型。

## 重新导出

```bash
pnpm exec slidev export --format pdf --output outputs/focaus-roadshow.pdf --wait 1500
pnpm exec slidev export --format pptx --output outputs/focaus-roadshow.pptx --wait 1500
```

打印/导出自动展开重点示例。PDF/PPTX 不保留 Web 交互、现场麦克风或翻页动效；源码和 HTML 是可继续修改的主版本。

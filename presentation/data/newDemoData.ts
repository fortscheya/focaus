// Bounded snapshot of the running L03 Demo; see docs/STORYBOARD.md for provenance.
export const newDemoData = {
  "course": "CS 280 · Deep Learning",
  "lesson": "L03 · CNNs I — Architecture & Training",
  "model": "qwen-audio-3.1-asr-flash-streaming",
  "region": "百炼 · 北京",
  "rawSegments": [
    {
      "id": "l03-raw-0143",
      "time": "35:45",
      "text": "就会，就 stuck 到一个 local minimum 里面了。所以这种 saturated 这种现象对 CNN 的优化其实也是非常不利的。还有一个比较隐蔽的作用是，呃，它不是 zero-centered。大家看后",
      "source": "原始句 143"
    },
    {
      "id": "l03-raw-0145",
      "time": "36:15",
      "text": "中心化调整啊，还有一些 normalization 的手段可以去避免。但是 sigmoid 的问题也是在于它不是 zero-centered。不是 zero-centered 会有什么问题呢？我们到后面讲到，咳",
      "source": "原始句 145"
    },
    {
      "id": "l03-raw-0148",
      "time": "37:00",
      "text": "[咳] 它不是 zero-centered，它全部都是正数。意思是说我们到后面的某一层，实际上的这个 x 会全是正的。全是正的。那么， 假设我们现在是二维的一个例子啊，那么我们会有一",
      "source": "原始句 148"
    },
    {
      "id": "l03-raw-0304",
      "time": "85:16",
      "text": "到的这个分布的方差的具体问题能够解决。那它的 weight 是什么意思呢？就是说它的一套基本的思路就是说，把我们的这个权重的标准差大概是我们的一套1除以我们的这个呃根号",
      "source": "原始句 304"
    },
    {
      "id": "l03-raw-0305",
      "time": "85:31",
      "text": "号 fan-in，fan-in 是指我们的这个呃输入的这个 feature 的维度。OK？那什么意思呢？就，嗯，但如果是一个 convolution，convolution 的话，它的一个 fan-in 其实就是我们 filter 的这个 size。",
      "source": "原始句 305"
    }
  ],
  "secondary": {
    "original": "Sigmoid 的输出是以零为中心的。",
    "suggestion": "Sigmoid 的输出不是以零为中心的。",
    "reason": "疑似漏掉了“不”，导致命题与函数性质相反。Sigmoid 的公式为 σ(x)=1/(1+e^(−x))，对有限实数 x，输出恒在 (0, 1)，不能以零为中心，因此建议补为“不是”。此处为人工缺字演示；真实第143、145、148句均含“不是”，可回原始句核查。二次正文不自动补字，由你确认后应用。",
    "demoLabel": "人工缺字示例 · 非课堂原句",
    "sourceId": "l03-raw-0148",
    "sourceTime": "37:00",
    "rawText": "[咳] 它不是 zero-centered，它全部都是正数。意思是说我们到后面的某一层，实际上的这个 x 会全是正的。全是正的。那么， 假设我们现在是二维的一个例子啊，那么我们会有一"
  },
  "focaus": {
    "title": "接上权重初始化的讲解",
    "window": "回看 75:46 – 85:46",
    "currentTopic": "初始化尺度与 fan-in",
    "prerequisite": "这里的两个“零”含义不同：数据零中心是在调整输入分布；权重全零则会保留神经元之间的对称性。随机性负责让神经元有所区别，尺度负责避免信号在深层传播时过度衰减或进入饱和区。",
    "page": "课件 p.79",
    "note": "3. Xavier 初始化原理与方差推导",
    "time": "85:46",
    "paragraphs": [
      {
        "id": "recap",
        "text": "过去十分钟，课堂从让输入特征尺度更均衡、尽量零中心，转向了权重初始化。权重全零会使神经元彼此没有区别；改用随机数后，数值尺度仍要选择：太小会让深层激活逐层趋近零，太大则让 tanh 落入两端的饱和区，两种情况都可能使梯度变小、训练难以推进。",
        "sourceSegmentIds": [
          "l03-raw-0268",
          "l03-raw-0269",
          "l03-raw-0270",
          "l03-raw-0271",
          "l03-raw-0273",
          "l03-raw-0285",
          "l03-raw-0287",
          "l03-raw-0288",
          "l03-raw-0297",
          "l03-raw-0298",
          "l03-raw-0299"
        ]
      },
      {
        "id": "bridge",
        "text": "因此，眼前的问题已经从“要不要随机初始化”变成了“随机权重的尺度该怎么选”。老师正在把“不能太大或太小”收束为一个可操作的关系：权重标准差约取 1/√fan-in，fan-in 指输入特征维度。此刻刚进入卷积层 fan-in 的说明，先跟住这个量在数什么，就能接上当前讨论；具体计数和方差推导还没有展开。",
        "sourceSegmentIds": [
          "l03-raw-0273",
          "l03-raw-0274",
          "l03-raw-0275",
          "l03-raw-0299",
          "l03-raw-0300",
          "l03-raw-0303",
          "l03-raw-0304",
          "l03-raw-0305"
        ]
      }
    ],
    "limitations": [
      "本次是根据真实 L03 资料预先整理的固定场景，未调用云端 focaus 生成；演示位置不随实时麦克风推进。",
      "范围按已有原转写时间字段选定，尚未与录音逐句对齐。跳转笔记是完整课后章节，其中的后续推导不属于这十分钟回顾。"
    ]
  },
  "panes": [
    {
      "id": "material",
      "label": "课件",
      "detail": "完整 L03 · 128 页",
      "tone": "accent"
    },
    {
      "id": "live",
      "label": "实时字幕",
      "detail": "仅来自本次麦克风",
      "tone": "live"
    },
    {
      "id": "raw",
      "label": "原始底稿",
      "detail": "559 句 · 只读",
      "tone": "muted"
    },
    {
      "id": "secondary",
      "label": "二次转写",
      "detail": "人工整理 · 关键改动待确认",
      "tone": "warning"
    },
    {
      "id": "note",
      "label": "课堂笔记",
      "detail": "本地已准备资料",
      "tone": "success"
    },
    {
      "id": "sources",
      "label": "来源与采纳",
      "detail": "来源回跳与操作记录",
      "tone": "muted"
    }
  ],
  "noteExcerpt": "Xavier 初始化通过选择随机权重的尺度，帮助信号在深层网络中传播。fan-in 表示输入特征的维度；本课在 p.79 开始讨论初始化尺度。",
  "convolution": {
    "input": [
      [
        1,
        0,
        2,
        1
      ],
      [
        2,
        1,
        0,
        2
      ],
      [
        0,
        1,
        3,
        1
      ],
      [
        1,
        2,
        1,
        0
      ]
    ],
    "kernel": [
      [
        1,
        -1
      ],
      [
        0,
        1
      ]
    ]
  }
} as const

export const realDemoUrl = "http://127.0.0.1:1422/?demo=1"
export const demoHost = "http://127.0.0.1:4319"
export const handoutMode = typeof location !== "undefined" && (new URLSearchParams(location.search).has("handout") || new URLSearchParams(location.search).has("print") || location.pathname.endsWith('/print'))
export function openRealDemo() { window.open(realDemoUrl, "_blank", "noopener,noreferrer") }

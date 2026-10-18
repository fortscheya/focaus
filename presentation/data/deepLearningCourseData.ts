export type Correction = {
  id: string
  word: string
  suggestion: string
  reason: string
}

export type TraceNode = {
  id: string
  label: string
  detail: string
}

export const deepLearningCourseData = {
  sourceLabel: 'L08 · 卷积与局部连接 · 本地课程示例',
  sourceDetail: '示例数据来自本地深度学习演示材料，保留页面与时间锚点。',
  terms: ['Backpropagation', 'Layer Normalization', 'Jacobian Matrix', '梯度消失'],
  documentRows: [
    ['卷积层', '局部窗口映射到新的特征空间', '3 × 3'],
    ['步幅', '窗口移动的间隔', 'stride = 1'],
    ['权值共享', '同一卷积核扫描不同位置', '参数更少'],
  ],
  polishedTranscript: '卷积核在局部窗口上滑动，用同一组权重提取不同位置的局部模式。',
  rawTranscript: '卷积核在局部窗口上滑动，步幅为二，提取不同位置的局部模式。',
  corrections: [
    {
      id: 'stride',
      word: '步幅为二',
      suggestion: '步幅为一',
      reason: '课件第 4 页的公式与当前窗口位置显示 stride = 1，声学候选与上下文不一致。',
    },
    {
      id: 'weight',
      word: '权重共享',
      suggestion: '权值共享',
      reason: '统一本课术语。它只修正文案表达，不改变原始事实。',
    },
  ] satisfies Correction[],
  alerts: [
    { label: '🚨 期末考点', text: '步幅改变会同时影响输出尺寸与计算覆盖范围。' },
    { label: '⭐ 重点推导', text: '权值共享让同一卷积核识别不同位置的局部模式。' },
    { label: '📝 课后作业与 DDL', text: '推导 3 × 3 卷积输出尺寸，并比较 stride = 1 / 2。' },
  ],
  noteLevels: [
    '卷积层在局部窗口上滑动，用同一组权重提取不同位置的局部模式。',
    '卷积层在局部窗口上滑动，提取不同位置的局部模式。核心缩写：CNN、Loss、GPU。',
    '卷积层通过 shared weights 在局部窗口上滑动，提取不同位置的 local patterns。这里的 Learning Rate 影响更新速度，Overfitting 需要正则化。',
    'The convolution layer slides across local windows with shared weights. Learning Rate controls the update step, while Overfitting calls for regularization.',
    'A convolution layer extracts local patterns with shared weights. The Learning Rate controls optimization, and regularization reduces Overfitting.',
  ],
  referenceExcerpt: '同一组卷积核可以扫描不同位置。保留原页结构后，术语会成为流式识别的上下文热词。',
  learningCurve: {
    animationDuration: 450,
    grid: { left: 42, right: 18, top: 18, bottom: 30, containLabel: true },
    tooltip: { trigger: 'axis' },
    legend: { data: ['训练损失', '验证损失'], textStyle: { color: '#5C6372' } },
    xAxis: { type: 'category', data: ['1', '2', '3', '4', '5', '6'], axisLabel: { color: '#5C6372' } },
    yAxis: { type: 'value', min: 0, max: 1, axisLabel: { color: '#5C6372' }, splitLine: { lineStyle: { color: '#DDE1E7' } } },
    series: [
      { name: '训练损失', type: 'line', smooth: false, data: [0.92, 0.67, 0.48, 0.34, 0.25, 0.19], symbolSize: 7, lineStyle: { width: 3 } },
      { name: '验证损失', type: 'line', smooth: false, data: [0.95, 0.72, 0.57, 0.49, 0.45, 0.44], symbolSize: 7, lineStyle: { width: 3, type: 'dashed' } },
    ],
  },
  traceNodes: [
    { id: 'note', label: '笔记', detail: '“步幅为 1，输出尺寸保持可控。”' },
    { id: 'secondary', label: '二次转写', detail: '保留原意，订正候选等待用户采纳。' },
    { id: 'raw', label: '原始转写', detail: '00:05.1 · “步幅为二。”' },
    { id: 'audio', label: '课堂音频', detail: '原始时间轴可定位，录音暂停后可回听。' },
    { id: 'material', label: '重构课件', detail: 'L08 · 第 4 页 · 公式与窗口示意。' },
  ] satisfies TraceNode[],
  formulaDefinitions: {
    adam: '局部变量 ε｜作用域：公式 1 · Adam。数值稳定平滑项，通常取 10⁻⁸，用于防止分母趋近于 0。',
    fgsm: '局部变量 ε｜作用域：公式 2 · FGSM。对抗扰动步长超参数，约束在 L∞ 邻域内，控制攻击强度。',
  },
} as const

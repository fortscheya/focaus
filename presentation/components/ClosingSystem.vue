<script setup lang="ts">
import { computed } from 'vue'
import { newDemoData, openRealDemo } from '../data/newDemoData'
import { useRoadshowSession } from '../composables/useRoadshowSession'
import { useRoadshowAsr } from '../composables/useRoadshowAsr'

const { segments, pinned, mode, streamLabel, reset, isStreaming, stop } = useRoadshowAsr()
const rawText = computed(() => segments.value.length ? segments.value.slice(-4).map(s => `${s.time}  ${s.raw}`).join('\n\n') : '本场尚未进行语音识别。第 2 页点击“开始实时识别”，文字会保留在这里。')
const { correctionDecision, courseExcerpt } = useRoadshowSession()
const secondaryText = computed(() => (correctionDecision.value === 'accepted' ? newDemoData.secondary.suggestion : newDemoData.secondary.original) + '\n\n' + ({pending:'等待用户确认',accepted:'本次已采纳',rejected:'本次选择保留原句'})[correctionDecision.value] + ' · 人工缺字情境\n原始课堂转写始终不变。')
const noteText = computed(() => {
  const lines = segments.value.filter(s => pinned.value.includes(s.id)).map(s => `「${s.raw}」\n来源：本场 ${s.time}`)
  if (courseExcerpt.value) lines.push(`「${courseExcerpt.value}」\n来源：L03 原始句 148 · ${newDemoData.secondary.sourceTime}`)
  return lines.join('\n\n') || '点击定稿句旁的“＋”，或在来源页摘录，原句和来源会留在这里。'
})
</script>

<template>
  <section class="demo-page closing-system-page">
    <header class="demo-heading"><div><p class="demo-eyebrow">07 · 课堂留下什么</p><h1>一场课结束，留下的是一条可回到的路径。</h1><p class="demo-subtitle">Focaus 把实时识别、可信订正、课件对象、课堂笔记和来源记录放在同一个课堂工作区里。</p></div><span class="demo-status-pill is-live"><i />{{ streamLabel }}</span></header>
    <div class="closing-system-grid">
      <article class="closing-system-card"><div class="closing-card-top"><span class="layer-index">01</span><div><p class="demo-kicker">RAW</p><h2>{{ mode === 'live' ? '本场识别' : '彩排回放' }}</h2></div><span class="layer-lock">🔒</span></div><pre>{{ rawText }}</pre><small>本场时间戳 · 保留服务返回原文</small></article>
      <article class="closing-system-card is-accent"><div class="closing-card-top"><span class="layer-index">02</span><div><p class="demo-kicker">SECONDARY</p><h2>课程订正示例</h2></div><span class="layer-lock">review</span></div><pre>{{ secondaryText }}</pre><small>人工准备 · 关键改动需采纳</small></article>
      <article class="closing-system-card"><div class="closing-card-top"><span class="layer-index">03</span><div><p class="demo-kicker">NOTES</p><h2>本场摘录</h2></div><span class="layer-lock">↗</span></div><pre>{{ noteText }}</pre><small>手动摘录原句 · 不冒充自动生成笔记</small></article>
    </div>
    <div class="closing-system-footer"><div class="closing-architecture"><span>麦克风</span><i>→</i><span>实时 ASR</span><i>→</i><span>六窗工作区</span><i>→</i><span>可追溯材料</span></div><div class="closing-actions"><button v-if="isStreaming" type="button" class="demo-secondary-button" @click="stop">停止现场识别</button><button type="button" class="demo-secondary-button" @click="reset">清空本场记录</button><button type="button" class="demo-primary-button" @click="openRealDemo">进入真实 Demo ↗</button></div></div>
  </section>
</template>

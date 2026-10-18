<script setup lang="ts">
import { computed, ref } from 'vue'
import { useLivePitchStream } from '../composables/useLivePitchStream'

const paused = ref(false)
const { visibleSegments, secondarySegments, liveNotes, streamLabel } = useLivePitchStream()
const rawText = computed(() => visibleSegments.value.map((segment) => `${segment.time}  ${segment.raw}`).join('\n'))
const secondaryText = computed(() => secondarySegments.value.map((segment) => `${segment.time}  ${segment.text}`).join('\n'))
const noteText = computed(() => liveNotes.value.map((segment) => segment.text).join('\n'))
</script>

<template>
  <section class="frame-page closing-page frame-night-page">
    <header class="page-heading"><div><p class="frame-kicker">终局闭环 · LIVE PITCH MEMORY</p><h1>一场路演结束，三层材料已经留下</h1></div><span class="page-source">{{ streamLabel }}</span></header>
    <div class="closing-grid" :class="{ paused }" @mouseenter="paused = true" @mouseleave="paused = false">
      <article class="closing-card"><div class="closing-card-head"><p class="frame-kicker">01</p><h2>本场路演 · 原始转写</h2><span class="closing-status">raw</span></div><div class="closing-scroll"><pre>{{ rawText }}</pre></div><small>保留时间戳 · 可回到音频</small></article>
      <article class="closing-card closing-card-accent"><div class="closing-card-head"><p class="frame-kicker">02</p><h2>本场路演 · 二次转写</h2><span class="closing-status">secondary</span></div><div class="closing-scroll"><pre>{{ secondaryText }}</pre></div><small>语义顺滑 · 原始层保持不变</small></article>
      <article class="closing-card"><div class="closing-card-head"><p class="frame-kicker">03</p><h2>本场路演 · 实时笔记</h2><span class="closing-status">notes</span></div><div class="closing-scroll"><pre>{{ noteText }}</pre><div class="closing-flow"><span>声音</span><i>→</i><span>结构</span><i>→</i><span>行动</span></div></div><small>悬停暂停滚动 · 结构图随内容生成</small></article>
    </div>
    <p class="closing-footer">Focaus 把“听得准、记更好、查得明”连成一条可回到当前讲授的路径。</p>
  </section>
</template>

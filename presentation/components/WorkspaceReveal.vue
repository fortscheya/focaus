<script setup lang="ts">
import { computed, ref } from 'vue'
import { gsap } from 'gsap'
import { handoutMode, newDemoData, openRealDemo } from '../data/newDemoData'

const shell = ref<HTMLElement | null>(null)
const revealStep = ref(handoutMode ? 2 : 0)
const showAll = computed(() => revealStep.value >= 2)

const primaryPanes = computed(() => newDemoData.panes.slice(0, 2))
const secondaryPanes = computed(() => newDemoData.panes.slice(2))

function reveal(kind: 'transcript' | 'all') {
  revealStep.value = kind === 'transcript' ? Math.max(revealStep.value, 1) : 2
  requestAnimationFrame(() => {
    if (!shell.value || window.matchMedia('(prefers-reduced-motion: reduce)').matches) return
    gsap.fromTo(shell.value.querySelectorAll('.workspace-pane:not(.is-primary)'), { opacity: 0, y: 12, scale: 0.98 }, { opacity: 1, y: 0, scale: 1, duration: 0.45, stagger: 0.08, ease: 'power2.out' })
  })
}
</script>

<template>
  <section ref="shell" class="demo-page workspace-page">
    <header class="demo-heading">
      <div>
        <p class="demo-eyebrow">03 · 课堂工作区</p>
        <h1>同一堂课，需要的内容就在一起。</h1>
        <p class="demo-subtitle">课件、实时字幕、原始底稿、二次转写、笔记和来源，都围绕同一堂课保持独立可展开。</p>
      </div>
      <div class="demo-heading-side"><span class="demo-status-pill"><i />{{ newDemoData.course }}</span><span class="demo-micro-copy">{{ newDemoData.lesson }}</span></div>
    </header>

    <div class="workspace-shell">
      <aside class="workspace-rail"><strong>focaus</strong><span>课程</span><div class="rail-course">深<br><small>Deep<br>Learning</small></div><div class="rail-spacer" /><span>system</span></aside>
      <div class="workspace-desktop">
        <div class="workspace-topbar"><div><strong>{{ newDemoData.lesson }}</strong><span>{{ newDemoData.course }} · 真实资料 Demo</span></div><div class="workspace-recording"><i />课堂布局示意<time>85:46</time></div><button type="button" class="workspace-top-button" @click="openRealDemo">打开真实工作区 ↗</button></div>
        <div class="workspace-caption"><span>课堂草稿 · 内容可以独立展开</span><span>与当前 L03 Demo 对应</span></div>
        <div class="workspace-grid">
          <article v-for="pane in primaryPanes" :key="pane.id" class="workspace-pane is-primary" :class="`tone-${pane.tone}`">
            <header><span class="window-dots">× ↗</span><strong>{{ pane.label }}</strong><small>{{ newDemoData.lesson }}</small></header>
            <div class="workspace-pane-body">
              <template v-if="pane.id === 'material'"><div class="material-sheet"><span class="sheet-tag">L03 · 128 PAGES</span><img class="workspace-real-page" src="/course/l03-p079.png" alt="L03 第 79 页真实课件" /></div></template>
              <template v-else><div class="live-pane-status">现场字幕独立于已有课堂资料</div><p class="workspace-live-hint">点击第 2 页的“开始实时识别”，本次说话才会进入实时字幕。</p><small class="pane-muted">已准备的原始转写不会回放成现场字幕。</small></template>
            </div>
          </article>
          <TransitionGroup name="pane-reveal" tag="div" class="workspace-secondary-grid">
            <article v-if="revealStep >= 1" v-for="pane in secondaryPanes.slice(0, 2)" :key="pane.id" class="workspace-pane" :class="`tone-${pane.tone}`">
              <header><span class="window-dots">× ↗</span><strong>{{ pane.label }}</strong><small>{{ pane.detail }}</small></header>
              <div class="workspace-pane-body"><div class="pane-lock" v-if="pane.id === 'raw'">🔒 原始时间轴 · 只读</div><div class="workspace-line" v-for="segment in newDemoData.rawSegments.slice(0, 2)" :key="segment.id"><time>{{ segment.time }}</time><p>{{ pane.id === 'raw' ? segment.text : segment.text.replace('这个', '') }}</p></div></div>
            </article>
            <article v-if="showAll" v-for="pane in secondaryPanes.slice(2)" :key="pane.id" class="workspace-pane" :class="`tone-${pane.tone}`">
              <header><span class="window-dots">× ↗</span><strong>{{ pane.label }}</strong><small>{{ pane.detail }}</small></header>
              <div class="workspace-pane-body"><template v-if="pane.id === 'note'"><h3>Xavier 初始化</h3><p class="note-copy">随机性与尺度各自解决不同问题。</p><span class="source-link">课件 p.79 ↗</span></template><template v-else><div class="source-count">1 <small>个人确认入口</small></div><p class="pane-muted">来源与订正都保留审计路径。</p></template></div>
            </article>
          </TransitionGroup>
        </div>
        <nav class="workspace-dock"><button v-for="pane in newDemoData.panes" :key="pane.id" type="button" :class="{ active: pane.id === 'material' || pane.id === 'live' || showAll }">{{ pane.label }}</button><span>{{ showAll ? '6 / 6' : revealStep === 1 ? '4 / 6' : '2 / 6' }}</span></nav>
      </div>
    </div>
    <div class="workspace-actions"><button type="button" class="demo-secondary-button" :disabled="revealStep >= 1" @click="reveal('transcript')">展开听写链</button><button type="button" class="demo-primary-button" :disabled="showAll" @click="reveal('all')">展开完整工作区</button><span>{{ showAll ? '同一课堂，六窗按需展开' : '按讲述顺序逐层点亮' }}</span></div>
  </section>
</template>

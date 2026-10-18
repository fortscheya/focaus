<script setup lang="ts">
import { computed, nextTick, ref } from 'vue'
import { gsap } from 'gsap'
import { handoutMode, newDemoData } from '../data/newDemoData'
import { useRoadshowSession } from '../composables/useRoadshowSession'

const stage = ref<HTMLElement | null>(null)
const suggestionOpen = ref(handoutMode)
const { correctionDecision } = useRoadshowSession()
const accepted = computed(() => correctionDecision.value === 'accepted')
const actionNote = ref('关键事实先过门禁')

function openSuggestion() {
  suggestionOpen.value = !suggestionOpen.value
  nextTick(() => {
    if (!stage.value || !suggestionOpen.value || window.matchMedia('(prefers-reduced-motion: reduce)').matches) return
    gsap.fromTo(stage.value.querySelector('.correction-popover'), { opacity: 0, y: 8, scale: 0.97 }, { opacity: 1, y: 0, scale: 1, duration: 0.28, ease: 'power2.out' })
  })
}

function accept() {
  correctionDecision.value = 'accepted'
  suggestionOpen.value = false
  actionNote.value = '已采纳到二次层 · 原始底稿未改变'
}

function reject() {
  correctionDecision.value = 'rejected'
  suggestionOpen.value = false
  actionNote.value = '保留原句 · 建议进入审计记录'
}
</script>

<template>
  <section ref="stage" class="demo-page trust-page">
    <header class="demo-heading">
      <div><p class="demo-eyebrow">02 · 可信订正</p><h1>原话留下，关键订正由你决定。</h1><p class="demo-subtitle">以 Sigmoid 漏掉“不”字为例：事实的正反可能只差一个字，二次转写保留建议，等你确认。</p></div>
      <span class="demo-status-pill is-safe"><i />{{ actionNote }}</span>
    </header>
    <div class="trust-grid">
      <section class="demo-card raw-layer">
        <div class="demo-card-head"><div><p class="demo-kicker">RAW TRANSCRIPT</p><h2>原始底稿</h2></div><span class="lock-badge">🔒 READ ONLY</span></div>
        <div class="raw-timeline"><time>{{ newDemoData.secondary.sourceTime }} · 原始句 148</time><p>{{ newDemoData.secondary.rawText }}</p></div>
        <div class="raw-proof"><span>原始时间轴</span><strong>L03 · 原始转写</strong><span>保留来源锚点</span></div>
        <p class="layer-footnote">真实原始句本来就包含“不是”。右侧刻意移除“不”字，用于演示关键订正，不能当作实测漏字。</p>
      </section>
      <section class="demo-card secondary-layer">
        <div class="demo-card-head"><div><p class="demo-kicker">SECONDARY TRANSCRIPT</p><h2>二次转写</h2></div><span class="review-badge">{{ accepted ? '已采纳 1 处' : '1 处待确认' }}</span></div>
        <p class="secondary-context">{{ newDemoData.secondary.demoLabel }}</p>
        <button type="button" class="redline-sentence" :class="{ accepted }" @click="openSuggestion"><del v-if="!accepted">Sigmoid 的输出是以零为中心的。</del><span v-else>Sigmoid 的输出不是以零为中心的。</span><small v-if="!accepted">点击查看订正依据</small></button>
        <div class="secondary-connection"><span>原始句 148 / 课件 p.35</span><i>→</i><span>建议 {{ accepted ? '已采纳' : '等待采纳' }}</span><i>→</i><span>采纳记录</span></div>
        <div v-if="suggestionOpen" class="correction-popover" role="dialog" aria-label="订正建议"><p class="demo-kicker">CRITICAL CORRECTION</p><h3>建议修改为：Sigmoid 的输出不是以零为中心的。</h3><p>{{ newDemoData.secondary.reason }}</p><div class="correction-actions"><button type="button" class="demo-secondary-button" @click="reject">保留原句</button><button type="button" class="demo-primary-button" @click="accept">采纳订正</button></div></div>
        <div v-if="accepted" class="accepted-note"><span>✓</span> 订正进入二次层，原始句保持原样。</div>
      </section>
    </div>
    <div class="trust-flow"><span class="trust-flow-active">原始事实</span><i>→</i><span :class="{ active: suggestionOpen || accepted }">候选订正</span><i>→</i><span :class="{ active: accepted }">用户采纳</span><i>→</i><span :class="{ active: accepted }">笔记沉淀</span></div>
    <p class="demo-footer-note">本页为人工缺字情境演示。真实 Demo 支持编辑建议、采纳、保留原句和撤销。</p>
  </section>
</template>

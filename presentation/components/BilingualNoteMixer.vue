<script setup lang="ts">
import { computed, ref } from 'vue'
import FrameChart from './FrameChart.vue'
import { deepLearningCourseData } from '../data/deepLearningCourseData'

const mixLevel = ref(3)
const added = ref(false)
const noteText = computed(() => deepLearningCourseData.noteLevels[mixLevel.value - 1])
const levelLabel = computed(() => `Level ${mixLevel.value}`)
</script>

<template>
  <section class="frame-page mixer-page">
    <header class="page-heading"><div><p class="frame-kicker">02 · 记更好</p><h1>从中文，to English，笔记可以以不同的 level mix</h1><p class="page-subtitle">适配前沿学术课堂的英文原词引用，保留用户真正需要的阅读习惯。</p></div><span class="page-source">5 档中英混合 · Level 3 推荐</span></header>
    <div class="mixer-grid">
      <section class="mixer-source-panel">
        <div class="panel-label"><span class="frame-kicker">REFERENCE MATERIAL</span><span>L08 · 第 4 页</span></div>
        <p>{{ deepLearningCourseData.referenceExcerpt }}</p>
        <div class="source-highlight"><span>Backpropagation</span><span>Gradient Vanishing</span><span>Learning Rate</span></div>
        <button class="frame-button frame-button-primary" type="button" @click="added = true">＋ 一键添加至笔记</button>
        <span v-if="added" class="action-confirm">✓ 已加入当前课堂笔记，正在生成可视化</span>
      </section>
      <section class="mixer-note-panel">
        <div class="mixer-controls"><div><p class="frame-kicker">LANGUAGE MIX</p><strong>{{ levelLabel }}</strong></div><output>{{ mixLevel }}/5</output></div>
        <input v-model.number="mixLevel" class="frame-slider" type="range" min="1" max="5" step="1" aria-label="中英混合等级" />
        <div class="level-labels"><span>中文</span><span>推荐</span><span>English</span></div>
        <article class="note-paper"><p class="frame-kicker">LIVE CLASS NOTE</p><p>{{ noteText }}</p><span class="note-meta">学习率 · 卷积核 · 当前主题</span></article>
        <div v-if="added" class="generated-visual"><div class="generated-visual-head"><strong>自动生成：训练过程可视化</strong><span>editable chart</span></div><FrameChart mode="canvas" :option="deepLearningCourseData.learningCurve" /></div>
        <div v-else class="visual-placeholder"><span>+</span><p>加入参考资料后，生成算法流程或网络结构图</p></div>
      </section>
    </div>
  </section>
</template>

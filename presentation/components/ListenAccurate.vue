<script setup lang="ts">
import { ref } from 'vue'
import { deepLearningCourseData } from '../data/deepLearningCourseData'

const mode = ref<'polished' | 'raw'>('polished')
const activeCorrection = ref<string | null>(null)
const accepted = ref<string[]>([])

function acceptCorrection(id: string) {
  if (!accepted.value.includes(id)) accepted.value = [...accepted.value, id]
  activeCorrection.value = null
}
</script>

<template>
  <section class="frame-page listen-page">
    <header class="page-heading">
      <div><p class="frame-kicker">01 · 听得准</p><h1>资料先被理解，语音才有上下文</h1></div>
      <span class="page-source">{{ deepLearningCourseData.sourceLabel }}</span>
    </header>
    <div class="listen-grid">
      <section class="course-doc-panel">
        <div class="doc-toolbar"><span class="doc-file">L08 · convolution.pdf</span><span class="doc-page">第 4 / 82 页</span></div>
        <div class="doc-preview">
          <div class="doc-preview-heading"><span>卷积层与局部连接</span><small>结构化重构视图</small></div>
          <div class="network-diagram" aria-label="卷积层结构化示意图">
            <div class="network-column"><span v-for="label in ['输入图像', '局部窗口', '输出特征图']" :key="label">{{ label }}</span></div>
            <div class="network-arrow">↗<br>↘</div>
            <div class="network-column network-column-accent"><span>3 × 3 kernel</span><span>stride = 1</span><span>shared weights</span></div>
          </div>
          <table class="doc-table"><thead><tr><th>对象</th><th>含义</th><th>参数</th></tr></thead><tbody><tr v-for="row in deepLearningCourseData.documentRows" :key="row[0]"><td>{{ row[0] }}</td><td>{{ row[1] }}</td><td>{{ row[2] }}</td></tr></tbody></table>
        </div>
        <div class="keyword-row"><span class="frame-kicker">上下文热词</span><button v-for="term in deepLearningCourseData.terms" :key="term" type="button" class="keyword-tag">{{ term }}</button></div>
      </section>

      <section class="correction-panel">
        <div class="segmented-control" role="tablist" aria-label="转写层级"><button type="button" :class="{ active: mode === 'polished' }" @click="mode = 'polished'">二次精修转写</button><button type="button" :class="{ active: mode === 'raw' }" @click="mode = 'raw'">原始语音转写</button></div>
        <div class="transcript-paper">
          <p class="frame-kicker">SECONDARY TRANSCRIPT</p>
          <p v-if="mode === 'polished'" class="transcript-body">{{ deepLearningCourseData.polishedTranscript }}</p>
          <p v-else class="transcript-body">{{ deepLearningCourseData.rawTranscript }}</p>
          <div class="redline-sentence">
            <span>当前建议：</span>
            <button v-if="!accepted.includes('stride')" type="button" class="redline-word" @click="activeCorrection = activeCorrection === 'stride' ? null : 'stride'">步幅为二</button>
            <span v-else class="accepted-word">步幅为一</span>
            <span>，输出尺寸随后变小。</span>
          </div>
          <div v-if="activeCorrection === 'stride'" class="correction-popover"><p class="frame-kicker">建议修改为</p><strong>步幅为一</strong><p>{{ deepLearningCourseData.corrections[0].reason }}</p><button class="frame-button frame-button-primary" type="button" @click="acceptCorrection('stride')">一键替换正文</button></div>
        </div>
        <div class="radar-grid"><article v-for="alert in deepLearningCourseData.alerts" :key="alert.label" class="radar-card"><strong>{{ alert.label }}</strong><p>{{ alert.text }}</p></article></div>
      </section>
    </div>
  </section>
</template>

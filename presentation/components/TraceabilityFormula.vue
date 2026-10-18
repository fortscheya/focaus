<script setup lang="ts">
import { ref } from 'vue'
import { deepLearningCourseData, type TraceNode } from '../data/deepLearningCourseData'

const selectedTrace = ref('note')
const selectedScope = ref<'adam' | 'fgsm' | null>(null)
const traceNode = (id: string): TraceNode => deepLearningCourseData.traceNodes.find((node) => node.id === id) ?? deepLearningCourseData.traceNodes[0]
</script>

<template>
  <section class="frame-page trace-page">
    <header class="page-heading"><div><p class="frame-kicker">03 · 查得明</p><h1>每个结论，都能回到它的事实来源</h1></div><span class="page-source">双链路溯源 · 局部变量作用域</span></header>
    <div class="trace-grid">
      <section class="trace-chain-panel">
        <p class="frame-kicker">TRACEABILITY CHAIN</p>
        <div class="trace-chain" aria-label="课堂内容双链路溯源">
          <button v-for="node in deepLearningCourseData.traceNodes.slice(0, 4)" :key="node.id" type="button" class="trace-node" :class="{ active: selectedTrace === node.id }" @click="selectedTrace = node.id"><strong>{{ node.label }}</strong><span>{{ node.id === 'raw' ? '00:05.1' : node.id === 'audio' ? '可回放' : '可定位' }}</span></button>
        </div>
        <div class="trace-route"><span>笔记</span><i>→</i><span>二次转写</span><i>→</i><span>原始时间</span><i>→</i><span>音频</span></div>
        <div class="trace-route secondary"><span>笔记</span><i>→</i><span>重构课件</span><i>→</i><span>原页 / 原表</span></div>
        <div class="trace-detail"><p class="frame-kicker">当前事实层</p><strong>{{ traceNode(selectedTrace).label }}</strong><p>{{ traceNode(selectedTrace).detail }}</p></div>
      </section>
      <section class="formula-panel">
        <p class="frame-kicker">FORMULA LOCAL SCOPE</p>
        <div class="formula-block"><span class="formula-label">公式 1 · Adam</span><p>θ<sub>t+1</sub> = θ<sub>t</sub> − <span>η</span> / (√v̂<sub>t</sub> + <button type="button" class="epsilon-hotspot" @click="selectedScope = 'adam'">ε</button>) · m̂<sub>t</sub></p></div>
        <div class="formula-block"><span class="formula-label">公式 2 · FGSM</span><p>x<sup>adv</sup> = x + <button type="button" class="epsilon-hotspot" @click="selectedScope = 'fgsm'">ε</button> · sign(∇<sub>x</sub>J(θ, x, y))</p></div>
        <div class="scope-callout"><span>同名符号</span><strong>ε</strong><span>各自只在所属公式内生效</span></div>
        <div v-if="selectedScope" class="scope-popover"><p class="frame-kicker">LOCAL VARIABLE ε</p><strong>{{ selectedScope === 'adam' ? '公式 1 · Adam' : '公式 2 · FGSM' }}</strong><p>{{ deepLearningCourseData.formulaDefinitions[selectedScope] }}</p><button type="button" class="frame-button frame-button-secondary" @click="selectedScope = null">关闭定义</button></div>
      </section>
    </div>
  </section>
</template>

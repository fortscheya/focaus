<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { newDemoData } from '../data/newDemoData'

const input = newDemoData.convolution.input.map((row) => [...row])
const kernel = newDemoData.convolution.kernel.map((row) => [...row])
const selected = ref({ row: 0, col: 0 })
const termIndex = ref(-1)
const playing = ref(false)
let playTimer: number | undefined
const lab = ref<HTMLElement | null>(null)
let observer: IntersectionObserver | undefined

const output = computed(() => Array.from({ length: 3 }, (_, row) => Array.from({ length: 3 }, (_, col) => {
  return kernel.reduce((sum, kernelRow, kr) => sum + kernelRow.reduce((rowSum, weight, kc) => rowSum + input[row + kr][col + kc] * weight, 0), 0)
})))
const currentTerms = computed(() => kernel.flatMap((row, kr) => row.map((weight, kc) => {
  const inputRow = selected.value.row + kr
  const inputCol = selected.value.col + kc
  const value = input[inputRow][inputCol]
  return { input: value, weight, product: value * weight, inputRow, inputCol, kernelRow: kr, kernelCol: kc }
})))
const currentSum = computed(() => output.value[selected.value.row][selected.value.col])
const currentTerm = computed(() => currentTerms.value[termIndex.value])
const currentTermLabel = computed(() => currentTerm.value ? `输入 (${currentTerm.value.inputRow + 1}, ${currentTerm.value.inputCol + 1}) × 核 (${currentTerm.value.kernelRow + 1}, ${currentTerm.value.kernelCol + 1}) = ${currentTerm.value.product}` : `输出 (${selected.value.row + 1}, ${selected.value.col + 1}) = ${currentSum.value}`)

function selectOutput(row: number, col: number) {
  selected.value = { row, col }
  termIndex.value = -1
}

function stepTerm() {
  termIndex.value = termIndex.value >= currentTerms.value.length - 1 ? -1 : termIndex.value + 1
}

function togglePlay() {
  playing.value = !playing.value
  if (!playing.value) { if (playTimer) window.clearInterval(playTimer); playTimer = undefined; return }
  playTimer = window.setInterval(stepTerm, 620)
}

onMounted(() => {
  observer = new IntersectionObserver(entries => {
    if (!entries[0]?.isIntersecting && playing.value) togglePlay()
  })
  if (lab.value) observer.observe(lab.value)
})
onBeforeUnmount(() => { if (playTimer) window.clearInterval(playTimer); observer?.disconnect() })
</script>

<template>
  <section ref="lab" class="demo-page lab-page">
    <header class="demo-heading"><div><p class="demo-eyebrow">06 · CONTROLLED VISUALIZATION</p><h1>让卷积走一遍，比再读一遍更清楚。</h1><p class="demo-subtitle">点选输出格，看对应的输入窗口；再逐项播放乘加，看看每一个数字从哪里来。</p></div><span class="demo-status-pill is-safe"><i />2D 卷积探索</span></header>
    <div class="lab-layout">
      <section class="demo-card matrix-card"><div class="demo-card-head"><div><p class="demo-kicker">INPUT → WINDOW → OUTPUT</p><h2>让卷积窗口走一遍</h2></div><span class="demo-chip">本页确定性演算 · 演示参数</span></div><div class="matrix-scene"><div class="matrix-block"><span>输入矩阵</span><div class="matrix-grid input-grid"> <button v-for="(value, index) in input.flat()" :key="`i-${index}`" type="button" :class="{ window: Math.floor(index / 4) >= selected.row && Math.floor(index / 4) < selected.row + 2 && index % 4 >= selected.col && index % 4 < selected.col + 2 }">{{ value }}</button></div></div><b class="matrix-arrow">×</b><div class="matrix-block"><span>卷积核</span><div class="matrix-grid kernel-grid"><button v-for="(value, index) in kernel.flat()" :key="`k-${index}`" :class="{ term: currentTerm && termIndex === index }">{{ value }}</button></div></div><b class="matrix-arrow">=</b><div class="matrix-block"><span>输出特征图</span><div class="matrix-grid output-grid"><button v-for="(value, index) in output.flat()" :key="`o-${index}`" type="button" :aria-label="`选择输出 ${Math.floor(index / 3)+1},${index % 3+1}`" :class="{ current: Math.floor(index / 3) === selected.row && index % 3 === selected.col }" @click="selectOutput(Math.floor(index / 3), index % 3)">{{ value }}</button></div></div></div><div class="lab-step-bar"><button type="button" class="demo-secondary-button" @click="stepTerm">下一项乘加</button><button type="button" class="demo-primary-button" @click="togglePlay">{{ playing ? '暂停逐项计算' : '自动播放计算' }}</button><span>{{ currentTermLabel }}</span></div></section>
      <aside class="demo-card lab-inspector"><p class="demo-kicker">CALCULATION INSPECTOR</p><h2>当前输出格</h2><div class="inspector-value"><span>output[{{ selected.row + 1 }}, {{ selected.col + 1 }}]</span><strong>{{ currentSum }}</strong></div><div class="inspector-terms"><div v-for="(term, index) in currentTerms" :key="index" :class="{ active: index === termIndex }"><span>{{ term.input }}</span><b>×</b><span>{{ term.weight }}</span><i>=</i><strong>{{ term.product }}</strong></div></div><div class="inspector-footer"><span>stride 1</span><span>padding 0</span><span>bias 0</span></div></aside>
    </div>
    <p class="demo-footer-note">本页使用 4×4 输入与 2×2 卷积核，按互相关规则现场演算。完整 Demo 还可编辑输入、核、步幅和填充，由数学宿主验证。</p>
  </section>
</template>

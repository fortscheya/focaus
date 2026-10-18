<script setup lang="ts">
import { computed, nextTick, ref } from 'vue'
import { gsap } from 'gsap'
import { handoutMode, newDemoData } from '../data/newDemoData'
import { useRoadshowSession } from '../composables/useRoadshowSession'
const stage = ref<HTMLElement | null>(null)
const selected = ref(handoutMode ? 'raw' : 'note')
const pulse = ref(0)
const copied = ref(false)
const { courseExcerpt } = useRoadshowSession()
function copySource() { copied.value = true; courseExcerpt.value = newDemoData.secondary.rawText }
const nodes = [
  {id:'note',label:'课堂笔记',detail:'Sigmoid · 非零中心',icon:'≡'},
  {id:'secondary',label:'二次转写',detail:'保留订正与原始句的关系',icon:'✓'},
  {id:'raw',label:'原始句 148',detail:`${newDemoData.secondary.sourceTime} · 原转写时间`,icon:'◷'},
  {id:'material',label:'课件 p.35',detail:'Sigmoid function · 原页对照',icon:'▣'},
]
const node = computed(() => nodes.find(n => n.id === selected.value)!)
function choose(id: string) {
  selected.value = id; pulse.value++
  nextTick(() => {
    if (!stage.value || window.matchMedia('(prefers-reduced-motion: reduce)').matches) return
    gsap.fromTo(stage.value.querySelectorAll('.source-target-highlight'), {backgroundColor:'#e9d57655'}, {backgroundColor:'transparent',duration:.65,ease:'power2.out'})
  })
}
</script>
<template>
  <section ref="stage" class="demo-page source-page">
    <header class="demo-heading"><div><p class="demo-eyebrow">05 · 查得明</p><h1>一句笔记，能回到它的原句和原页。</h1><p class="demo-subtitle">从课堂笔记点开来源，找到原始句，再对照课件。修改内容时，也留下当时的依据。</p></div><span class="demo-status-pill"><i />L03 · 来源回跳</span></header>
    <div class="source-layout">
      <section class="demo-card source-chain-card"><div class="demo-card-head"><div><p class="demo-kicker">沿着同一事实</p><h2>点击一个来源</h2></div><span class="demo-chip">来源始终保留</span></div><div class="source-chain"><button v-for="item in nodes" :key="item.id" type="button" class="source-node" :class="{active: selected === item.id}" @click="choose(item.id)"><span class="source-node-icon">{{ item.icon }}</span><span><strong>{{ item.label }}</strong><small>{{ item.detail }}</small></span><b>↗</b></button></div><div class="source-detail source-target-highlight"><span class="demo-kicker">当前来源</span><strong>{{ node.label }}</strong><p>{{ node.detail }}</p><div class="source-route"><span>笔记</span><i>↔</i><span>原始句</span><i>↔</i><span>课件原页</span></div></div></section>
      <section class="demo-card source-preview"><div class="demo-card-head"><div><p class="demo-kicker">来源对照</p><h2>{{ node.label }}</h2></div><span class="source-count-badge">{{ pulse ? `已定位 ${pulse} 次` : '待选择' }}</span></div><Transition name="evidence" mode="out-in"><div :key="selected" class="source-preview-window source-target-highlight"><template v-if="selected === 'material'"><img class="evidence-page" src="/course/l03-p035.png" alt="真实 L03 第 35 页 Sigmoid function" /><small>128 页原课件中的第 35 页</small></template><template v-else-if="selected === 'raw'"><div class="source-preview-toolbar"><span>原始句 148</span><span>{{ newDemoData.secondary.sourceTime }} · 只读</span></div><h3>“它不是 zero-centered”</h3><p class="raw-quote">{{ newDemoData.secondary.rawText }}</p><small>直接来自已有原始转写，时间尚未逐句对齐录音。</small></template><template v-else-if="selected === 'secondary'"><div class="source-preview-toolbar"><span>人工缺字情境</span><span>关键订正需采纳</span></div><h3>“是”与“不是”，含义相反。</h3><p>{{ newDemoData.secondary.suggestion }}</p><small>此例刻意移除了“不”字；真实课堂原句原本包含否定。</small></template><template v-else><div class="source-preview-toolbar"><span>课程笔记示例</span><span>Sigmoid</span></div><h3>输出不以零为中心</h3><p>Sigmoid 的输出范围在 (0,1)，始终为正。可以从笔记继续查看老师的原始讲述和原课件。</p><button type="button" class="demo-secondary-button" @click="choose('raw')">查看原始依据 ↗</button></template></div></Transition><div class="source-callout"><span>{{ copied ? '已摘录 ✓' : '保留依据' }}</span><button type="button" class="demo-ghost-button" @click="copySource">{{ copied ? '已保留来源链接' : '摘录并保留来源' }}</button></div></section>
    </div>
    <p class="demo-footer-note">focaus 帮助接回讲授脉络；来源回跳用于核对某个事实。课堂资料和现场识别分别保留。</p>
  </section>
</template>

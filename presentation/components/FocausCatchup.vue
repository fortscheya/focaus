<script setup lang="ts">
import { nextTick, ref } from 'vue'
import { gsap } from 'gsap'
import { handoutMode, newDemoData } from '../data/newDemoData'
const page = ref<HTMLElement | null>(null)
const open = ref(handoutMode)
const followed = ref(false)
function openPanel() { open.value = true }
function follow() {
  followed.value = true
  nextTick(() => {
    if (!page.value || window.matchMedia('(prefers-reduced-motion: reduce)').matches) return
    gsap.fromTo(page.value.querySelectorAll('.catchup-target'), {backgroundColor: '#b6a5ff44'}, {backgroundColor: 'transparent', duration: .75, repeat: 1, yoyo: true, ease: 'power2.out'})
  })
}
</script>
<template>
  <section ref="page" class="demo-page catchup-page">
    <header class="demo-heading"><div><p class="demo-eyebrow">04 · 一键跟上</p><h1>先补上这十分钟，再回到老师眼前。</h1><p class="demo-subtitle">从“初始化太大或太小”接到 fan-in。回顾讲授脉络后，同时定位课件和笔记，继续听下去。</p></div><span class="demo-status-pill"><i />L03 · 固定演示时点 {{ newDemoData.focaus.time }}</span></header>
    <div class="catchup-stage">
      <section class="catchup-classroom demo-card">
        <div class="classroom-bar"><span>课堂当前画面</span><strong>Weight Initialization</strong><time>{{ newDemoData.focaus.time }}</time></div>
        <div class="classroom-body"><div class="classroom-slide"><img class="actual-page" src="/course/l03-p079.png" alt="真实 L03 课件第 79 页：Weight Initialization" /><p class="catchup-target">{{ followed ? '已定位 p.79：初始化尺度与 fan-in' : '课件已讲到：初始化尺度与 fan-in' }}</p></div><div class="classroom-note"><span class="demo-kicker">课堂笔记</span><h3 class="catchup-target">{{ followed ? newDemoData.focaus.note : '刚刚还在讨论：输入特征的尺度' }}</h3><p>{{ followed ? '权重全零无法打破对称性；随机权重的尺度仍需选择。此刻正在解释 fan-in。' : '你理解的进度落后了一段。需要补上为什么从数据预处理，走到了随机权重的尺度。' }}</p><span class="source-link">{{ followed ? '课件与笔记已同步定位 ✓' : '等待接回当前位置' }}</span></div></div>
        <div class="classroom-timeline"><span>75:46</span><div class="timeline-track"><i /><b style="left: 4%" /><b style="left: 92%" /><em style="left: 92%">当前</em></div><span>85:46</span></div>
      </section>
      <Transition name="focaus-panel" mode="out-in"><aside v-if="open" class="focaus-panel demo-card"><header><div><span class="demo-kicker">{{ newDemoData.focaus.window }}</span><h2>focaus</h2></div><button type="button" class="panel-close" aria-label="关闭 focaus 回顾" @click="open = false">×</button></header><div class="focaus-reading"><div class="focaus-range"><strong>{{ newDemoData.focaus.title }}</strong><span>课堂资料预先整理 · 不随现场语音推进</span></div><div class="focaus-reading-row"><span>01</span><p>权重全零无法打破对称性；随机数太小会让激活逐层趋近零，太大则可能进入饱和区。</p></div><div class="focaus-reading-row"><span>02</span><p>现在的问题是随机权重的尺度怎么选。老师开始解释 1/√fan-in，其中 fan-in 是输入特征维度。</p></div><aside class="focaus-prerequisite"><span>系统补充解释</span><p>输入零中心与权重全零含义不同：前者调整输入分布，后者会保留神经元之间的对称性。</p></aside></div><footer><div><span>演示中的当前位置</span><strong>{{ newDemoData.focaus.currentTopic }}</strong><small>p.79 · Xavier 初始化原理与方差推导</small></div><button type="button" class="demo-primary-button" :class="{ done: followed }" @click="follow">{{ followed ? '已跟上当前位置 ✓' : '跟上当前位置 →' }}</button></footer></aside><aside v-else class="focaus-invitation demo-card"><p class="demo-kicker">刚刚错过了什么？</p><h2>不从头重听，<br>补上理解的桥梁。</h2><p>回顾讲授脉络、必要前置和当前主题。</p><button type="button" class="demo-primary-button" @click="openPanel">focaus · 回顾刚刚十分钟</button></aside></Transition>
    </div>
    <div class="catchup-gesture"><span>理解回顾</span><strong>回顾十分钟 → 跟上当前位置</strong><small>固定 L03 场景 · 原转写时间未与音频逐句核验</small></div>
  </section>
</template>

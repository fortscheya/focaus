<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref } from 'vue'
import { gsap } from 'gsap'
import { handoutMode, newDemoData } from '../data/newDemoData'
const page = ref<HTMLElement | null>(null)
const stage = ref<HTMLElement | null>(null)
const splash = ref<HTMLElement | null>(null)
const open = ref(handoutMode)
const followed = ref(false)
const opening = ref(false)
const splashing = ref(false)
const origin = ref({ x: 0, y: 0 })
const droplets = Array.from({ length: 20 }, (_, i) => {
  const angle = Math.PI * (1.08 + i / 19 * .84)
  const reach = 112 + i % 4 * 19
  return { x: Math.cos(angle) * reach, y: Math.sin(angle) * reach, size: 6 + i % 4 * 2, rotation: angle * 180 / Math.PI + 45 }
})
let burst: gsap.core.Timeline | undefined
let visibility: IntersectionObserver | undefined

async function openPanel(event: MouseEvent) {
  if (opening.value || open.value) return
  if (!stage.value || window.matchMedia('(prefers-reduced-motion: reduce)').matches) { open.value = true; return }
  const button = event.currentTarget as HTMLButtonElement
  const buttonBounds = button.getBoundingClientRect()
  const stageBounds = stage.value.getBoundingClientRect()
  // Slidev scales the canvas; convert viewport coordinates back to slide units.
  origin.value = {
    x: (buttonBounds.left + buttonBounds.width / 2 - stageBounds.left) * stage.value.offsetWidth / stageBounds.width,
    y: (buttonBounds.top + buttonBounds.height / 2 - stageBounds.top) * stage.value.offsetHeight / stageBounds.height,
  }
  opening.value = true
  splashing.value = true
  await nextTick()
  if (!splash.value) { opening.value = false; open.value = true; return }
  burst?.kill()
  burst = gsap.timeline({ onComplete: () => { splashing.value = false; opening.value = false } })
  burst.fromTo(button, { scale: .95 }, { scale: 1.045, duration: .2, ease: 'back.out(2.4)' }, 0)
    .to(button, { scale: 1, duration: .16, ease: 'power2.out' }, .2)
    .fromTo(splash.value.querySelector('.focaus-splash-glint'), { scale: .2, autoAlpha: .85 }, { scale: 2.1, autoAlpha: 0, duration: .5, ease: 'power2.out' }, .03)
    .fromTo(splash.value.querySelectorAll('.focaus-splash-ripple'), { scale: .45, autoAlpha: .9 }, { scale: 2.5, autoAlpha: 0, duration: .8, stagger: .12, ease: 'power2.out' }, .06)
    .fromTo(splash.value.querySelector('.focaus-splash-crown'), { scaleX: .15, scaleY: .08, autoAlpha: 0 }, { scaleX: 1, scaleY: 1, autoAlpha: .85, duration: .25, ease: 'power2.out' }, .05)
    .to(splash.value.querySelector('.focaus-splash-crown'), { scaleX: 1.18, scaleY: .4, autoAlpha: 0, duration: .38, ease: 'power2.in' }, .3)
    .call(() => { open.value = true }, [], .23)
  splash.value.querySelectorAll('.focaus-splash-drop').forEach((element, i) => {
    const drop = droplets[i]
    burst!.fromTo(element, { x: 0, y: 0, scale: .1, autoAlpha: 0, rotation: drop.rotation }, {
      keyframes: [
        { x: drop.x * .65, y: drop.y, scale: 1, autoAlpha: 1, duration: .3, ease: 'power2.out' },
        { x: drop.x, y: drop.y + 90, scale: .35, autoAlpha: 0, duration: .65, ease: 'power2.in' },
      ],
    }, .06 + i % 3 * .025)
  })
}
function focusPanel(element: Element) {
  element.querySelector<HTMLButtonElement>('button')?.focus({ preventScroll: true })
}
function stopSplash() {
  burst?.kill()
  splashing.value = false
  opening.value = false
  const button = stage.value?.querySelector('.focaus-trigger')
  if (button) gsap.set(button, { clearProps: 'transform' })
}
onMounted(() => {
  visibility = new IntersectionObserver(entries => { if (!entries[0]?.isIntersecting) stopSplash() })
  if (stage.value) visibility.observe(stage.value)
})
onBeforeUnmount(() => { stopSplash(); visibility?.disconnect() })
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
    <div ref="stage" class="catchup-stage">
      <section class="catchup-classroom demo-card">
        <div class="classroom-bar"><span>课堂当前画面</span><strong>Weight Initialization</strong><time>{{ newDemoData.focaus.time }}</time></div>
        <div class="classroom-body"><div class="classroom-slide"><img class="actual-page" src="/course/l03-p079.png" alt="真实 L03 课件第 79 页：Weight Initialization" /><p class="catchup-target">{{ followed ? '已定位 p.79：初始化尺度与 fan-in' : '课件已讲到：初始化尺度与 fan-in' }}</p></div><div class="classroom-note"><span class="demo-kicker">课堂笔记</span><h3 class="catchup-target">{{ followed ? newDemoData.focaus.note : '刚刚还在讨论：输入特征的尺度' }}</h3><p>{{ followed ? '权重全零无法打破对称性；随机权重的尺度仍需选择。此刻正在解释 fan-in。' : '你理解的进度落后了一段。需要补上为什么从数据预处理，走到了随机权重的尺度。' }}</p><span class="source-link">{{ followed ? '课件与笔记已同步定位 ✓' : '等待接回当前位置' }}</span></div></div>
        <div class="classroom-timeline"><span>75:46</span><div class="timeline-track"><i /><b style="left: 4%" /><b style="left: 92%" /><em style="left: 92%">当前</em></div><span>85:46</span></div>
      </section>
      <Transition name="focaus-panel" mode="out-in" @after-enter="focusPanel"><aside v-if="open" class="focaus-panel demo-card"><header><div><span class="demo-kicker">{{ newDemoData.focaus.window }}</span><h2>focaus</h2></div><button type="button" class="panel-close" aria-label="关闭 focaus 回顾" @click="open = false">×</button></header><div class="focaus-reading"><div class="focaus-range"><strong>{{ newDemoData.focaus.title }}</strong><span>课堂资料预先整理 · 不随现场语音推进</span></div><div class="focaus-reading-row"><span>01</span><p>权重全零无法打破对称性；随机数太小会让激活逐层趋近零，太大则可能进入饱和区。</p></div><div class="focaus-reading-row"><span>02</span><p>现在的问题是随机权重的尺度怎么选。老师开始解释 1/√fan-in，其中 fan-in 是输入特征维度。</p></div><aside class="focaus-prerequisite"><span>系统补充解释</span><p>输入零中心与权重全零含义不同：前者调整输入分布，后者会保留神经元之间的对称性。</p></aside></div><footer><div><span>演示中的当前位置</span><strong>{{ newDemoData.focaus.currentTopic }}</strong><small>p.79 · Xavier 初始化原理与方差推导</small></div><button type="button" class="demo-primary-button" :class="{ done: followed }" @click="follow">{{ followed ? '已跟上当前位置 ✓' : '跟上当前位置 →' }}</button></footer></aside><aside v-else class="focaus-invitation demo-card"><p class="demo-kicker">刚刚错过了什么？</p><h2>不从头重听，<br>补上理解的桥梁。</h2><p>回顾讲授脉络、必要前置和当前主题。</p><button type="button" class="demo-primary-button focaus-trigger" :disabled="opening" :aria-expanded="open" @click="openPanel"><svg class="focaus-trigger-mark" aria-hidden="true" viewBox="0 0 24 24"><path d="M8 3H5a2 2 0 0 0-2 2v3m13-5h3a2 2 0 0 1 2 2v3M3 16v3a2 2 0 0 0 2 2h3m13-5v3a2 2 0 0 1-2 2h-3"/><circle cx="12" cy="12" r="3"/></svg><span>focaus · 回顾刚刚十分钟</span></button></aside></Transition>
      <div ref="splash" v-show="splashing" class="focaus-splash" :style="{ left: `${origin.x}px`, top: `${origin.y}px` }" aria-hidden="true">
        <span class="focaus-splash-glint" />
        <span v-for="ring in 2" :key="`ring-${ring}`" class="focaus-splash-ripple" />
        <svg class="focaus-splash-crown" viewBox="0 0 320 160"><path d="M30 136C54 126 56 110 39 83C76 112 86 115 82 57C103 99 114 112 128 80C143 51 141 32 152 14C150 63 166 76 177 98C191 119 203 99 215 61C211 107 225 119 257 91C244 118 255 126 290 136C232 150 89 150 30 136Z"/><path class="focaus-splash-crest" d="M45 136C98 119 216 119 275 136"/></svg>
        <i v-for="(drop, i) in droplets" :key="i" class="focaus-splash-drop" :style="{ width: `${drop.size}px`, height: `${drop.size * 1.55}px` }" />
      </div>
    </div>
    <div class="catchup-gesture"><span>理解回顾</span><strong>回顾十分钟 → 跟上当前位置</strong><small>固定 L03 场景 · 原转写时间未与音频逐句核验</small></div>
  </section>
</template>

<style scoped>
.catchup-stage { position: relative; }
.focaus-trigger {
  position: relative;
  display: inline-flex;
  align-items: center;
  gap: 10px;
  min-height: 48px;
  padding-inline: 17px;
  box-shadow: 0 0 0 5px #b6a5ff12, 0 6px 24px #b6a5ff24;
  transform-origin: center;
  transition: box-shadow .25s ease, background-color .25s ease;
}
.focaus-trigger:hover, .focaus-trigger:focus-visible { box-shadow: 0 0 0 7px #b6a5ff20, 0 8px 30px #b6a5ff44; }
.focaus-trigger:focus-visible { outline: 2px solid #bceef4; outline-offset: 5px; }
.focaus-trigger:disabled { opacity: 1; cursor: default; }
.focaus-trigger-mark { width: 21px; height: 21px; flex: none; fill: none; stroke: currentColor; stroke-width: 1.8; stroke-linecap: round; }
.focaus-splash { position: absolute; z-index: 8; width: 0; height: 0; pointer-events: none; color: #b6a5ff; }
.focaus-splash-glint { position: absolute; left: -42px; top: -42px; width: 84px; height: 84px; border-radius: 50%; background: radial-gradient(circle, #fff 0, #b6a5ff88 23%, #88e8ed33 45%, transparent 70%); }
.focaus-splash-ripple { position: absolute; left: -83px; top: -30px; width: 166px; height: 60px; border: 2px solid #c6baff; border-radius: 50%; box-shadow: 0 0 12px #b6a5ff55, inset 0 0 8px #b6a5ff22; }
.focaus-splash-ripple:nth-of-type(3) { border-color: #9fe6ed; height: 46px; top: -23px; }
.focaus-splash-crown { position: absolute; left: -160px; top: -136px; width: 320px; height: 160px; overflow: visible; transform-origin: 50% 85%; fill: #b6a5ff66; stroke: #ddd5ffbb; stroke-width: 1.3; stroke-linejoin: round; }
.focaus-splash-crest { fill: none; stroke: #e2ffff; stroke-width: 2; }
.focaus-splash-drop { position: absolute; left: -4px; top: -6px; border-radius: 70% 60% 65% 16%; background: linear-gradient(135deg, #f5ffff 5%, #b6a5ff 45%, #8d7ae3 100%); box-shadow: inset 1px 1px 2px #ffffffaa, 0 0 8px #b6a5ff66; }
.focaus-splash-drop:nth-of-type(3n) { background: linear-gradient(135deg, #edffff, #93e2ea 55%, #62b9d0); }
@media (prefers-reduced-motion: reduce) {
  .focaus-trigger { transition: none; }
  .focaus-splash { display: none !important; }
}
</style>

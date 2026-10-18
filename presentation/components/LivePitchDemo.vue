<script setup lang="ts">
import { computed, nextTick, ref } from 'vue'
import { gsap } from 'gsap'
import { useLivePitchStream } from '../composables/useLivePitchStream'

const stage = ref<HTMLElement | null>(null)
const expanded = ref(false)
const activeCard = ref(0)
const { visibleSegments, isStreaming, streamLabel, startStreaming, resetStreaming } = useLivePitchStream()

const cards = [
  { title: '近期讲解总结', body: '剖析了 2,135.77 万本科生在无进度条的高密度课堂中“走神 10 秒即掉线”的核心痛点。' },
  { title: '逻辑过渡桥梁', body: '通过现场按下 Focaus 按钮，演示实时语音流与课件进度的毫秒级对齐能力。' },
  { title: '👉 正在进入', body: '揭秘 Focaus 如何攻克“听得准、记更好、查得明”三大核心技术壁垒。' },
]

const visibleCardCount = computed(() => Math.min(cards.length, Math.max(activeCard.value + 1, visibleSegments.value.length > 2 ? 3 : visibleSegments.value.length)))

function activate() {
  expanded.value = true
  activeCard.value = 0
  startStreaming()
  nextTick(() => {
    if (!stage.value) return
    gsap.fromTo(stage.value.querySelectorAll('[data-reveal]'), { opacity: 0, y: 18 }, { opacity: 1, y: 0, duration: 0.45, stagger: 0.12, ease: 'power2.out' })
  })
}

function reset() {
  resetStreaming()
  expanded.value = false
  activeCard.value = 0
}

function showNextCard() {
  activeCard.value = Math.min(activeCard.value + 1, cards.length - 1)
}
</script>

<template>
  <section ref="stage" class="pitch-stage" :class="{ 'is-expanded': expanded }">
    <button class="frame-reset pitch-reset" type="button" @click="reset">重置</button>
    <div v-if="!expanded" class="pitch-statement">
      <div class="pitch-metric"><strong>2,135.77 万</strong><span>全国大学本科生共同困境</span></div>
      <h1>真实课堂，是一场没有进度条的直播。</h1>
      <div class="pitch-pain-points" v-click>
        <span>信息密度太高让你“记不全”</span>
        <span>推演流速太快让你“跟不上”</span>
        <span>走神 10 秒的代价，往往是听不懂剩下的整堂课</span>
      </div>
      <button class="frame-button frame-button-primary pitch-start" type="button" @click="activate">✦ Focaus · 一键跟上</button>
    </div>

    <div v-else class="pitch-split">
      <section class="pitch-live-column" data-reveal aria-labelledby="pitch-live-title">
        <div class="pitch-column-head">
          <div><p class="frame-kicker">LIVE STREAM</p><h2 id="pitch-live-title">现场语音流</h2></div>
          <span class="live-indicator"><i />{{ streamLabel }}</span>
        </div>
        <div class="pitch-wave" aria-hidden="true"><i v-for="index in 18" :key="index" :style="{ animationDelay: `${index * 40}ms` }" /></div>
        <div class="pitch-transcript-list" aria-live="polite">
          <article v-for="segment in visibleSegments.slice(0, 5)" :key="segment.id" class="pitch-transcript-row">
            <time>{{ segment.time }}</time><p>{{ segment.raw }}</p>
          </article>
        </div>
        <p class="pitch-stream-note">本地彩排兜底：浏览器只尝试打开麦克风，文字按预置路演流逐段到达。</p>
      </section>

      <section class="pitch-align-column" data-reveal aria-labelledby="pitch-align-title">
        <div class="pitch-align-status"><span>⚡ 决策模型课件对齐耗时：16ms</span><span>已锁定当前演讲节点</span></div>
        <h2 id="pitch-align-title">Focaus 10s 认知对齐</h2>
        <div class="pitch-callouts">
          <article v-for="(card, index) in cards.slice(0, visibleCardCount)" :key="card.title" class="frame-callout" :class="{ 'is-focus': index === 2 }">
            <p class="frame-kicker">0{{ index + 1 }}</p><h3>{{ card.title }}</h3><p>{{ card.body }}</p>
          </article>
        </div>
        <div class="pitch-align-actions">
          <button class="frame-button frame-button-primary" type="button" @click="showNextCard">继续演示</button>
          <span v-if="isStreaming">流式状态已跨页保活</span>
        </div>
      </section>
    </div>
  </section>
</template>

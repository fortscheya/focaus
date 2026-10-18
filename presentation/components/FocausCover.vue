<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { gsap } from 'gsap'

const cover = ref<HTMLElement | null>(null)

onMounted(() => {
  if (!cover.value) return
  gsap.fromTo(cover.value.querySelectorAll('[data-enter]'), { opacity: 0, y: 18 }, { opacity: 1, y: 0, duration: 0.8, stagger: 0.12, ease: 'power2.out' })
})
</script>

<template>
  <section ref="cover" class="frame-cover-real">
    <div class="cover-copy">
      <p class="frame-eyebrow" data-enter>F O C A U S</p>
      <h1 data-enter>Focaus</h1>
      <p class="cover-subtitle" data-enter>把实时语音、课件和笔记放回同一堂课</p>
      <p class="cover-meta" data-enter>真实 ASR 已接入。让每一次错过，都有一条回到当前讲授的路径。</p>
    </div>
    <div class="cover-visual" data-enter aria-label="声波与结构化课件的抽象示意">
      <svg viewBox="0 0 520 320" role="img" aria-hidden="true">
        <defs>
          <linearGradient id="coverWave" x1="0" x2="1">
            <stop offset="0" stop-color="#4056D5" stop-opacity="0.2" />
            <stop offset="1" stop-color="#4056D5" />
          </linearGradient>
        </defs>
        <rect x="266" y="36" width="190" height="238" rx="18" fill="#fff" stroke="#DDE1E7" />
        <path d="M296 88h116M296 116h78M296 150h130M296 178h92" stroke="#DDE1E7" stroke-width="10" stroke-linecap="round" />
        <path d="M296 222h56" stroke="#4056D5" stroke-width="10" stroke-linecap="round" />
        <path d="M32 174c28 0 28-58 56-58s28 116 56 116 28-78 56-78 28 40 56 40" fill="none" stroke="url(#coverWave)" stroke-width="7" stroke-linecap="round" />
        <circle cx="256" cy="174" r="8" fill="#4056D5" />
        <path d="M256 174h35" stroke="#4056D5" stroke-width="3" stroke-dasharray="6 8" />
        <circle cx="340" cy="222" r="10" fill="#16867C" opacity="0.16" />
        <circle cx="340" cy="222" r="4" fill="#16867C" />
      </svg>
    </div>
  </section>
</template>

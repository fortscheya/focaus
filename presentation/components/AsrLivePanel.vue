<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue'
import { newDemoData, openRealDemo } from '../data/newDemoData'
import { useRoadshowAsr } from '../composables/useRoadshowAsr'

const { mode, state, event, segments, partial, error, audioLevel, elapsed, pinned, isStreaming, statusText, streamLabel, start, stop, selectMode, pin, time } = useRoadshowAsr()
const transcript = ref<HTMLElement | null>(null)
const recent = computed(() => segments.value.slice(-3))
watch([() => segments.value.length, partial], () => nextTick(() => { if (transcript.value) transcript.value.scrollTop = transcript.value.scrollHeight }))
</script>

<template>
  <section class="demo-page asr-page">
    <header class="demo-heading">
      <div><p class="demo-eyebrow">01 · 听得准</p><h1>你说出的这一句，马上成为字幕。</h1><p class="demo-subtitle">麦克风现场输入，临时句持续更新；定稿后保留原始时间，翻页后仍可继续识别。</p></div>
      <div class="demo-heading-side"><span class="demo-status-pill" :class="{ 'is-live': state === 'recording' && mode === 'live' }"><i />{{ streamLabel }}</span><button type="button" class="demo-ghost-button" @click="openRealDemo">打开完整 Demo ↗</button></div>
    </header>
    <div class="asr-layout">
      <section class="demo-card asr-console">
        <div class="demo-card-head"><div><p class="demo-kicker">{{ mode === 'live' ? 'LIVE TRANSCRIPT' : 'REHEARSAL · 回放示例' }}</p><h2>实时字幕</h2></div><div class="asr-actions"><button type="button" class="demo-primary-button" :disabled="state === 'stopping'" :class="{ 'is-stop': isStreaming }" @click="isStreaming ? stop() : start()">{{ isStreaming ? (state === 'stopping' ? '等待尾句' : '停止识别') : (mode === 'live' ? '开始实时识别' : '开始彩排回放') }}</button></div></div>
        <div class="asr-mode" role="group" aria-label="语音演示模式"><button type="button" :aria-pressed="mode === 'live'" @click="selectMode('live')">现场识别</button><button type="button" :aria-pressed="mode === 'rehearsal'" @click="selectMode('rehearsal')">彩排回放</button><small>{{ mode === 'live' ? '仅在点击开始后使用麦克风' : '本地课堂片段 · 不打开麦克风' }}</small></div>
        <div class="asr-meter" aria-label="麦克风输入电平"><i v-for="n in 24" :key="n" :style="{ height: `${5 + Math.min(1, audioLevel * (1 + n % 4 / 3)) * 17}px`, opacity: mode === 'live' && audioLevel > 0.02 ? .85 : .2 }" /></div>
        <div class="asr-state-line"><span class="asr-dot" :class="`is-${state}`" /><strong>{{ statusText }}</strong><time>{{ time(elapsed) }}</time><span>{{ segments.length }} 个定稿句</span></div>
        <p v-if="error" class="asr-error" role="alert">{{ error }}</p>
        <div ref="transcript" class="asr-transcript" aria-live="polite">
          <TransitionGroup name="asr-row" tag="div"><article v-for="segment in recent" :key="segment.id" class="asr-transcript-row is-final"><time>{{ segment.time }}</time><div><p>{{ segment.raw }}</p><small>{{ mode === 'live' ? '本场识别' : '课堂回放' }} · final · 已定稿</small></div><button class="pin-quote" type="button" :aria-label="`摘录句子 ${segment.id}`" :disabled="pinned.includes(segment.id)" @click="pin(segment.id)">{{ pinned.includes(segment.id) ? '✓' : '＋' }}</button></article></TransitionGroup>
          <article v-if="partial" class="asr-transcript-row is-partial"><time>now</time><div><p>{{ partial }}<span class="asr-caret" /></p><small>partial · 尚未定稿</small></div><b>…</b></article>
          <div v-if="!segments.length && !partial && !error" class="asr-empty"><div><strong>{{ mode === 'live' ? '现场说一句与课堂有关的话' : '用已准备的课堂片段彩排' }}</strong><span>{{ mode === 'live' ? '例如：Sigmoid 的输出不是以零为中心的。' : '回放与真实识别始终分别标注。' }}</span></div></div>
        </div>
      </section>
      <aside class="demo-card asr-pipeline"><div class="demo-card-head"><div><p class="demo-kicker">从听见，到留下</p><h2>每个阶段都看得见</h2></div></div>
        <div class="asr-event-list"><div v-for="(step,index) in [{id:'ready',title:'连接就绪',body:'语音服务准备好后才开始发送声音'},{id:'partial',title:'文字跟着说话更新',body:'临时句可以变化，尚不作为定稿'},{id:'final',title:'定稿留在时间轴',body:'保留原句，可摘录到本场记录'}]" :key="step.id" class="asr-event-step" :class="{ active: event === step.id, done: index === 0 && segments.length > 0 }"><span class="asr-event-index">0{{ index+1 }}</span><div><strong>{{ step.title }}</strong><p>{{ step.body }}</p></div><i /></div></div>
        <div class="asr-rule"><span>现场 / 彩排</span><strong>{{ mode === 'live' ? '只显示服务返回的文字' : '已准备资料的回放' }}</strong><small>停止时先关闭麦克风，再等待最后一句定稿。</small></div><div class="asr-contract"><span>{{ newDemoData.region }}</span><strong>{{ newDemoData.model }}</strong></div>
      </aside>
    </div>
    <p class="demo-footer-note">真实识别需要正在运行的本地语音宿主；彩排可离线。点击定稿句旁的“＋”，把原句摘录到结尾。</p>
  </section>
</template>

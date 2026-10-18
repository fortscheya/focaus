<script setup lang="ts">
import { ref } from 'vue'
import FrameChart from './FrameChart.vue'

const rehearsalCount = ref(0)
</script>

<template>
  <div class="smoke-grid">
    <div class="smoke-card">
      <div class="smoke-card__heading">
        <span class="smoke-dot" aria-hidden="true" />
        <strong>Vue 交互已连接</strong>
      </div>
      <p>按钮、响应式状态和键盘焦点可作为后续交互页的基础。</p>
      <button type="button" @click="rehearsalCount += 1">
        彩排动作 {{ rehearsalCount }} 次
      </button>
    </div>
    <div class="smoke-card smoke-card--chart">
      <div class="smoke-card__heading">
        <span class="smoke-dot smoke-dot--accent" aria-hidden="true" />
        <strong>ECharts 已连接</strong>
      </div>
      <FrameChart mode="play" />
    </div>
  </div>
</template>

<style scoped>
.smoke-grid {
  display: grid;
  grid-template-columns: 0.8fr 1.2fr;
  gap: 24px;
  margin-top: 44px;
}

.smoke-card {
  min-height: 280px;
  padding: 28px;
  border: 1px solid var(--frame-border);
  border-radius: var(--frame-radius-card);
  background: var(--frame-surface);
}

.smoke-card--chart {
  min-width: 0;
}

.smoke-card__heading {
  display: flex;
  align-items: center;
  gap: 12px;
  font-size: 24px;
}

.smoke-card p {
  max-width: 380px;
  margin: 22px 0 28px;
  color: var(--frame-muted);
  font-size: 20px;
  line-height: 1.45;
}

.smoke-card button {
  min-height: 44px;
  padding: 0 18px;
  border: 0;
  border-radius: var(--frame-radius-control);
  color: var(--frame-on-accent);
  background: var(--frame-accent);
  font: inherit;
  font-size: 18px;
  cursor: pointer;
}

.smoke-dot {
  width: 12px;
  height: 12px;
  border-radius: 999px;
  background: #16867c;
}

.smoke-dot--accent {
  background: var(--frame-accent);
}
</style>

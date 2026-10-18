<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { use } from 'echarts/core'
import { GridComponent, LegendComponent, TooltipComponent } from 'echarts/components'
import { LineChart } from 'echarts/charts'
import { CanvasRenderer } from 'echarts/renderers'
import VChart from 'vue-echarts'

use([GridComponent, LegendComponent, TooltipComponent, LineChart, CanvasRenderer])

const props = withDefaults(defineProps<{ mode?: 'canvas' | 'nocturne' | 'play' | 'clay'; option?: Record<string, unknown> }>(), {
  mode: 'canvas',
})

const host = ref<HTMLElement | null>(null)
const chartColor = ref('#4056d5')

function readChartColor() {
  const scope = host.value?.closest('.slidev-layout') ?? host.value ?? document.documentElement
  const value = getComputedStyle(scope).getPropertyValue('--frame-chart-1').trim()
  if (value) chartColor.value = value
}

function handleResize() {
  readChartColor()
}

const chartOptions = computed(() => ({
  ...(props.option ?? {}),
  animationDuration: 450,
  color: (props.option?.color as string[] | undefined) ?? [chartColor.value, '#16867C'],
  grid: { left: 36, right: 18, top: 18, bottom: 28, containLabel: true },
  tooltip: { trigger: 'axis' },
  xAxis: {
    type: 'category',
    data: ['准备', '拆稿', '编排', '彩排'],
    axisLine: { lineStyle: { color: 'var(--frame-border)' } },
    axisLabel: { color: 'var(--frame-muted)', fontSize: 14 },
  },
  yAxis: {
    type: 'value',
    min: 0,
    max: 100,
    splitNumber: 4,
    splitLine: { lineStyle: { color: 'var(--frame-border)' } },
    axisLabel: { color: 'var(--frame-muted)', fontSize: 14 },
  },
  series: props.option?.series ?? [{
    type: 'line',
    smooth: false,
    data: [24, 48, 72, 88],
    symbolSize: 8,
    lineStyle: { width: 4 },
    areaStyle: { opacity: 0.08 },
  }],
}))

onMounted(() => {
  nextTick(readChartColor)
  window.addEventListener('resize', handleResize)
})

watch(() => props.mode, () => nextTick(readChartColor))

onBeforeUnmount(() => {
  window.removeEventListener('resize', handleResize)
})
</script>

<template>
  <div ref="host" class="frame-chart-host">
    <VChart class="frame-chart" :class="`frame-chart--${props.mode}`" :option="chartOptions" autoresize />
  </div>
</template>

<style scoped>
.frame-chart-host {
  width: 100%;
}

.frame-chart {
  width: 100%;
  height: 220px;
}
</style>

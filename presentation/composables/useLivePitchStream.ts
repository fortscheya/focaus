import { computed, ref } from 'vue'

export type LiveSegment = {
  id: string
  time: string
  raw: string
  secondary: string
  note: string
}

const fallbackSegments: LiveSegment[] = [
  { id: 'pitch-1', time: '00:00.8', raw: '真实课堂，是一场没有进度条的直播。', secondary: '真实课堂是一场没有进度条的直播。', note: '核心痛点：课堂信息持续到达，理解状态却没有回看入口。' },
  { id: 'pitch-2', time: '00:03.6', raw: '信息密度太高让你记不全，推演流速太快让你跟不上。', secondary: '信息密度与推演流速同时增加，学生很快失去当前讲授位置。', note: '问题被拆成两个状态：听得准，才能继续记；记得住，才能回头查。' },
  { id: 'pitch-3', time: '00:06.4', raw: '走神十秒，后面一整堂课就开始听不懂。', secondary: '十秒的理解断点，会放大成一整段无法定位的知识缺口。', note: 'Focaus 先找回当前讲授节点，再补上必要的衔接和前置知识。' },
  { id: 'pitch-4', time: '00:09.2', raw: '所以我现在按下 Focaus，一键跟上。', secondary: '按下 Focaus，系统把现场语音、课件进度和已有内容对齐。', note: '现场演示：保留原始时间轴，同时生成可读、可追溯的学习材料。' },
  { id: 'pitch-5', time: '00:12.0', raw: '它要解决的，是听得准、记更好、查得明。', secondary: '三条能力线分别处理识别、理解与追溯。', note: '接下来进入三页能力演示：资料重构、笔记调节、事实溯源。' },
]

const rawSegments = ref<LiveSegment[]>([])
const isStreaming = ref(false)
const micState = ref<'idle' | 'live' | 'fallback'>('idle')
const startedAt = ref<number | null>(null)
let timer: number | undefined
let micStream: MediaStream | null = null
let nextIndex = 0

function appendNext() {
  const segment = fallbackSegments[nextIndex]
  if (!segment) {
    if (timer) window.clearInterval(timer)
    timer = undefined
    return
  }
  rawSegments.value = [...rawSegments.value, segment]
  nextIndex += 1
}

async function tryLocalMic() {
  if (!navigator.mediaDevices?.getUserMedia) {
    micState.value = 'fallback'
    return
  }
  try {
    micStream = await navigator.mediaDevices.getUserMedia({ audio: true })
    if (isStreaming.value) micState.value = 'live'
  } catch {
    micState.value = 'fallback'
  }
}

function startStreaming() {
  if (isStreaming.value) return
  isStreaming.value = true
  micState.value = 'fallback'
  startedAt.value = Date.now()
  rawSegments.value = []
  nextIndex = 0
  appendNext()
  timer = window.setInterval(appendNext, 1150)
  void tryLocalMic()
}

function stopLocalMic() {
  micStream?.getTracks().forEach((track) => track.stop())
  micStream = null
}

function resetStreaming() {
  if (timer) window.clearInterval(timer)
  timer = undefined
  stopLocalMic()
  rawSegments.value = []
  nextIndex = 0
  isStreaming.value = false
  micState.value = 'idle'
  startedAt.value = null
}

const visibleSegments = computed(() => rawSegments.value.length ? rawSegments.value : fallbackSegments)
const secondarySegments = computed(() => visibleSegments.value.map((segment) => ({ ...segment, text: segment.secondary })))
const liveNotes = computed(() => visibleSegments.value.map((segment) => ({ ...segment, text: segment.note })))
const streamLabel = computed(() => micState.value === 'live' ? '本机麦克风已接入' : isStreaming.value ? '本地彩排流 · 只在浏览器内运行' : '等待启动')

export function useLivePitchStream() {
  return {
    fallbackSegments,
    rawSegments,
    secondarySegments,
    liveNotes,
    visibleSegments,
    isStreaming,
    micState,
    startedAt,
    streamLabel,
    startStreaming,
    resetStreaming,
  }
}

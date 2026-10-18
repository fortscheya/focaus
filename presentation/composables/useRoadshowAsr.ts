import { computed, ref } from 'vue'
import { demoHost, handoutMode, newDemoData } from '../data/newDemoData'

type State = 'idle' | 'connecting' | 'recording' | 'stopping' | 'error'
export type SpeechSegment = { id: string; time: string; raw: string; startMs: number; endMs: number }
type Capture = { stream: MediaStream; context: AudioContext; source: MediaStreamAudioSourceNode; node: AudioWorkletNode; mute: GainNode }
const mode = ref<'live' | 'rehearsal'>(handoutMode ? 'rehearsal' : 'live')
const state = ref<State>('idle')
const segments = ref<SpeechSegment[]>(handoutMode ? rehearsalSegments().slice(0,2) : [])
const partial = ref('')
const event = ref('idle')
const error = ref('')
const configured = ref<boolean | null>(null)
const audioLevel = ref(0)
const elapsed = ref(0)
const pinned = ref<string[]>([])
let generation = 0
let socket: WebSocket | null = null
let capture: Capture | null = null
let readyTimer: ReturnType<typeof setTimeout> | undefined
let stopTimer: ReturnType<typeof setTimeout> | undefined
let clock: ReturnType<typeof setInterval> | undefined
let replayTimer: ReturnType<typeof setInterval> | undefined
let flushDone: (() => void) | undefined

function time(ms: number) { return `${Math.floor(ms / 60000).toString().padStart(2, '0')}:${Math.floor(ms / 1000 % 60).toString().padStart(2, '0')}` }
function rehearsalSegments() {
  return newDemoData.rawSegments.slice(0, 3).map((s, i) => ({id: s.id, raw: s.text, time: s.time, startMs: i * 15000, endMs: (i+1)*15000}))
}
async function release() {
  const current = capture; capture = null
  if (!current) return
  current.node.port.onmessage = null
  current.stream.getTracks().forEach(track => track.stop())
  current.source.disconnect(); current.node.disconnect(); current.mute.disconnect()
  await current.context.close().catch(() => undefined)
  audioLevel.value = 0
}
function clearTimers() {
  clearTimeout(readyTimer); clearTimeout(stopTimer); clearInterval(clock); clearInterval(replayTimer)
  readyTimer = stopTimer = undefined; clock = replayTimer = undefined
}
function finish(message = '') {
  ++generation
  clearTimers()
  const current = socket; socket = null
  if (current && current.readyState < WebSocket.CLOSING) current.close()
  void release()
  partial.value = ''; audioLevel.value = 0
  error.value = message; state.value = message ? 'error' : 'idle'; event.value = message ? 'error' : 'stopped'
}
async function refreshConfig() {
  try {
    const r = await fetch(`${demoHost}/api/demo/status`, { cache: 'no-store', signal: AbortSignal.timeout(25000) })
    if (!r.ok) throw new Error()
    const status = await r.json()
    configured.value = status.configured === true
    return status as { configured: boolean; remainingSeconds?: number }
  } catch {
    configured.value = null
    throw new Error('语音宿主暂不可达，请使用配套启动器并确认真实 Demo 正常运行。')
  }
}
async function start() {
  if (['connecting', 'recording', 'stopping'].includes(state.value)) return
  segments.value = []; partial.value = ''; pinned.value = []; error.value = ''; elapsed.value = 0
  const run = ++generation
  if (mode.value === 'rehearsal') {
    state.value = 'recording'; event.value = 'ready'
    const sample = rehearsalSegments(); let index = 0
    partial.value = sample[0].raw.slice(0, 32)
    replayTimer = setInterval(() => {
      if (run !== generation) return
      if (index >= sample.length) { finish(); return }
      segments.value = [...segments.value, sample[index++]]; partial.value = ''; event.value = 'final'
      elapsed.value = index * 15000
      if (index < sample.length) setTimeout(() => { if (run === generation) { partial.value = sample[index].raw.slice(0, 32); event.value = 'partial' } }, 450)
    }, 1600)
    return
  }
  state.value = 'connecting'; event.value = 'connecting'
  let pendingStream: MediaStream | undefined
  let pendingContext: AudioContext | undefined
  try {
    const status = await refreshConfig()
    if (run !== generation) return
    if (!status.configured) throw new Error('实时 ASR 尚未配置，请先在真实 Demo 中完成配置。')
    if (status.remainingSeconds !== undefined && status.remainingSeconds <= 0) throw new Error('宿主的本轮音频额度已用完。可改用彩排回放继续讲解。')
    if (!navigator.mediaDevices?.getUserMedia) throw new Error('当前浏览器无法访问麦克风，请在本机 Chrome 打开演示。')
    pendingStream = await navigator.mediaDevices.getUserMedia({audio: {channelCount: 1, echoCancellation: true, noiseSuppression: true, autoGainControl: true}})
    if (run !== generation) { pendingStream.getTracks().forEach(t => t.stop()); return }
    pendingContext = new AudioContext()
    await pendingContext.audioWorklet.addModule('/pcm-worklet.js')
    if (run !== generation) { pendingStream.getTracks().forEach(t => t.stop()); await pendingContext.close(); return }
    const context = pendingContext
    const source = context.createMediaStreamSource(pendingStream)
    const node = new AudioWorkletNode(context, 'focaus-presentation-pcm', {numberOfInputs: 1, numberOfOutputs: 1, outputChannelCount: [1]})
    const mute = context.createGain(); mute.gain.value = 0
    capture = { stream: pendingStream, context, source, node, mute }
    const ws = new WebSocket(`${demoHost.replace('http:', 'ws:')}/api/demo/asr`)
    socket = ws
    readyTimer = setTimeout(() => { if (run === generation) finish('语音服务连接超时，麦克风已关闭。') }, 20000)
    node.port.onmessage = e => {
      if (run !== generation) return
      if (e.data.type === 'level') audioLevel.value = e.data.value
      if (e.data.type === 'flushed') { flushDone?.(); flushDone = undefined }
      if (e.data.type === 'pcm' && ws.readyState === WebSocket.OPEN) {
        if (ws.bufferedAmount > 1048576) { finish('发送积压，麦克风已停止。请检查连接后重试。'); return }
        ws.send(e.data.pcm)
      }
    }
    ws.onopen = () => { if (run === generation) ws.send(JSON.stringify({type: 'start', sampleRate: 16000, channels: 1})) }
    ws.onmessage = e => {
      if (run !== generation || socket !== ws) return
      let update: {type?: string; text?: string; segmentId?: string; startMs?: number; endMs?: number; code?: string}
      try { update = JSON.parse(String(e.data)) } catch { finish('语音服务返回格式异常，麦克风已关闭。'); return }
      if (update.type === 'ready') {
        clearTimeout(readyTimer); event.value = 'ready'; state.value = 'recording'
        const started = performance.now()
        clock = setInterval(() => elapsed.value = performance.now() - started, 200)
        source.connect(node); node.connect(mute); mute.connect(context.destination)
        void context.resume().catch(() => finish('音频处理未能启动。'))
      } else if (update.type === 'partial') { partial.value = update.text || ''; event.value = 'partial' }
      else if (update.type === 'final' && typeof update.text === 'string' && update.text.trim()) {
        const id = update.segmentId || `${update.startMs}-${update.endMs}`
        if (!segments.value.some(s => s.id === id)) segments.value.push({id, raw: update.text, time: time(update.startMs || 0), startMs: update.startMs || 0, endMs: update.endMs || 0})
        partial.value = ''; event.value = 'final'
      } else if (update.type === 'stopped') finish()
      else if (update.type === 'error') finish(update.code === 'BUDGET_EXHAUSTED' ? '本轮音频额度已用完，麦克风已关闭。' : '语音服务返回错误，麦克风已关闭。请到真实 Demo 检查状态。')
    }
    ws.onerror = () => { if (run === generation) finish('无法连接本地语音宿主，麦克风已关闭。') }
    ws.onclose = () => { if (run === generation && socket === ws) finish('实时连接已断开，麦克风已关闭。') }
  } catch (cause) {
    pendingStream?.getTracks().forEach(t => t.stop())
    if (pendingContext && pendingContext.state !== 'closed') void pendingContext.close()
    if (run === generation) finish(cause instanceof Error && cause.name !== 'NotAllowedError' ? cause.message : '麦克风权限未授予。可重新授权或选择彩排回放。')
  }
}
async function stop() {
  if (mode.value === 'rehearsal' || state.value === 'connecting') { finish(); return }
  if (state.value !== 'recording') return
  state.value = 'stopping'
  const run = generation
  if (capture) {
    const current = capture
    await new Promise<void>(resolve => { const timeout = setTimeout(resolve, 300); flushDone = () => {clearTimeout(timeout); resolve()}; current.node.port.postMessage('flush') })
  }
  await release()
  if (run !== generation) return
  if (socket?.readyState === WebSocket.OPEN) {
    socket.send(JSON.stringify({type: 'stop'}))
    stopTimer = setTimeout(() => finish('麦克风已关闭，服务尾句确认超时。'), 10000)
  } else finish()
}
function selectMode(value: 'live' | 'rehearsal') { finish(); mode.value = value; segments.value = []; pinned.value = []; elapsed.value = 0; error.value = ''; event.value = 'idle' }
function pin(id: string) { if (!pinned.value.includes(id)) pinned.value.push(id) }
function reset() { finish(); segments.value = []; pinned.value = []; elapsed.value = 0; error.value = ''; event.value = 'idle' }
if (typeof window !== 'undefined') window.addEventListener('pagehide', () => { if (socket?.readyState === WebSocket.OPEN) socket.send(JSON.stringify({type:'stop'})); finish() })
const isStreaming = computed(() => ['connecting','recording','stopping'].includes(state.value))
const statusText = computed(() => ({idle:'尚未开始',connecting:'正在连接',recording: mode.value === 'live' ? '真实识别中' : '彩排回放中',stopping:'等待尾句',error:'连接未完成'})[state.value])
const streamLabel = computed(() => mode.value === 'rehearsal' ? '彩排回放 · 本地已准备转写' : state.value === 'recording' ? '真实 ASR · 百炼北京' : configured.value === true ? 'ASR 已配置 · 等待开始' : '现场识别 · 点击开始')
export function useRoadshowAsr() { return {mode,state,event,segments,partial,error,configured,audioLevel,elapsed,pinned,isStreaming,statusText,streamLabel,time,start,stop,selectMode,pin,reset,refreshConfig} }

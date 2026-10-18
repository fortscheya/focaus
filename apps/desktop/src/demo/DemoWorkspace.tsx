import {useCallback, useEffect, useMemo, useRef, useState} from 'react';
import {Button} from '@heroui/react';
import {StatusBadge} from '../components/StatusBadge.tsx';

type DemoSegment = {
  id: string;
  time: string;
  end: string;
  text: string;
  page: number;
  confidence: string;
};

type DemoCorrection = {
  id: string;
  time: string;
  original: string;
  suggested: string;
  reason: string;
  risk: '普通' | '关键';
  state: 'pending' | 'accepted' | 'rejected';
};

const DEMO_SEGMENTS: DemoSegment[] = [
  {id: 's1', time: '00:00.8', end: '00:04.6', text: '卷积层会把局部窗口映射到新的特征空间。', page: 4, confidence: '0.96'},
  {id: 's2', time: '00:05.1', end: '00:09.8', text: '窗口向右移动一个步幅，输出位置就得到一个新的响应值。', page: 4, confidence: '0.94'},
  {id: 's3', time: '00:10.4', end: '00:14.9', text: '这里的权重共享让同一组卷积核可以识别不同位置的局部模式。', page: 5, confidence: '0.92'},
  {id: 's4', time: '00:15.5', end: '00:19.3', text: '如果步幅变大，输出尺寸会变小，但计算范围会更快覆盖整张图。', page: 5, confidence: '0.90'},
  {id: 's5', time: '00:20.0', end: '00:24.8', text: '课件中的公式可以重构成可编辑对象，原页锚点仍然保留。', page: 6, confidence: '0.95'},
];

const DEMO_CORRECTIONS: DemoCorrection[] = [
  {id: 'c1', time: '00:05.1', original: '步幅为 2。', suggested: '步幅为 1。', reason: '数值改变，必须由用户明确采纳。', risk: '关键', state: 'pending'},
  {id: 'c2', time: '00:10.4', original: '权重共享让同一组卷积核', suggested: '权值共享让同一组卷积核', reason: '术语统一，不改变事实关系。', risk: '普通', state: 'pending'},
];

const DEMO_OBJECTS = [
  {id: 'input', label: '输入图像', detail: '28 × 28 像素 · 来源：课件第 4 页'},
  {id: 'kernel', label: '卷积核', detail: '3 × 3 可编辑权重 · 可逐格查看计算'},
  {id: 'window', label: '局部窗口', detail: '当前位置 [2, 2] · 与原话 00:05.1 对齐'},
  {id: 'output', label: '输出特征图', detail: '26 × 26 · 数学结果由 N 确定性计算校验'},
];

function jumpToSource(id: string, setFocus: (value: string) => void): void {
  setFocus(id);
  document.getElementById(`demo-source-${id}`)?.scrollIntoView({behavior: 'smooth', block: 'center'});
}

export function DemoWorkspace() {
  const [running, setRunning] = useState(false);
  const [visibleCount, setVisibleCount] = useState(0);
  const [selectedIds, setSelectedIds] = useState<string[]>(['s1']);
  const [courseware, setCourseware] = useState<string[]>(['s2']);
  const [focusedId, setFocusedId] = useState('s1');
  const [selectedObject, setSelectedObject] = useState('kernel');
  const [corrections, setCorrections] = useState(DEMO_CORRECTIONS);
  const [micState, setMicState] = useState<'idle' | 'live' | 'blocked'>('idle');
  const [chunkCount, setChunkCount] = useState(0);
  const [notice, setNotice] = useState('点击“开始流式识别”后，文字会按时间轴增量到达。');
  const recorderRef = useRef<MediaRecorder | null>(null);
  const streamRef = useRef<MediaStream | null>(null);

  const visibleSegments = useMemo(() => DEMO_SEGMENTS.slice(0, visibleCount), [visibleCount]);
  const selectedObjectData = DEMO_OBJECTS.find((object) => object.id === selectedObject) ?? DEMO_OBJECTS[1];
  const pendingCorrections = corrections.filter((correction) => correction.state === 'pending').length;

  const stopCapture = useCallback(() => {
    if (recorderRef.current && recorderRef.current.state !== 'inactive') recorderRef.current.stop();
    recorderRef.current = null;
    streamRef.current?.getTracks().forEach((track) => track.stop());
    streamRef.current = null;
    setMicState('idle');
    setRunning(false);
  }, []);

  useEffect(() => () => stopCapture(), [stopCapture]);

  useEffect(() => {
    if (!running) return undefined;
    const timer = window.setInterval(() => {
      setVisibleCount((current) => {
        const next = Math.min(current + 1, DEMO_SEGMENTS.length);
        if (next === DEMO_SEGMENTS.length) {
          setNotice('流式段落已全部到达；原始时间轴和来源页锚点保持可跳转。');
          window.clearInterval(timer);
        } else {
          setNotice(`收到 sentence ${next} · connection_epoch 1 · 仅追加新片段`);
        }
        return next;
      });
    }, 900);
    return () => window.clearInterval(timer);
  }, [running]);

  const startCapture = async () => {
    setVisibleCount(0);
    setSelectedIds([]);
    setRunning(true);
    setNotice('正在打开本机麦克风；Demo 结果会以流式 sentence 事件逐段显示。');
    try {
      const stream = await navigator.mediaDevices?.getUserMedia({audio: true});
      if (stream) {
        streamRef.current = stream;
        setMicState('live');
        if ('MediaRecorder' in window) {
          const recorder = new MediaRecorder(stream);
          recorder.ondataavailable = (event) => {
            if (event.data.size > 0) setChunkCount((count) => count + 1);
          };
          recorder.start(250);
          recorderRef.current = recorder;
        }
        setNotice('本机麦克风已打开 · 音频只在本地生成短 chunk，不上传；文字为本地 Demo stream adapter。');
      } else {
        setMicState('blocked');
        setNotice('当前浏览器没有麦克风能力，已进入本地流式回放；文字仍按 sentence 事件增量显示。');
      }
    } catch {
      setMicState('blocked');
      setNotice('未授予麦克风权限，已保留本地流式回放；不会把权限失败当成 ASR 成功。');
    }
  };

  const toggleSelection = (id: string) => {
    setSelectedIds((current) => current.includes(id) ? current.filter((item) => item !== id) : [...current, id]);
  };

  const addToCourseware = () => {
    const next = [...courseware];
    selectedIds.forEach((id) => { if (!next.includes(id)) next.push(id); });
    setCourseware(next);
    setNotice(selectedIds.length ? `已把 ${selectedIds.length} 条带时间锚点的原话加入课件草稿。` : '先选择一条已到达的原话，再加入课件。');
  };

  const updateCorrection = (id: string, state: DemoCorrection['state']) => {
    setCorrections((current) => current.map((correction) => correction.id === id ? {...correction, state} : correction));
    setNotice(state === 'accepted' ? '已采纳二次转写订正；原始转写仍保持只读。' : '已拒绝该订正；原始转写和时间锚点保持不变。');
  };

  return (
    <div className="demo-shell">
      <header className="demo-topbar">
        <div className="app-brand"><span className="app-brand-mark" aria-hidden="true">F</span><span><strong>focaus demo</strong><small>流式课堂理解工作台</small></span></div>
        <div className="demo-topbar-center"><StatusBadge tone="warning" dot>本地 Demo 数据</StatusBadge><span>不调用云模型 · 不上传音频</span></div>
        <div className="demo-topbar-actions"><Button size="sm" variant="ghost" onPress={() => {window.location.href = `${window.location.pathname}?fixture=1`;}}>打开工程 fixture</Button><Button size="sm" variant="ghost" onPress={() => window.location.reload()}>重置 Demo</Button></div>
      </header>

      <main className="demo-page">
        <section className="demo-hero">
          <div>
            <p className="eyebrow">L08 / 卷积与局部连接 · 演示课堂</p>
            <h1>从一句话，到一页可继续学习的课件。</h1>
            <p className="demo-hero-copy">这条 Demo 链把实时原话、课件来源、可编辑重构对象和二次订正放在同一个时间轴里。先演示流式入口，再把选中的内容变成可回跳的课堂材料。</p>
          </div>
          <div className="demo-hero-stats"><div><strong>{visibleSegments.length}/{DEMO_SEGMENTS.length}</strong><span>流式句段</span></div><div><strong>{courseware.length}</strong><span>已入课件</span></div><div><strong>{pendingCorrections}</strong><span>待订正</span></div></div>
        </section>

        <div className="demo-notice" role="status"><span className={`demo-live-dot ${running ? 'is-live' : ''}`} />{notice}</div>

        <section className="demo-stream-panel demo-card" aria-labelledby="demo-stream-title">
          <div className="demo-section-heading"><div><p className="eyebrow">01 · ASR stream</p><h2 id="demo-stream-title">实时流式语音识别</h2><p>本地麦克风采集 + sentence 事件回放。真实模型连接位保留专用 streaming adapter。</p></div><div className="demo-heading-actions"><StatusBadge tone={running ? 'success' : 'neutral'} dot>{running ? 'streaming' : 'idle'}</StatusBadge><Button size="sm" onPress={running ? stopCapture : startCapture}>{running ? '停止流式识别' : '开始流式识别'}</Button></div></div>
          <div className="demo-stream-meta"><span>model slot <strong>qwen-audio-3.1-asr-flash-streaming</strong></span><span>connection_epoch 1</span><span>本地 chunk {chunkCount}</span><span>{micState === 'live' ? '麦克风已连接' : micState === 'blocked' ? '回放模式' : '等待开始'}</span></div>
          <div className="demo-stream-layout">
            <div className="demo-transcript-list" aria-label="流式转写结果">
              {visibleSegments.map((segment) => <article key={segment.id} id={`demo-source-${segment.id}`} className={`demo-transcript-row ${focusedId === segment.id ? 'is-focused' : ''}`}><button type="button" className={`demo-select-box ${selectedIds.includes(segment.id) ? 'is-selected' : ''}`} onClick={() => toggleSelection(segment.id)} aria-label={`选择 ${segment.text}`} aria-pressed={selectedIds.includes(segment.id)}>{selectedIds.includes(segment.id) ? '✓' : ''}</button><div className="demo-transcript-time"><strong>{segment.time}</strong><span>→ {segment.end}</span></div><div className="demo-transcript-copy"><p>{segment.text}</p><div><span className="demo-source-pill">原始时间</span><span>课件第 {segment.page} 页</span><span>置信 {segment.confidence}</span></div></div><Button size="sm" variant="ghost" onPress={() => jumpToSource(segment.id, setFocusedId)}>定位</Button></article>)}
              {running && visibleCount < DEMO_SEGMENTS.length ? <div className="demo-partial-row"><span className="demo-pulse" /><span>partial · 正在等待下一次 final sentence…</span></div> : null}
              {!visibleSegments.length && !running ? <div className="demo-empty-stream"><span>◌</span><p>开始后，流式句段会从这里按时间顺序出现。</p></div> : null}
            </div>
            <aside className="demo-stream-side"><div className="demo-wave" aria-label="麦克风本地波形"><span /><span /><span /><span /><span /><span /><span /><span /><span /><span /><span /></div><p className="demo-side-title">实时采集状态</p><div className="demo-side-line"><span>音频来源</span><strong>{micState === 'live' ? 'Mac 麦克风' : '未连接'}</strong></div><div className="demo-side-line"><span>输出策略</span><strong>只追加 final</strong></div><div className="demo-side-line"><span>时间锚点</span><strong>原始 recording</strong></div><Button fullWidth variant="secondary" onPress={addToCourseware}>＋ 将选中原话加入课件</Button></aside>
          </div>
        </section>

        <div className="demo-main-grid">
          <section className="demo-card demo-courseware-panel" aria-labelledby="demo-courseware-title"><div className="demo-section-heading compact"><div><p className="eyebrow">02 · Courseware</p><h2 id="demo-courseware-title">课件重构结果</h2></div><StatusBadge tone="success">可编辑对象</StatusBadge></div><div className="demo-reconstructed-slide"><div className="demo-slide-label">L08 · PAGE 04 · RECONSTRUCTED</div><h3>卷积：局部窗口如何产生新特征</h3><div className="demo-slide-equation">y[i,j] = Σ x[i+m,j+n] · k[m,n]</div><ul><li>窗口向右移动一个步幅，得到新的响应值</li><li>权重共享让同一卷积核识别不同位置的局部模式</li></ul><div className="demo-slide-footer"><span>来源锚点 · material v2 / page 4 / region 2</span><Button size="sm" variant="outline" onPress={() => jumpToSource('s2', setFocusedId)}>↗ 跳到原话</Button></div></div><div className="demo-courseware-list"><div className="demo-courseware-list-head"><strong>我的课堂课件</strong><span>{courseware.length} 条来源</span></div>{courseware.map((id) => {const segment = DEMO_SEGMENTS.find((item) => item.id === id); return segment ? <button key={id} type="button" className="demo-courseware-item" onClick={() => jumpToSource(id, setFocusedId)}><span className="demo-item-index">{courseware.indexOf(id) + 1}</span><span><strong>{segment.text}</strong><small>{segment.time} · 第 {segment.page} 页 · 点击跳回原话</small></span><span>→</span></button> : null;})}</div></section>

          <section className="demo-card demo-visual-panel" aria-labelledby="demo-visual-title"><div className="demo-section-heading compact"><div><p className="eyebrow">03 · 2D visualization</p><h2 id="demo-visual-title">原生 2D 交互对象</h2></div><StatusBadge tone="accent">交互演示</StatusBadge></div><p className="demo-panel-copy">选中节点查看确定性计算依据。Demo 聚焦 2D 对象，不做 3D。</p><div className="demo-2d-stage"><div className="demo-2d-line line-a" /><div className="demo-2d-line line-b" /><div className="demo-2d-line line-c" />{DEMO_OBJECTS.map((object, index) => <button key={object.id} type="button" className={`demo-2d-node node-${index + 1} ${selectedObject === object.id ? 'is-selected' : ''}`} onClick={() => setSelectedObject(object.id)}><span>{index + 1}</span>{object.label}</button>)}</div><div className="demo-object-detail"><span className="demo-object-icon">{selectedObjectData.id === 'kernel' ? '∑' : '◇'}</span><div><strong>{selectedObjectData.label}</strong><p>{selectedObjectData.detail}</p></div></div><div className="demo-kernel-grid" aria-label="卷积核可编辑网格">{[1, 0, -1, 1, 0, -1, 1, 0, -1].map((value, index) => <button key={index} type="button" className={value === 0 ? 'is-zero' : ''} onClick={() => setNotice(`2D 对象已选中第 ${index + 1} 格；实际数学值由 N 校验。`)}>{value}</button>)}</div></section>

          <section className="demo-card demo-correction-panel" aria-labelledby="demo-correction-title"><div className="demo-section-heading compact"><div><p className="eyebrow">04 · Secondary transcript</p><h2 id="demo-correction-title">二次转写订正</h2></div><StatusBadge tone={pendingCorrections ? 'warning' : 'success'}>{pendingCorrections ? `${pendingCorrections} 条待确认` : '已处理'}</StatusBadge></div><div className="demo-protection-note"><strong>原始转写只读</strong><span>订正只进入二次层，关键事实必须用户采纳。</span></div><div className="demo-correction-list">{corrections.map((correction) => <article key={correction.id} className={`demo-correction ${correction.state !== 'pending' ? `is-${correction.state}` : ''}`}><div className="demo-correction-head"><span>{correction.time}</span><StatusBadge tone={correction.risk === '关键' ? 'danger' : 'accent'}>{correction.risk}风险</StatusBadge></div><div className="demo-redline"><del>{correction.original}</del><span>→</span><ins>{correction.suggested}</ins></div><p>{correction.reason}</p>{correction.state === 'pending' ? <div className="demo-correction-actions"><Button size="sm" onPress={() => updateCorrection(correction.id, 'accepted')}>采纳</Button><Button size="sm" variant="tertiary" onPress={() => updateCorrection(correction.id, 'rejected')}>拒绝</Button></div> : <span className="demo-resolved">{correction.state === 'accepted' ? '✓ 已采纳到二次层' : '× 已拒绝，保留原句'}</span>}</article>)}</div></section>
        </div>

        <footer className="demo-footer"><span>focaus demo · 数据来自本地定制的 L08/L09 课堂样本</span><span>流式入口已实现本地采集与增量事件展示 · 云模型待专用凭据与费用授权</span></footer>
      </main>
    </div>
  );
}

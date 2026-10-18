import {useCallback, useEffect, useMemo, useState} from 'react';
import {Button} from '@heroui/react';
import type {CommandName, EventBatch, HostAdapter, RecordingState} from '@focaus/contracts';
import {CourseWorkspace} from './components/CourseWorkspace.tsx';
import {StartHome} from './components/StartHome.tsx';
import {SearchPanel} from './search/SearchPanel.tsx';
import {StatusBadge} from './components/StatusBadge.tsx';
import {ids, initialRecording, workspaceViews, type WorkspaceViewFixture} from './app/fixtures.ts';
import {
  bootstrapNativeRuntime,
  FIXTURE_WINDOW_ID,
  FixtureHostAdapter,
  hostCapabilities,
  makeEnvelope,
  setActiveWindowId,
  type HostRuntime,
} from './app/hostAdapter.ts';
import type {SearchResultView} from './search/searchIndex.ts';

type Screen = 'home' | 'workspace';
type Theme = 'system' | 'light' | 'dark';

function isFixtureRequested(): boolean {
  return new URLSearchParams(window.location.search).get('fixture') === '1';
}

function isRecordingState(value: unknown): value is RecordingState {
  return value === 'starting' || value === 'recording' || value === 'paused' || value === 'stopping' || value === 'stopped' || value === 'interrupted' || value === 'failed';
}

function readRecordingFromEvent(batch: EventBatch): RecordingState | null {
  for (const event of batch.events) {
    if (!event.event_type.startsWith('recording.') || !event.payload || typeof event.payload !== 'object') continue;
    const state = (event.payload as Record<string, unknown>).state;
    if (isRecordingState(state)) return state;
  }
  return null;
}

function extractRecordingFromSnapshot(snapshot: unknown): RecordingState | null {
  if (!snapshot || typeof snapshot !== 'object') return null;
  const global = (snapshot as {global?: unknown}).global;
  if (!global || typeof global !== 'object') return null;
  const candidate = (global as Record<string, unknown>).recording_state;
  return isRecordingState(candidate) ? candidate : null;
}

export function App() {
  const fixtureRequested = useMemo(isFixtureRequested, []);
  const fixtureAdapter = useMemo(() => new FixtureHostAdapter(), []);
  const [runtime, setRuntime] = useState<HostRuntime | null>(null);
  const [bootError, setBootError] = useState('');
  const [bootPhase, setBootPhase] = useState(fixtureRequested ? 'fixture mode' : 'connecting to focaus_bootstrap');
  const [eventSeq, setEventSeq] = useState(0);
  const [screen, setScreen] = useState<Screen>('home');
  const [selectedViewId, setSelectedViewId] = useState(workspaceViews[0].id);
  const [searchOpen, setSearchOpen] = useState(false);
  const [searchNotice, setSearchNotice] = useState('');
  const [theme, setTheme] = useState<Theme>('system');
  const [recordingState, setRecordingState] = useState<RecordingState>(initialRecording.state);
  const [elapsedMs, setElapsedMs] = useState(initialRecording.elapsedMs);
  const [watermarkMs, setWatermarkMs] = useState(initialRecording.watermarkMs);
  const [moveState, setMoveState] = useState<'grid' | 'preparing' | 'detached'>('grid');
  const [nativeNotice, setNativeNotice] = useState('');

  const handleEventBatch = useCallback((batch: EventBatch) => {
    setEventSeq(batch.through_seq);
    const nextRecordingState = readRecordingFromEvent(batch);
    if (nextRecordingState) setRecordingState(nextRecordingState);
  }, []);

  useEffect(() => {
    let alive = true;
    let stopEvents: (() => void) | undefined;
    if (fixtureRequested) {
      setActiveWindowId(FIXTURE_WINDOW_ID);
      setRuntime({mode: 'fixture', windowId: FIXTURE_WINDOW_ID, adapter: fixtureAdapter, eventSeq: 0});
      return () => { alive = false; };
    }

    setBootPhase('focaus_bootstrap → native window identity');
    bootstrapNativeRuntime(handleEventBatch)
      .then((nextRuntime) => {
        stopEvents = nextRuntime.stopEvents;
        if (!alive) {
          nextRuntime.stopEvents();
          return;
        }
        setRuntime(nextRuntime);
        setEventSeq(nextRuntime.state.snapshot_seq);
        const snapshotRecordingState = extractRecordingFromSnapshot(nextRuntime.state.snapshot);
        if (snapshotRecordingState) setRecordingState(snapshotRecordingState);
        setBootPhase(`state.open → snapshot ${nextRuntime.state.snapshot_seq} → state.ack → events ready`);
      })
      .catch((error: unknown) => {
        if (!alive) return;
        setBootError(error instanceof Error ? error.message : String(error));
        setBootPhase('native bootstrap failed');
      });
    return () => {
      alive = false;
      stopEvents?.();
    };
  }, [fixtureAdapter, fixtureRequested, handleEventBatch]);

  useEffect(() => {
    const root = document.documentElement;
    if (theme === 'system') root.removeAttribute('data-theme');
    else root.setAttribute('data-theme', theme);
  }, [theme]);

  useEffect(() => {
    if (recordingState !== 'recording') return;
    const interval = window.setInterval(() => {
      setElapsedMs((current) => current + 1000);
      setWatermarkMs((current) => current + 1000);
    }, 1000);
    return () => window.clearInterval(interval);
  }, [recordingState]);

  const adapter: HostAdapter = runtime?.adapter ?? fixtureAdapter;
  const selectedView = workspaceViews.find((view) => view.id === selectedViewId) ?? workspaceViews[0];

  const invoke = useCallback(async <N extends CommandName>(name: N, payload: Parameters<HostAdapter['invoke']>[1]): Promise<Awaited<ReturnType<HostAdapter['invoke']>>> => {
    return adapter.invoke(name, payload as never, makeEnvelope(name));
  }, [adapter]);

  const updateRecordingFixture = (nextState: RecordingState) => {
    setRecordingState(nextState);
    if (nextState === 'recording' && recordingState === 'stopped') setElapsedMs(0);
  };

  const startRecording = async () => {
    setScreen('workspace');
    setNativeNotice('');
    if (runtime?.mode === 'native') {
      setRecordingState('starting');
      const response = await invoke('recording.start', {lesson_id: ids.lesson, device_id: null});
      if (response.status === 'error') {
        setRecordingState('failed');
        setNativeNotice(`真实宿主拒绝 recording.start：${response.error?.message ?? response.error?.code ?? 'unknown error'}`);
      } else {
        setNativeNotice('recording.start 已提交，等待 recording.changed 事件确认采集状态；前端不假装已开始。');
      }
      return;
    }
    const response = await invoke('recording.start', {lesson_id: ids.lesson, device_id: null});
    if (response.status === 'error') setNativeNotice(response.error?.message ?? 'fixture start failed');
    else updateRecordingFixture('recording');
  };

  const toggleRecording = async () => {
    if (recordingState === 'stopped' || recordingState === 'failed' || recordingState === 'interrupted') return startRecording();
    const name = recordingState === 'recording' ? 'recording.pause' : recordingState === 'paused' ? 'recording.resume' : null;
    if (!name) return;
    setNativeNotice('');
    if (runtime?.mode === 'native') {
      const response = await invoke(name, {recording_id: ids.recording, expected_state: recordingState});
      if (response.status === 'error') setNativeNotice(`真实宿主拒绝 ${name}：${response.error?.message ?? response.error?.code ?? 'unknown error'}`);
      else setNativeNotice(`${name} 已提交，等待宿主事件确认；UI 不在 native 模式本地伪造状态。`);
      return;
    }
    const response = await invoke(name, {recording_id: ids.recording, expected_state: recordingState});
    if (response.status === 'error') setNativeNotice(response.error?.message ?? `${name} failed`);
    else updateRecordingFixture(recordingState === 'recording' ? 'paused' : 'recording');
  };

  const stopRecording = async () => {
    if (recordingState === 'stopped') return;
    if (runtime?.mode === 'native') {
      setRecordingState('stopping');
      const response = await invoke('recording.stop', {recording_id: ids.recording, expected_state: recordingState});
      if (response.status === 'error') {
        setRecordingState('failed');
        setNativeNotice(`真实宿主拒绝 recording.stop：${response.error?.message ?? response.error?.code ?? 'unknown error'}`);
      } else setNativeNotice('recording.stop 已提交，等待 stopped 事件封口。');
      return;
    }
    const response = await invoke('recording.stop', {recording_id: ids.recording, expected_state: recordingState});
    if (response.status === 'error') setNativeNotice(response.error?.message ?? 'fixture stop failed');
    else updateRecordingFixture('stopped');
  };

  const moveView = async () => {
    if (moveState === 'preparing') return;
    const targetWindowId = moveState === 'detached' ? null : 'fixture-detached-window';
    setMoveState('preparing');
    setNativeNotice('');
    const response = await invoke('view.move', {view_id: ids.view, target_window_id: targetWindowId, base_view_revision: 'fixture-view-revision-04'});
    if (response.status === 'error') {
      setMoveState(moveState === 'detached' ? 'detached' : 'grid');
      setNativeNotice(`view.move 失败：${response.error?.message ?? response.error?.code ?? 'unknown error'}；原承载保留。`);
      return;
    }
    if (runtime?.mode === 'native') {
      setNativeNotice('view.move 已提交；等待目标承载 view.ready。草稿、光标和滚动仍由宿主持有。');
      return;
    }
    window.setTimeout(async () => {
      const ready = await invoke('view.ready', {transfer_id: 'fixture-transfer-01', view_revision: 'fixture-view-revision-04'});
      if (ready.status === 'error') {
        setMoveState(moveState === 'detached' ? 'detached' : 'grid');
        setNativeNotice('view.ready 失败；已回滚到原承载，没有重复任务。');
      } else {
        setMoveState(moveState === 'detached' ? 'grid' : 'detached');
        setNativeNotice(moveState === 'detached' ? '已放回原六格；草稿、焦点和滚动状态保留。' : '已拆出当前内容窗；原六格释放，未启动重复任务。');
      }
    }, 450);
  };

  useEffect(() => {
    if (screen !== 'workspace') return;
    const handleShortcut = (event: KeyboardEvent) => {
      if (event.metaKey && event.key.toLowerCase() === 'f') {
        event.preventDefault();
        setSearchOpen(true);
      }
    };
    window.addEventListener('keydown', handleShortcut);
    return () => window.removeEventListener('keydown', handleShortcut);
  }, [screen]);

  const locateSearchResult = (result: SearchResultView) => {
    setSearchNotice(`${result.entry.anchorLabel} · 已请求稳定 anchor 校验；不会以 DOM offset 代替宿主定位。`);
    const element = document.querySelector(`[data-anchor="${selectedView.anchorId}"]`);
    if (element instanceof HTMLElement) element.scrollIntoView({behavior: 'smooth', block: 'center'});
  };

  if (!runtime && !bootError && fixtureRequested) {
    return <div className="boot-screen"><div className="boot-mark">F</div><p>正在准备 fixture 浏览审查…</p></div>;
  }
  if (!runtime && !bootError) {
    return <div className="boot-screen"><div className="boot-mark">F</div><p>{bootPhase}</p><span>不会自动回退 fixture；Tauri host 未确认前不显示模拟状态。</span></div>;
  }
  if (!runtime && bootError) {
    return <div className="boot-screen boot-screen-error"><div className="boot-mark">F</div><StatusBadge tone="danger">native bootstrap 失败</StatusBadge><h1>无法连接本地宿主</h1><p>{bootError}</p><p className="boot-detail">{bootPhase} · 已停止启动链，不会把 fixture 状态冒充 native 成功。</p><Button onPress={() => {window.location.href = `${window.location.pathname}?fixture=1`;}}>明确打开 fixture 浏览审查</Button></div>;
  }

  const activeRuntime = runtime;
  if (!activeRuntime) return null;
  const isNativeRuntime = activeRuntime.mode === 'native';
  const snapshotSeq = isNativeRuntime ? activeRuntime.state.snapshot_seq : 0;

  return (
    <div className={`app-shell mode-${isNativeRuntime ? 'native' : 'fixture'}`}>
      <header className="app-topbar">
        <div className="app-brand"><span className="app-brand-mark" aria-hidden="true">F</span><span><strong>focaus</strong><small>课堂理解工作台</small></span></div>
        <div className="app-topbar-status"><StatusBadge tone={isNativeRuntime ? 'success' : 'warning'} dot={isNativeRuntime}>{isNativeRuntime ? `native · snapshot ${snapshotSeq}` : 'fixture · 显式审查模式'}</StatusBadge>{nativeNotice ? <span className="topbar-notice">{nativeNotice}</span> : null}</div>
        <div className="app-topbar-actions"><Button size="sm" variant="ghost" onPress={() => setSearchOpen(true)} isDisabled={screen !== 'workspace'}>⌘ F <span className="topbar-action-text">搜索当前窗</span></Button><Button size="sm" variant="ghost" onPress={() => setTheme(theme === 'system' ? 'dark' : theme === 'dark' ? 'light' : 'system')} aria-label={`切换主题，当前为${theme}`}>{theme === 'dark' ? '深色' : theme === 'light' ? '浅色' : '系统'}</Button></div>
      </header>
      {screen === 'home' ? <StartHome recordingState={recordingState} onStartRecording={() => void startRecording()} onOpenWorkspace={() => setScreen('workspace')} /> : <div className="workspace-layout"><CourseWorkspace adapter={adapter} selectedView={selectedView} onSelectView={(view) => setSelectedViewId(view.id)} recordingState={recordingState} elapsedMs={elapsedMs} watermarkMs={watermarkMs} assignmentEpoch={initialRecording.assignmentEpoch} onToggleRecording={() => void toggleRecording()} onStopRecording={() => void stopRecording()} moveState={moveState} onMoveView={() => void moveView()} onBack={() => setScreen('home')} mode={isNativeRuntime ? 'native' : 'fixture'} eventSeq={eventSeq} /><SearchPanel open={searchOpen} view={selectedView} adapter={adapter} onClose={() => setSearchOpen(false)} onLocate={locateSearchResult} /></div>}
      {searchNotice ? <div className="floating-notice" role="status">{searchNotice}<Button size="sm" variant="ghost" onPress={() => setSearchNotice('')}>知道了</Button></div> : null}
      <footer className="app-footer"><span>契约 {hostCapabilities.supported_contract_majors[0]}.0.0 · 资料 focaus.material/2.0.0 · 组件 focaus.visualization/1.0.0</span><span>{isNativeRuntime ? '事件通道 focaus://events 已订阅' : '浏览器审查：所有外部操作均为 fixture'}</span></footer>
    </div>
  );
}

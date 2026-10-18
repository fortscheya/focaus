import {useCallback, useEffect, useMemo, useRef, useState} from 'react';
import {Button} from '@heroui/react';
import type {CommandName, CommandResult, ContentView, Course, EventBatch, HostAdapter, Lesson, RecordingSession, RecordingState} from '@focaus/contracts';
import {CourseWorkspace} from './components/CourseWorkspace.tsx';
import {NativeWorkspace} from './components/NativeWorkspace.tsx';
import {DemoWorkspace} from './demo/DemoWorkspace.tsx';
import {StartHome} from './components/StartHome.tsx';
import {SearchPanel} from './search/SearchPanel.tsx';
import {StatusBadge} from './components/StatusBadge.tsx';
import {ids, initialRecording, workspaceViews, type WorkspaceViewFixture} from './app/fixtures.ts';
import {
  bootstrapNativeRuntime,
  createNativeMaterialImportAdapter,
  FIXTURE_WINDOW_ID,
  FixtureHostAdapter,
  hostCapabilities,
  makeEnvelope,
  setActiveWindowId,
  type HostRuntime,
  type NativeRuntime,
} from './app/hostAdapter.ts';
import {SerialNativeEventQueue} from './app/nativeEventQueue.ts';
import {classifyRecordingEvent} from './app/nativeRecordingEvents.ts';
import {detectNativeProjection, nativeProjectionRevision, NativeProjectionGate, type NativeProjection} from './nativeContentProjection.ts';
import type {MaterialTaskFeedback} from './material/materialAdapter.ts';
import type {SearchResultView} from './search/searchIndex.ts';

type Screen = 'home' | 'workspace';
type Theme = 'system' | 'light' | 'dark';

function isFixtureRequested(): boolean {
  return new URLSearchParams(window.location.search).get('fixture') === '1';
}

function isDemoRequested(): boolean {
  return new URLSearchParams(window.location.search).get('demo') === '1';
}

function isRecordingState(value: unknown): value is RecordingState {
  return value === 'starting' || value === 'recording' || value === 'paused' || value === 'stopping' || value === 'stopped' || value === 'interrupted' || value === 'failed';
}

function readRecordingFromEvent(batch: EventBatch): RecordingState | null {
  void batch;
  return null;
}

function asRecord(value: unknown): Record<string, unknown> | null {
  return value && typeof value === 'object' ? value as Record<string, unknown> : null;
}

function safeHostErrorMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === 'string') return error;
  const record = asRecord(error);
  const code = typeof record?.code === 'string' ? record.code : null;
  const message = typeof record?.message === 'string' ? record.message : null;
  if (code && message) return `${code}: ${message}`;
  if (message) return message;
  if (code) return code;
  return 'native bootstrap returned an unknown safe error';
}

function parseCourse(value: unknown): Course | null {
  const record = asRecord(value);
  if (!record || typeof record.course_id !== 'string' || typeof record.name !== 'string' || typeof record.revision_id !== 'string' || typeof record.entity_seq !== 'number') return null;
  return record as unknown as Course;
}

function parseLesson(value: unknown): Lesson | null {
  const record = asRecord(value);
  if (!record || typeof record.lesson_id !== 'string' || typeof record.course_id !== 'string' || typeof record.title !== 'string' || typeof record.revision_id !== 'string' || typeof record.entity_seq !== 'number') return null;
  return record as unknown as Lesson;
}

function parseRecording(value: unknown): RecordingSession | null {
  const record = asRecord(value);
  if (!record || typeof record.recording_id !== 'string' || typeof record.state !== 'string' || !isRecordingState(record.state) || typeof record.assignment_epoch !== 'number' || typeof record.durable_watermark_ms !== 'number' || typeof record.revision_id !== 'string' || typeof record.entity_seq !== 'number') return null;
  return record as unknown as RecordingSession;
}

function unwrapEntity(value: unknown, key: string): unknown {
  const record = asRecord(value);
  return record && key in record ? record[key] : value;
}

function firstRecord(records: unknown, key: string): unknown {
  const record = asRecord(records);
  const group = record ? asRecord(record[key]) : null;
  return group ? Object.values(group)[0] : null;
}

function extractLibraryFromSnapshot(snapshot: unknown): {course: Course | null; lesson: Lesson | null; recording: RecordingSession | null} {
  const record = asRecord(snapshot);
  const global = record ? asRecord(record.global) : null;
  const records = record?.records;
  return {
    course: parseCourse(global?.course) ?? parseCourse(firstRecord(records, 'courses')),
    lesson: parseLesson(global?.lesson) ?? parseLesson(firstRecord(records, 'lessons')),
    recording: parseRecording(global?.recording) ?? parseRecording(global?.recording_session) ?? parseRecording(firstRecord(records, 'recordings')),
  };
}

function parseRecordingResponse(result: unknown): RecordingSession | null {
  return parseRecording(unwrapEntity(result, 'recording')) ?? parseRecording(unwrapEntity(result, 'session')) ?? parseRecording(result);
}

function parseCourseResponse(result: unknown): Course | null {
  return parseCourse(unwrapEntity(result, 'course')) ?? parseCourse(result);
}

function parseLessonResponse(result: unknown): Lesson | null {
  return parseLesson(unwrapEntity(result, 'lesson')) ?? parseLesson(result);
}

function parseContentView(value: unknown): ContentView | null {
  const record = asRecord(value);
  if (!record || typeof record.view_id !== 'string' || typeof record.lesson_id !== 'string' || typeof record.host_window_id !== 'string' || typeof record.view_revision !== 'string' || typeof record.state !== 'string' || !record.target || typeof record.target !== 'object') return null;
  return record as unknown as ContentView;
}

function parseContentViewResponse(result: unknown): ContentView | null {
  return parseContentView(unwrapEntity(result, 'view')) ?? parseContentView(result);
}

function parseRecordingCoverage(value: unknown): {recording_id: string; kind: 'asset' | 'span'} | null {
  const record = asRecord(value);
  if (!record || typeof record.recording_id !== 'string') return null;
  if (typeof record.audio_asset_id === 'string' || typeof record.resource_id === 'string') return {recording_id: record.recording_id, kind: 'asset'};
  if (typeof record.audio_span_id === 'string' || typeof record.coverage_status === 'string') return {recording_id: record.recording_id, kind: 'span'};
  return null;
}

function taskStatus(value: unknown): MaterialTaskFeedback['status'] | null {
  if (value === 'queued' || value === 'waiting_network' || value === 'waiting_input' || value === 'retry_wait' || value === 'cancel_requested') return 'accepted';
  if (value === 'running' || value === 'paused' || value === 'interrupted') return 'running';
  if (value === 'succeeded') return 'completed';
  if (value === 'failed' || value === 'cancelled' || value === 'superseded') return 'failed';
  return null;
}

function materialTaskFeedbackFromEvent(event: EventBatch['events'][number], current: MaterialTaskFeedback | null): MaterialTaskFeedback | null {
  const payload = asRecord(event.payload);
  const task = asRecord(payload?.task) ?? payload;
  const kind = task?.kind ?? payload?.kind;
  const taskId = typeof task?.task_id === 'string' ? task.task_id : event.entity_id;
  if (kind !== 'material_convert' && kind !== 'material_reparse' && current?.taskId !== taskId) return null;
  const status = taskStatus(task?.state ?? payload?.state);
  if (!status) return null;
  return {
    status,
    taskId,
    materialId: typeof payload?.material_id === 'string' ? payload.material_id : current?.materialId ?? null,
    sourceVersionId: typeof payload?.source_version_id === 'string' ? payload.source_version_id : current?.sourceVersionId ?? null,
    parseVersionId: typeof payload?.parse_version_id === 'string' ? payload.parse_version_id : current?.parseVersionId ?? null,
    message: status === 'completed' ? '宿主任务已完成；仍以新的 content head/projection revision 为准。' : status === 'failed' ? '宿主资料任务失败；保留旧 projection，不以失败结果替换当前内容。' : status === 'running' ? '宿主资料任务运行中；已接受不等于完成。' : '宿主已接受资料任务，等待 page_ready/parse_changed 和最终 task 状态。',
  };
}

function extractViewsFromSnapshot(snapshot: unknown): ContentView[] {
  const record = asRecord(snapshot);
  const records = record ? asRecord(record.records) : null;
  const group = records ? asRecord(records.view_states) : null;
  return group ? Object.values(group).map(parseContentView).filter((view): view is ContentView => Boolean(view)) : [];
}

function extractProjectionsFromSnapshot(snapshot: unknown): Record<string, NativeProjection> {
  const record = asRecord(snapshot);
  const records = record ? asRecord(record.records) : null;
  const heads = records ? asRecord(records.content_heads) : null;
  if (!heads) return {};
  return Object.fromEntries(Object.entries(heads).map(([key, value]) => [key, detectNativeProjection(value, key, asRecord(value)?.revision_id as string | null ?? null)]));
}

function extractContentHeadsFromSnapshot(snapshot: unknown): Record<string, unknown> {
  const record = asRecord(snapshot);
  const records = record ? asRecord(record.records) : null;
  const heads = records ? asRecord(records.content_heads) : null;
  return heads ? {...heads} : {};
}

export function App() {
  const demoRequested = useMemo(isDemoRequested, []);
  if (demoRequested) return <DemoWorkspace />;
  const fixtureRequested = useMemo(isFixtureRequested, []);
  const fixtureAdapter = useMemo(() => new FixtureHostAdapter(), []);
  const [runtime, setRuntime] = useState<HostRuntime | null>(null);
  const [bootError, setBootError] = useState('');
  const [bootPhase, setBootPhase] = useState(fixtureRequested ? 'fixture mode' : 'connecting to focaus_bootstrap');
  const [eventSeq, setEventSeq] = useState(0);
  const [screen, setScreen] = useState<Screen>('home');
  const [course, setCourse] = useState<Course | null>(null);
  const [lesson, setLesson] = useState<Lesson | null>(null);
  const [recordingSession, setRecordingSession] = useState<RecordingSession | null>(null);
  const [nativeContentHeads, setNativeContentHeads] = useState<Record<string, unknown>>({});
  const [nativeProjections, setNativeProjections] = useState<Record<string, NativeProjection>>({});
  const [nativeViews, setNativeViews] = useState<ContentView[]>([]);
  const [activeNativeViewId, setActiveNativeViewId] = useState<string | null>(null);
  const [selectedViewId, setSelectedViewId] = useState(workspaceViews[0].id);
  const [searchOpen, setSearchOpen] = useState(false);
  const [searchNotice, setSearchNotice] = useState('');
  const [theme, setTheme] = useState<Theme>('system');
  const [recordingState, setRecordingState] = useState<RecordingState>(initialRecording.state);
  const [elapsedMs, setElapsedMs] = useState(initialRecording.elapsedMs);
  const [watermarkMs, setWatermarkMs] = useState(initialRecording.watermarkMs);
  const [moveState, setMoveState] = useState<'grid' | 'preparing' | 'detached'>('grid');
  const [nativeNotice, setNativeNotice] = useState('');
  const [materialTaskFeedback, setMaterialTaskFeedback] = useState<MaterialTaskFeedback | null>(null);
  const nativeRuntimeRef = useRef<NativeRuntime | null>(null);
  const pendingEventBatches = useRef<EventBatch[]>([]);
  const failedNativeBatch = useRef<EventBatch | null>(null);
  const bufferedAfterFailure = useRef<EventBatch[]>([]);
  const nativeEventQueue = useRef(new SerialNativeEventQueue());
  const courseRef = useRef<Course | null>(null);
  const lessonRef = useRef<Lesson | null>(null);
  const recordingRef = useRef<RecordingSession | null>(null);
  const contentHeadsRef = useRef<Record<string, unknown>>({});
  const materialTaskFeedbackRef = useRef<MaterialTaskFeedback | null>(null);
  const projectionGate = useRef(new NativeProjectionGate());
  const adapter: HostAdapter = runtime?.adapter ?? fixtureAdapter;
  const materialImportAdapter = useMemo(() => runtime?.mode === 'native' ? createNativeMaterialImportAdapter((result) => {
    if (result.status === 'accepted') {
      setMaterialTaskFeedback({
        status: 'accepted',
        taskId: result.taskId ?? null,
        materialId: result.materialId ?? null,
        sourceVersionId: result.sourceVersionId ?? null,
        parseVersionId: result.parseVersionId ?? null,
        message: result.message,
      });
    }
  }) : null, [runtime?.mode]);

  useEffect(() => { courseRef.current = course; }, [course]);
  useEffect(() => { lessonRef.current = lesson; }, [lesson]);
  useEffect(() => { recordingRef.current = recordingSession; }, [recordingSession]);
  useEffect(() => { materialTaskFeedbackRef.current = materialTaskFeedback; }, [materialTaskFeedback]);

  const processNativeEventBatch = useCallback((batch: EventBatch) => {
    const scheduled = nativeEventQueue.current.enqueue(batch, async (nextBatch) => {
      const activeRuntime = nativeRuntimeRef.current;
      if (!activeRuntime) throw new Error('native runtime disappeared before event projection');
      for (const event of nextBatch.events) {
        const payloadRecord = asRecord(event.payload);
        const isMaterialProgressEvent = event.event_type === 'material.page_ready' || event.event_type === 'material.parse_changed';
        const payloadMaterialId = typeof payloadRecord?.material_id === 'string' ? payloadRecord.material_id : null;
        const scopedMaterialId = lessonRef.current?.material_scope.includes(event.entity_id) ? event.entity_id : null;
        const materialProjectionId = isMaterialProgressEvent ? payloadMaterialId ?? scopedMaterialId : null;
        const requestedProjectionRevision = isMaterialProgressEvent ? (typeof payloadRecord?.parse_version_id === 'string' ? payloadRecord.parse_version_id : null) : event.revision_id;
        const isViewEvent = event.event_type.startsWith('workspace.view_') || event.event_type.startsWith('view.');
        const isVisualizationChangedEvent = event.event_type === 'visualization.changed';
        const isContentProjectionEvent = event.event_type.startsWith('content.') || isVisualizationChangedEvent || materialProjectionId !== null;
        const projectionEntityId = materialProjectionId ?? event.entity_id;
        const recordingEventKind = event.event_type.startsWith('recording.') ? classifyRecordingEvent(event.event_type, event.entity_id, recordingRef.current?.recording_id ?? null) : 'unknown';
        const isRecordingCoverageEvent = recordingEventKind === 'coverage';
        const isRecordingWatermarkEvent = recordingEventKind === 'watermark';
        const isRecordingSessionEvent = recordingEventKind === 'session';
        const isNormalEntityEvent = isRecordingSessionEvent || isRecordingCoverageEvent || isRecordingWatermarkEvent || event.event_type.startsWith('course.') || event.event_type.startsWith('lesson.') || isContentProjectionEvent || isViewEvent;
        const isKnownProjectionEvent = event.event_type.startsWith('search.') || event.event_type.startsWith('task.') || event.event_type.startsWith('visualization.') || event.event_type.startsWith('focaus.') || event.event_type.startsWith('progress.') || event.event_type.startsWith('material.') || event.event_type.startsWith('language.') || event.event_type.startsWith('state.') || event.event_type.startsWith('workspace.');
        if (event.event_type === 'task.changed') {
          const feedback = materialTaskFeedbackFromEvent(event, materialTaskFeedbackRef.current);
          if (feedback) {
            materialTaskFeedbackRef.current = feedback;
            setMaterialTaskFeedback(feedback);
            setNativeNotice(feedback.message);
          }
        }
        if (!isNormalEntityEvent && !isKnownProjectionEvent) throw new Error(`未知 native event type：${event.event_type}；未 ACK，等待重试或重新 state.open`);
        if (isKnownProjectionEvent && !isNormalEntityEvent) {
          setNativeNotice(`已路由 ${event.event_type} projection 事件；当前 native 内容等待宿主投影读取。`);
          continue;
        }
        const affectsProjection = isNormalEntityEvent;
        if (!affectsProjection) continue;
        const isDeleted = event.event_type.includes('deleted') || event.event_type.includes('removed');
        if (isDeleted) {
          if (isContentProjectionEvent) projectionGate.current.begin(projectionEntityId, requestedProjectionRevision);
          if (recordingRef.current?.recording_id === event.entity_id) {
            recordingRef.current = null;
            setRecordingSession(null);
            setRecordingState('stopped');
          }
          if (courseRef.current?.course_id === event.entity_id) {
            courseRef.current = null;
            setCourse(null);
          }
          if (lessonRef.current?.lesson_id === event.entity_id) {
            lessonRef.current = null;
            setLesson(null);
          }
          if (isViewEvent) {
            setNativeViews((current) => current.filter((view) => view.view_id !== event.entity_id));
            setActiveNativeViewId((current) => current === event.entity_id ? null : current);
          }
          if (isContentProjectionEvent) {
            delete contentHeadsRef.current[projectionEntityId];
            setNativeContentHeads({...contentHeadsRef.current});
            setNativeProjections((current) => {const next = {...current}; delete next[projectionEntityId]; return next;});
          }
          continue;
        }
        if (isContentProjectionEvent) projectionGate.current.begin(projectionEntityId, requestedProjectionRevision);
        const response = await activeRuntime.adapter.invoke('content.get', {entity_id: projectionEntityId, revision_id: requestedProjectionRevision, selector: null}, makeEnvelope('content.get', undefined, requestedProjectionRevision ? {[projectionEntityId]: requestedProjectionRevision} : {}));
        if (response.status === 'error') throw new Error(`事件 ${event.event_type} 的 content.get 失败：${response.error?.message ?? response.error?.code ?? 'unknown error'}`);
        if (isRecordingCoverageEvent) {
          const coverageRecord = asRecord(response.result);
          const coverage = parseRecordingCoverage(coverageRecord?.asset ?? coverageRecord?.span ?? coverageRecord?.coverage ?? response.result);
          if (!coverage) throw new Error(`事件 ${event.event_type} 的 content.get 未返回合法 AudioAsset/AudioSpan`);
          contentHeadsRef.current[event.entity_id] = response.result;
          setNativeContentHeads({...contentHeadsRef.current});
          const recordingResponse = await activeRuntime.adapter.invoke('content.get', {entity_id: coverage.recording_id, revision_id: null, selector: null}, makeEnvelope('content.get'));
          if (recordingResponse.status === 'error') throw new Error(`coverage_changed 关联 RecordingSession 读取失败：${recordingResponse.error?.message ?? recordingResponse.error?.code ?? 'unknown error'}`);
          const linkedRecording = parseRecordingResponse(recordingResponse.result);
          if (!linkedRecording) throw new Error('coverage_changed 关联 content.get 未返回合法 RecordingSession');
          recordingRef.current = linkedRecording;
          setRecordingSession(linkedRecording);
          setRecordingState(linkedRecording.state);
        } else if (isRecordingWatermarkEvent) {
          const nextRecording = parseRecordingResponse(response.result);
          if (nextRecording) {
            recordingRef.current = nextRecording;
            setRecordingSession(nextRecording);
            setRecordingState(nextRecording.state);
          } else {
            const payloadRecord = asRecord(event.payload);
            const linkedRecordingId = typeof payloadRecord?.recording_id === 'string' ? payloadRecord.recording_id : recordingRef.current?.recording_id ?? null;
            if (!linkedRecordingId) throw new Error(`事件 ${event.event_type} 缺少 recording_id，无法读取 durable watermark`);
            const recordingResponse = await activeRuntime.adapter.invoke('content.get', {entity_id: linkedRecordingId, revision_id: null, selector: null}, makeEnvelope('content.get'));
            if (recordingResponse.status === 'error') throw new Error(`watermark_changed 关联 RecordingSession 读取失败：${recordingResponse.error?.message ?? recordingResponse.error?.code ?? 'unknown error'}`);
            const linkedRecording = parseRecordingResponse(recordingResponse.result);
            if (!linkedRecording) throw new Error('watermark_changed 关联 content.get 未返回合法 RecordingSession');
            recordingRef.current = linkedRecording;
            setRecordingSession(linkedRecording);
            setRecordingState(linkedRecording.state);
          }
        } else if (isRecordingSessionEvent) {
          const nextRecording = parseRecordingResponse(response.result);
          if (nextRecording) {
            recordingRef.current = nextRecording;
            setRecordingSession(nextRecording);
            setRecordingState(nextRecording.state);
          } else {
            throw new Error(`事件 ${event.event_type} 的 content.get 未返回合法 RecordingSession`);
          }
        } else if (event.event_type.startsWith('course.')) {
          const nextCourse = parseCourseResponse(response.result);
          if (!nextCourse) throw new Error(`事件 ${event.event_type} 的 content.get 未返回合法 Course`);
          courseRef.current = nextCourse;
          setCourse(nextCourse);
        } else if (event.event_type.startsWith('lesson.')) {
          const nextLesson = parseLessonResponse(response.result);
          if (!nextLesson) throw new Error(`事件 ${event.event_type} 的 content.get 未返回合法 Lesson`);
          lessonRef.current = nextLesson;
          setLesson(nextLesson);
        } else if (isContentProjectionEvent) {
          const responseRevision = nativeProjectionRevision(response.result, null);
          if (!responseRevision || requestedProjectionRevision !== null && responseRevision !== requestedProjectionRevision) throw new Error('projection response revision mismatch');
          const nextProjection = detectNativeProjection(response.result, projectionEntityId, responseRevision);
          if (nextProjection.revisionId !== responseRevision) throw new Error('projection revision was not returned');
          contentHeadsRef.current[projectionEntityId] = response.result;
          setNativeContentHeads({...contentHeadsRef.current});
          setNativeProjections((current) => ({...current, [projectionEntityId]: nextProjection}));
          if (isMaterialProgressEvent && materialTaskFeedbackRef.current) {
            const materialValue = asRecord(nextProjection.value);
            const feedback: MaterialTaskFeedback = {
              ...materialTaskFeedbackRef.current,
              status: materialTaskFeedbackRef.current.status === 'completed' || materialTaskFeedbackRef.current.status === 'failed' ? materialTaskFeedbackRef.current.status : 'partial',
              materialId: projectionEntityId,
              parseVersionId: typeof materialValue?.parse_version_id === 'string' ? materialValue.parse_version_id : materialTaskFeedbackRef.current.parseVersionId,
              message: '宿主已发布新的 material page/parse projection；仍等待 task 终态，旧 revision 未被顶替。',
            };
            materialTaskFeedbackRef.current = feedback;
            setMaterialTaskFeedback(feedback);
          }
        } else if (isViewEvent) {
          const nextView = parseContentViewResponse(response.result);
          if (!nextView) throw new Error(`事件 ${event.event_type} 的 content.get 未返回合法 ContentView`);
          setNativeViews((current) => [...current.filter((view) => view.view_id !== nextView.view_id), nextView]);
          setActiveNativeViewId(nextView.view_id);
        }
      }
      const ack = await activeRuntime.adapter.invoke('state.ack', {subscription_id: activeRuntime.state.subscription_id, upto_seq: nextBatch.through_seq}, makeEnvelope('state.ack'));
      if (ack.status === 'error') throw new Error(`state.ack 失败：${ack.error?.message ?? ack.error?.code ?? 'unknown error'}`);
      setEventSeq(nextBatch.through_seq);
    });
    void scheduled.catch((error: unknown) => {
      if (!failedNativeBatch.current) failedNativeBatch.current = batch;
      if (!failedNativeBatch.current || failedNativeBatch.current === batch) {
        setNativeNotice(`事件批次 ${batch.through_seq} 未确认：${error instanceof Error ? error.message : String(error)}。未 ACK，可重试。`);
      }
    });
    return scheduled;
  }, []);

  const handleEventBatch = useCallback((batch: EventBatch) => {
    if (!nativeRuntimeRef.current) {
      pendingEventBatches.current.push(batch);
      return;
    }
    if (nativeEventQueue.current.blocked) {
      bufferedAfterFailure.current.push(batch);
      setNativeNotice(`事件批次 ${batch.through_seq} 已排队；前一批未 ACK，先重试失败批次。`);
      return;
    }
    processNativeEventBatch(batch);
  }, [processNativeEventBatch]);

  const retryNativeEvents = useCallback(() => {
    const failed = failedNativeBatch.current;
    if (!failed) return;
    failedNativeBatch.current = null;
    nativeEventQueue.current.reset();
    void processNativeEventBatch(failed).then(() => {
      const buffered = bufferedAfterFailure.current;
      bufferedAfterFailure.current = [];
      buffered.forEach((batch) => processNativeEventBatch(batch));
    }).catch(() => undefined);
  }, [processNativeEventBatch]);

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
        nativeRuntimeRef.current = nextRuntime;
        setRuntime(nextRuntime);
        setEventSeq(nextRuntime.state.snapshot_seq);
        const library = extractLibraryFromSnapshot(nextRuntime.state.snapshot);
        courseRef.current = library.course;
        lessonRef.current = library.lesson;
        recordingRef.current = library.recording;
        const snapshotHeads = extractContentHeadsFromSnapshot(nextRuntime.state.snapshot);
        contentHeadsRef.current = snapshotHeads;
        setNativeContentHeads(snapshotHeads);
        setNativeProjections(extractProjectionsFromSnapshot(nextRuntime.state.snapshot));
        const snapshotViews = extractViewsFromSnapshot(nextRuntime.state.snapshot);
        const readyViews = nextRuntime.readyView ? [...snapshotViews.filter((view) => view.view_id !== nextRuntime.readyView?.view_id), nextRuntime.readyView] : snapshotViews;
        setNativeViews(readyViews);
        setActiveNativeViewId(nextRuntime.readyView?.view_id ?? readyViews[0]?.view_id ?? null);
        setCourse(library.course);
        setLesson(library.lesson);
        setRecordingSession(library.recording);
        if (library.recording) setRecordingState(library.recording.state);
        const buffered = pendingEventBatches.current;
        pendingEventBatches.current = [];
        buffered.forEach(processNativeEventBatch);
        setBootPhase(`state.open → snapshot ${nextRuntime.state.snapshot_seq} → state.ack → events ready`);
      })
      .catch((error: unknown) => {
        if (!alive) return;
        setBootError(safeHostErrorMessage(error));
        setBootPhase('native bootstrap failed');
      });
    return () => {
      alive = false;
      stopEvents?.();
    };
  }, [fixtureAdapter, fixtureRequested, handleEventBatch, processNativeEventBatch]);

  useEffect(() => {
    if (runtime?.mode !== 'native') return;
    for (const view of nativeViews) {
      let key: string;
      let revisionId: string | null;
      let selector: import('@focaus/contracts').Selector | null;
      if (view.target.kind === 'entity') {
        key = view.target.entity_id;
        revisionId = view.target.revision_id;
        selector = view.target.selector;
      } else if (view.target.content_kind === 'material' && lessonRef.current?.material_scope[0]) {
        key = lessonRef.current.material_scope[0];
        revisionId = null;
        selector = null;
      } else {
        continue;
      }
      const currentProjection = nativeProjections[key];
      if (currentProjection && (revisionId === null || currentProjection.revisionId === revisionId)) continue;
      const token = projectionGate.current.begin(key, revisionId);
      if (currentProjection && revisionId !== null && currentProjection.revisionId !== revisionId) {
        setNativeProjections((current) => current[key] === currentProjection ? (() => {const next = {...current}; delete next[key]; return next;})() : current);
      }
      void adapter.invoke('content.get', {entity_id: key, revision_id: revisionId, selector}, makeEnvelope('content.get', undefined, revisionId ? {[key]: revisionId} : {})).then((response) => {
        const responseRevision = response.status === 'error' ? null : nativeProjectionRevision(response.result, null);
        if (!projectionGate.current.accepts(key, token, responseRevision) || response.status === 'error' || !response.result) return;
        if (revisionId !== null && responseRevision !== revisionId) {
          setNativeNotice(`内容 ${key} 的 content.get 返回旧 revision；已丢弃旧投影，等待宿主重试。`);
          return;
        }
        const nextProjection = detectNativeProjection(response.result, key, revisionId);
        if (revisionId !== null && nextProjection.revisionId !== revisionId) return;
        contentHeadsRef.current[key] = response.result;
        setNativeContentHeads({...contentHeadsRef.current});
        setNativeProjections((current) => ({...current, [key]: nextProjection}));
      });
    }
  }, [adapter, nativeProjections, nativeViews, runtime?.mode]);

  useEffect(() => {
    const root = document.documentElement;
    if (theme === 'system') root.removeAttribute('data-theme');
    else root.setAttribute('data-theme', theme);
  }, [theme]);

  useEffect(() => {
    if (runtime?.mode !== 'fixture') return;
    if (recordingState !== 'recording') return;
    const interval = window.setInterval(() => {
      setElapsedMs((current) => current + 1000);
      setWatermarkMs((current) => current + 1000);
    }, 1000);
    return () => window.clearInterval(interval);
  }, [recordingState, runtime?.mode]);

  const selectedView = workspaceViews.find((view) => view.id === selectedViewId) ?? workspaceViews[0];

  const invoke = useCallback(async <N extends CommandName>(name: N, payload: unknown, expectedRevisions: Record<string, string> = {}): Promise<CommandResult> => {
    return adapter.invoke(name, payload as never, makeEnvelope(name, undefined, expectedRevisions));
  }, [adapter]);

  const refreshNativeMaterial = useCallback(async () => {
    if (runtime?.mode !== 'native') return;
    const materialId = lessonRef.current?.material_scope[0] ?? null;
    if (!materialId) {
      setNativeNotice('当前 lesson 尚未返回 material_scope；不会猜测资料实体 ID。');
      return;
    }
    const token = projectionGate.current.begin(materialId, null);
    setNativeNotice('正在读取宿主当前 material head；旧 projection 保持可见，直到新 revision 校验通过。');
    const response = await adapter.invoke('content.get', {entity_id: materialId, revision_id: null, selector: null}, makeEnvelope('content.get'));
    if (response.status === 'error' || !response.result) {
      setNativeNotice('读取 material head 失败：' + (response.error?.message ?? response.error?.code ?? 'unknown error'));
      return;
    }
    const responseRevision = nativeProjectionRevision(response.result, null);
    if (!responseRevision || !projectionGate.current.accepts(materialId, token, responseRevision)) {
      setNativeNotice('material head 没有返回可校验 revision；保留当前 projection。');
      return;
    }
    const current = nativeProjections[materialId];
    if (current?.revisionId === responseRevision) {
      setNativeNotice('宿主返回的 material head 与当前 revision 相同；没有替换当前 projection。');
      return;
    }
    const nextProjection = detectNativeProjection(response.result, materialId, responseRevision);
    contentHeadsRef.current[materialId] = response.result;
    setNativeContentHeads({...contentHeadsRef.current});
    setNativeProjections((currentProjections) => ({...currentProjections, [materialId]: nextProjection}));
    const value = asRecord(nextProjection.value);
    if (materialTaskFeedbackRef.current) {
      const feedback: MaterialTaskFeedback = {
        ...materialTaskFeedbackRef.current,
        materialId,
        sourceVersionId: typeof value?.source_version_id === 'string' ? value.source_version_id : materialTaskFeedbackRef.current.sourceVersionId,
        parseVersionId: typeof value?.parse_version_id === 'string' ? value.parse_version_id : responseRevision,
        message: '已刷新宿主 material head；旧 revision 未覆盖新 revision。',
      };
      materialTaskFeedbackRef.current = feedback;
      setMaterialTaskFeedback(feedback);
    }
    setNativeNotice('material head 已刷新为 revision ' + responseRevision + '；搜索 scope 需由宿主 search.refresh 重新确认。');
  }, [adapter, nativeProjections, runtime?.mode]);

  const updateRecordingFixture = (nextState: RecordingState) => {
    setRecordingState(nextState);
    if (nextState === 'recording' && recordingState === 'stopped') setElapsedMs(0);
  };

  const createCourseLesson = async (courseName: string, lessonTitle: string) => {
    if (runtime?.mode !== 'native') return;
    try {
      let confirmedCourse = courseRef.current ?? course;
      if (!confirmedCourse) {
        setNativeNotice('正在提交 course.create；课程 ID 与 revision 只采用宿主响应。');
        const courseResponse = await invoke('course.create', {name: courseName}, {});
        if (courseResponse.status === 'error') {
          setNativeNotice(`真实宿主拒绝 course.create：${courseResponse.error?.message ?? courseResponse.error?.code ?? 'unknown error'}`);
          return;
        }
        confirmedCourse = parseCourseResponse(courseResponse.result);
        if (!confirmedCourse) {
          setNativeNotice('course.create 返回缺少合法 Course；没有本地补造课程。');
          return;
        }
        courseRef.current = confirmedCourse;
        setCourse(confirmedCourse);
      } else {
        setNativeNotice('已复用宿主确认的 course_id，正在提交 lesson.create；不会重复创建课程。');
      }
      setNativeNotice('课程已确认，正在提交 lesson.create。');
      const lessonResponse = await invoke('lesson.create', {course_id: confirmedCourse.course_id, title: lessonTitle, actual_date: null, timezone: 'Asia/Shanghai'}, {});
      if (lessonResponse.status === 'error') {
        setNativeNotice(`真实宿主拒绝 lesson.create：${lessonResponse.error?.message ?? lessonResponse.error?.code ?? 'unknown error'}`);
        return;
      }
      const createdLesson = parseLessonResponse(lessonResponse.result);
      if (!createdLesson) {
        setNativeNotice('lesson.create 返回缺少合法 Lesson；保留已确认课程，不补造课堂。');
        return;
      }
      lessonRef.current = createdLesson;
      setLesson(createdLesson);
      setScreen('workspace');
      setNativeNotice('课程与课堂均已由宿主确认；现在可以提交 recording.start。');
    } catch (error) {
      setNativeNotice(`课程/课堂创建失败：${safeHostErrorMessage(error)}`);
    }
  };

  const startRecording = async () => {
    setScreen('workspace');
    setNativeNotice('');
    if (runtime?.mode === 'native') {
      setRecordingState('starting');
      const response = await invoke('recording.start', {lesson_id: lesson?.lesson_id ?? null, device_id: null});
      if (response.status === 'error') {
        setRecordingState('failed');
        setNativeNotice(`真实宿主拒绝 recording.start：${response.error?.message ?? response.error?.code ?? 'unknown error'}`);
      } else if (response.status === 'ok') {
        const nextRecording = parseRecordingResponse(response.result);
        if (nextRecording) {
          setRecordingSession(nextRecording);
          setRecordingState(nextRecording.state);
          setNativeNotice('宿主已返回 recording session；后续水位只随 durable_watermark_ms 更新。');
        } else {
          setNativeNotice('recording.start 返回 ok 但缺少 RecordingSession；等待事件补读，未在前端伪造录音状态。');
        }
      } else {
        setNativeNotice('recording.start 已提交，等待 recording.changed 事件确认采集状态；前端不假装已开始。');
      }
      return;
    }
    const response = await invoke('recording.start', {lesson_id: ids.lesson, device_id: null});
    if (response.status === 'error') setNativeNotice(response.error?.message ?? 'fixture start failed');
    else updateRecordingFixture('recording');
  };

  const refreshNativeRecording = async (): Promise<RecordingSession | null> => {
    const current = recordingRef.current ?? recordingSession;
    if (!current || runtime?.mode !== 'native') {
      setNativeNotice('真实宿主尚未返回 recording_id，无法读取最新录音 head。');
      return null;
    }
    const response = await invoke('content.get', {entity_id: current.recording_id, revision_id: null, selector: null});
    if (response.status === 'error') {
      setNativeNotice(`读取最新录音 head 失败：${response.error?.message ?? response.error?.code ?? 'unknown error'}`);
      return null;
    }
    const fresh = parseRecordingResponse(response.result);
    if (!fresh) {
      setNativeNotice('最新录音 head 不是合法 RecordingSession；没有使用旧 revision 发命令。');
      return null;
    }
    recordingRef.current = fresh;
    setRecordingSession(fresh);
    setRecordingState(fresh.state);
    return fresh;
  };

  const toggleRecording = async () => {
    if (recordingState === 'stopped' || recordingState === 'failed' || recordingState === 'interrupted') return startRecording();
    setNativeNotice('');
    if (runtime?.mode === 'native') {
      const fresh = await refreshNativeRecording();
      if (!fresh) return;
      const name = fresh.state === 'recording' ? 'recording.pause' : fresh.state === 'paused' ? 'recording.resume' : null;
      if (!name) return;
      const response = await invoke(name, {recording_id: fresh.recording_id, expected_state: fresh.state}, {[fresh.recording_id]: fresh.revision_id});
      if (response.status === 'error') setNativeNotice(`真实宿主拒绝 ${name}：${response.error?.message ?? response.error?.code ?? 'unknown error'}`);
      else setNativeNotice(`${name} 已提交，等待宿主事件确认；UI 不在 native 模式本地伪造状态。`);
      return;
    }
    const name = recordingState === 'recording' ? 'recording.pause' : recordingState === 'paused' ? 'recording.resume' : null;
    if (!name) return;
    const response = await invoke(name, {recording_id: ids.recording, expected_state: recordingState}, {[ids.recording]: '4c11a85f-f3da-4f4f-9701-000000000007'});
    if (response.status === 'error') setNativeNotice(response.error?.message ?? `${name} failed`);
    else updateRecordingFixture(recordingState === 'recording' ? 'paused' : 'recording');
  };

  const stopRecording = async () => {
    if (recordingState === 'stopped') return;
    if (runtime?.mode === 'native') {
      const fresh = await refreshNativeRecording();
      if (!fresh || (fresh.state !== 'recording' && fresh.state !== 'paused' && fresh.state !== 'starting' && fresh.state !== 'interrupted')) return;
      const response = await invoke('recording.stop', {recording_id: fresh.recording_id, expected_state: fresh.state}, {[fresh.recording_id]: fresh.revision_id});
      if (response.status === 'error') {
        setRecordingState(fresh.state);
        setNativeNotice(`真实宿主拒绝 recording.stop：${response.error?.message ?? response.error?.code ?? 'unknown error'}；保留最新录音 head。`);
      } else setNativeNotice('recording.stop 已提交，等待 stopped 事件封口；水位仍只来自宿主。');
      return;
    }
    const response = await invoke('recording.stop', {recording_id: ids.recording, expected_state: recordingState}, {[ids.recording]: '4c11a85f-f3da-4f4f-9701-000000000007'});
    if (response.status === 'error') setNativeNotice(response.error?.message ?? 'fixture stop failed');
    else updateRecordingFixture('stopped');
  };

  const openNativeView = async (placement: 'grid' | 'standalone') => {
    if (runtime?.mode !== 'native') return;
    const activeLesson = lessonRef.current ?? lesson;
    if (!activeLesson) {
      setNativeNotice('请先由宿主确认 lesson，再请求 view.open。');
      return;
    }
    const response = await invoke('view.open', {
      lesson_id: activeLesson.lesson_id,
      target: {kind: 'lesson_collection', lesson_id: activeLesson.lesson_id, content_kind: 'note'},
      placement,
    });
    if (response.status === 'error') {
      setNativeNotice(`view.open 失败：${response.error?.message ?? response.error?.code ?? 'unknown error'}`);
      return;
    }
    const openedView = parseContentViewResponse(response.result);
    if (!openedView) {
      setNativeNotice('view.open 已提交但尚未返回 ContentView；等待 workspace.view_changed，不创建本地 view。');
      return;
    }
    setNativeViews((current) => [...current.filter((view) => view.view_id !== openedView.view_id), openedView]);
    setActiveNativeViewId(openedView.view_id);
    setScreen('workspace');
    setNativeNotice(`${placement === 'standalone' ? '独立' : '六格'} view 已由宿主确认：${openedView.view_id}`);
  };

  const moveView = async () => {
    if (moveState === 'preparing') return;
    const targetWindowId = moveState === 'detached' ? null : ids.detachedWindow;
    setMoveState('preparing');
    setNativeNotice('');
    const response = await invoke('view.move', {view_id: ids.view, target_window_id: targetWindowId, base_view_revision: ids.viewRevision}, {[ids.view]: ids.viewRevision});
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
      const ready = await invoke('view.ready', {transfer_id: ids.transfer, view_revision: ids.viewRevision}, {[ids.view]: ids.viewRevision});
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

  if (isNativeRuntime) {
    const activeNativeView = nativeViews.find((view) => view.view_id === activeNativeViewId) ?? nativeViews[0] ?? null;
    return <NativeWorkspace course={course} lesson={lesson} recording={recordingSession} adapter={adapter} activeView={activeNativeView} views={nativeViews} projections={nativeProjections} materialImportAdapter={materialImportAdapter} materialTaskFeedback={materialTaskFeedback} onRefreshMaterial={refreshNativeMaterial} contentHeadCount={Object.keys(nativeContentHeads).length} eventSeq={eventSeq} snapshotSeq={snapshotSeq} notice={nativeNotice} screen={screen} onCreateCourseLesson={createCourseLesson} onOpenWorkspace={() => setScreen('workspace')} onBack={() => setScreen('home')} onToggleRecording={() => void toggleRecording()} onStopRecording={() => void stopRecording()} onStartUnassigned={() => void startRecording()} onRetryEvents={retryNativeEvents} onOpenView={(placement) => void openNativeView(placement)} searchOpen={searchOpen} onOpenSearch={() => setSearchOpen(true)} onCloseSearch={() => setSearchOpen(false)} />;
  }

  return (
    <div className={`app-shell mode-${isNativeRuntime ? 'native' : 'fixture'}`}>
      <header className="app-topbar">
        <div className="app-brand"><span className="app-brand-mark" aria-hidden="true">F</span><span><strong>focaus</strong><small>课堂理解工作台</small></span></div>
        <div className="app-topbar-status"><StatusBadge tone={isNativeRuntime ? 'success' : 'warning'} dot={isNativeRuntime}>{isNativeRuntime ? `native · snapshot ${snapshotSeq}` : 'fixture · 显式审查模式'}</StatusBadge>{nativeNotice ? <span className="topbar-notice">{nativeNotice}</span> : null}</div>
        <div className="app-topbar-actions"><Button size="sm" variant="ghost" onPress={() => setSearchOpen(true)} isDisabled={screen !== 'workspace'}>⌘ F <span className="topbar-action-text">搜索当前窗</span></Button><Button size="sm" variant="ghost" onPress={() => setTheme(theme === 'system' ? 'dark' : theme === 'dark' ? 'light' : 'system')} aria-label={`切换主题，当前为${theme}`}>{theme === 'dark' ? '深色' : theme === 'light' ? '浅色' : '系统'}</Button></div>
      </header>
      {screen === 'home' ? <StartHome recordingState={recordingState} onStartRecording={() => void startRecording()} onOpenWorkspace={() => setScreen('workspace')} /> : <div className="workspace-layout"><CourseWorkspace adapter={adapter} selectedView={selectedView} onSelectView={(view) => setSelectedViewId(view.id)} recordingState={recordingState} elapsedMs={elapsedMs} watermarkMs={watermarkMs} assignmentEpoch={initialRecording.assignmentEpoch} onToggleRecording={() => void toggleRecording()} onStopRecording={() => void stopRecording()} moveState={moveState} onMoveView={() => void moveView()} onBack={() => setScreen('home')} mode="fixture" eventSeq={eventSeq} /><SearchPanel open={searchOpen} view={selectedView} adapter={adapter} onClose={() => setSearchOpen(false)} onLocate={locateSearchResult} /></div>}
      {searchNotice ? <div className="floating-notice" role="status">{searchNotice}<Button size="sm" variant="ghost" onPress={() => setSearchNotice('')}>知道了</Button></div> : null}
      <footer className="app-footer"><span>契约 {hostCapabilities.supported_contract_majors[0]}.0.0 · 资料 focaus.material/2.0.0 · 组件 focaus.visualization/1.0.0</span><span>{isNativeRuntime ? '事件通道 focaus://events 已订阅' : '浏览器审查：所有外部操作均为 fixture'}</span></footer>
    </div>
  );
}

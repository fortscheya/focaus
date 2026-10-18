import {useEffect, useState} from 'react';
import {Button, Card, Description, Input, Label, TextField} from '@heroui/react';
import type {ContentView, Course, HostAdapter, Lesson, RecordingSession} from '@focaus/contracts';
import {RecordingBar} from './RecordingBar.tsx';
import {StatusBadge} from './StatusBadge.tsx';
import {NativeSearchPanel} from './NativeSearchPanel.tsx';
import {NativeContentRenderer} from './NativeContentRenderer.tsx';
import type {NativeProjection} from '../nativeContentProjection.ts';
import type {MaterialImportAdapter} from '../material/materialAdapter.ts';
import {MaterialStatePanel} from '../material/MaterialStatePanel.tsx';
import {buildNativePaneSlots, type NativePaneSlot} from './nativeWorkspaceLayout.ts';

function paneStateTone(state: NativePaneSlot['state']): 'neutral' | 'success' | 'warning' | 'danger' {
  if (state === 'open') return 'success';
  if (state === 'transferring') return 'warning';
  if (state === 'closed') return 'danger';
  return 'neutral';
}

function projectionTone(status: NonNullable<NativePaneSlot['projection']>['status']): 'neutral' | 'success' | 'warning' | 'danger' {
  if (status === 'complete') return 'success';
  if (status === 'failed' || status === 'unsupported') return 'danger';
  if (status === 'partial') return 'warning';
  return 'neutral';
}

function NativeLayerSummary({projections}: {projections: Record<string, NativeProjection>}) {
  const layers = [
    {kind: 'raw', label: '原始证据', detail: '只读底稿与音频时间坐标'},
    {kind: 'secondary', label: '二次转写', detail: '原话修剪与订正候选'},
    {kind: 'note', label: '综合笔记', detail: '用户编辑与逐级来源'},
  ];
  return <section className="native-layer-summary" aria-label="真实三层内容状态">{layers.map((layer) => {
    const projection = Object.values(projections).find((item) => item.kind === layer.kind);
    const status = projection?.status ?? null;
    return <Card key={layer.kind} className="native-layer-card" variant="tertiary"><Card.Content><div className="native-layer-card-heading"><strong>{layer.label}</strong><StatusBadge tone={status ? projectionTone(status) : 'neutral'}>{status ?? '等待'}</StatusBadge></div><span>{layer.detail}</span><small>{projection?.revisionId ? 'revision ' + projection.revisionId : '宿主尚未返回当前 projection'}</small></Card.Content></Card>;
  })}</section>;
}

function NativePane({slot, adapter, onOpenView, onOpenSearch}: {slot: NativePaneSlot; adapter: HostAdapter; onOpenView: (placement: 'grid' | 'standalone') => void; onOpenSearch: () => void}) {
  const status = slot.projection?.status ?? null;
  return <Card className={`native-pane-card ${slot.primary ? 'is-primary' : ''} ${slot.projection ? 'has-projection' : 'is-empty'}`} variant={slot.primary ? 'secondary' : 'tertiary'}>
    <Card.Header>
      <div className="native-pane-heading"><div><span className="native-pane-kicker">{slot.primary ? '当前焦点' : '内容窗'}</span><Card.Title>{slot.title}</Card.Title><Card.Description>{slot.description}{status ? ' · projection ' + status : ''}</Card.Description></div><StatusBadge tone={status ? projectionTone(status) : paneStateTone(slot.state)}>{status ?? slot.state}</StatusBadge></div>
    </Card.Header>
    <Card.Content>
      {slot.projection ? <NativeContentRenderer projection={slot.projection} adapter={adapter} view={slot.view} /> : <div className="native-pane-empty"><div className="native-empty-icon" aria-hidden="true">{slot.primary ? '·' : '○'}</div><strong>等待真实内容投影</strong><p>{slot.description}。此窗口不会填充本地 fixture 文本。</p>{slot.primary ? <div className="native-pane-empty-actions"><Button size="sm" onPress={() => onOpenView('grid')}>打开真实内容窗</Button><Button size="sm" variant="ghost" onPress={onOpenSearch}>搜索宿主内容</Button></div> : null}</div>}
    </Card.Content>
  </Card>;
}

export function NativeWorkspace({
  course,
  lesson,
  recording,
  adapter,
  activeView,
  views,
  projections,
  materialImportAdapter,
  contentHeadCount,
  eventSeq,
  snapshotSeq,
  notice,
  screen,
  onCreateCourseLesson,
  onOpenWorkspace,
  onBack,
  onToggleRecording,
  onStopRecording,
  onStartUnassigned,
  onRetryEvents,
  onOpenView,
  searchOpen,
  onOpenSearch,
  onCloseSearch,
}: {
  course: Course | null;
  lesson: Lesson | null;
  recording: RecordingSession | null;
  adapter: HostAdapter;
  activeView: ContentView | null;
  views: ContentView[];
  projections: Record<string, NativeProjection>;
  materialImportAdapter: MaterialImportAdapter | null;
  contentHeadCount: number;
  eventSeq: number;
  snapshotSeq: number;
  notice: string;
  screen: 'home' | 'workspace';
  onCreateCourseLesson: (courseName: string, lessonTitle: string) => Promise<void>;
  onOpenWorkspace: () => void;
  onBack: () => void;
  onToggleRecording: () => void;
  onStopRecording: () => void;
  onStartUnassigned: () => void;
  onRetryEvents: () => void;
  onOpenView: (placement: 'grid' | 'standalone') => void;
  searchOpen: boolean;
  onOpenSearch: () => void;
  onCloseSearch: () => void;
}) {
  const [courseName, setCourseName] = useState(course?.name ?? '');
  const [lessonTitle, setLessonTitle] = useState(lesson?.title ?? '');
  const [formError, setFormError] = useState('');
  const [creating, setCreating] = useState(false);
  const bindingLabel = course && lesson ? `${course.name} · ${lesson.title}` : course ? `${course.name} · 尚未创建课堂` : '未归属即时采集';

  const paneSlots = buildNativePaneSlots(views, projections, activeView);
  const expandedPrimary = paneSlots.length <= 5;

  useEffect(() => {
    if (course) setCourseName(course.name);
  }, [course?.course_id, course?.name]);

  useEffect(() => {
    if (lesson) setLessonTitle(lesson.title);
  }, [lesson?.lesson_id, lesson?.title]);

  const submit = async () => {
    if ((!course && !courseName.trim()) || !lessonTitle.trim()) {
      setFormError('课程名称和课堂标题都需要由用户明确填写。');
      return;
    }
    setFormError('');
    setCreating(true);
    try {
      await onCreateCourseLesson(courseName.trim() || course?.name || '', lessonTitle.trim());
    } finally {
      setCreating(false);
    }
  };

  return (
    <div className="native-shell">
      <header className="app-topbar">
        <div className="app-brand"><span className="app-brand-mark" aria-hidden="true">F</span><span><strong>focaus</strong><small>真实 native 宿主</small></span></div>
        <div className="app-topbar-status"><StatusBadge tone="success" dot>native · snapshot {snapshotSeq}</StatusBadge><span className="topbar-notice">事件通道 focaus://events · ACK {eventSeq}</span></div>
        <div className="app-topbar-actions"><Button size="sm" variant="ghost" onPress={() => onOpenView('grid')}>打开六格 view</Button><Button size="sm" variant="ghost" onPress={() => onOpenView('standalone')}>独立窗口</Button><Button size="sm" variant="ghost" onPress={onOpenWorkspace}>课程窗</Button><Button size="sm" variant="ghost" onPress={onOpenSearch}>⌘ F 搜索</Button></div>
      </header>
      <div className="native-recording-dock"><RecordingBar state={recording?.state ?? 'stopped'} elapsedMs={recording?.durable_watermark_ms ?? 0} watermarkMs={recording?.durable_watermark_ms ?? 0} assignmentEpoch={recording?.assignment_epoch ?? 0} bindingLabel={bindingLabel} onToggle={onToggleRecording} onStop={onStopRecording} /></div>
      {screen === 'home' ? (
        <main className="native-page page-shell">
          <section className="native-hero">
            <div>
              <p className="eyebrow">真实宿主快照</p>
              <h1>{course ? '继续配置当前课程' : '从第一堂课开始'}</h1>
              <p>当前页面只显示 Tauri 宿主返回的课程、课堂、录音与事件状态。空库不会被本地 fixture 内容填充。</p>
            </div>
            <StatusBadge tone="success">state.open → snapshot → state.ack</StatusBadge>
          </section>
          <Card className="native-create-card" variant="tertiary">
            <Card.Header><Card.Title>{course ? '为当前课程创建第一堂课' : '创建课程并创建课堂'}</Card.Title><Card.Description>将依次调用 course.create → lesson.create；ID、revision 和归属都以宿主响应为准。</Card.Description></Card.Header>
            <Card.Content>
              {course ? <div className="native-existing-record"><span>已存在课程</span><strong>{course.name}</strong><small>course_id {course.course_id}</small></div> : null}
              <div className="native-form-grid">
                {!course ? <TextField value={courseName} onChange={setCourseName} isRequired><Label>课程名称</Label><Input placeholder="例如：概率论" /><Description>写入前不会生成本地 ID。</Description></TextField> : null}
                <TextField value={lessonTitle} onChange={setLessonTitle} isRequired><Label>课堂标题</Label><Input placeholder="例如：第 4 讲 · 卷积" /><Description>课堂实际日期与时区由宿主后续返回。</Description></TextField>
              </div>
              {formError ? <div className="error-notice" role="alert">{formError}</div> : null}
            </Card.Content>
            <Card.Footer><Button onPress={() => void submit()} isPending={creating}>{creating ? '提交中…' : '创建并打开课程窗'}</Button><Button variant="secondary" onPress={onStartUnassigned}>先开始未归属录音</Button></Card.Footer>
          </Card>
          {notice ? <div className="inline-notice" role="status">{notice}{notice.includes('事件批次') ? <Button size="sm" variant="outline" onPress={onRetryEvents}>重试未确认事件</Button> : null}</div> : null}
        </main>
      ) : (
        <main className="native-page page-shell">
          <header className="native-workspace-header"><div><Button size="sm" variant="ghost" onPress={onBack}>← 返回</Button><p className="eyebrow">正式课程窗 · 宿主数据</p><h1>{bindingLabel}</h1><p className="native-workspace-subtitle">内容窗、录音归属、资料版本和来源状态都以 native 宿主返回为准。</p></div><div className="native-workspace-header-status"><StatusBadge tone={lesson ? 'success' : 'warning'} dot>{lesson ? '已绑定课堂' : '未归属录音'}</StatusBadge><span>views {views.filter((view) => view.state !== 'closed').length}/6 · heads {contentHeadCount}</span></div></header>
          {notice ? <div className="inline-notice" role="status">{notice}{notice.includes('事件批次') ? <Button size="sm" variant="outline" onPress={onRetryEvents}>重试未确认事件</Button> : null}</div> : null}
          <NativeLayerSummary projections={projections} />
          <section className="native-content-surface" aria-labelledby="native-content-title"><div className="native-surface-heading"><div><p className="eyebrow">ContentView · content.get projection</p><h2 id="native-content-title">课堂内容窗</h2><p>主窗显示当前真实 projection，其余 view 保持独立状态；空态只说明宿主尚未返回内容。</p></div><div className="native-surface-actions"><Button size="sm" variant="ghost" onPress={onOpenSearch}>搜索当前课堂</Button><Button size="sm" variant="secondary" onPress={() => onOpenView('grid')}>打开六格 view</Button><Button size="sm" variant="ghost" onPress={() => onOpenView('standalone')}>独立窗口</Button></div></div><div className={`native-pane-grid ${expandedPrimary ? 'is-primary-expanded' : ''}`} aria-label="正式 native 内容窗">{paneSlots.map((slot) => <NativePane key={slot.id} slot={slot} adapter={adapter} onOpenView={onOpenView} onOpenSearch={onOpenSearch} />)}</div></section>
          <MaterialStatePanel lessonId={lesson?.lesson_id ?? null} projections={projections} views={views} activeView={activeView} importAdapter={materialImportAdapter} />
          {!lesson ? <div className="native-empty-action"><Button onPress={onBack}>返回创建课堂</Button></div> : null}
        </main>
      )}
      <NativeSearchPanel adapter={adapter} open={searchOpen} view={activeView} onClose={onCloseSearch} />
      <footer className="app-footer"><span>契约 2.0.0 · native adapter</span><span>recording watermark 只来自宿主 durable_watermark_ms</span></footer>
    </div>
  );
}


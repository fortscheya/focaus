import {Button, Card} from '@heroui/react';
import type {HostAdapter, RecordingState} from '@focaus/contracts';
import {FocausPanel} from '../focaus/FocausPanel.tsx';
import {ContentEditor} from '../editor/ContentEditor.tsx';
import {VisualizationPanel} from '../visualizations/VisualizationPanel.tsx';
import {workspaceViews, type WorkspaceViewFixture} from '../app/fixtures.ts';
import {StatusBadge} from './StatusBadge.tsx';
import {RecordingBar} from './RecordingBar.tsx';

function kindIcon(kind: WorkspaceViewFixture['kind']): string {
  if (kind === 'note') return 'N';
  if (kind === 'raw') return 'R';
  if (kind === 'secondary') return 'S';
  if (kind === 'material') return 'M';
  if (kind === 'focaus') return '↳';
  return '◇';
}

function SecondaryView() {
  return (
    <section className="transcript-panel" data-anchor="anchor-secondary" aria-labelledby="secondary-title">
      <div className="panel-heading-row"><div><p className="eyebrow">原话修剪 · 二次层</p><h2 id="secondary-title">二次转写</h2></div><StatusBadge tone="warning">2 条订正待采纳</StatusBadge></div>
      <div className="transcript-callout"><strong>原始底稿保持不变。</strong><span>二次层只保存原话修剪、说话人分段与用户采纳结果；不会把第三人称摘要写回转写。</span></div>
      <div className="transcript-segment"><div className="transcript-time">00:05:12.400</div><p>这个卷积核在每个位置移动，窗口向右走一个步幅。</p><div className="transcript-meta"><StatusBadge tone="danger">原始时间</StatusBadge><span>来源：原始句 13 · recording epoch 3</span></div></div>
      <div className="transcript-segment transcript-segment-redline"><div className="transcript-time">00:05:17.020</div><p><del>步幅为 2。</del> <ins>步幅为 1。</ins></p><div className="transcript-meta"><StatusBadge tone="warning">关键红线</StatusBadge><span>等待用户明确采纳 · 不自动写入笔记</span></div></div>
      <div className="transcript-footer"><span>可搜索覆盖：已索引当前段</span><Button size="sm" variant="outline">在编辑器中打开订正</Button></div>
    </section>
  );
}

function RawTranscriptView() {
  return (
    <section className="transcript-panel" data-anchor="anchor-raw" aria-labelledby="raw-title">
      <div className="panel-heading-row"><div><p className="eyebrow">原始证据 · 只读底稿</p><h2 id="raw-title">原始转写</h2></div><StatusBadge tone="danger" dot>实时段追加中</StatusBadge></div>
      <div className="transcript-callout"><strong>原始时间是唯一可回到音频的坐标。</strong><span>实时追加只增量更新可得范围，不能让搜索或生成读取未来音频。</span></div>
      {[['00:05:08.040', '我们现在把这个输入分成局部窗口。'], ['00:05:12.400', '这个窗口向右移动一个步幅。'], ['00:05:16.020', '接着对窗口和卷积核做逐项相乘。']].map(([time, text]) => <div className="transcript-segment" key={time}><div className="transcript-time">{time}</div><p>{text}</p><div className="transcript-meta"><StatusBadge tone="danger">teacher_said</StatusBadge><span>可定位 · 试听入口在录音暂停后开放</span></div></div>)}
      <div className="transcript-footer"><span>当前录音正在进行时，回听入口保持禁用。</span><Button size="sm" variant="tertiary" isDisabled>播放原声</Button></div>
    </section>
  );
}

function MaterialView() {
  return (
    <section className="material-panel" data-anchor="anchor-material" aria-labelledby="material-title">
      <div className="panel-heading-row"><div><p className="eyebrow">资料来源 · 当前课堂范围</p><h2 id="material-title">课件</h2></div><StatusBadge tone="accent">focaus.material/2.0.0</StatusBadge></div>
      <div className="material-preview-layout">
        <div className="material-page-preview" aria-label="课件第 4 页预览"><div className="material-page-topline">PROBABILITY / LECTURE 04</div><div className="material-equation">y[i,j] = Σ x[i+m,j+n] · k[m,n]</div><div className="material-window-grid"><span>1</span><span>2</span><span>4</span><span className="is-focus">5</span></div><div className="material-page-caption">局部窗口与输出格的对应</div></div>
        <Card className="material-source-card" variant="secondary"><Card.Header><Card.Title>来源与覆盖</Card.Title><Card.Description>原页与重构对象同源一次计数</Card.Description></Card.Header><Card.Content><dl className="detail-list"><div><dt>来源版本</dt><dd>source v3 · parse v2</dd></div><div><dt>当前页</dt><dd>第 4 页 · region 2</dd></div><div><dt>提取</dt><dd>native text + OCR</dd></div><div><dt>覆盖</dt><dd><StatusBadge tone="warning">partial · 第 9 页未 OCR</StatusBadge></dd></div></dl></Card.Content><Card.Footer><Button size="sm" variant="outline">打开原页锚点</Button><Button size="sm" variant="ghost">查看旧版本 diff</Button></Card.Footer></Card>
      </div>
      <div className="material-object-strip"><span>可编辑对象</span><StatusBadge tone="success">公式</StatusBadge><StatusBadge tone="success">表格</StatusBadge><StatusBadge tone="warning">结构图 partial</StatusBadge><span className="muted-copy">未知内容保留原图并显示未完成原因。</span></div>
    </section>
  );
}

export function CourseWorkspace({
  adapter,
  selectedView,
  onSelectView,
  recordingState,
  elapsedMs,
  watermarkMs,
  assignmentEpoch,
  onToggleRecording,
  onStopRecording,
  moveState,
  onMoveView,
  onBack,
  mode,
  eventSeq,
}: {
  adapter: HostAdapter;
  selectedView: WorkspaceViewFixture;
  onSelectView: (view: WorkspaceViewFixture) => void;
  recordingState: RecordingState;
  elapsedMs: number;
  watermarkMs: number;
  assignmentEpoch: number;
  onToggleRecording: () => void;
  onStopRecording: () => void;
  moveState: 'grid' | 'preparing' | 'detached';
  onMoveView: () => void;
  onBack: () => void;
  mode: 'fixture' | 'native';
  eventSeq: number;
}) {
  return (
    <main className="workspace-page page-shell">
      <header className="workspace-header">
        <div className="workspace-header-leading">
          <Button size="sm" variant="ghost" onPress={onBack} aria-label="返回开始页">← 返回</Button>
          <div className="workspace-title-wrap"><p className="eyebrow">课程窗 · 已绑定课堂</p><h1>概率论 · 第 4 讲</h1></div>
          <StatusBadge tone="success" dot>当前课堂</StatusBadge>
        </div>
        <div className="workspace-header-trailing"><span className="binding-meta">2026-10-18 · Asia/Shanghai · 课堂时区已明确</span><Button size="sm" variant="outline" onPress={onMoveView}>{moveState === 'preparing' ? '准备移交…' : moveState === 'detached' ? '已拆出 · 放回' : '拆出当前窗'}</Button></div>
      </header>
      <RecordingBar state={recordingState} elapsedMs={elapsedMs} watermarkMs={watermarkMs} assignmentEpoch={assignmentEpoch} onToggle={onToggleRecording} onStop={onStopRecording} />
      <div className="workspace-binding-strip"><span className="binding-strip-mark" aria-hidden="true" /> <span>所有内容窗共享 <strong>lesson_id</strong>，录音通过 recording_id + assignment_epoch 归属；来源视图不会随左栏选择暗改绑定。</span><StatusBadge tone={mode === 'native' ? 'success' : 'warning'}>{mode === 'native' ? `native · events ${eventSeq}` : 'fixture adapter · 未连接宿主'}</StatusBadge></div>
      <section className="six-grid-section" aria-labelledby="views-title">
        <div className="section-heading-row compact"><div><p className="eyebrow">六格工作区</p><h2 id="views-title">当前课堂的六个内容窗</h2></div><span className="muted-copy">无固定标签布局 · 由内容和焦点表达身份</span></div>
        <div className="six-grid" aria-label="六格内容窗">
          {workspaceViews.map((view) => <button key={view.id} type="button" className={`view-cell view-cell-${view.kind} ${view.id === selectedView.id ? 'is-active' : ''} ${moveState === 'detached' && view.id === selectedView.id ? 'is-detached' : ''}`} onClick={() => onSelectView(view)} aria-label={`选择${view.title}内容窗`} aria-pressed={view.id === selectedView.id}><span className="view-cell-icon" aria-hidden="true">{kindIcon(view.kind)}</span><span className="view-cell-copy"><strong>{view.title}</strong><small>{view.description}</small></span><span className="view-cell-state">{view.id === selectedView.id ? '当前焦点' : '可聚焦'}</span></button>)}
        </div>
      </section>
      <section className="workspace-detail" aria-label={`${selectedView.title}详细内容`}>
        {selectedView.kind === 'note' ? <ContentEditor adapter={adapter} /> : null}
        {selectedView.kind === 'raw' ? <RawTranscriptView /> : null}
        {selectedView.kind === 'secondary' ? <SecondaryView /> : null}
        {selectedView.kind === 'material' ? <MaterialView /> : null}
        {selectedView.kind === 'focaus' ? <FocausPanel adapter={adapter} viewId={selectedView.id} /> : null}
        {selectedView.kind === 'visualization' ? <VisualizationPanel adapter={adapter} /> : null}
      </section>
      <footer className="workspace-footer"><span>view_id {selectedView.id} · view revision fixture-04</span><span>{moveState === 'detached' ? (mode === 'native' ? 'view.move → view.ready 已由宿主确认；草稿/焦点状态保留。' : 'view.move → view.ready 已完成演示；草稿/焦点状态保留。') : '拆出/放回由宿主 view.move / view.ready 持有生命周期。'}</span></footer>
    </main>
  );
}

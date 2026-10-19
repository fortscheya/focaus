import {useState} from 'react';
import {Button, Card} from '@heroui/react';
import type {ContentView} from '@focaus/contracts';
import type {NativeProjection} from '../nativeContentProjection.ts';
import {StatusBadge} from '../components/StatusBadge.tsx';
import type {MaterialImportAdapter, MaterialPurpose, MaterialTaskFeedback} from './materialAdapter.ts';
import {buildMaterialLayerStates, type MaterialLayerState} from './materialState.ts';

const layerLabels: Record<MaterialLayerState['layer'], string> = {
  original: '原件',
  primary: 'primary 基础文字',
  raw: '原始转写',
  secondary: '二次转写',
  material: 'material 结构化资料',
  content_view: 'content_view',
  unsupported: '宿主能力状态',
};

function toneFor(status: MaterialLayerState['status']): 'neutral' | 'success' | 'warning' | 'danger' | 'accent' {
  if (status === 'ready') return 'success';
  if (status === 'partial' || status === 'unknown') return 'warning';
  if (status === 'unsupported' || status === 'failed') return 'danger';
  return 'neutral';
}

function statusLabel(status: MaterialLayerState['status']): string {
  if (status === 'ready') return 'ready';
  if (status === 'partial') return 'partial';
  if (status === 'unsupported') return 'unsupported';
  if (status === 'failed') return 'failed';
  if (status === 'unknown') return 'unknown';
  return 'empty';
}

function taskTone(status: MaterialTaskFeedback['status']): 'neutral' | 'success' | 'warning' | 'danger' {
  if (status === 'completed') return 'success';
  if (status === 'failed' || status === 'error') return 'danger';
  if (status === 'partial' || status === 'running' || status === 'accepted') return 'warning';
  return 'neutral';
}

function taskLabel(status: MaterialTaskFeedback['status']): string {
  if (status === 'accepted') return '已受理';
  if (status === 'running') return '处理中';
  if (status === 'partial') return '部分已到达';
  if (status === 'completed') return '任务完成';
  if (status === 'failed') return '任务失败';
  if (status === 'error') return '导入错误';
  return '未开始';
}

function taskDescription(status: MaterialTaskFeedback['status']): string {
  if (status === 'accepted') return '宿主已创建资料任务，尚未产生完整 projection。';
  if (status === 'running') return '宿主正在处理资料；已受理不等于任务完成。';
  if (status === 'partial') return '宿主已发布部分页或区域；完整性仍以新的 content head 与 revision 为准。';
  if (status === 'completed') return '任务已进入终态；内容是否完整仍由当前 material projection 单独确认。';
  if (status === 'failed') return '宿主任务失败；已有 projection 保留，失败结果不会替换它。';
  if (status === 'error') return '导入请求返回错误；前端不会把错误状态当作已完成资料。';
  return '';
}

export function MaterialStatePanel({
  lessonId,
  projections,
  views,
  activeView,
  importAdapter,
  taskFeedback = null,
  onRefresh,
  fixture = false,
}: {
  lessonId: string | null;
  projections: Record<string, NativeProjection>;
  views: ContentView[];
  activeView: ContentView | null;
  importAdapter: MaterialImportAdapter | null;
  taskFeedback?: MaterialTaskFeedback | null;
  onRefresh?: () => Promise<void> | void;
  fixture?: boolean;
}) {
  const [purpose, setPurpose] = useState<MaterialPurpose>('courseware');
  const [importState, setImportState] = useState<MaterialTaskFeedback>({status: 'idle', taskId: null, materialId: null, sourceVersionId: null, parseVersionId: null, message: '', error: null});
  const states = buildMaterialLayerStates({projections, views, activeView});
  const hasLesson = Boolean(lessonId);
  const feedback = taskFeedback ?? importState;

  const requestImport = async () => {
    if (!lessonId || !importAdapter) {
      setImportState({status: 'error', taskId: null, materialId: null, sourceVersionId: null, parseVersionId: null, message: '等待宿主文件选择入口；前端不接收或猜测文件路径。', error: null});
      return;
    }
    setImportState({status: 'running', taskId: null, materialId: null, sourceVersionId: null, parseVersionId: null, message: '正在请求宿主文件选择…', error: null});
    const result = await importAdapter.requestMaterialImport({lessonId, purpose, pages: []});
    setImportState({
      status: result.status === 'accepted' ? 'accepted' : 'error',
      taskId: result.taskId ?? null,
      materialId: result.materialId ?? null,
      sourceVersionId: result.sourceVersionId ?? null,
      parseVersionId: result.parseVersionId ?? null,
      message: result.message,
      error: result.error ?? null,
    });
  };

  return (
    <section className="material-state-panel" aria-labelledby="material-state-title">
      <div className="material-state-heading">
        <div><p className="eyebrow">资料导入 · 三层内容边界</p><h2 id="material-state-title">材料与内容状态</h2></div>
        <div className="material-state-heading-actions">
          {fixture ? <StatusBadge tone="warning">fixture · 只读演示</StatusBadge> : <StatusBadge tone="success">native · content.get</StatusBadge>}
          {feedback.status !== 'idle' ? <StatusBadge tone={taskTone(feedback.status)}>{taskLabel(feedback.status)}</StatusBadge> : null}
          <select className="native-select" aria-label="资料用途" value={purpose} onChange={(event) => setPurpose(event.target.value as MaterialPurpose)}><option value="courseware">课件</option><option value="textbook">教材</option><option value="reference">参考资料</option></select>
          <Button size="sm" onPress={() => void requestImport()} isPending={feedback.status === 'running'} isDisabled={!hasLesson || fixture || feedback.status === 'accepted' || feedback.status === 'running'}>{fixture ? 'fixture 不导入' : '从宿主选择文件'}</Button>
          {onRefresh ? <Button size="sm" variant="ghost" onPress={() => void onRefresh()}>刷新宿主状态</Button> : null}
        </div>
      </div>
      <div className="material-import-boundary"><span className="material-import-lock" aria-hidden="true">▣</span><span>导入只传 lesson、用途和页范围给宿主 adapter；文件路径、授权和托管 source handle 由宿主控制。</span><span className="fixture-label">{importAdapter ? `adapter · ${importAdapter.mode}` : 'adapter · 未接线'}</span></div>
      {feedback.status !== 'idle' ? <div className={`material-task-summary material-task-summary-${feedback.status}`} role="status"><div className="material-task-summary-heading"><div><span className="eyebrow">宿主资料任务</span><strong>{taskLabel(feedback.status)}</strong></div><StatusBadge tone={taskTone(feedback.status)}>{feedback.status}</StatusBadge></div><p>{taskDescription(feedback.status)}</p>{feedback.message ? <p className="material-task-message">{feedback.message}</p> : null}<dl className="material-task-details">{feedback.taskId ? <div><dt>task</dt><dd>{feedback.taskId}</dd></div> : null}{feedback.materialId ? <div><dt>material</dt><dd>{feedback.materialId}</dd></div> : null}{feedback.sourceVersionId ? <div><dt>source</dt><dd>{feedback.sourceVersionId}</dd></div> : null}{feedback.parseVersionId ? <div><dt>parse</dt><dd>{feedback.parseVersionId}</dd></div> : null}</dl>{feedback.error ? <div className="material-task-error"><strong>{feedback.error.code ?? 'HOST_ERROR'}</strong><span>{feedback.error.message}</span><small>{[feedback.error.scope ? 'scope=' + feedback.error.scope : null, feedback.error.retryable === null ? null : 'retryable=' + feedback.error.retryable].filter((value): value is string => Boolean(value)).join(' · ')}</small></div> : null}</div> : null}
      <div className="material-layer-grid">
        {states.map((state) => (
          <Card key={state.layer} className="material-layer-card" variant="secondary">
            <Card.Header><div className="material-layer-title"><Card.Title>{layerLabels[state.layer]}</Card.Title><StatusBadge tone={toneFor(state.status)}>{statusLabel(state.status)}</StatusBadge></div><Card.Description>{state.reason ?? '状态由宿主投影返回；空态不等于已删除。'}</Card.Description></Card.Header>
            <Card.Content><dl className="material-layer-details"><div><dt>revision</dt><dd>{state.revisionId ?? '—'}</dd></div><div><dt>source</dt><dd>{state.sourceVersionId ?? '—'}</dd></div><div><dt>parse</dt><dd>{state.parseVersionId ?? '—'}</dd></div><div><dt>source refs</dt><dd>{state.sourceRefCount}</dd></div></dl></Card.Content>
          </Card>
        ))}
      </div>
      <p className="material-state-note">原件、primary、raw、secondary、material 与 content_view 的身份和版本分别显示；partial/unsupported 会保留原因，不以旧 revision 顶替当前状态。</p>
    </section>
  );
}


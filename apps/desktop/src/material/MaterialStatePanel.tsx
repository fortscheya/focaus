import {useState} from 'react';
import {Button, Card} from '@heroui/react';
import type {ContentView} from '@focaus/contracts';
import type {NativeProjection} from '../nativeContentProjection.ts';
import {StatusBadge} from '../components/StatusBadge.tsx';
import type {MaterialImportAdapter, MaterialPurpose} from './materialAdapter.ts';
import {buildMaterialLayerStates, type MaterialLayerState} from './materialState.ts';

const layerLabels: Record<MaterialLayerState['layer'], string> = {
  original: '原件',
  primary: 'primary 基础文字',
  raw: '原始转写',
  secondary: '二次转写',
  material: 'material 结构化资料',
  content_view: 'content_view',
};

function toneFor(status: MaterialLayerState['status']): 'neutral' | 'success' | 'warning' | 'danger' | 'accent' {
  if (status === 'ready') return 'success';
  if (status === 'partial') return 'warning';
  if (status === 'unsupported' || status === 'failed') return 'danger';
  return 'neutral';
}

function statusLabel(status: MaterialLayerState['status']): string {
  if (status === 'ready') return 'ready';
  if (status === 'partial') return 'partial';
  if (status === 'unsupported') return 'unsupported';
  if (status === 'failed') return 'failed';
  return 'empty';
}

export function MaterialStatePanel({
  lessonId,
  projections,
  views,
  activeView,
  importAdapter,
  fixture = false,
}: {
  lessonId: string | null;
  projections: Record<string, NativeProjection>;
  views: ContentView[];
  activeView: ContentView | null;
  importAdapter: MaterialImportAdapter | null;
  fixture?: boolean;
}) {
  const [purpose, setPurpose] = useState<MaterialPurpose>('courseware');
  const [importState, setImportState] = useState<{status: 'idle' | 'running' | 'accepted' | 'error'; message: string}>({status: 'idle', message: ''});
  const states = buildMaterialLayerStates({projections, views, activeView});
  const hasLesson = Boolean(lessonId);

  const requestImport = async () => {
    if (!lessonId || !importAdapter) {
      setImportState({status: 'error', message: '等待宿主文件选择入口；前端不接收或猜测文件路径。'});
      return;
    }
    setImportState({status: 'running', message: '正在请求宿主文件选择…'});
    const result = await importAdapter.requestMaterialImport({lessonId, purpose, pages: []});
    setImportState({status: result.status === 'accepted' ? 'accepted' : 'error', message: result.message});
  };

  return (
    <section className="material-state-panel" aria-labelledby="material-state-title">
      <div className="material-state-heading">
        <div><p className="eyebrow">资料导入 · 三层内容边界</p><h2 id="material-state-title">材料与内容状态</h2></div>
        <div className="material-state-heading-actions">
          {fixture ? <StatusBadge tone="warning">fixture · 只读演示</StatusBadge> : <StatusBadge tone="success">native · content.get</StatusBadge>}
          <select className="native-select" aria-label="资料用途" value={purpose} onChange={(event) => setPurpose(event.target.value as MaterialPurpose)}><option value="courseware">课件</option><option value="textbook">教材</option><option value="reference">参考资料</option></select>
          <Button size="sm" onPress={() => void requestImport()} isPending={importState.status === 'running'} isDisabled={!hasLesson || fixture}>{fixture ? 'fixture 不导入' : '从宿主选择文件'}</Button>
        </div>
      </div>
      <div className="material-import-boundary"><span className="material-import-lock" aria-hidden="true">▣</span><span>导入只传 lesson、用途和页范围给宿主 adapter；文件路径、授权和托管 source handle 由宿主控制。</span><span className="fixture-label">{importAdapter ? `adapter · ${importAdapter.mode}` : 'adapter · 未接线'}</span></div>
      {importState.message ? <div className={importState.status === 'error' ? 'error-notice' : 'inline-notice'} role="status">{importState.message}</div> : null}
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


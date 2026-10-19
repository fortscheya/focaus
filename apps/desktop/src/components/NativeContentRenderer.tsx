import {useEffect, useMemo, useState} from 'react';
import {Button} from '@heroui/react';
import type {ContentView, HostAdapter, SourceRef} from '@focaus/contracts';
import {makeEnvelope} from '../app/hostAdapter.ts';
import type {NativeProjection} from '../nativeContentProjection.ts';
import {StatusBadge} from './StatusBadge.tsx';
import {extractNativePageSourceRefs, extractNativeProjectionActions, type NativeProjectionActions, type NativeTextPatchTarget} from './nativeProjectionActions.ts';
import {NativeVisualization2D} from '../visualizations/NativeVisualization2D.tsx';

function objectRecord(value: unknown): Record<string, unknown> | null {
  return value && typeof value === 'object' ? value as Record<string, unknown> : null;
}

function textFromRuns(value: unknown): string {
  const object = objectRecord(value);
  const runs = Array.isArray(object?.runs) ? object.runs : [];
  return runs.map((run) => {
    const record = objectRecord(run);
    return typeof record?.text === 'string' ? record.text : '';
  }).join('');
}

function ObjectPreview({object, adapter, view}: {object: unknown; adapter?: HostAdapter; view: ContentView | null}) {
  const record = objectRecord(object);
  if (!record) return null;
  const type = typeof record.type === 'string' ? record.type : 'unknown';
  const content = record.content;
  if (type === 'text') return <div className="native-object-preview"><StatusBadge tone="success">text</StatusBadge><p>{textFromRuns(content)}</p></div>;
  if (type === 'formula') {
    const formula = objectRecord(content);
    return <div className="native-object-preview"><StatusBadge tone="accent">formula</StatusBadge><p className="native-formula-text">{typeof formula?.latex === 'string' ? formula.latex : '公式内容不可用'}</p></div>;
  }
  if (type === 'table') {
    const table = objectRecord(content);
    const cells = Array.isArray(table?.cells) ? table.cells : [];
    return <div className="native-object-preview"><StatusBadge tone="success">table</StatusBadge><div className="native-table-preview">{cells.slice(0, 12).map((cell, index) => {const item = objectRecord(cell); return <span key={String(item?.cell_id ?? index)}>{item?.status === 'unknown' ? '未知' : String(item?.value ?? '—')}</span>;})}</div></div>;
  }
  if (type === 'data_chart') {
    const chart = objectRecord(content);
    const series = Array.isArray(chart?.series) ? chart.series : [];
    return <div className="native-object-preview"><StatusBadge tone="accent">data_chart</StatusBadge><p>{typeof chart?.chart_kind === 'string' ? chart.chart_kind : '图表'} · {series.length} 个 series</p><div className="native-chart-preview">{series.slice(0, 4).map((seriesItem, index) => {const item = objectRecord(seriesItem); const points = Array.isArray(item?.points) ? item.points : []; return <span key={String(item?.series_id ?? index)} style={{height: `${Math.min(92, 24 + points.length * 8)}%`}} />;})}</div></div>;
  }
  if (type === 'structure_diagram') {
    const diagram = objectRecord(content);
    const nodes = Array.isArray(diagram?.nodes) ? diagram.nodes : [];
    const edges = Array.isArray(diagram?.edges) ? diagram.edges : [];
    return <div className="native-object-preview"><StatusBadge tone="accent">structure_diagram</StatusBadge><p>{nodes.length} 个节点 · {edges.length} 条边</p><div className="native-diagram-preview">{nodes.slice(0, 5).map((node, index) => {const item = objectRecord(node); return <span key={String(item?.node_id ?? index)}>{String(item?.label ?? '节点')}</span>;})}</div></div>;
  }
  if (type === 'interactive_visualization') {
    const visualization = objectRecord(content);
    if (!adapter) return <div className="native-object-preview"><StatusBadge tone="warning">native only</StatusBadge><p>交互组件需要真实 native adapter 才能请求 N 计算。</p></div>;
    return <div className="native-object-preview"><NativeVisualization2D value={visualization} adapter={adapter} view={view} /></div>;
  }
  return <div className="native-object-preview"><StatusBadge tone="warning">{type}</StatusBadge><p>该对象暂不支持编辑展示，原始结构已保留。</p></div>;
}

function ProjectionState({projection}: {projection: NativeProjection}) {
  const value = objectRecord(projection.value);
  const reason = value?.reason ?? value?.unsupported_reason ?? value?.error ?? value?.completion_reason;
  if (projection.status === 'unsupported') return <div className="native-projection-empty"><StatusBadge tone="danger">unsupported</StatusBadge><p>{typeof reason === 'string' ? reason : '宿主返回了不支持的内容能力；原始数据仍保留，可等待能力协商或静态回退。'}</p></div>;
  if (projection.status === 'failed') return <div className="native-projection-empty"><StatusBadge tone="danger">failed</StatusBadge><p>{typeof reason === 'string' ? reason : '该内容投影失败；不会用 fixture 或旧 revision 顶替。'}</p></div>;
  if (projection.status === 'partial') return <div className="native-projection-banner"><StatusBadge tone="warning">partial</StatusBadge><span>内容只覆盖宿主已完成的页/区域；未处理范围仍明确保留。</span></div>;
  return null;
}

function responseError(response: {status: string; error?: {message?: string; code?: string}}): string | null {
  return response.status === 'error' ? response.error?.message ?? response.error?.code ?? '宿主命令失败。' : null;
}

function safeErrorLabel(value: unknown): string | null {
  const item = objectRecord(value);
  if (!item) return typeof value === 'string' ? value : null;
  const code = typeof item.code === 'string' ? item.code : null;
  const message = typeof item.message === 'string' ? item.message : null;
  const scope = typeof item.scope === 'string' ? item.scope : null;
  const retryable = typeof item.retryable === 'boolean' ? item.retryable : null;
  const metadata = [scope ? 'scope=' + scope : null, retryable === null ? null : 'retryable=' + retryable].filter((entry): entry is string => Boolean(entry)).join(', ');
  if (!code && !message && !metadata) return null;
  return [code, message].filter((entry): entry is string => Boolean(entry)).join(': ') + (metadata ? ` (${metadata})` : '');
}

function pageTone(status: unknown): 'neutral' | 'success' | 'warning' | 'danger' {
  if (status === 'complete') return 'success';
  if (status === 'failed' || status === 'unsupported') return 'danger';
  if (status === 'partial' || status === 'basic_ready') return 'warning';
  return 'neutral';
}

function CommandFeedback({message}: {message: string}) {
  return message ? <p className="native-command-feedback" role="status">{message}</p> : null;
}

function PageSourceAction({sourceRefs, adapter, view}: {sourceRefs: SourceRef[]; adapter?: HostAdapter; view: ContentView | null}) {
  const [working, setWorking] = useState(false);
  const [message, setMessage] = useState('');
  const source = sourceRefs[0] ?? null;
  const openOriginalPage = async () => {
    if (!source || !adapter) return;
    setWorking(true); setMessage('');
    const response = await adapter.invoke('content.get', {entity_id: source.entity_id, revision_id: source.revision_id, selector: source.selector}, makeEnvelope('content.get', view?.host_window_id, {[source.entity_id]: source.revision_id}));
    setWorking(false);
    const error = responseError(response);
    setMessage(error ? `原页读取失败：${error}` : response.status === 'accepted' ? `已提交原页读取，等待宿主返回 ${source.entity_id} · ${source.revision_id}。` : `已请求原页 ${source.entity_id} · ${source.revision_id}。`);
  };
  return <div className="native-page-source-action"><Button size="sm" variant="ghost" isDisabled={!source || !adapter || working} isPending={working} onPress={() => void openOriginalPage()}>查看原页{sourceRefs.length > 1 ? `（${sourceRefs.length}）` : ''}</Button>{source ? <span>{source.entity_id} · {source.revision_id}</span> : <span>原页待真实 material_region SourceRef</span>}{message ? <p className="native-command-feedback" role="status">{message}</p> : null}</div>;
}

function SourceActions({actions, adapter, view, projection}: {actions: NativeProjectionActions; adapter: HostAdapter; view: ContentView | null; projection: NativeProjection}) {
  const [message, setMessage] = useState('');
  const [working, setWorking] = useState<string | null>(null);
  const [selectionWorking, setSelectionWorking] = useState(false);
  const canAct = projection.status === 'complete';
  const sourceRefs = actions.sourceRefs.slice(0, 3);
  const canResolveSource = sourceRefs.length > 0;
  const sourceLabel = actions.sourceRefs.length > 3 ? `来源（${actions.sourceRefs.length}）` : '查看来源';
  const addSelectionReason = !actions.noteId ? '当前 projection 没有合法 note_id，请先打开真实笔记。' : !actions.noteRevisionId ? '当前 projection 缺少 note revision，不能安全提交写入。' : !actions.sourceRefs.length || !actions.objectIds.length ? '需要 projection 同时返回可用 SourceRef 和 complete object_id。' : projection.status !== 'complete' ? `当前 projection 为 ${projection.status}，加入笔记待完整 revision。` : '';

  const resolveSource = async (index: number) => {
    const source = sourceRefs[index];
    if (!source) return;
    setWorking(`${source.entity_id}:${source.revision_id}`); setMessage('');
    const response = await adapter.invoke('content.get', {entity_id: source.entity_id, revision_id: source.revision_id, selector: source.selector}, makeEnvelope('content.get', view?.host_window_id, {[source.entity_id]: source.revision_id}));
    setWorking(null);
    const error = responseError(response);
    setMessage(error ? `来源读取失败：${error}` : `已请求来源 ${source.entity_id} · revision ${source.revision_id}，等待宿主返回内容。`);
  };

  const addSelection = async () => {
    if (!actions.noteId || !actions.noteRevisionId || !actions.sourceRefs.length || !actions.objectIds.length || !canAct) return;
    setSelectionWorking(true); setMessage('');
    const expectedRevisions = actions.noteRevisionId ? {[actions.noteId]: actions.noteRevisionId} : {};
    const response = await adapter.invoke('note.add_selection', {note_id: actions.noteId, sources: actions.sourceRefs, object_ids: actions.objectIds}, makeEnvelope('note.add_selection', view?.host_window_id, expectedRevisions));
    setSelectionWorking(false);
    const error = responseError(response);
    setMessage(error ? `加入笔记失败：${error}` : `已向宿主提交 ${actions.objectIds.length} 个真实对象，等待笔记 projection 更新。`);
  };

  return <div className="native-projection-actions" aria-label="真实 projection 操作"><div className="native-projection-action-row"><span className="native-action-label">来源与取材</span>{sourceRefs.map((source, index) => <Button key={`${source.entity_id}:${source.revision_id}`} size="sm" variant="ghost" isDisabled={!canResolveSource || working !== null} isPending={working === `${source.entity_id}:${source.revision_id}`} onPress={() => void resolveSource(index)}>{sourceRefs.length > 1 ? `${sourceLabel} ${index + 1}` : sourceLabel}</Button>)}<Button size="sm" variant="secondary" isDisabled={!actions.noteId || !actions.noteRevisionId || !actions.sourceRefs.length || !actions.objectIds.length || !canAct} isPending={selectionWorking} onPress={() => void addSelection()}>加入笔记{actions.objectIds.length ? '（' + actions.objectIds.length + '）' : ''}</Button>{!sourceRefs.length ? <span className="native-action-disabled">当前 projection 没有合法 SourceRef</span> : null}{!actions.noteId && actions.objectIds.length ? <span className="native-action-disabled">缺少 note_id</span> : null}{actions.noteId && !actions.noteRevisionId && actions.objectIds.length ? <span className="native-action-disabled">缺少 note revision</span> : null}{addSelectionReason && actions.noteId && actions.noteRevisionId && actions.objectIds.length ? <span className="native-action-disabled">{addSelectionReason}</span> : null}</div><CommandFeedback message={message} /></div>;
}

function TextPatchEditor({target, adapter, view}: {target: NativeTextPatchTarget; adapter: HostAdapter; view: ContentView | null}) {
  const [draft, setDraft] = useState(target.text);
  const [message, setMessage] = useState('');
  const [working, setWorking] = useState(false);
  useEffect(() => {setDraft(target.text); setMessage('');}, [target.objectId, target.revisionId, target.text]);
  const submit = async () => {
    setWorking(true); setMessage('');
    const patch = {patch_id: crypto.randomUUID(), target_id: target.objectId, base_revision: target.revisionId, operation: 'text_replace' as const, selector: target.selector, before_hash: target.beforeHash, after_value: {text: draft}, author: 'user' as const, intent: '修改真实 native 文本对象', undo_group_id: crypto.randomUUID(), created_at: new Date().toISOString()};
    const response = await adapter.invoke('content.patch', {patch, lease_epoch: target.leaseEpoch}, makeEnvelope('content.patch', view?.host_window_id, {[target.objectId]: target.revisionId}));
    setWorking(false);
    const error = responseError(response);
    setMessage(error ? `保存失败：${error}` : '宿主已接受 content.patch，等待新的 projection revision。');
  };
  return <div className="native-text-edit"><div className="native-edit-heading"><span>可编辑真实文字对象</span><StatusBadge tone="accent">object {target.objectId}</StatusBadge></div><textarea aria-label={`编辑真实文字对象 ${target.objectId}`} value={draft} onChange={(event) => setDraft(event.target.value)} /><div className="native-edit-actions"><Button size="sm" isDisabled={working || draft === target.text} isPending={working} onPress={() => void submit()}>提交 content.patch</Button><span>base revision {target.revisionId}</span></div><CommandFeedback message={message} /></div>;
}

function CorrectionEditor({actions, adapter, view, projection}: {actions: NativeProjectionActions; adapter: HostAdapter; view: ContentView | null; projection: NativeProjection}) {
  const pending = actions.suggestions.filter((suggestion) => suggestion.state === 'pending').slice(0, 8);
  const [drafts, setDrafts] = useState<Record<string, string>>({});
  const [message, setMessage] = useState('');
  const [working, setWorking] = useState<string | null>(null);
  useEffect(() => {setDrafts(Object.fromEntries(pending.map((suggestion) => [suggestion.suggestion_id, suggestion.suggested_text])));}, [projection.key, projection.revisionId, pending.map((suggestion) => suggestion.suggestion_id).join('|')]);
  if (!pending.length) return null;
  const invoke = async (suggestion: typeof pending[number], action: 'draft' | 'accept' | 'reject') => {
    setWorking(`${suggestion.suggestion_id}:${action}`); setMessage('');
    const text = drafts[suggestion.suggestion_id] ?? suggestion.suggested_text;
    const response = action === 'draft'
      ? await adapter.invoke('correction.draft', {suggestion_id: suggestion.suggestion_id, draft: text, composition_active: false}, makeEnvelope('correction.draft', view?.host_window_id, {[suggestion.secondary_id]: suggestion.baseRevision}))
      : await adapter.invoke(action === 'accept' ? 'correction.accept' : 'correction.reject', {suggestion_id: suggestion.suggestion_id, base_revision: suggestion.baseRevision, final_text: action === 'reject' ? suggestion.original_text : text}, makeEnvelope(action === 'accept' ? 'correction.accept' : 'correction.reject', view?.host_window_id, {[suggestion.secondary_id]: suggestion.baseRevision}));
    setWorking(null);
    const error = responseError(response);
    setMessage(error ? `${action === 'draft' ? '订正草稿' : '订正提交'}失败：${error}` : action === 'draft' ? '草稿已交给宿主，未改变当前 projection 正文。' : '宿主已接受订正事务，等待新的 secondary projection。');
  };
  return <div className="native-correction-editor"><div className="native-edit-heading"><span>真实订正候选</span><StatusBadge tone="warning">{pending.length} pending</StatusBadge></div>{pending.map((suggestion) => <div className="native-correction-row" key={suggestion.suggestion_id}><div><span className="native-correction-original">原句：{suggestion.original_text}</span><small>{suggestion.reason} · {suggestion.semantic_risk}</small></div><textarea aria-label={`订正 ${suggestion.suggestion_id}`} value={drafts[suggestion.suggestion_id] ?? ''} onChange={(event) => setDrafts((current) => ({...current, [suggestion.suggestion_id]: event.target.value}))} /><div className="native-edit-actions"><Button size="sm" variant="ghost" isDisabled={working !== null} isPending={working === `${suggestion.suggestion_id}:draft`} onPress={() => void invoke(suggestion, 'draft')}>保存草稿</Button><Button size="sm" isDisabled={working !== null} isPending={working === `${suggestion.suggestion_id}:accept`} onPress={() => void invoke(suggestion, 'accept')}>应用</Button><Button size="sm" variant="tertiary" isDisabled={working !== null} isPending={working === `${suggestion.suggestion_id}:reject`} onPress={() => void invoke(suggestion, 'reject')}>保留原句</Button></div></div>)}<CommandFeedback message={message} /></div>;
}

function ProjectionActions({projection, adapter, view}: {projection: NativeProjection; adapter?: HostAdapter; view: ContentView | null}) {
  const actions = useMemo(() => extractNativeProjectionActions(projection), [projection]);
  if (!adapter) return null;
  const canAddToNote = projection.status === 'complete' && Boolean(actions.noteId && actions.noteRevisionId && actions.sourceRefs.length && actions.objectIds.length);
  const editableTargets = projection.status === 'complete' ? actions.textTargets : [];
  const editLabel = actions.textTargets.length && projection.status !== 'complete' ? `编辑待完整 ${projection.status} revision` : actions.textTargets.length ? '可编辑文本 ' + actions.textTargets.length : '只读或无编辑 lease';
  return <><div className="native-projection-capabilities" aria-label="真实 projection 能力"><StatusBadge tone={actions.sourceRefs.length ? 'success' : 'warning'}>来源 {actions.sourceRefs.length}</StatusBadge><StatusBadge tone={actions.objectIds.length ? 'accent' : 'neutral'}>可取对象 {actions.objectIds.length}</StatusBadge><StatusBadge tone={canAddToNote ? 'success' : 'neutral'}>{canAddToNote ? '可加入笔记' : '加入笔记待真实字段'}</StatusBadge><StatusBadge tone={editableTargets.length ? 'success' : actions.textTargets.length ? 'warning' : 'neutral'}>{editLabel}</StatusBadge></div><SourceActions actions={actions} adapter={adapter} view={view} projection={projection} />{editableTargets.map((target) => <TextPatchEditor key={target.objectId} target={target} adapter={adapter} view={view} />)}{projection.kind === 'secondary' && projection.status === 'complete' ? <CorrectionEditor actions={actions} adapter={adapter} view={view} projection={projection} /> : null}</>;
}

function MaterialPageRow({page, material, adapter, view}: {page: unknown; material: Record<string, unknown>; adapter?: HostAdapter; view: ContentView | null}) {
  const item = objectRecord(page);
  const pageId = typeof item?.page_id === 'string' ? item.page_id : null;
  const status = typeof item?.status === 'string' ? item.status : 'unknown';
  const errors = Array.isArray(item?.errors) ? item.errors.map(safeErrorLabel).filter((error): error is string => Boolean(error)) : [];
  const sourceRefs = extractNativePageSourceRefs(material, item);
  return <div className={`native-page-row native-page-row-${status}`}><div className="native-page-row-heading"><strong>{String(item?.display_label ?? '未命名页')}</strong><StatusBadge tone={pageTone(status)}>{status}</StatusBadge></div><div className="native-page-row-meta"><span>{pageId ? 'page ' + pageId : '缺少 page_id'}</span>{errors[0] ? <span className="native-page-error">{errors[0]}</span> : <span>{sourceRefs.length ? `已绑定 ${sourceRefs.length} 个原页 SourceRef` : '原页 SourceRef 未返回'}</span>}</div><PageSourceAction sourceRefs={sourceRefs} adapter={adapter} view={view} /></div>;
}

export function NativeContentRenderer({projection, adapter, view = null}: {projection: NativeProjection | null; adapter?: HostAdapter; view?: ContentView | null}) {
  if (!projection) return <div className="native-projection-empty"><div className="native-empty-icon" aria-hidden="true">·</div><p>宿主尚未返回该内容的真实投影。</p></div>;
  const value = objectRecord(projection.value);
  const projectionState = <ProjectionState projection={projection} />;
  const actions = <ProjectionActions projection={projection} adapter={adapter} view={view} />;
  if (projection.kind === 'content_view') return <div className="native-projection-empty"><StatusBadge tone="success">view ready</StatusBadge><p>ContentView 已准备，正文由其 target 的 content head 继续提供。</p></div>;
  if (projection.kind === 'material') {
    const pages = Array.isArray(value?.pages) ? value.pages : [];
    const objects = Array.isArray(value?.objects) ? value.objects : [];
    const searchCoverage = objectRecord(value?.search_coverage);
    return <div className="native-projection-body">{projectionState}{actions}<div className="native-material-meta"><span>format focaus.material/2.0.0</span><span>source {String(value?.source_version_id ?? 'unknown')}</span><span>parse {String(value?.parse_version_id ?? projection.revisionId ?? 'unknown')}</span><span>completion {String(value?.completion ?? projection.status)}</span><span>search {String(searchCoverage?.state ?? 'unknown')}</span><span>{pages.length} 页 · {objects.length} 个对象</span></div><div className="native-pages-list">{pages.slice(0, 8).map((page, index) => <MaterialPageRow key={String(objectRecord(page)?.page_id ?? index)} page={page} material={value ?? {}} adapter={adapter} view={view} />)}</div><div className="native-object-list">{objects.slice(0, 12).map((object, index) => <ObjectPreview key={String(objectRecord(object)?.object_id ?? index)} object={object} adapter={adapter} view={view} />)}</div></div>;
  }
  if (projection.kind === 'note') {
    const objects = Array.isArray(value?.objects) ? value.objects : [];
    return <div className="native-projection-body">{projectionState}{actions}<div className="native-material-meta"><span>note {String(value?.note_id ?? projection.key)}</span><span>revision {String(value?.revision_id ?? projection.revisionId ?? 'unknown')}</span><span>{objects.length} 个对象</span></div><div className="native-object-list">{objects.map((object, index) => <ObjectPreview key={String(objectRecord(object)?.object_id ?? index)} object={object} adapter={adapter} view={view} />)}</div></div>;
  }
  if (projection.kind === 'raw' || projection.kind === 'secondary') {
    const listKey = projection.kind === 'raw' ? 'utterances' : 'segments';
    const entries = Array.isArray(value?.[listKey]) ? value[listKey] as unknown[] : [value];
    return <div className="native-projection-body">{projectionState}{actions}<div className="native-transcript-list">{entries.slice(0, 24).map((entry, index) => {const item = objectRecord(entry); const text = typeof item?.text === 'string' ? item.text : '文本暂不可用'; const state = typeof item?.state === 'string' ? item.state : null; return <div className="native-transcript-row" key={String(item?.utterance_id ?? item?.secondary_id ?? index)}><span className="native-transcript-index">{typeof item?.ordinal === 'number' ? item.ordinal : index + 1}</span><div><p>{text}</p><div className="native-transcript-meta">{state ? <StatusBadge tone={state === 'final' ? 'success' : 'warning'}>{state}</StatusBadge> : null}<span>{projection.kind === 'raw' ? 'raw utterance · 原始时间由 audio_spans 定位' : 'secondary segment · 保留订正候选'}</span></div></div></div>;})}</div></div>;
  }
  return <div className="native-projection-empty"><StatusBadge tone="warning">{projection.status}</StatusBadge><p>宿主返回了结构化内容，但当前前端尚未识别其投影类型。</p></div>;
}


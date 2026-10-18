import {Card} from '@heroui/react';
import type {NativeProjection} from '../nativeContentProjection.ts';
import {StatusBadge} from './StatusBadge.tsx';

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

function ObjectPreview({object}: {object: unknown}) {
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
    const status = visualization?.generation_status === 'unsupported' ? 'unsupported' : visualization?.generation_status === 'ready' ? 'ready' : 'partial';
    return <div className="native-object-preview"><StatusBadge tone={status === 'ready' ? 'success' : status === 'unsupported' ? 'danger' : 'warning'}>{status}</StatusBadge><p>交互组件数据已保留，当前承载等待对应 visualization renderer。</p></div>;
  }
  return <div className="native-object-preview"><StatusBadge tone="warning">{type}</StatusBadge><p>该对象暂不支持编辑展示，原始结构已保留。</p></div>;
}

function ProjectionState({projection}: {projection: NativeProjection}) {
  if (projection.status === 'unsupported') return <div className="native-projection-empty"><StatusBadge tone="danger">unsupported</StatusBadge><p>宿主返回了不支持的内容能力；原始数据仍保留，可等待能力协商或静态回退。</p></div>;
  if (projection.status === 'failed') return <div className="native-projection-empty"><StatusBadge tone="danger">failed</StatusBadge><p>该内容投影失败；不会用 fixture 或旧 revision 顶替。</p></div>;
  if (projection.status === 'partial') return <div className="native-projection-banner"><StatusBadge tone="warning">partial</StatusBadge><span>内容只覆盖宿主已完成的页/区域；未处理范围仍明确保留。</span></div>;
  return null;
}

export function NativeContentRenderer({projection}: {projection: NativeProjection | null}) {
  if (!projection) return <div className="native-projection-empty"><div className="native-empty-icon" aria-hidden="true">·</div><p>宿主尚未返回该内容的真实投影。</p></div>;
  const value = objectRecord(projection.value);
  const projectionState = <ProjectionState projection={projection} />;
  if (projection.kind === 'content_view') return <div className="native-projection-empty"><StatusBadge tone="success">view ready</StatusBadge><p>ContentView 已准备，正文由其 target 的 content head 继续提供。</p></div>;
  if (projection.kind === 'material') {
    const pages = Array.isArray(value?.pages) ? value.pages : [];
    const objects = Array.isArray(value?.objects) ? value.objects : [];
    return <div className="native-projection-body">{projectionState}<div className="native-material-meta"><span>format focaus.material/2.0.0</span><span>parse {String(value?.parse_version_id ?? projection.revisionId ?? 'unknown')}</span><span>{pages.length} 页 · {objects.length} 个对象</span></div><div className="native-pages-list">{pages.slice(0, 8).map((page, index) => {const item = objectRecord(page); const status = typeof item?.status === 'string' ? item.status : 'unknown'; return <div key={String(item?.page_id ?? index)} className="native-page-row"><strong>{String(item?.display_label ?? `第 ${index + 1} 页`)}</strong><StatusBadge tone={status === 'complete' ? 'success' : status === 'unsupported' || status === 'failed' ? 'danger' : 'warning'}>{status}</StatusBadge></div>;})}</div><div className="native-object-list">{objects.slice(0, 12).map((object, index) => <ObjectPreview key={String(objectRecord(object)?.object_id ?? index)} object={object} />)}</div></div>;
  }
  if (projection.kind === 'note') {
    const objects = Array.isArray(value?.objects) ? value.objects : [];
    return <div className="native-projection-body">{projectionState}<div className="native-material-meta"><span>note {String(value?.note_id ?? projection.key)}</span><span>revision {String(value?.revision_id ?? projection.revisionId ?? 'unknown')}</span><span>{objects.length} 个对象</span></div><div className="native-object-list">{objects.map((object, index) => <ObjectPreview key={String(objectRecord(object)?.object_id ?? index)} object={object} />)}</div></div>;
  }
  if (projection.kind === 'raw' || projection.kind === 'secondary') {
    const listKey = projection.kind === 'raw' ? 'utterances' : 'segments';
    const entries = Array.isArray(value?.[listKey]) ? value[listKey] as unknown[] : [value];
    return <div className="native-projection-body">{projectionState}<div className="native-transcript-list">{entries.slice(0, 24).map((entry, index) => {const item = objectRecord(entry); const text = typeof item?.text === 'string' ? item.text : '文本暂不可用'; const state = typeof item?.state === 'string' ? item.state : null; return <div className="native-transcript-row" key={String(item?.utterance_id ?? item?.secondary_id ?? index)}><span className="native-transcript-index">{typeof item?.ordinal === 'number' ? item.ordinal : index + 1}</span><div><p>{text}</p><div className="native-transcript-meta">{state ? <StatusBadge tone={state === 'final' ? 'success' : 'warning'}>{state}</StatusBadge> : null}<span>{projection.kind === 'raw' ? 'raw utterance · 原始时间由 audio_spans 定位' : 'secondary segment · 保留订正候选'}</span></div></div></div>;})}</div></div>;
  }
  return <div className="native-projection-empty"><StatusBadge tone="warning">{projection.status}</StatusBadge><p>宿主返回了结构化内容，但当前前端尚未识别其投影类型。</p></div>;
}


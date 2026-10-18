import type {ContentView} from '@focaus/contracts';
import type {NativeProjection} from '../nativeContentProjection.ts';

export type MaterialLayer = 'original' | 'primary' | 'raw' | 'secondary' | 'material' | 'content_view';
export type MaterialLayerStatus = 'empty' | 'ready' | 'partial' | 'unsupported' | 'failed';

export interface MaterialLayerState {
  layer: MaterialLayer;
  status: MaterialLayerStatus;
  revisionId: string | null;
  sourceVersionId: string | null;
  parseVersionId: string | null;
  sourceRefCount: number;
  reason: string | null;
  projection: NativeProjection | null;
}

function objectRecord(value: unknown): Record<string, unknown> | null {
  return value && typeof value === 'object' ? value as Record<string, unknown> : null;
}

function sourceRefCount(value: unknown): number {
  const record = objectRecord(value);
  return Array.isArray(record?.source_refs) ? record.source_refs.length : 0;
}

function layerStatus(projection: NativeProjection | null): MaterialLayerStatus {
  if (!projection) return 'empty';
  if (projection.status === 'unsupported' || projection.kind === 'unsupported') return 'unsupported';
  if (projection.status === 'failed') return 'failed';
  if (projection.status === 'partial') return 'partial';
  return 'ready';
}

function projectionReason(projection: NativeProjection | null): string | null {
  if (!projection) return null;
  const record = objectRecord(projection.value);
  const reason = record?.reason ?? record?.unsupported_reason ?? record?.error ?? record?.completion_reason;
  return typeof reason === 'string' ? reason : projection.status === 'partial' ? '宿主尚未完成全部请求页/区域。' : projection.status === 'unsupported' ? '宿主能力协商未支持该内容。' : projection.status === 'failed' ? '宿主投影失败。' : null;
}

function stateFor(layer: MaterialLayer, projection: NativeProjection | null, sourceVersionId: string | null, parseVersionId: string | null): MaterialLayerState {
  const value = projection?.value;
  return {
    layer,
    status: layerStatus(projection),
    revisionId: projection?.revisionId ?? null,
    sourceVersionId,
    parseVersionId,
    sourceRefCount: sourceRefCount(value),
    reason: projectionReason(projection),
    projection,
  };
}

export function buildMaterialLayerStates({
  projections,
  views,
  activeView,
}: {
  projections: Record<string, NativeProjection>;
  views: ContentView[];
  activeView: ContentView | null;
}): MaterialLayerState[] {
  const materialProjection = Object.values(projections).find((projection) => projection.kind === 'material') ?? null;
  const noteProjection = Object.values(projections).find((projection) => projection.kind === 'note') ?? null;
  const unsupportedProjection = Object.values(projections).find((projection) => projection.kind === 'unsupported' || projection.status === 'unsupported') ?? null;
  const rawProjection = Object.values(projections).find((projection) => projection.kind === 'raw') ?? null;
  const secondaryProjection = Object.values(projections).find((projection) => projection.kind === 'secondary') ?? null;
  const materialValue = objectRecord(materialProjection?.value);
  const sourceVersionId = typeof materialValue?.source_version_id === 'string' ? materialValue.source_version_id : null;
  const parseVersionId = typeof materialValue?.parse_version_id === 'string' ? materialValue.parse_version_id : materialProjection?.revisionId ?? null;
  const hasContentView = Boolean(activeView ?? views[0]);
  const contentViewProjection: NativeProjection | null = hasContentView ? {
    key: activeView?.view_id ?? views[0]?.view_id ?? 'content-view',
    kind: 'content_view',
    revisionId: activeView?.view_revision ?? views[0]?.view_revision ?? null,
    status: 'complete',
    value: activeView ?? views[0],
  } : null;

  return [
    stateFor('original', materialProjection, sourceVersionId, null),
    stateFor('primary', noteProjection ?? unsupportedProjection ?? materialProjection, sourceVersionId, parseVersionId),
    stateFor('raw', rawProjection, null, rawProjection?.revisionId ?? null),
    stateFor('secondary', secondaryProjection, null, secondaryProjection?.revisionId ?? null),
    stateFor('material', materialProjection, sourceVersionId, parseVersionId),
    stateFor('content_view', contentViewProjection, sourceVersionId, activeView?.view_revision ?? null),
  ];
}

export class MaterialRevisionGate {
  private latest = new Map<string, number>();
  private revisions = new Map<string, string | null>();
  private sequence = 0;

  begin(key: string, revisionId: string | null = null): number {
    const token = ++this.sequence;
    this.latest.set(key, token);
    this.revisions.set(key, revisionId);
    return token;
  }

  accepts(key: string, token: number, actualRevisionId: string | null = null): boolean {
    if (this.latest.get(key) !== token) return false;
    const requestedRevisionId = this.revisions.get(key) ?? null;
    return requestedRevisionId === null || requestedRevisionId === actualRevisionId;
  }
}


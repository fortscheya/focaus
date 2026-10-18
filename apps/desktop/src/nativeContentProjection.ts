import type {Selector, SourceRef} from '@focaus/contracts';

export type NativeProjectionKind = 'note' | 'raw' | 'secondary' | 'material' | 'content_view' | 'unsupported' | 'unknown';

export interface NativeProjection {
  key: string;
  kind: NativeProjectionKind;
  revisionId: string | null;
  status: 'complete' | 'partial' | 'failed' | 'unsupported' | 'unknown';
  value: unknown;
}

function record(value: unknown): Record<string, unknown> | null {
  return value && typeof value === 'object' ? value as Record<string, unknown> : null;
}

function selector(value: unknown): Selector | null {
  const item = record(value);
  return item && (item.kind === 'audio' || item.kind === 'text' || item.kind === 'material_region' || item.kind === 'object' || item.kind === 'whole') ? item as Selector : null;
}

export function isNativeSourceRef(value: unknown): value is SourceRef {
  const item = record(value);
  return Boolean(item && typeof item.entity_id === 'string' && typeof item.revision_id === 'string' && typeof item.content_hash === 'string' && selector(item.selector));
}

const sourceContainerKeys = ['content', 'manifest', 'note', 'raw', 'secondary', 'result', 'value', 'objects', 'segments', 'utterances', 'source_refs', 'raw_refs', 'source_edges', 'sources', 'source', 'source_ref', 'copied_from', 'to'] as const;

/**
 * Source refs may be attached to a response envelope, an object, or a
 * SourceEdge's `to` field. Only contract-shaped refs are returned; IDs are
 * never inferred from an object or event ID.
 */
export function extractNativeSourceRefs(value: unknown): SourceRef[] {
  const refs = new Map<string, SourceRef>();
  const visited = new Set<object>();
  const visit = (candidate: unknown, depth: number) => {
    if (depth > 8 || candidate === null || candidate === undefined) return;
    if (isNativeSourceRef(candidate)) {
      const key = `${candidate.entity_id}:${candidate.revision_id}:${JSON.stringify(candidate.selector)}`;
      refs.set(key, candidate);
      return;
    }
    if (typeof candidate !== 'object') return;
    if (visited.has(candidate)) return;
    visited.add(candidate);
    if (Array.isArray(candidate)) {
      candidate.forEach((item) => visit(item, depth + 1));
      return;
    }
    const object = record(candidate);
    if (!object) return;
    sourceContainerKeys.forEach((key) => {
      if (key in object) visit(object[key], depth + 1);
    });
  };
  visit(value, 0);
  return [...refs.values()];
}

const revisionKeys = ['revision_id', 'content_revision_id', 'view_revision', 'parse_version_id'] as const;

function revisionFrom(value: unknown): string | null {
  const object = record(value);
  if (!object) return null;
  for (const key of revisionKeys) if (typeof object[key] === 'string') return object[key] as string;
  return null;
}

function unwrap(value: unknown): unknown {
  const object = record(value);
  if (!object) return value;
  for (const key of ['content', 'manifest', 'note', 'raw', 'secondary', 'result', 'value']) {
    if (key in object && object[key] !== null && object[key] !== undefined) return object[key];
  }
  return value;
}

/** Read the response/head revision without falling back to the requested revision. */
export function nativeProjectionRevision(value: unknown, fallback: string | null = null): string | null {
  return revisionFrom(value) ?? revisionFrom(unwrap(value)) ?? fallback;
}

function enrichWithBackfilledSources(value: unknown, sourceRefs: SourceRef[]): unknown {
  const object = record(value);
  if (!object || !sourceRefs.length) return value;
  const existing = extractNativeSourceRefs(value);
  if (existing.length >= sourceRefs.length && Array.isArray(object.source_refs)) return value;
  return {...object, source_refs: sourceRefs};
}

export function detectNativeProjection(value: unknown, key: string, revisionId: string | null = null): NativeProjection {
  const sourceRefs = extractNativeSourceRefs(value);
  const candidate = enrichWithBackfilledSources(unwrap(value), sourceRefs);
  const object = record(candidate);
  const statusValue = object?.status ?? object?.completion;
  const status: NativeProjection['status'] = statusValue === 'complete' ? 'complete' : statusValue === 'partial' ? 'partial' : statusValue === 'failed' ? 'failed' : statusValue === 'unsupported' ? 'unsupported' : 'unknown';
  const detectedRevision = nativeProjectionRevision(value, revisionId);
  if (object?.format === 'focaus.material' || Array.isArray(object?.pages) && Array.isArray(object?.objects)) return {key, kind: 'material', revisionId: detectedRevision, status: status === 'unknown' ? 'partial' : status, value: candidate};
  if (Array.isArray(object?.objects) && typeof object.note_id === 'string') return {key, kind: 'note', revisionId: detectedRevision, status, value: candidate};
  if (Array.isArray(object?.utterances) || typeof object?.utterance_id === 'string') return {key, kind: 'raw', revisionId: detectedRevision, status, value: candidate};
  if (Array.isArray(object?.segments) || typeof object?.secondary_id === 'string') return {key, kind: 'secondary', revisionId: detectedRevision, status, value: candidate};
  if (typeof object?.view_id === 'string' && typeof object?.view_revision === 'string') return {key, kind: 'content_view', revisionId: object.view_revision, status: 'complete', value: candidate};
  if (statusValue === 'unsupported') return {key, kind: 'unsupported', revisionId: detectedRevision, status: 'unsupported', value: candidate};
  return {key, kind: 'unknown', revisionId: detectedRevision, status, value: candidate};
}

export class NativeProjectionGate {
  private latest = new Map<string, number>();
  private counter = 0;

  begin(key: string, revisionId: string | null = null): number {
    const token = ++this.counter;
    this.latest.set(key, token);
    this.revisions.set(key, revisionId);
    return token;
  }

  private revisions = new Map<string, string | null>();

  accepts(key: string, token: number, actualRevisionId: string | null = null): boolean {
    if (this.latest.get(key) !== token) return false;
    const requestedRevisionId = this.revisions.get(key) ?? null;
    return requestedRevisionId === null || actualRevisionId === requestedRevisionId;
  }
}


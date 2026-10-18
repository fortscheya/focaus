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

function unwrap(value: unknown): unknown {
  const object = record(value);
  if (!object) return value;
  for (const key of ['content', 'manifest', 'note', 'raw', 'secondary', 'result', 'value']) {
    if (key in object && object[key] !== null && object[key] !== undefined) return object[key];
  }
  return value;
}

export function detectNativeProjection(value: unknown, key: string, revisionId: string | null = null): NativeProjection {
  const candidate = unwrap(value);
  const object = record(candidate);
  const statusValue = object?.status ?? object?.completion;
  const status: NativeProjection['status'] = statusValue === 'complete' ? 'complete' : statusValue === 'partial' ? 'partial' : statusValue === 'failed' ? 'failed' : statusValue === 'unsupported' ? 'unsupported' : 'unknown';
  if (object?.format === 'focaus.material' || Array.isArray(object?.pages) && Array.isArray(object?.objects)) return {key, kind: 'material', revisionId: typeof object.parse_version_id === 'string' ? object.parse_version_id : revisionId, status: status === 'unknown' ? 'partial' : status, value: candidate};
  if (Array.isArray(object?.objects) && typeof object.note_id === 'string') return {key, kind: 'note', revisionId: typeof object.revision_id === 'string' ? object.revision_id : revisionId, status, value: candidate};
  if (Array.isArray(object?.utterances) || typeof object?.utterance_id === 'string') return {key, kind: 'raw', revisionId: typeof object.revision_id === 'string' ? object.revision_id : revisionId, status, value: candidate};
  if (Array.isArray(object?.segments) || typeof object?.secondary_id === 'string') return {key, kind: 'secondary', revisionId: typeof object.revision_id === 'string' ? object.revision_id : revisionId, status, value: candidate};
  if (typeof object?.view_id === 'string' && typeof object?.view_revision === 'string') return {key, kind: 'content_view', revisionId: object.view_revision, status: 'complete', value: candidate};
  if (statusValue === 'unsupported') return {key, kind: 'unsupported', revisionId, status: 'unsupported', value: candidate};
  return {key, kind: 'unknown', revisionId, status, value: candidate};
}

export class NativeProjectionGate {
  private latest = new Map<string, number>();
  private counter = 0;

  begin(key: string): number {
    const token = ++this.counter;
    this.latest.set(key, token);
    return token;
  }

  accepts(key: string, token: number): boolean {
    return this.latest.get(key) === token;
  }
}


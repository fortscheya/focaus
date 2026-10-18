import type {CorrectionSuggestion, Selector, SourceRef, TextAnchor} from '@focaus/contracts';
import {extractNativeSourceRefs, type NativeProjection} from '../nativeContentProjection.ts';

export type NativeTextPatchTarget = {
  objectId: string;
  revisionId: string;
  beforeHash: string;
  selector: Selector;
  leaseEpoch: number;
  text: string;
};

export type NativeProjectionActions = {
  sourceRefs: SourceRef[];
  objectIds: string[];
  noteId: string | null;
  noteRevisionId: string | null;
  textTargets: NativeTextPatchTarget[];
  suggestions: Array<CorrectionSuggestion & {baseRevision: string}>;
};

function record(value: unknown): Record<string, unknown> | null {
  return value && typeof value === 'object' ? value as Record<string, unknown> : null;
}

function textAnchor(value: unknown): TextAnchor | null {
  const item = record(value);
  const range = record(item?.scalar_range);
  const rawElements = item?.element_ids;
  const elements = Array.isArray(rawElements) ? rawElements.filter((element): element is string => typeof element === 'string') : [];
  if (!item || !range || !Array.isArray(rawElements) || typeof range.start !== 'number' || typeof range.end !== 'number' || typeof item.left_element_id !== 'string' && item.left_element_id !== null || typeof item.right_element_id !== 'string' && item.right_element_id !== null || elements.length !== rawElements.length) return null;
  return {left_element_id: item.left_element_id, right_element_id: item.right_element_id, element_ids: elements, scalar_range: {start: range.start, end: range.end}};
}

function objectIds(value: unknown): string[] {
  if (!Array.isArray(value)) return [];
  return value.flatMap((entry) => {
    const item = record(entry);
    return item && typeof item.object_id === 'string' ? [item.object_id] : [];
  });
}

function textTargets(value: unknown): NativeTextPatchTarget[] {
  if (!Array.isArray(value)) return [];
  return value.flatMap((entry) => {
    const item = record(entry);
    const content = record(item?.content);
    const children = Array.isArray(item?.children) ? item.children.filter((child): child is string => typeof child === 'string') : [];
    const objectId = item && typeof item.object_id === 'string' ? item.object_id : null;
    const itemSelector = objectId && children.length ? {kind: 'object' as const, object_id: objectId, element_ids: children} : null;
    const revisionId = item && typeof item.revision_id === 'string' ? item.revision_id : null;
    const beforeHash = item && typeof item.content_hash === 'string' ? item.content_hash : null;
    const leaseEpoch = item && typeof item.lease_epoch === 'number' && Number.isInteger(item.lease_epoch) ? item.lease_epoch : null;
    if (!item || !objectId || item.type !== 'text' || item.editability !== 'native' || !itemSelector || !revisionId || !beforeHash || leaseEpoch === null || !content) return [];
    const text = Array.isArray(content.runs) ? content.runs.map((run) => record(run)?.text).filter((part): part is string => typeof part === 'string').join('') : '';
    return [{objectId, revisionId, beforeHash, selector: itemSelector, leaseEpoch, text}];
  });
}

function suggestions(value: unknown, fallbackRevision: string | null): Array<CorrectionSuggestion & {baseRevision: string}> {
  const item = record(value);
  const entries = Array.isArray(item?.segments) ? item.segments : item?.secondary_id ? [item] : [];
  return entries.flatMap((segment) => {
    const segmentRecord = record(segment);
    const baseRevision = typeof segmentRecord?.revision_id === 'string' ? segmentRecord.revision_id : fallbackRevision;
    const items = Array.isArray(segmentRecord?.suggestions) ? segmentRecord.suggestions : [];
    if (!baseRevision) return [];
    return items.flatMap((suggestion) => {
      const candidate = record(suggestion);
      const anchor = textAnchor(candidate?.anchor);
      if (!candidate || !anchor || typeof candidate.suggestion_id !== 'string' || typeof candidate.secondary_id !== 'string' || typeof candidate.source_revision !== 'string' || typeof candidate.original_text !== 'string' || typeof candidate.suggested_text !== 'string' || typeof candidate.reason !== 'string' || !['ordinary', 'critical'].includes(String(candidate.semantic_risk)) || !['pending', 'accepted', 'rejected', 'obsolete'].includes(String(candidate.state))) return [];
      return [{suggestion_id: candidate.suggestion_id, secondary_id: candidate.secondary_id, source_revision: candidate.source_revision, anchor, original_text: candidate.original_text, suggested_text: candidate.suggested_text, reason: candidate.reason, semantic_risk: candidate.semantic_risk as CorrectionSuggestion['semantic_risk'], state: candidate.state as CorrectionSuggestion['state'], baseRevision}];
    });
  });
}

export function extractNativeProjectionActions(projection: NativeProjection | null): NativeProjectionActions {
  if (!projection) return {sourceRefs: [], objectIds: [], noteId: null, noteRevisionId: null, textTargets: [], suggestions: []};
  const value = record(projection.value);
  const objects = Array.isArray(value?.objects) ? value.objects : [];
  const refs = extractNativeSourceRefs(projection.value);
  const uniqueRefs = new Map<string, SourceRef>();
  refs.forEach((source) => uniqueRefs.set(`${source.entity_id}:${source.revision_id}:${JSON.stringify(source.selector)}`, source));
  return {
    sourceRefs: [...uniqueRefs.values()],
    objectIds: objectIds(objects),
    noteId: typeof value?.note_id === 'string' ? value.note_id : null,
    noteRevisionId: typeof value?.revision_id === 'string' ? value.revision_id : null,
    textTargets: textTargets(objects),
    suggestions: suggestions(value, projection.revisionId),
  };
}

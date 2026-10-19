import {describe, expect, it} from 'vitest';
import {extractNativePageSourceRefs, extractNativeProjectionActions} from './nativeProjectionActions.ts';
import type {NativeProjection} from '../nativeContentProjection.ts';

const source = {entity_id: 'raw-1', revision_id: 'raw-r1', content_hash: 'hash-1', selector: {kind: 'whole' as const}};

describe('native projection action extraction', () => {
  it('only exposes selection/edit/correction actions when projection fields are complete', () => {
    const projection: NativeProjection = {key: 'note-1', kind: 'note', revisionId: 'note-r1', status: 'complete', value: {
      note_id: 'note-1', revision_id: 'note-r1', source_refs: [source],
      objects: [{object_id: 'object-1', source_refs: [source], type: 'text', status: 'complete', editability: 'native', revision_id: 'object-r1', content_hash: 'object-hash', lease_epoch: 4, children: ['element-1'], content: {runs: [{text: '真实正文'}]}}],
    }};
    const actions = extractNativeProjectionActions(projection);
    expect(actions.noteId).toBe('note-1');
    expect(actions.noteRevisionId).toBe('note-r1');
    expect(actions.objectIds).toEqual(['object-1']);
    expect(actions.sourceRefs).toHaveLength(1);
    expect(actions.textTargets[0]).toMatchObject({objectId: 'object-1', leaseEpoch: 4, text: '真实正文'});
  });

  it('does not invent note, object, source or correction targets for incomplete payloads', () => {
    const projection: NativeProjection = {key: 'secondary-1', kind: 'secondary', revisionId: 'secondary-r1', status: 'partial', value: {segments: [{revision_id: 'secondary-r1', suggestions: [{suggestion_id: 'suggestion-1', secondary_id: 'secondary-1', source_revision: 'raw-r1', original_text: '原句', suggested_text: '建议', reason: '依据', semantic_risk: 'critical', state: 'pending'}]}]}};
    const actions = extractNativeProjectionActions(projection);
    expect(actions.noteId).toBeNull();
    expect(actions.noteRevisionId).toBeNull();
    expect(actions.objectIds).toEqual([]);
    expect(actions.sourceRefs).toEqual([]);
    expect(actions.textTargets).toEqual([]);
    expect(actions.suggestions).toEqual([]);
  });

  it('does not expose partial objects as writable note or text targets', () => {
    const projection: NativeProjection = {key: 'note-1', kind: 'note', revisionId: 'note-r1', status: 'complete', value: {
      note_id: 'note-1', revision_id: 'note-r1', source_refs: [source],
      objects: [{object_id: 'object-1', source_refs: [source], type: 'text', status: 'partial', editability: 'native', revision_id: 'object-r1', content_hash: 'object-hash', lease_epoch: 4, children: ['element-1'], content: {runs: [{text: '不完整正文'}]}}],
    }};
    const actions = extractNativeProjectionActions(projection);
    expect(actions.objectIds).toEqual([]);
    expect(actions.textTargets).toEqual([]);
    expect(actions.noteRevisionId).toBe('note-r1');
  });

  it('keeps real raw refs from a secondary segment available for source actions', () => {
    const projection: NativeProjection = {key: 'secondary-1', kind: 'secondary', revisionId: 'secondary-r1', status: 'complete', value: {segments: [{secondary_id: 'secondary-1', revision_id: 'secondary-r1', raw_refs: [source], text: '二次段'}]}};
    expect(extractNativeProjectionActions(projection).sourceRefs).toEqual([source]);
  });

  it('only exposes an original-page jump when a real material_region ref names that page', () => {
    const page = {page_id: 'page-1', region_ids: ['region-1'], reading_order: ['object-1']};
    const pageSource = {...source, selector: {kind: 'material_region' as const, material_id: 'material-1', source_version_id: 'source-1', parse_version_id: 'parse-1', page_id: 'page-1', region_id: 'region-1', bbox: [0, 0, 1, 1] as [number, number, number, number], coordinate_space: 'rotated_page_normalized' as const}};
    const refs = extractNativePageSourceRefs({objects: [{object_id: 'object-1', source_refs: [pageSource]}]}, page);
    expect(refs).toEqual([pageSource]);
    expect(extractNativePageSourceRefs({objects: [{object_id: 'object-1', source_refs: [pageSource]}]}, {...page, page_id: 'page-2'})).toEqual([]);
  });
});

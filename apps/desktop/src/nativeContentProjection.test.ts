import {describe, expect, it} from 'vitest';
import {detectNativeProjection, extractNativeSourceRefs, NativeProjectionGate} from './nativeContentProjection.ts';

describe('native content projection parsing', () => {
  it('preserves material partial and unsupported semantics', () => {
    const material = detectNativeProjection({format: 'focaus.material', format_version: '2.0.0', parse_version_id: 'parse-1', completion: 'partial', pages: [], objects: [], resources: []}, 'material-1');
    expect(material.kind).toBe('material');
    expect(material.status).toBe('partial');

    const unsupported = detectNativeProjection({status: 'unsupported', reason: 'unknown template'}, 'object-1');
    expect(unsupported.kind).toBe('unsupported');
    expect(unsupported.status).toBe('unsupported');
  });

  it('drops stale asynchronous projection responses', () => {
    const gate = new NativeProjectionGate();
    const first = gate.begin('view-1', 'revision-1');
    const second = gate.begin('view-1', 'revision-2');
    expect(gate.accepts('view-1', first, 'revision-1')).toBe(false);
    expect(gate.accepts('view-1', second, 'revision-1')).toBe(false);
    expect(gate.accepts('view-1', second, 'revision-2')).toBe(true);
  });

  it('backfills only contract-shaped source refs from response envelopes and source edges', () => {
    const source = {entity_id: 'source-1', revision_id: 'source-r1', content_hash: 'hash-1', selector: {kind: 'whole' as const}};
    const refs = extractNativeSourceRefs({content: {objects: [{source_edges: [{to: source}]}]}, sources: [source], source_id: 'do-not-infer'});
    expect(refs).toEqual([source]);
    expect(detectNativeProjection({revision_id: 'head-r1', content: {note_id: 'note-1', objects: []}, sources: [source]}, 'note-1').revisionId).toBe('head-r1');
  });
});


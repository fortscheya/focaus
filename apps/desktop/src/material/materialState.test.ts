import {describe, expect, it} from 'vitest';
import {buildMaterialLayerStates, MaterialRevisionGate} from './materialState.ts';
import type {NativeProjection} from '../nativeContentProjection.ts';

const partialMaterial: NativeProjection = {
  key: 'material-1',
  kind: 'material',
  revisionId: 'parse-2',
  status: 'partial',
  value: {format: 'focaus.material', source_version_id: 'source-2', parse_version_id: 'parse-2', completion: 'partial', source_refs: []},
};

describe('material projection state', () => {
  it('returns explicit empty layers without inventing a material result', () => {
    const states = buildMaterialLayerStates({projections: {}, views: [], activeView: null});
    expect(states).toHaveLength(6);
    expect(states.every((state) => state.status === 'empty')).toBe(true);
  });

  it('keeps partial and unsupported status visible with source/parse metadata', () => {
    const unsupported: NativeProjection = {key: 'unsupported', kind: 'unsupported', revisionId: 'r-1', status: 'unsupported', value: {reason: 'engine unavailable'}};
    const states = buildMaterialLayerStates({projections: {material: partialMaterial, unsupported}, views: [], activeView: null});
    const material = states.find((state) => state.layer === 'material');
    expect(material?.status).toBe('partial');
    expect(material?.sourceVersionId).toBe('source-2');
    expect(material?.parseVersionId).toBe('parse-2');
    expect(states.some((state) => state.status === 'unsupported')).toBe(true);
  });

  it('does not present another layer as primary and does not treat unknown projection as ready', () => {
    const unknownMaterial: NativeProjection = {key: 'unknown-material', kind: 'material', revisionId: 'r-unknown', status: 'unknown', value: {reason: 'new host projection'} };
    const states = buildMaterialLayerStates({projections: {material: partialMaterial}, views: [], activeView: null});
    expect(states.find((state) => state.layer === 'primary')?.status).toBe('empty');
    expect(states.find((state) => state.layer === 'primary')?.projection).toBeNull();
    expect(states.find((state) => state.layer === 'material')?.status).toBe('partial');
    expect(states.find((state) => state.layer === 'content_view')?.status).toBe('empty');
    const unknownStates = buildMaterialLayerStates({projections: {material: unknownMaterial}, views: [], activeView: null});
    expect(unknownStates.find((state) => state.layer === 'material')?.status).toBe('unknown');
  });

  it('drops an old async material revision before it can overwrite the latest request', () => {
    const gate = new MaterialRevisionGate();
    const oldRequest = gate.begin('material-1', 'parse-1');
    const newRequest = gate.begin('material-1', 'parse-2');
    expect(gate.accepts('material-1', oldRequest, 'parse-1')).toBe(false);
    expect(gate.accepts('material-1', newRequest, 'parse-1')).toBe(false);
    expect(gate.accepts('material-1', newRequest, 'parse-2')).toBe(true);
  });
});


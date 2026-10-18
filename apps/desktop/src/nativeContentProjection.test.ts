import {describe, expect, it} from 'vitest';
import {detectNativeProjection, NativeProjectionGate} from './nativeContentProjection.ts';

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
    const first = gate.begin('view-1');
    const second = gate.begin('view-1');
    expect(gate.accepts('view-1', first)).toBe(false);
    expect(gate.accepts('view-1', second)).toBe(true);
  });
});


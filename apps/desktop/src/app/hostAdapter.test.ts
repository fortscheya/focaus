import {describe, expect, it} from 'vitest';
import type {VisualizationEvaluateInput} from '@focaus/contracts';
import {FixtureHostAdapter, formatNativeError, makeEnvelope} from './hostAdapter.ts';

const parameters = {
  operator_dimension: 2 as const,
  scene_dimension: 3 as const,
  operator_semantics: 'cross_correlation' as const,
  axis_order: 'CHW' as const,
  input: [[[1, 2], [3, 4]]],
  kernel: [[[1, 0], [0, 1]]],
  stride: [1, 1],
  padding: [0, 0, 0, 0],
  dilation: [1, 1],
  bias: null,
};

function input(overrides: Partial<VisualizationEvaluateInput> = {}): VisualizationEvaluateInput {
  return {
    object_id: 'object-1',
    revision_id: 'revision-1',
    parameters,
    interaction_seq: 1,
    algorithm_version: '1.0.0',
    ...overrides,
  };
}

describe('fixture host visualization contract', () => {
  it('returns a parameter-dependent hash and complete 2D steps', async () => {
    const adapter = new FixtureHostAdapter();
    const first = await adapter.invoke('visualization.evaluate', input(), makeEnvelope('visualization.evaluate', undefined, {'object-1': 'revision-1'}));
    const second = await adapter.invoke('visualization.evaluate', input({parameters: {...parameters, bias: 1}, interaction_seq: 2}), makeEnvelope('visualization.evaluate', undefined, {'object-1': 'revision-1'}));
    expect(first.status).toBe('ok');
    expect(second.status).toBe('ok');
    const firstResult = first.result as {parameters_hash: string; validation: {parameters_hash: string; steps?: unknown[]}; values: number[]; steps: unknown[]};
    const secondResult = second.result as {parameters_hash: string; values: number[]};
    expect(firstResult.parameters_hash).toMatch(/^[0-9a-f]{64}$/);
    expect(firstResult.validation.parameters_hash).toBe(firstResult.parameters_hash);
    expect(firstResult.steps).toHaveLength(firstResult.values.length);
    expect(secondResult.parameters_hash).not.toBe(firstResult.parameters_hash);
    expect(secondResult.values).not.toEqual(firstResult.values);
  });

  it('returns a structured invalid-argument result for invalid stride', async () => {
    const adapter = new FixtureHostAdapter();
    const response = await adapter.invoke('visualization.evaluate', input({parameters: {...parameters, stride: [0, 0]}}), makeEnvelope('visualization.evaluate'));
    expect(response.status).toBe('error');
    expect(response.error?.code).toBe('INVALID_ARGUMENT');
  });
});

describe('native error formatting', () => {
  it('keeps structured SafeError fields visible instead of rendering [object Object]', () => {
    expect(formatNativeError({code: 'AUTH_REQUIRED', message: 'picker denied', scope: 'asset', retryable: false})).toBe('AUTH_REQUIRED: picker denied (scope=asset, retryable=false)');
    expect(formatNativeError({reason: 'fallback', nested: {value: 1}})).toBe('{"reason":"fallback","nested":{"value":1}}');
  });
});

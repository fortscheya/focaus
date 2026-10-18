import {describe, expect, it} from 'vitest';
import {evaluateConv2d, type Conv2dParameters} from './math.ts';

const base: Conv2dParameters = {
  operator_dimension: 2,
  scene_dimension: 3,
  operator_semantics: 'cross_correlation',
  axis_order: 'CHW',
  input: [[[1, 2, 3], [4, 5, 6], [7, 8, 9]]],
  kernel: [[[1, 0], [0, -1]]],
  stride: [1, 1],
  padding: [0, 0, 0, 0],
  dilation: [1, 1],
  bias: null,
};

describe('conv2d fixture calculation', () => {
  it('matches cross-correlation and convolution signs', () => {
    const cross = evaluateConv2d(base, 1);
    expect(cross.shape).toEqual([2, 2]);
    expect(cross.values).toEqual([-4, -4, -4, -4]);

    const convolution = evaluateConv2d({...base, operator_semantics: 'convolution'}, 2);
    expect(convolution.values).toEqual([4, 4, 4, 4]);
  });

  it('uses registry padding order top/bottom/left/right', () => {
    const padded = evaluateConv2d({...base, padding: [1, 0, 2, 0]}, 3);
    expect(padded.shape).toEqual([3, 4]);
    expect(padded.steps[0].input_indices.slice(0, 2)).toEqual([[ -1, -2], [ -1, -1]]);
  });
});


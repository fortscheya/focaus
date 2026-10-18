import {describe, expect, it} from 'vitest';
import {visualizationMotionMode} from './motion.ts';

describe('visualization motion policy', () => {
  it('pauses automatic motion for reduced-motion users or hidden panes', () => {
    expect(visualizationMotionMode(true, true)).toBe('paused');
    expect(visualizationMotionMode(false, false)).toBe('paused');
    expect(visualizationMotionMode(false, true)).toBe('interactive');
  });
});

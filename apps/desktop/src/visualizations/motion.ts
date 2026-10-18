export type VisualizationMotionMode = 'interactive' | 'paused';

export function visualizationMotionMode(reducedMotion: boolean, visible: boolean): VisualizationMotionMode {
  return reducedMotion || !visible ? 'paused' : 'interactive';
}

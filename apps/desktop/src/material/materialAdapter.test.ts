import {describe, expect, it} from 'vitest';
import {parseMaterialTaskError, type MaterialImportAdapter} from './materialAdapter.ts';

describe('formal material import boundary', () => {
  it('models accepted picker/convert work as a task, not a completed projection', async () => {
    const accepted = {status: 'accepted' as const, taskId: 'task-1', materialId: 'material-1', sourceVersionId: 'source-1', parseVersionId: 'parse-1', message: 'accepted'};
    const adapter: MaterialImportAdapter = {
      mode: 'native',
      requestMaterialImport: async () => accepted,
    };
    const result = await adapter.requestMaterialImport({lessonId: 'lesson-1', purpose: 'courseware', pages: []});
    expect(result.status).toBe('accepted');
    expect(result.taskId).toBe('task-1');
    expect(result.message).toContain('accepted');
  });

  it('preserves structured host failure fields for the task status panel', () => {
    expect(parseMaterialTaskError({code: 'PARSE_FAILED', message: 'page 3 unavailable', scope: 'material.page', retryable: true})).toEqual({code: 'PARSE_FAILED', message: 'page 3 unavailable', scope: 'material.page', retryable: true});
  });
});

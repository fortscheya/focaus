import {describe, expect, it, vi} from 'vitest';
import type {CommandResult, HostAdapter, Lesson} from '@focaus/contracts';
import {hydrateLessonMaterialScope, lessonNeedsMaterialHydration, parseLesson} from './nativeLessonHydration.ts';

const baseLesson = (scope: string[] = []): Lesson => ({
  lesson_id: 'lesson-1',
  course_id: 'course-1',
  title: 'L08',
  actual_date: null,
  timezone: 'Asia/Shanghai',
  requested_language_tier: 3,
  last_recording_tier: null,
  material_scope: scope,
  revision_id: 'lesson-revision-1',
  entity_seq: 2,
});

function adapterWith(result: unknown): HostAdapter {
  return {
    mode: 'native',
    invoke: vi.fn(async () => ({status: 'ok', result}) as unknown as CommandResult),
  } as HostAdapter;
}

describe('native lesson material scope hydration', () => {
  it('keeps an explicit scope and does not call the host again', async () => {
    const adapter = adapterWith({lesson: baseLesson(['material-1'])});
    const lesson = parseLesson(baseLesson(['material-1']));
    expect(lessonNeedsMaterialHydration(lesson)).toBe(false);
    const hydrated = await hydrateLessonMaterialScope(adapter, lesson, 'window-1');
    expect(hydrated).toEqual({lesson, attempted: false, hydrated: false, error: null});
    expect(adapter.invoke).not.toHaveBeenCalled();
  });

  it('re-reads the current lesson head and accepts a newer host revision', async () => {
    const adapter = adapterWith({lesson: {...baseLesson(['material-1']), revision_id: 'lesson-revision-2'}});
    const hydrated = await hydrateLessonMaterialScope(adapter, baseLesson(), 'window-1');
    expect(hydrated.hydrated).toBe(true);
    expect(hydrated.lesson?.material_scope).toEqual(['material-1']);
    expect(hydrated.lesson?.revision_id).toBe('lesson-revision-2');
    expect(adapter.invoke).toHaveBeenCalledWith('content.get', {
      entity_id: 'lesson-1',
      revision_id: null,
      selector: null,
    }, expect.objectContaining({window_id: 'window-1'}));
    expect((adapter.invoke as ReturnType<typeof vi.fn>).mock.calls[0][2].expected_revisions).toEqual({});
  });

  it('does not invent an ID when the authoritative lesson still has no scope', async () => {
    const hydrated = await hydrateLessonMaterialScope(adapterWith({lesson: baseLesson()}), baseLesson());
    expect(hydrated.hydrated).toBe(false);
    expect(hydrated.lesson?.material_scope).toEqual([]);
    expect(hydrated.error).toContain('不会猜测');
  });
});

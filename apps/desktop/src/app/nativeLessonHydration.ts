import type {HostAdapter, Lesson} from '@focaus/contracts';
import {makeEnvelope} from './hostAdapter.ts';

function record(value: unknown): Record<string, unknown> | null {
  return value && typeof value === 'object' && !Array.isArray(value) ? value as Record<string, unknown> : null;
}

function unwrapLesson(value: unknown): unknown {
  const item = record(value);
  return item && 'lesson' in item ? item.lesson : value;
}

/** Parse only host-shaped lesson data; an absent scope remains an empty scope. */
export function parseLesson(value: unknown): Lesson | null {
  const item = record(value);
  if (!item || typeof item.lesson_id !== 'string' || typeof item.course_id !== 'string' || typeof item.title !== 'string' || typeof item.revision_id !== 'string' || typeof item.entity_seq !== 'number') return null;
  const rawScope = item.material_scope;
  if (rawScope !== undefined && (!Array.isArray(rawScope) || rawScope.some((entry) => typeof entry !== 'string'))) return null;
  return {...item, material_scope: rawScope === undefined ? [] : rawScope} as unknown as Lesson;
}

export function parseLessonResponse(value: unknown): Lesson | null {
  return parseLesson(unwrapLesson(value));
}

export function lessonNeedsMaterialHydration(lesson: Lesson | null): boolean {
  return Boolean(lesson && lesson.material_scope.length === 0);
}

export interface LessonHydrationResult {
  lesson: Lesson | null;
  attempted: boolean;
  hydrated: boolean;
  error: string | null;
}

function responseError(response: {status: string; error?: {message?: string; code?: string}}): string {
  return response.error?.message ?? response.error?.code ?? '宿主没有返回 lesson hydration 结果。';
}

/** Re-read the authoritative lesson head when state.open omitted its material scope. */
export async function hydrateLessonMaterialScope(adapter: HostAdapter, lesson: Lesson | null, windowId?: string): Promise<LessonHydrationResult> {
  if (!lesson || !lessonNeedsMaterialHydration(lesson)) return {lesson, attempted: false, hydrated: false, error: null};
  try {
    // The lesson revision carried by state.open can be older than a material
    // import that committed a new authoritative lesson head. Asking for that
    // old revision would correctly return an empty historical scope forever.
    const response = await adapter.invoke('content.get', {entity_id: lesson.lesson_id, revision_id: null, selector: null}, makeEnvelope('content.get', windowId));
    if (response.status !== 'ok' || !response.result) return {lesson, attempted: true, hydrated: false, error: response.status === 'error' ? responseError(response) : '宿主尚未返回可用 lesson projection。'};
    const hydrated = parseLessonResponse(response.result);
    if (!hydrated || hydrated.lesson_id !== lesson.lesson_id) return {lesson, attempted: true, hydrated: false, error: 'lesson hydration 返回的实体与当前课堂不一致。'};
    if (!hydrated.material_scope.length) return {lesson, attempted: true, hydrated: false, error: 'lesson hydration 仍缺少 material_scope；不会猜测资料实体 ID。'};
    return {lesson: hydrated, attempted: true, hydrated: true, error: null};
  } catch (error) {
    return {lesson, attempted: true, hydrated: false, error: error instanceof Error ? error.message : String(error)};
  }
}

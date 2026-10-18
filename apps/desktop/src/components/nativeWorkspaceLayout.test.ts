import {describe, expect, it} from 'vitest';
import type {ContentView} from '@focaus/contracts';
import {buildNativePaneSlots} from './nativeWorkspaceLayout.ts';
import type {NativeProjection} from '../nativeContentProjection.ts';

function view(id: string, kind: ContentView['target'] extends infer _ ? 'note' | 'raw' | 'secondary' | 'material' : never, state: ContentView['state'] = 'open'): ContentView {
  return {view_id: id, lesson_id: 'lesson-1', host_window_id: 'window-1', view_revision: `${id}-r1`, current_owner_lesson_id: 'lesson-1', state, target: {kind: 'lesson_collection', lesson_id: 'lesson-1', content_kind: kind}};
}

const projection = (key: string, kind: NativeProjection['kind']): NativeProjection => ({key, kind, revisionId: 'revision-1', status: 'complete', value: {}});

describe('native workspace pane layout', () => {
  it('keeps native empty state honest and actionable without inventing views', () => {
    const slots = buildNativePaneSlots([], {}, null);
    expect(slots.map((slot) => slot.title)).toEqual(['当前课堂内容', '转写与笔记', '课件与资料']);
    expect(slots.filter((slot) => slot.view !== null)).toHaveLength(0);
    expect(slots.filter((slot) => slot.primary)).toHaveLength(1);
  });

  it('puts the active real view first, preserves its projection, skips closed views and caps six panes', () => {
    const views = [view('note-1', 'note'), view('raw-1', 'raw'), view('closed-1', 'material', 'closed'), view('secondary-1', 'secondary'), view('material-1', 'material'), view('note-2', 'note'), view('raw-2', 'raw'), view('raw-3', 'raw')];
    const slots = buildNativePaneSlots(views, {'raw-1': projection('raw-1', 'raw'), 'material-1': projection('material-1', 'material')}, views[3]);
    expect(slots).toHaveLength(6);
    expect(slots[0].id).toBe('secondary-1');
    expect(slots[0].primary).toBe(true);
    expect(slots.find((slot) => slot.id === 'raw-1')?.projection?.kind).toBe('raw');
    expect(slots.some((slot) => slot.id === 'closed-1')).toBe(false);
  });
});

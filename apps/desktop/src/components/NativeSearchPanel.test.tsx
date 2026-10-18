// @vitest-environment jsdom
import {act, type ReactNode} from 'react';
import {createRoot, type Root} from 'react-dom/client';
import {afterEach, beforeEach, describe, expect, it, vi} from 'vitest';
import type {ContentView, HostAdapter, SearchPage, SearchSession} from '@focaus/contracts';
import {NativeSearchPanel} from './NativeSearchPanel.tsx';

vi.mock('@heroui/react', () => ({
  Button: ({children, onPress, isDisabled, isPending, isIconOnly: _isIconOnly, ...props}: {children: ReactNode; onPress?: () => void; isDisabled?: boolean; isPending?: boolean; isIconOnly?: boolean}) => <button {...props} disabled={isDisabled} aria-busy={isPending} onClick={onPress}>{children}</button>,
  Description: ({children}: {children: ReactNode}) => <small>{children}</small>,
  Input: (props: Record<string, unknown>) => <input {...props} />,
  Label: ({children}: {children: ReactNode}) => <span>{children}</span>,
  TextField: ({value, onChange, onKeyDown, onCompositionStart, onCompositionEnd}: {value: string; onChange: (value: string) => void; onKeyDown?: (event: React.KeyboardEvent<HTMLInputElement>) => void; onCompositionStart?: () => void; onCompositionEnd?: () => void}) => <input aria-label="搜索真实内容窗" value={value} onChange={(event) => onChange(event.target.value)} onKeyDown={onKeyDown} onCompositionStart={onCompositionStart} onCompositionEnd={onCompositionEnd} />,
}));

const view: ContentView = {
  view_id: 'view-1',
  lesson_id: 'lesson-1',
  host_window_id: 'window-1',
  view_revision: 'view-r1',
  current_owner_lesson_id: 'lesson-1',
  state: 'open',
  target: {kind: 'lesson_collection', lesson_id: 'lesson-1', content_kind: 'note'},
};

const session: SearchSession = {
  search_id: 'search-1',
  view_id: 'view-1',
  query_seq: 0,
  query: '',
  case_sensitive: false,
  scope: {kind: 'note', view_id: 'view-1', lesson_id: 'lesson-1', target_ids: ['note-1'], revision_ids: ['note-r1'], actual_visible_tier: null, version_mode: 'current'},
  scope_snapshot_id: 'scope-1',
  index_generation: 1,
  visibility_epoch: 1,
  selected_hit_id: null,
  cursor: null,
  status: 'ready',
};

const page: SearchPage = {
  search_id: 'search-1',
  query_seq: 1,
  scope_snapshot_id: 'scope-1',
  hits: [{
    hit_id: 'hit-1',
    search_unit_id: 'unit-1',
    revision_id: 'note-r1',
    matched_anchor: {owner_entity_id: 'note-1', revision_id: 'note-r1', selector: {kind: 'whole'}, field_path: 'objects[0].text', scalar_range: {start: 0, end: 4}, audio_spans: [], fallback: null},
    ranges: [{start: 0, end: 4}],
    display_snippet: '真实结果',
    equivalence_group_id: null,
    nav_kind: 'text',
  }],
  total_count: 1,
  count_status: 'exact',
  coverage: {scope_snapshot_id: 'scope-1', required_units: 1, indexed_units: 1, required_pages: [], indexed_pages: [], unprocessed: [], failed: [], stale: [], state: 'complete', source_extraction_revision: null},
  index_generation: 1,
  visibility_epoch: 1,
  next_cursor: null,
};

let host: HTMLDivElement;
let root: Root;
let adapter: HostAdapter;
let calls: Array<{name: string; payload: unknown; envelope: Record<string, unknown>}>;

beforeEach(() => {
  (globalThis as {IS_REACT_ACT_ENVIRONMENT?: boolean}).IS_REACT_ACT_ENVIRONMENT = true;
  vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {callback(0); return 0;});
  calls = [];
  adapter = {
    mode: 'native',
    invoke: vi.fn(async (name, payload, envelope) => {
      calls.push({name, payload, envelope: envelope as Record<string, unknown>});
      const result = name === 'search.open' ? session : name === 'search.page' ? page : name === 'search.locate' ? page.hits[0].matched_anchor : name === 'content.get' ? {note_id: 'note-1', revision_id: 'note-r1', objects: [], status: 'complete'} : undefined;
      return {contract_version: '2.0.0', command_id: envelope.command_id, status: name === 'search.close' || name === 'content.get' ? 'ok' : 'accepted', result, committed_revisions: {}, event_seq: 1};
    }),
  } as HostAdapter;
  host = document.createElement('div'); document.body.append(host); root = createRoot(host);
});

afterEach(() => {
  act(() => root.unmount()); host.remove(); vi.restoreAllMocks(); vi.unstubAllGlobals();
});

const render = () => act(() => root.render(<NativeSearchPanel adapter={adapter} open view={view} onClose={() => undefined} />));
const flush = () => act(async () => {await Promise.resolve(); await Promise.resolve();});

describe('native search command loop', () => {
  it('opens and focuses the host scope, then queries, locates, and refreshes without fixture results', async () => {
    render(); await flush();
    const input = host.querySelector('input[aria-label="搜索真实内容窗"]')!;
    expect(input).toBeTruthy();
    act(() => {Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')!.set!.call(input, '真实'); input.dispatchEvent(new Event('input', {bubbles: true}));});
    await flush();
    expect(calls.map((call) => call.name)).toEqual(['search.open', 'search.query', 'search.page']);
    expect(host.textContent).toContain('真实结果');
    act(() => {host.querySelector<HTMLButtonElement>('.native-search-hit')!.click();});
    await flush();
    expect(calls.at(-2)?.name).toBe('search.locate');
    expect((calls.at(-2)?.envelope.expected_revisions as Record<string, string>)['view-1']).toBe('view-r1');
    expect(calls.at(-1)?.name).toBe('content.get');
    expect((calls.at(-1)?.envelope.expected_revisions as Record<string, string>)['note-1']).toBe('note-r1');
    expect(host.textContent).toContain('来源已读取');
    act(() => {[...host.querySelectorAll('button')].find((button) => button.textContent === '刷新')?.click();});
    await flush();
    expect(calls.some((call) => call.name === 'search.refresh')).toBe(true);
  });

  it('does not close while IME composition is active, then closes on Escape', async () => {
    const onClose = vi.fn();
    act(() => root.render(<NativeSearchPanel adapter={adapter} open view={view} onClose={onClose} />)); await flush();
    const input = host.querySelector('input[aria-label="搜索真实内容窗"]')!;
    act(() => {input.dispatchEvent(new CompositionEvent('compositionstart', {bubbles: true})); input.dispatchEvent(new KeyboardEvent('keydown', {key: 'Escape', bubbles: true}));});
    expect(onClose).not.toHaveBeenCalled();
    act(() => {input.dispatchEvent(new CompositionEvent('compositionend', {bubbles: true})); input.dispatchEvent(new KeyboardEvent('keydown', {key: 'Escape', bubbles: true}));});
    expect(onClose).toHaveBeenCalledTimes(1);
  });
});

import {useCallback, useEffect, useRef, useState} from 'react';
import {Button, Description, Input, Label, TextField} from '@heroui/react';
import type {ContentView, HostAdapter, SearchAnchor, SearchHit, SearchOpenInput, SearchPage, SearchQueryInput, SearchRefreshInput, SearchSession, Selector} from '@focaus/contracts';
import {makeEnvelope} from '../app/hostAdapter.ts';
import {detectNativeProjection, type NativeProjection} from '../nativeContentProjection.ts';
import {NativeContentRenderer} from './NativeContentRenderer.tsx';
import {StatusBadge} from './StatusBadge.tsx';

function record(value: unknown): Record<string, unknown> | null {
  return value && typeof value === 'object' && !Array.isArray(value) ? value as Record<string, unknown> : null;
}

function asSearchSession(value: unknown): SearchSession | null {
  const item = record(value);
  return item && typeof item.search_id === 'string' && typeof item.view_id === 'string' && typeof item.scope_snapshot_id === 'string' ? value as SearchSession : null;
}

function asSearchPage(value: unknown): SearchPage | null {
  const item = record(value);
  if (!item || typeof item.search_id !== 'string' || typeof item.query_seq !== 'number' || typeof item.scope_snapshot_id !== 'string' || !Array.isArray(item.hits) || typeof item.total_count !== 'number' || typeof item.next_cursor !== 'string' && item.next_cursor !== null) return null;
  return value as SearchPage;
}

function asSearchHit(value: unknown): SearchHit | null {
  const item = record(value);
  const anchor = record(item?.matched_anchor);
  const range = record(anchor?.scalar_range);
  return item && typeof item.hit_id === 'string' && typeof item.search_unit_id === 'string' && typeof item.revision_id === 'string' && anchor && typeof anchor.field_path === 'string' && range && typeof range.start === 'number' && typeof range.end === 'number' ? value as SearchHit : null;
}

function asSearchAnchor(value: unknown): SearchAnchor | null {
  const item = record(value);
  const selector = record(item?.selector);
  const range = record(item?.scalar_range);
  if (!item || typeof item.owner_entity_id !== 'string' || typeof item.revision_id !== 'string' || !selector || typeof selector.kind !== 'string' || !range || typeof range.start !== 'number' || typeof range.end !== 'number' || typeof item.field_path !== 'string' || !Array.isArray(item.audio_spans) || item.fallback !== null && typeof item.fallback !== 'string') return null;
  return {...item, selector: selector as Selector, scalar_range: {start: range.start, end: range.end}, audio_spans: item.audio_spans.filter((id): id is string => typeof id === 'string')} as SearchAnchor;
}

function responseError(response: {status: string; error?: {message?: string; code?: string}}): string | null {
  return response.status === 'error' ? response.error?.message ?? response.error?.code ?? '宿主命令失败。' : null;
}

function hitLabel(hit: SearchHit): string {
  return hit.display_snippet || hit.matched_anchor.field_path || hit.hit_id;
}

export function NativeSearchPanel({adapter, open, view, onClose}: {adapter: HostAdapter; open: boolean; view: ContentView | null; onClose: () => void}) {
  const [session, setSession] = useState<SearchSession | null>(null);
  const [page, setPage] = useState<SearchPage | null>(null);
  const [query, setQuery] = useState('');
  const [notice, setNotice] = useState('');
  const [selectedHitId, setSelectedHitId] = useState<string | null>(null);
  const [sourceJump, setSourceJump] = useState<{anchor: SearchAnchor; projection: NativeProjection | null; message: string} | null>(null);
  const [busy, setBusy] = useState<'query' | 'locate' | 'refresh' | 'page' | null>(null);
  const querySeq = useRef(0);
  const generation = useRef(0);
  const sessionRef = useRef<SearchSession | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const composing = useRef(false);

  useEffect(() => {
    sessionRef.current = session;
  }, [session]);

  const closeRemote = useCallback(async (target: SearchSession) => {
    await adapter.invoke('search.close', {search_id: target.search_id}, makeEnvelope('search.close'));
  }, [adapter]);

  useEffect(() => () => {
    const current = sessionRef.current;
    if (current) void closeRemote(current);
  }, [closeRemote]);

  useEffect(() => {
    const currentGeneration = ++generation.current;
    const previous = sessionRef.current;
    if (previous) void closeRemote(previous);
    sessionRef.current = null;
    setSession(null);
    setPage(null);
    setQuery('');
    setSelectedHitId(null);
    setSourceJump(null);
    setNotice('');
    setBusy(null);
    querySeq.current = 0;
    if (!open || !view) return () => undefined;
    window.requestAnimationFrame(() => inputRef.current?.focus());
    const payload: SearchOpenInput = {
      view_id: view.view_id,
      target: view.target,
      version_mode: 'current',
      actual_visible_tier: null,
      revision_ids: [view.view_revision],
    };
    let alive = true;
    void adapter.invoke('search.open', payload, makeEnvelope('search.open', view.host_window_id, {[view.view_id]: view.view_revision})).then((response) => {
      if (!alive || generation.current !== currentGeneration) return;
      const error = responseError(response);
      if (error) {
        setNotice('search.open 失败：' + error);
        return;
      }
      const nextSession = asSearchSession(response.result);
      if (!nextSession) {
        setNotice('search.open 返回缺少合法 SearchSession。');
        return;
      }
      sessionRef.current = nextSession;
      setSession(nextSession);
    });
    return () => {alive = false;};
  }, [adapter, closeRemote, open, view?.host_window_id, view?.view_id, view?.view_revision]);

  const expectedRevisions = view ? {[view.view_id]: view.view_revision} : {};

  const fetchPage = useCallback(async (targetSession: SearchSession, nextSeq: number, cursor: string | null, append: boolean) => {
    const response = await adapter.invoke('search.page', {
      search_id: targetSession.search_id,
      query_seq: nextSeq,
      scope_snapshot_id: targetSession.scope_snapshot_id,
      cursor,
    }, makeEnvelope('search.page', view?.host_window_id, expectedRevisions));
    const error = responseError(response);
    if (error) {
      setNotice('search.page 失败：' + error);
      return;
    }
    const nextPage = asSearchPage(response.result);
    if (!nextPage || !nextPage.hits.every((hit) => Boolean(asSearchHit(hit)))) {
      setNotice('search.page 返回缺少合法 SearchPage/SearchHit。');
      return;
    }
    if (nextPage.query_seq !== querySeq.current || nextPage.search_id !== targetSession.search_id) return;
    setPage((current) => append && current ? {...nextPage, hits: [...current.hits, ...nextPage.hits]} : nextPage);
    setNotice('');
  }, [adapter, expectedRevisions, view?.host_window_id]);

  const updateQuery = async (value: string) => {
    setQuery(value);
    setPage(null);
    setSelectedHitId(null);
    setSourceJump(null);
    const targetSession = sessionRef.current;
    if (!targetSession) {
      setNotice('SearchSession 尚未准备好；查询不会回退到 fixture。');
      return;
    }
    const nextSeq = ++querySeq.current;
    setBusy('query');
    const payload: SearchQueryInput = {search_id: targetSession.search_id, query_seq: nextSeq, query: value, case_sensitive: false};
    const response = await adapter.invoke('search.query', payload, makeEnvelope('search.query', view?.host_window_id, expectedRevisions));
    if (nextSeq !== querySeq.current) return;
    const error = responseError(response);
    if (error) {
      setBusy(null);
      setNotice('search.query 失败：' + error);
      return;
    }
    if (!value.trim()) {
      setBusy(null);
      setNotice('');
      return;
    }
    await fetchPage(targetSession, nextSeq, null, false);
    if (nextSeq === querySeq.current) setBusy(null);
  };

  const locate = async (hit: SearchHit) => {
    const targetSession = sessionRef.current;
    if (!targetSession || !view) return;
    setSelectedHitId(hit.hit_id);
    setBusy('locate');
    const response = await adapter.invoke('search.locate', {
      search_id: targetSession.search_id,
      hit_id: hit.hit_id,
      view_id: view.view_id,
      expected_revision: hit.revision_id,
    }, makeEnvelope('search.locate', view.host_window_id, {[view.view_id]: view.view_revision, [hit.search_unit_id]: hit.revision_id}));
    setBusy(null);
    const error = responseError(response);
    if (error) {
      setNotice('search.locate 失败：' + error);
      return;
    }
    const anchor = asSearchAnchor(response.result);
    if (!anchor) {
      setNotice('search.locate 返回缺少合法 SearchAnchor；不会猜测来源实体或 selector。');
      return;
    }
    setSourceJump({anchor, projection: null, message: '已校验来源锚点，正在读取 source content…'});
    const sourceResponse = await adapter.invoke('content.get', {entity_id: anchor.owner_entity_id, revision_id: anchor.revision_id, selector: anchor.selector}, makeEnvelope('content.get', view.host_window_id, {[view.view_id]: view.view_revision, [anchor.owner_entity_id]: anchor.revision_id}));
    const sourceError = responseError(sourceResponse);
    if (sourceError || sourceResponse.status === 'accepted' || sourceResponse.status === 'error' || !sourceResponse.result) {
      setSourceJump({anchor, projection: null, message: sourceError ? '来源读取失败：' + sourceError : '来源 content.get 尚未返回可显示 projection。'});
      return;
    }
    const projection = detectNativeProjection(sourceResponse.result, anchor.owner_entity_id, anchor.revision_id);
    setSourceJump({anchor, projection, message: '来源已读取：' + anchor.field_path + ' · revision ' + anchor.revision_id});
  };

  const refresh = async () => {
    const targetSession = sessionRef.current;
    if (!targetSession) return;
    const nextSeq = querySeq.current;
    setBusy('refresh');
    const payload: SearchRefreshInput = {search_id: targetSession.search_id, query_seq: nextSeq, selected_hit_id: selectedHitId};
    const response = await adapter.invoke('search.refresh', payload, makeEnvelope('search.refresh', view?.host_window_id, expectedRevisions));
    const error = responseError(response);
    if (error) {
      setBusy(null);
      setNotice('search.refresh 失败：' + error);
      return;
    }
    if (query.trim()) await fetchPage(targetSession, nextSeq, null, false);
    setBusy(null);
    setNotice('已请求宿主刷新当前 SearchSession；结果仍受当前 scope/revision 门控。');
  };

  const loadMore = async () => {
    const targetSession = sessionRef.current;
    if (!targetSession || !page?.next_cursor) return;
    setBusy('page');
    await fetchPage(targetSession, querySeq.current, page.next_cursor, true);
    setBusy(null);
  };

  const close = () => {
    const current = sessionRef.current;
    if (current) void closeRemote(current);
    sessionRef.current = null;
    setSession(null);
    onClose();
  };

  if (!open) return null;

  return <aside className="search-panel native-search-panel" aria-label="真实内容窗全文搜索">
    <div className="search-panel-header"><div><p className="eyebrow">Command+F · native search</p><h2>搜索当前内容窗</h2></div><div className="native-search-heading-actions"><Button size="sm" variant="ghost" isDisabled={!session || busy !== null} onPress={() => void refresh()}>刷新</Button><Button isIconOnly size="sm" variant="ghost" aria-label="关闭搜索" onPress={close}>×</Button></div></div>
    <TextField value={query} onChange={(value) => void updateQuery(value)} aria-label="搜索真实内容窗" onCompositionStart={() => {composing.current = true;}} onCompositionEnd={() => {composing.current = false;}} onKeyDown={(event) => {if (composing.current || event.keyCode === 229) return; if (event.key === 'Escape') {event.preventDefault(); close();}}}>
      <Label>{view ? '当前 view · ' + view.view_id : '当前 view'}</Label>
      <Input ref={inputRef} placeholder={view ? '搜索已提交内容' : '先打开一个真实内容窗'} disabled={!view || !session || busy === 'query'} />
      <Description>命中来自宿主 SearchSession/SearchPage；空内容会显示真实 0 结果或索引状态，不使用 DOM 或 fixture 搜索。</Description>
    </TextField>
    {notice ? <div className="search-notice" role="status">{notice}</div> : null}
    {page ? <div className="search-result-summary"><strong>{page.total_count}</strong><span>个真实命中 · {page.count_status} · coverage {page.coverage.state}</span></div> : null}
    {sourceJump ? <section className="native-search-source-jump" aria-label="已定位来源"><div className="native-search-source-heading"><div><strong>已定位来源</strong><span>{sourceJump.anchor.field_path} · {sourceJump.anchor.owner_entity_id}</span></div><StatusBadge tone={sourceJump.projection ? 'success' : 'warning'}>{sourceJump.projection ? '已读取' : '校验中'}</StatusBadge></div><p className="native-command-feedback">{sourceJump.message}</p>{sourceJump.projection ? <NativeContentRenderer projection={sourceJump.projection} adapter={adapter} view={view} /> : null}</section> : null}
    <div className="search-results native-search-results" role="list" aria-label="真实搜索结果">
      {page?.hits.map((hit) => <button key={hit.hit_id} type="button" className={'native-search-hit ' + (selectedHitId === hit.hit_id ? 'is-selected' : '')} onClick={() => void locate(hit)} disabled={busy === 'locate'} role="listitem">
        <strong>{hitLabel(hit)}</strong><span>{hit.nav_kind} · revision {hit.revision_id}</span><small>{hit.matched_anchor.field_path} · {hit.ranges.length} 个范围</small>
      </button>)}
      {page && page.hits.length === 0 ? <div className="search-empty"><strong>宿主返回 0 个命中</strong><p>这代表当前 SearchPage 的真实结果，不补充 fixture 内容。</p></div> : null}
      {!page && !notice ? <div className="search-empty"><strong>{view ? '输入查询以请求 search.query' : '没有当前内容窗'}</strong><p>{view ? '搜索范围由当前 view.target 和 revision 固定。' : '先用六格或独立窗口打开一个真实 view。'}</p></div> : null}
    </div>
    {page?.next_cursor ? <div className="native-search-footer"><Button size="sm" variant="ghost" isDisabled={busy !== null} isPending={busy === 'page'} onPress={() => void loadMore()}>加载下一页</Button><span>分页 cursor 由宿主持有</span></div> : null}
  </aside>;
}

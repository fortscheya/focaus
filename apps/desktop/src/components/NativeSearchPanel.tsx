import {useEffect, useRef, useState} from 'react';
import {Button, Card, Description, Input, Label, TextField} from '@heroui/react';
import type {ContentView, HostAdapter, SearchOpenInput, SearchPage, SearchQueryInput, SearchSession} from '@focaus/contracts';
import {makeEnvelope} from '../app/hostAdapter.ts';

function asSearchSession(value: unknown): SearchSession | null {
  return value && typeof value === 'object' && typeof (value as {search_id?: unknown}).search_id === 'string' ? value as SearchSession : null;
}

function asSearchPage(value: unknown): SearchPage | null {
  return value && typeof value === 'object' && Array.isArray((value as {hits?: unknown}).hits) ? value as SearchPage : null;
}

export function NativeSearchPanel({
  adapter,
  open,
  view,
  onClose,
}: {
  adapter: HostAdapter;
  open: boolean;
  view: ContentView | null;
  onClose: () => void;
}) {
  const [session, setSession] = useState<SearchSession | null>(null);
  const [page, setPage] = useState<SearchPage | null>(null);
  const [query, setQuery] = useState('');
  const [notice, setNotice] = useState('');
  const querySeq = useRef(0);

  useEffect(() => {
    if (!open || !view) {
      setSession(null);
      setPage(null);
      return;
    }
    const payload: SearchOpenInput = {
      view_id: view.view_id,
      target: view.target,
      version_mode: 'current',
      actual_visible_tier: null,
      revision_ids: [view.view_revision],
    };
    void adapter.invoke('search.open', payload, makeEnvelope('search.open')).then((response) => {
      if (response.status === 'error') {
        setNotice(`search.open 失败：${response.error?.message ?? response.error?.code ?? 'unknown error'}`);
        return;
      }
      const nextSession = asSearchSession(response.result);
      if (!nextSession) {
        setNotice('search.open 返回缺少合法 SearchSession。');
        return;
      }
      setSession(nextSession);
      setNotice('');
    });
  }, [adapter, open, view?.view_id, view?.view_revision]);

  useEffect(() => () => {
    if (session) void adapter.invoke('search.close', {search_id: session.search_id}, makeEnvelope('search.close'));
  }, [adapter, session]);

  if (!open) return null;

  const close = () => {
    if (session) void adapter.invoke('search.close', {search_id: session.search_id}, makeEnvelope('search.close'));
    onClose();
  };

  const updateQuery = async (value: string) => {
    setQuery(value);
    setPage(null);
    if (!session) return;
    querySeq.current += 1;
    const currentSeq = querySeq.current;
    const payload: SearchQueryInput = {search_id: session.search_id, query_seq: currentSeq, query: value, case_sensitive: false};
    const response = await adapter.invoke('search.query', payload, makeEnvelope('search.query'));
    if (response.status === 'error') {
      setNotice(`search.query 失败：${response.error?.message ?? response.error?.code ?? 'unknown error'}`);
      return;
    }
    if (!value.trim()) {
      setNotice('');
      return;
    }
    const pageResponse = await adapter.invoke('search.page', {search_id: session.search_id, query_seq: currentSeq, scope_snapshot_id: session.scope_snapshot_id, cursor: null}, makeEnvelope('search.page'));
    if (pageResponse.status === 'error') {
      setNotice(`search.page 失败：${pageResponse.error?.message ?? pageResponse.error?.code ?? 'unknown error'}`);
      return;
    }
    const nextPage = asSearchPage(pageResponse.result);
    if (!nextPage) {
      setNotice('search.page 返回缺少合法 SearchPage。');
      return;
    }
    setPage(nextPage);
    setNotice('');
  };

  return (
    <aside className="search-panel native-search-panel" aria-label="真实内容窗全文搜索">
      <div className="search-panel-header"><div><p className="eyebrow">Command+F · native search</p><h2>搜索当前内容窗</h2></div><Button isIconOnly size="sm" variant="ghost" aria-label="关闭搜索" onPress={close}>×</Button></div>
      <TextField value={query} onChange={(value) => void updateQuery(value)} aria-label="搜索真实内容窗" onKeyDown={(event) => {if (event.key === 'Escape') {event.preventDefault(); close();}}}>
        <Label>{view ? `当前 view · ${view.view_id}` : '当前 view'}</Label>
        <Input placeholder={view ? '搜索已提交内容' : '先打开一个真实内容窗'} disabled={!view || !session} />
        <Description>命中来自宿主 SearchSession/SearchPage；空内容会显示真实 0 结果或索引状态。</Description>
      </TextField>
      {notice ? <div className="search-notice" role="status">{notice}</div> : null}
      {page ? <div className="search-result-summary"><strong>{page.total_count}</strong><span>个真实命中 · {page.count_status} · coverage {page.coverage.state}</span></div> : null}
      <div className="search-results native-search-results" role="list" aria-label="真实搜索结果">
        {page?.hits.map((hit) => <Card key={hit.hit_id} className="search-result-card" variant="transparent" role="listitem"><Card.Header><Card.Title>{hit.display_snippet || hit.hit_id}</Card.Title><Card.Description>{hit.nav_kind} · revision {hit.revision_id}</Card.Description></Card.Header><Card.Content><span className="muted-copy">命中锚点 field {hit.matched_anchor.field_path} · ranges {hit.ranges.length}</span></Card.Content></Card>)}
        {page && page.hits.length === 0 ? <div className="search-empty"><strong>宿主返回 0 个命中</strong><p>这代表当前 SearchPage 的真实结果，不补充 fixture 内容。</p></div> : null}
        {!page && !notice ? <div className="search-empty"><strong>{view ? '输入查询以请求 search.query' : '没有当前内容窗'}</strong><p>{view ? '搜索范围由当前 view.target 和 revision 固定。' : '先用六格或独立窗口打开一个真实 view。'}</p></div> : null}
      </div>
    </aside>
  );
}


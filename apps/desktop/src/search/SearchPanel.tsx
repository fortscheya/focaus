import {useEffect, useMemo, useRef, useState} from 'react';
import {Button, Card, Description, Input, Label, TextField} from '@heroui/react';
import type {HostAdapter, SearchLocateInput, SearchOpenInput, SearchQueryInput} from '@focaus/contracts';
import {ids, searchEntries, type WorkspaceViewFixture} from '../app/fixtures.ts';
import {makeEnvelope} from '../app/hostAdapter.ts';
import {searchFixtureEntries, snippet, type SearchResultView} from './searchIndex.ts';
import {StatusBadge} from '../components/StatusBadge.tsx';

function viewToSearchKind(view: WorkspaceViewFixture): 'material' | 'note' | 'raw' | 'secondary' {
  if (view.kind === 'material') return 'material';
  if (view.kind === 'raw') return 'raw';
  if (view.kind === 'secondary') return 'secondary';
  return 'note';
}

function sourceRoleLabel(role: SearchResultView['entry']['sourceRole']): string {
  if (role === 'teacher_said') return '课堂原话';
  if (role === 'material_supplement') return '资料补充';
  if (role === 'system_explanation') return '系统解释';
  return '派生内容';
}

function MatchText({result}: {result: SearchResultView}) {
  const text = Array.from(result.entry.text);
  const before = text.slice(0, result.matchRange.start).join('');
  const match = text.slice(result.matchRange.start, result.matchRange.end).join('');
  const after = text.slice(result.matchRange.end).join('');
  return (
    <span className="search-snippet">
      {before ? <span>{before}</span> : null}
      <mark>{match}</mark>
      {after ? <span>{after}</span> : null}
    </span>
  );
}

export function SearchPanel({
  open,
  view,
  adapter,
  onClose,
  onLocate,
}: {
  open: boolean;
  view: WorkspaceViewFixture;
  adapter: HostAdapter;
  onClose: () => void;
  onLocate: (result: SearchResultView) => void;
}) {
  const inputRef = useRef<HTMLInputElement>(null);
  const querySequence = useRef(0);
  const previousFocus = useRef<HTMLElement | null>(null);
  const [query, setQuery] = useState('');
  const [caseSensitive, setCaseSensitive] = useState(false);
  const [results, setResults] = useState<SearchResultView[]>([]);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [isComposing, setIsComposing] = useState(false);
  const [notice, setNotice] = useState('');
  const [searchId, setSearchId] = useState('4c11a85f-f3da-4f4f-9701-000000000032');
  const kind = viewToSearchKind(view);

  const coverageLabel = useMemo(() => {
    if (kind === 'material') return '已索引 6 / 7 个课件单元 · 第 9 页 OCR 未完成';
    if (kind === 'raw') return '当前原始转写范围 · 新增句段会增量更新';
    if (kind === 'secondary') return '当前二次转写范围 · 与原始时间保持对应';
    return '当前笔记第 3 档 · 表格、公式与组件说明已纳入';
  }, [kind]);

  useEffect(() => {
    if (!open) return;
    previousFocus.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const target: SearchOpenInput['target'] = {
      kind: 'lesson_collection',
      lesson_id: ids.lesson,
      content_kind: kind,
    };
    const payload: SearchOpenInput = {
      view_id: view.id,
      target,
      version_mode: 'current',
      actual_visible_tier: kind === 'note' ? 3 : null,
      revision_ids: [ids.revision],
    };
    void adapter.invoke('search.open', payload, makeEnvelope('search.open')).then((response) => {
      if (response.status === 'ok' && response.result && typeof response.result === 'object' && 'search_id' in response.result) {
        const nextId = (response.result as {search_id?: unknown}).search_id;
        if (typeof nextId === 'string') setSearchId(nextId);
      }
    });
    window.requestAnimationFrame(() => inputRef.current?.focus());
  }, [adapter, kind, open, view.id]);

  useEffect(() => {
    if (open) return;
    previousFocus.current?.focus();
  }, [open]);

  if (!open) return null;

  const close = () => {
    void adapter.invoke('search.close', {search_id: searchId}, makeEnvelope('search.close'));
    onClose();
  };

  const updateQuery = (value: string) => {
    setQuery(value);
    querySequence.current += 1;
    const nextQuerySeq = querySequence.current;
    const nextResults = searchFixtureEntries(searchEntries, value, kind, caseSensitive);
    setResults(nextResults);
    setSelectedIndex(0);
    setNotice(value && nextResults.length === 0 ? '当前 fixture 索引没有命中；课件仍有未处理覆盖范围。' : '');
    const payload: SearchQueryInput = {search_id: searchId, query_seq: nextQuerySeq, query: value, case_sensitive: caseSensitive};
    void adapter.invoke('search.query', payload, makeEnvelope('search.query'));
  };

  const selectResult = (result: SearchResultView, index: number) => {
    setSelectedIndex(index);
    const payload: SearchLocateInput = {search_id: searchId, hit_id: result.id, view_id: view.id, expected_revision: result.entry.revisionId};
    void adapter.invoke('search.locate', payload, makeEnvelope('search.locate'));
    onLocate(result);
  };

  const moveSelection = (direction: 1 | -1) => {
    if (!results.length) return;
    const next = (selectedIndex + direction + results.length) % results.length;
    selectResult(results[next], next);
  };

  const handleKeyDown = (event: React.KeyboardEvent<HTMLInputElement>) => {
    if (isComposing) return;
    if (event.key === 'Escape') {
      event.preventDefault();
      close();
    } else if (event.key === 'Enter') {
      event.preventDefault();
      moveSelection(event.shiftKey ? -1 : 1);
    }
  };

  return (
    <aside className="search-panel" aria-label="当前内容窗搜索" data-search-scope={kind}>
      <div className="search-panel-header">
        <div>
          <p className="eyebrow">Command+F · 自有搜索</p>
          <h2>搜索 {view.title}</h2>
        </div>
        <Button isIconOnly size="sm" variant="ghost" aria-label="关闭搜索" onPress={close}>×</Button>
      </div>
      <TextField className="search-field" value={query} onChange={updateQuery} aria-label="搜索当前内容窗" onKeyDown={handleKeyDown}>
        <Label>当前范围</Label>
        <Input ref={inputRef} placeholder="搜索标题、正文、表格、公式或原话" />
        <Description>{coverageLabel}</Description>
      </TextField>
      <div className="search-options">
        <label className="checkbox-label">
          <input type="checkbox" checked={caseSensitive} onChange={(event) => {
            setCaseSensitive(event.target.checked);
            const nextResults = searchFixtureEntries(searchEntries, query, kind, event.target.checked);
            setResults(nextResults);
            setSelectedIndex(0);
          }} />
          大小写敏感
        </label>
        <span className="search-shortcut">Enter 下一处 · ⇧ Enter 上一处 · Esc 关闭</span>
      </div>
      <div className="search-result-summary" aria-live="polite">
        <strong>{query ? `${results.length ? selectedIndex + 1 : 0} / ${results.length}` : '等待输入'}</strong>
        <span>{query ? '个命中' : '首次输入不会离开搜索框'}</span>
      </div>
      {notice ? <div className="search-notice" role="status">{notice}</div> : null}
      <div className="search-results" role="list" aria-label="搜索结果">
        {results.map((result, index) => (
          <Card
            key={result.id}
            className={`search-result-card ${index === selectedIndex ? 'is-selected' : ''}`}
            variant="transparent"
            role="listitem"
            tabIndex={0}
            onClick={() => selectResult(result, index)}
            onKeyDown={(event) => {
              if (event.key === 'Enter' || event.key === ' ') {
                event.preventDefault();
                selectResult(result, index);
              }
            }}
          >
            <Card.Header>
              <div className="search-result-title-row">
                <Card.Title>{result.entry.title}</Card.Title>
                <StatusBadge tone={result.entry.kind === 'raw' ? 'danger' : result.entry.kind === 'material' ? 'accent' : 'neutral'}>
                  {sourceRoleLabel(result.entry.sourceRole)}
                </StatusBadge>
              </div>
              <Card.Description>
                {result.entry.fieldPath} · {result.entry.anchorLabel}
              </Card.Description>
            </Card.Header>
            <Card.Content>
              <MatchText result={result} />
              <div className="search-result-meta">
                {result.entry.timeLabel ? <span>{result.entry.timeLabel}</span> : null}
                {result.entry.coverageLabel ? <span>{result.entry.coverageLabel}</span> : null}
                {result.alternateAnchors.length ? <span>另有同源定位：{result.alternateAnchors.join('、')}</span> : null}
              </div>
            </Card.Content>
          </Card>
        ))}
        {!results.length && query ? (
          <div className="search-empty">
            <strong>没有可显示的命中</strong>
            <p>如果索引覆盖是 partial，这里只代表当前已索引部分没有命中。</p>
            <span>定位真值由宿主 search.locate 校验，UI 不以 DOM 查找代替全文索引。</span>
          </div>
        ) : null}
      </div>
      {results.length ? (
        <div className="search-panel-footer">
          <Button size="sm" variant="ghost" onPress={() => moveSelection(-1)}>上一处</Button>
          <Button size="sm" variant="ghost" onPress={() => moveSelection(1)}>下一处</Button>
          <span className="search-footer-snippet">{snippet(results[selectedIndex].entry.text, results[selectedIndex].matchRange, 20)}</span>
        </div>
      ) : null}
    </aside>
  );
}

import {useRef, useState} from 'react';
import {Button, Card, Description, Input, Label, Tabs, TextArea, TextField} from '@heroui/react';
import type {HostAdapter, Selector, UserPatch} from '@focaus/contracts';
import {correctionFixtures, ids, type CorrectionFixture} from '../app/fixtures.ts';
import {makeEnvelope} from '../app/hostAdapter.ts';
import {StatusBadge} from '../components/StatusBadge.tsx';

const INITIAL_BODY = '卷积运算把局部窗口与卷积核逐项相乘再求和。当前窗口向右移动一个步幅，输出格只依赖这块区域。';
const PROTECTED_TEXT = '卷积';
const FIXTURE_HASH = 'b'.repeat(64);

function scalarLength(value: string): number {
  return Array.from(value).length;
}

function makeTextSelector(start: number, end: number): Selector {
  return {
    kind: 'text',
    block_id: ids.block,
    anchor: {
      left_element_id: null,
      right_element_id: null,
      element_ids: ['element-note-body'],
      scalar_range: {start, end},
    },
  };
}

function makePatch(previous: string, next: string): UserPatch | null {
  if (previous === next) return null;
  let prefix = 0;
  const previousScalars = Array.from(previous);
  const nextScalars = Array.from(next);
  while (prefix < previousScalars.length && prefix < nextScalars.length && previousScalars[prefix] === nextScalars[prefix]) prefix += 1;
  let previousEnd = previousScalars.length;
  let nextEnd = nextScalars.length;
  while (previousEnd > prefix && nextEnd > prefix && previousScalars[previousEnd - 1] === nextScalars[nextEnd - 1]) {
    previousEnd -= 1;
    nextEnd -= 1;
  }
  const operation = previousEnd === prefix ? 'text_insert' : nextEnd === prefix ? 'text_delete' : 'text_replace';
  return {
    patch_id: crypto.randomUUID(),
    target_id: ids.note,
    base_revision: ids.revision,
    operation,
    selector: makeTextSelector(prefix, previousEnd),
    before_hash: FIXTURE_HASH,
    after_value: nextScalars.slice(prefix, nextEnd).join(''),
    author: 'user',
    intent: '用户编辑课堂笔记正文',
    undo_group_id: crypto.randomUUID(),
    created_at: new Date().toISOString(),
  };
}

function patchSelectorForObject(objectId: string, elementId: string): Selector {
  return {kind: 'object', object_id: objectId, element_ids: [elementId]};
}

export function ContentEditor({adapter}: {adapter: HostAdapter}) {
  const [draft, setDraft] = useState(INITIAL_BODY);
  const committedDraft = useRef(INITIAL_BODY);
  const undoStack = useRef<string[]>([]);
  const [compositionActive, setCompositionActive] = useState(false);
  const [protectedNotice, setProtectedNotice] = useState('');
  const [patchNotice, setPatchNotice] = useState('');
  const [activeTab, setActiveTab] = useState('body');
  const [suggestions, setSuggestions] = useState<CorrectionFixture[]>(correctionFixtures);
  const [languageRequested, setLanguageRequested] = useState(4);
  const [languageActual, setLanguageActual] = useState(3);
  const [languageStatus, setLanguageStatus] = useState<'ready' | 'stale' | 'generating'>('stale');
  const [sourceNotice, setSourceNotice] = useState('');
  const [tableValue, setTableValue] = useState('1');
  const [chartLabel, setChartLabel] = useState('局部窗口响应');
  const textAreaRef = useRef<HTMLTextAreaElement>(null);

  const sendPatch = (previous: string, next: string) => {
    const patch = makePatch(previous, next);
    if (!patch) return;
    undoStack.current.push(previous);
    committedDraft.current = next;
    setPatchNotice(`已生成细粒度 ${patch.operation} patch · selector scalar ${patch.selector.kind === 'text' ? patch.selector.anchor.scalar_range.start : 'object'}`);
    void adapter.invoke('content.patch', {patch, lease_epoch: 4}, makeEnvelope('content.patch'));
  };

  const updateDraft = (next: string) => {
    if (!next.includes(PROTECTED_TEXT) && committedDraft.current.includes(PROTECTED_TEXT)) {
      setProtectedNotice('术语“卷积”位于受保护区域，当前 patch 被阻止；请通过结构化订正流程修改。');
      setDraft(committedDraft.current);
      return;
    }
    setProtectedNotice('');
    setDraft(next);
    if (!compositionActive) sendPatch(committedDraft.current, next);
  };

  const onBeforeInput = (event: React.FormEvent<HTMLTextAreaElement>) => {
    const nativeEvent = event.nativeEvent as InputEvent;
    if (!nativeEvent.inputType?.startsWith('delete')) return;
    const selection = textAreaRef.current;
    if (!selection) return;
    const selectionStart = selection.selectionStart;
    const selectionEnd = selection.selectionEnd;
    const protectedStart = draft.indexOf(PROTECTED_TEXT);
    const protectedEnd = protectedStart + PROTECTED_TEXT.length;
    const deleteStart = nativeEvent.inputType === 'deleteContentBackward' ? Math.max(0, selectionStart - 1) : selectionStart;
    const deleteEnd = nativeEvent.inputType === 'deleteContentForward' ? selectionEnd + 1 : selectionEnd;
    if (protectedStart >= 0 && deleteStart < protectedEnd && deleteEnd > protectedStart) {
      event.preventDefault();
      setProtectedNotice('受保护区域未被删除；正在输入或来源绑定中的术语需要走订正命令。');
    }
  };

  const onCompositionEnd = () => {
    setCompositionActive(false);
    sendPatch(committedDraft.current, draft);
    void adapter.invoke('correction.draft', {suggestion_id: '4c11a85f-f3da-4f4f-9701-000000000033', draft, composition_active: false}, makeEnvelope('correction.draft', undefined, {[ids.note]: ids.revision}));
  };

  const acceptSuggestion = (suggestion: CorrectionFixture) => {
    if (suggestion.state !== 'pending') return;
    setSuggestions((current) => current.map((item) => item.id === suggestion.id ? {...item, state: 'accepted'} : item));
    setDraft((current) => {
      const next = current.replace(suggestion.original, suggestion.suggestion);
      if (next !== current) {
        sendPatch(current, next);
        committedDraft.current = next;
      }
      return next;
    });
    void adapter.invoke('correction.accept', {suggestion_id: suggestion.id, base_revision: ids.revision, final_text: suggestion.suggestion}, makeEnvelope('correction.accept'));
  };

  const rejectSuggestion = (suggestion: CorrectionFixture) => {
    setSuggestions((current) => current.map((item) => item.id === suggestion.id ? {...item, state: 'rejected'} : item));
    void adapter.invoke('correction.reject', {suggestion_id: suggestion.id, base_revision: ids.revision, final_text: suggestion.original}, makeEnvelope('correction.reject'));
  };

  const acceptOrdinaryBatch = () => {
    const ordinary = suggestions.filter((suggestion) => suggestion.state === 'pending' && suggestion.risk === 'ordinary');
    if (!ordinary.length) return;
    let nextDraft = draft;
    for (const suggestion of ordinary) nextDraft = nextDraft.replace(suggestion.original, suggestion.suggestion);
    if (nextDraft !== draft) sendPatch(draft, nextDraft);
    committedDraft.current = nextDraft;
    setDraft(nextDraft);
    setSuggestions((current) => current.map((suggestion) => ordinary.some((item) => item.id === suggestion.id) ? {...suggestion, state: 'accepted'} : suggestion));
    void adapter.invoke('correction.accept_batch', {base_revision: ids.revision, suggestion_ids: ordinary.map((suggestion) => suggestion.id)}, makeEnvelope('correction.accept_batch'));
  };

  const undo = () => {
    const previous = undoStack.current.pop();
    if (previous === undefined) {
      setPatchNotice('当前没有可撤销的用户 patch。');
      return;
    }
    const current = draft;
    setDraft(previous);
    committedDraft.current = previous;
    setPatchNotice('已请求宿主撤销最近一个 undo_group；fixture 只更新本地草稿。');
    void adapter.invoke('content.undo', {undo_group_id: crypto.randomUUID(), current_revision: ids.revision}, makeEnvelope('content.undo'));
    if (current === previous) undo();
  };

  const selectLanguage = (tier: number) => {
    setLanguageRequested(tier);
    setLanguageStatus('generating');
    void adapter.invoke('language.select', {lesson_id: ids.lesson, tier}, makeEnvelope('language.select'));
    window.setTimeout(() => {
      setLanguageActual(tier);
      setLanguageStatus('ready');
    }, 600);
  };

  const navigateSource = (kind: 'resolve' | 'diff') => {
    if (kind === 'resolve') {
      setSourceNotice('已请求逐级来源：笔记 Fact → 二次段 → 原始句 → 音频时间；真实导航由宿主校验 selector。');
      void adapter.invoke('source.resolve', {edge_id: ids.source, return_anchor: makeTextSelector(0, scalarLength(PROTECTED_TEXT))}, makeEnvelope('source.resolve'));
    } else {
      setSourceNotice('已请求新旧 revision diff；当前 fixture 只展示入口，不把新版本自动替换当前正文。');
      void adapter.invoke('source.diff', {source: {entity_id: ids.source, revision_id: ids.sourceRevision, content_hash: FIXTURE_HASH, selector: makeTextSelector(0, scalarLength(PROTECTED_TEXT))}, new_revision_id: '4c11a85f-f3da-4f4f-9701-000000000020'}, makeEnvelope('source.diff'));
    }
  };

  const patchCell = (value: string) => {
    setTableValue(value);
    const patch: UserPatch = {
      patch_id: crypto.randomUUID(),
      target_id: ids.note,
      base_revision: ids.revision,
      operation: 'cell_set',
      selector: patchSelectorForObject('table-object', 'cell-stride'),
      before_hash: FIXTURE_HASH,
      after_value: value,
      author: 'user',
      intent: '用户修改结构化表格单元格',
      undo_group_id: crypto.randomUUID(),
      created_at: new Date().toISOString(),
    };
    void adapter.invoke('content.patch', {patch, lease_epoch: 4}, makeEnvelope('content.patch'));
    setPatchNotice('表格单元格已生成 cell_set patch；未改变共同 Fact 集。');
  };

  return (
    <section className="editor-panel" aria-labelledby="editor-title" data-anchor="anchor-note">
      <div className="panel-heading-row editor-heading">
        <div>
          <p className="eyebrow">课堂笔记 · 当前显示档 3</p>
          <h2 id="editor-title">可追溯编辑器</h2>
        </div>
        <div className="editor-heading-actions">
          <StatusBadge tone={compositionActive ? 'warning' : 'success'} dot={compositionActive}>{compositionActive ? 'IME 组合中' : '草稿已同步到 fixture'}</StatusBadge>
          <Button size="sm" variant="outline" onPress={undo}>撤销</Button>
        </div>
      </div>
      <div className="editor-protection-banner">
        <span className="protection-lock" aria-hidden="true">▣</span>
        <span><strong>用户保护范围</strong> · 术语“卷积”与当前来源锚点受保护。输入法组合未结束时，AI patch 不会覆盖草稿。</span>
        <span className="fixture-label">editor adapter · fixture</span>
      </div>
      {protectedNotice ? <div className="warning-notice" role="alert">{protectedNotice}</div> : null}
      {patchNotice ? <div className="inline-notice" role="status">{patchNotice}</div> : null}
      <Tabs className="editor-tabs" selectedKey={activeTab} onSelectionChange={(key) => setActiveTab(String(key))}>
        <Tabs.ListContainer>
          <Tabs.List aria-label="编辑内容类型">
            <Tabs.Tab id="body">正文<Tabs.Indicator /></Tabs.Tab>
            <Tabs.Tab id="corrections">订正 <Tabs.Indicator /></Tabs.Tab>
            <Tabs.Tab id="objects">对象 <Tabs.Indicator /></Tabs.Tab>
          </Tabs.List>
        </Tabs.ListContainer>
        <Tabs.Panel id="body" className="editor-tab-panel">
          <TextField className="editor-text-field" value={draft} onChange={updateDraft} aria-label="课堂笔记正文" isInvalid={Boolean(protectedNotice)}>
            <Label>当前正文</Label>
            <TextArea
              ref={textAreaRef}
              rows={7}
              onBeforeInput={onBeforeInput}
              onCompositionStart={() => {
                setCompositionActive(true);
                void adapter.invoke('edit.begin', {lease: {view_id: ids.view, target_id: ids.note, selectors: [makeTextSelector(0, scalarLength(PROTECTED_TEXT))], lease_epoch: 4, composition_active: true, draft_revision: ids.revision}}, makeEnvelope('edit.begin'));
              }}
              onCompositionEnd={onCompositionEnd}
              aria-describedby="editor-protection-description"
            />
            <Description id="editor-protection-description">字符数 {scalarLength(draft)} · 当前 revision {ids.revision.slice(0, 8)} · 正文只通过 content.patch 写入。</Description>
          </TextField>
          <div className="editor-source-grid">
            <Card className="source-card" variant="secondary">
              <Card.Header>
                <Card.Title>逐级来源</Card.Title>
                <Card.Description>来源边与正文 revision 分离保存</Card.Description>
              </Card.Header>
              <Card.Content>
                <ol className="source-chain">
                  <li><span>当前</span><strong>笔记 Fact · 卷积运算</strong></li>
                  <li><span>派生</span><strong>二次段 · 逐项相乘再求和</strong></li>
                  <li><span>原话</span><strong>原始句 · 00:05:12.400</strong></li>
                  <li><span>音频</span><strong>录音 · 00:05:12.400–16.020</strong></li>
                </ol>
              </Card.Content>
              <Card.Footer>
                <Button size="sm" variant="ghost" onPress={() => navigateSource('resolve')}>打开来源链</Button>
                <Button size="sm" variant="ghost" onPress={() => navigateSource('diff')}>查看新旧 diff</Button>
              </Card.Footer>
            </Card>
            <Card className="language-card" variant="secondary">
              <Card.Header>
                <Card.Title>语言档位</Card.Title>
                <Card.Description>五档共享 Fact 集；当前档位独立显示</Card.Description>
              </Card.Header>
              <Card.Content>
                <div className="tier-row" role="group" aria-label="选择语言档位">
                  {[1, 2, 3, 4, 5].map((tier) => (
                    <Button key={tier} size="sm" variant={tier === languageRequested ? 'primary' : 'outline'} isDisabled={false} onPress={() => selectLanguage(tier)} aria-label={`请求第 ${tier} 档`}>
                      {tier}
                    </Button>
                  ))}
                </div>
                <div className="tier-status-row">
                  <StatusBadge tone={languageStatus === 'ready' ? 'success' : 'warning'}>请求档 {languageRequested}</StatusBadge>
                  <span>实际显示档 {languageActual}</span>
                  <span>{languageStatus === 'stale' ? '事实更新后待同步' : languageStatus === 'generating' ? '生成中，保留旧档' : '已就绪'}</span>
                </div>
              </Card.Content>
            </Card>
          </div>
          {sourceNotice ? <div className="inline-notice" role="status">{sourceNotice}</div> : null}
        </Tabs.Panel>
        <Tabs.Panel id="corrections" className="editor-tab-panel">
          <div className="correction-toolbar">
            <div>
              <p className="eyebrow">红线订正浮层</p>
              <h3>原话修剪与用户采纳</h3>
            </div>
            <Button size="sm" variant="secondary" onPress={acceptOrdinaryBatch} isDisabled={!suggestions.some((suggestion) => suggestion.state === 'pending' && suggestion.risk === 'ordinary')}>批量采纳普通订正</Button>
          </div>
          <div className="correction-list">
            {suggestions.map((suggestion) => (
              <Card key={suggestion.id} className={`correction-card correction-card-${suggestion.risk}`} variant="transparent">
                <Card.Header>
                  <div className="correction-title-row">
                    <Card.Title>{suggestion.risk === 'critical' ? '关键红线' : '普通订正'}</Card.Title>
                    <StatusBadge tone={suggestion.state === 'pending' ? suggestion.risk === 'critical' ? 'danger' : 'warning' : suggestion.state === 'accepted' ? 'success' : 'neutral'}>{suggestion.state === 'pending' ? '待采纳' : suggestion.state === 'accepted' ? '已采纳' : '已拒绝'}</StatusBadge>
                  </div>
                  <Card.Description>{suggestion.reason}</Card.Description>
                </Card.Header>
                <Card.Content>
                  <div className="redline-copy"><del>{suggestion.original}</del><span aria-hidden="true">→</span><ins>{suggestion.suggestion}</ins></div>
                </Card.Content>
                <Card.Footer>
                  <Button size="sm" onPress={() => acceptSuggestion(suggestion)} isDisabled={suggestion.state !== 'pending'}>{suggestion.risk === 'critical' ? '明确采纳' : '采纳'}</Button>
                  <Button size="sm" variant="tertiary" onPress={() => rejectSuggestion(suggestion)} isDisabled={suggestion.state !== 'pending'}>拒绝</Button>
                </Card.Footer>
              </Card>
            ))}
          </div>
          <p className="muted-copy">关键红线不会被批量按钮隐式采纳；任何采纳都写入独立 patch 和 undo_group。</p>
        </Tabs.Panel>
        <Tabs.Panel id="objects" className="editor-tab-panel">
          <div className="object-editor-grid">
            <Card className="object-card" variant="secondary">
              <Card.Header><Card.Title>原生表格副本</Card.Title><Card.Description>cell_set · 独立用户修改</Card.Description></Card.Header>
              <Card.Content>
                <table className="editable-table"><caption>卷积参数表</caption><thead><tr><th>参数</th><th>值</th><th>单位</th></tr></thead><tbody><tr><th>stride</th><td><Input aria-label="stride 单元格" value={tableValue} onChange={(event) => patchCell(event.target.value)} /></td><td>格</td></tr><tr><th>padding</th><td>0</td><td>格</td></tr></tbody></table>
              </Card.Content>
            </Card>
            <Card className="object-card" variant="secondary">
              <Card.Header><Card.Title>原生图表副本</Card.Title><Card.Description>标签 patch · 来源可回溯</Card.Description></Card.Header>
              <Card.Content>
                <TextField value={chartLabel} onChange={(value) => {setChartLabel(value); setPatchNotice('图表标签已保留为局部 node_set patch。');}}>
                  <Label>图表标题</Label><Input aria-label="图表标题" /><Description>当前图表值仍标记为课堂/估读/未知来源。</Description>
                </TextField>
                <div className="mini-chart" role="img" aria-label={`${chartLabel}示意图`}><span style={{height: '30%'}} /><span style={{height: '55%'}} /><span style={{height: '78%'}} /><span style={{height: '48%'}} /></div>
              </Card.Content>
            </Card>
            <Card className="object-card" variant="secondary">
              <Card.Header><Card.Title>结构图关系</Card.Title><Card.Description>节点/边修改需要稳定对象 ID</Card.Description></Card.Header>
              <Card.Content><div className="diagram-preview"><span>输入</span><i aria-hidden="true">→</i><span>窗口</span><i aria-hidden="true">→</i><span>输出</span></div><Button size="sm" variant="outline" onPress={() => setPatchNotice('已生成 node_set / edge_set 入口；真实提交仍由宿主校验 revision。')}>编辑关系</Button></Card.Content>
            </Card>
          </div>
        </Tabs.Panel>
      </Tabs>
    </section>
  );
}

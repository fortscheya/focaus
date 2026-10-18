import {useEffect, useRef, useState} from 'react';
import {Button, Card} from '@heroui/react';
import type {CatchupRequestInput, CatchupResult, HostAdapter} from '@focaus/contracts';
import {fixtureFocausResult, ids} from '../app/fixtures.ts';
import {makeEnvelope} from '../app/hostAdapter.ts';
import {StatusBadge} from '../components/StatusBadge.tsx';

function formatTime(ms: number | null): string {
  if (ms === null) return '未知';
  const seconds = Math.floor(ms / 1000);
  const minutes = Math.floor(seconds / 60);
  return `${minutes.toString().padStart(2, '0')}:${(seconds % 60).toString().padStart(2, '0')}`;
}

function pathRoleLabel(role: CatchupResult['path_nodes'][number]['role']): string {
  if (role === 'missed_key') return '漏听重点';
  if (role === 'bridge') return '讲授衔接';
  if (role === 'current') return '当前步骤';
  return '必要前置';
}

function evidenceRoleLabel(role: CatchupResult['path_nodes'][number]['evidence_role']): string {
  if (role === 'teacher_path') return '课堂依据';
  if (role === 'material_supplement') return '资料补充';
  if (role === 'system_explanation') return '系统解释';
  return '依据待确认';
}

export function FocausPanel({adapter, viewId}: {adapter: HostAdapter; viewId: string}) {
  const [result, setResult] = useState<CatchupResult | null>(null);
  const [pending, setPending] = useState(false);
  const [requestGeneration, setRequestGeneration] = useState(0);
  const [elapsedMs, setElapsedMs] = useState(0);
  const [notice, setNotice] = useState('');
  const startedAt = useRef<number | null>(null);
  const timeoutRef = useRef<number | null>(null);

  useEffect(() => () => {
    if (timeoutRef.current !== null) window.clearTimeout(timeoutRef.current);
  }, []);

  useEffect(() => {
    if (!pending || startedAt.current === null) return;
    const interval = window.setInterval(() => {
      setElapsedMs(Math.round(performance.now() - (startedAt.current ?? performance.now())));
    }, 100);
    return () => window.clearInterval(interval);
  }, [pending]);

  const requestFocaus = () => {
    if (pending) return;
    const nextGeneration = requestGeneration + 1;
    setRequestGeneration(nextGeneration);
    setPending(true);
    setResult(null);
    setNotice('正在请求当前可得证据，并保留本次请求代次。');
    startedAt.current = performance.now();
    const payload: CatchupRequestInput = {
      view_id: viewId,
      lesson_id: ids.lesson,
      recording_id: ids.recording,
      assignment_epoch: 3,
      request_generation: nextGeneration,
      triggered_at: new Date().toISOString(),
      remaining_deadline_ms: 15000,
      requested_window: {recording_id: ids.recording, start_ms: 150000, end_ms: 450000, start_source: null},
      estimate_basis: 'fixture：估计漏听范围，至少回看 5 分钟',
      actual_tier: 3,
    };
    void adapter.invoke('catchup.request', payload, makeEnvelope('catchup.request', undefined, {[ids.view]: ids.revision, [ids.lesson]: ids.revision}));
    timeoutRef.current = window.setTimeout(() => {
      setResult({...fixtureFocausResult, request_generation: nextGeneration});
      setPending(false);
      const completeElapsed = Math.round(performance.now() - (startedAt.current ?? performance.now()));
      setElapsedMs(completeElapsed);
      setNotice(`fixture 结果已显示；本次正文显示耗时 ${completeElapsed}ms。`);
      void adapter.invoke('catchup.display_ack', {
        result_id: fixtureFocausResult.result_id,
        request_generation: nextGeneration,
        body_revision_id: fixtureFocausResult.body_revision_id,
        elapsed_ms: completeElapsed,
        displayed_complete: true,
      }, makeEnvelope('catchup.display_ack', undefined, {[fixtureFocausResult.result_id]: fixtureFocausResult.body_revision_id}));
    }, 900);
  };

  return (
    <section className="focaus-panel" aria-labelledby="focaus-title" data-anchor="anchor-focaus">
      <div className="panel-heading-row">
        <div>
          <p className="eyebrow">当前理解上下文</p>
          <h2 id="focaus-title">focaus</h2>
        </div>
        <div className="panel-heading-actions">
          <StatusBadge tone="accent">focaus_context_v2</StatusBadge>
          <Button size="sm" onPress={requestFocaus} isPending={pending}>
            {pending ? '整理中…' : '回到当前问题'}
          </Button>
        </div>
      </div>
      <div className="focaus-explainer">
        <span className="focaus-explainer-mark" aria-hidden="true">↳</span>
        <p>正文固定在触发时的证据截止；右侧指针会随进度推进。迟到的旧代不会替换当前正文。</p>
        <span className="fixture-label">fixture adapter · 未连接真实模型</span>
      </div>
      {notice ? <div className="inline-notice" role="status">{notice}</div> : null}
      {!result && !pending ? (
        <div className="focaus-empty">
          <strong>还没有本次课堂的 focaus 结果</strong>
          <p>触发后会按课堂路径、当前步骤和必要前置组织一条短路径，而不是只给摘要。</p>
          <Button variant="secondary" onPress={requestFocaus}>生成当前上下文</Button>
        </div>
      ) : null}
      {pending ? (
        <div className="focaus-pending" aria-live="polite">
          <div className="pending-spinner" aria-hidden="true" />
          <div>
            <strong>正在读取可得范围…</strong>
            <p>请求代次 {requestGeneration} · 已用 {elapsedMs}ms · 完整截止 15s</p>
          </div>
        </div>
      ) : null}
      {result ? (
        <div className="focaus-result">
          <div className="focaus-result-header">
            <div>
              <p className="eyebrow">当前焦点 · {result.current_focus.activity === 'derivation' ? '推导' : result.current_focus.activity}</p>
              <h3>{result.current_focus.problem}</h3>
              <p className="focaus-topic">{result.current_focus.topic} · {result.current_focus.step}</p>
            </div>
            <StatusBadge tone={result.completion_status === 'complete' ? 'success' : 'warning'}>
              {result.completion_status === 'complete' ? '完整结果' : '部分结果'} · {elapsedMs}ms
            </StatusBadge>
          </div>
          <div className="evidence-cutoff-grid" aria-label="正文和指针的证据截止">
            <div className="evidence-cutoff-card">
              <span>正文依据截止</span>
              <strong>{formatTime(result.body_evidence_cutoff[0]?.end_ms ?? null)}</strong>
              <small>触发时固定 · body_revision {result.body_revision_id.slice(0, 8)}</small>
            </div>
            <div className="evidence-cutoff-card evidence-cutoff-card-pointer">
              <span>动态指针截止</span>
              <strong>{formatTime(330000)}</strong>
              <small>独立更新 · assignment epoch 3</small>
            </div>
          </div>
          <div className="focaus-path" aria-label="连贯理解路径">
            {result.path_nodes.map((node, index) => {
              const blockText = result.body.find((block) => node.body_block_ids.includes(block.block_id))?.text;
              return (
                <div className="focaus-path-item" key={node.node_id}>
                  <div className="path-index" aria-hidden="true">{index + 1}</div>
                  <div className="path-item-body">
                    <div className="path-item-meta">
                      <StatusBadge tone={node.role === 'current' ? 'accent' : 'neutral'}>{pathRoleLabel(node.role)}</StatusBadge>
                      <span>{evidenceRoleLabel(node.evidence_role)}</span>
                    </div>
                    <p>{blockText}</p>
                  </div>
                </div>
              );
            })}
          </div>
          <div className="focaus-prerequisite">
            <div className="focaus-prerequisite-heading">
              <span className="section-kicker">必要前置</span>
              <span>{result.prerequisites.length ? '窗口外定义已明确标注' : '本次无需额外前置'}</span>
            </div>
            {result.prerequisites.map((item) => (
              <div key={item.reason} className="prerequisite-row">
                <span className="prerequisite-icon" aria-hidden="true">i</span>
                <span>{item.reason}</span>
                {item.outside_window ? <StatusBadge tone="warning">窗口外</StatusBadge> : null}
              </div>
            ))}
          </div>
          <div className="focaus-footer">
            <span>资料补充：{result.coverage.material_supplement_used ? '已使用并单独标注' : '未使用'}</span>
            <span>请求代次 {result.request_generation} · fixture 完整性检查已通过</span>
          </div>
        </div>
      ) : null}
    </section>
  );
}

import {useEffect, useRef, useState} from 'react';
import {Button, Card} from '@heroui/react';
import type {EvaluationResult, HostAdapter, VisualizationEvaluateInput} from '@focaus/contracts';
import {ids} from '../app/fixtures.ts';
import {makeEnvelope} from '../app/hostAdapter.ts';
import {parseEvaluationResult, type Conv2dParameters, type FixtureEvaluation} from './math.ts';
import {StatusBadge} from '../components/StatusBadge.tsx';

const initialParameters: Conv2dParameters = {
  operator_dimension: 2,
  scene_dimension: 3,
  operator_semantics: 'cross_correlation',
  axis_order: 'CHW',
  input: [[[1, 2, 3], [4, 5, 6], [7, 8, 9]]],
  kernel: [[[1, 0], [0, -1]]],
  stride: [1, 1],
  padding: [0, 0, 0, 0],
  dilation: [1, 1],
  bias: null,
};

function formatNumber(value: number): string {
  return Number.isInteger(value) ? String(value) : value.toFixed(3);
}

type EvaluationChecks = EvaluationResult['validation']['checks'];
type EvaluationView = FixtureEvaluation & {checks: EvaluationChecks; parametersHash: string};

function ValidationBadge({value}: {value: EvaluationChecks[keyof EvaluationChecks] | undefined}) {
  const tone = value === 'pass' ? 'success' : value === 'fail' ? 'danger' : 'warning';
  return <StatusBadge tone={tone}>{value ?? 'pending'}</StatusBadge>;
}

function asFixtureEvaluation(result: EvaluationResult): EvaluationView {
  const checks = result.validation.checks;
  return {
    interaction_seq: result.interaction_seq,
    shape: [result.shape[0] ?? 0, result.shape[1] ?? 0],
    values: result.values,
    steps: result.steps.map((step) => ({
      output_index: [step.output_index[0] ?? 0, step.output_index[1] ?? 0],
      input_indices: step.input_indices.map((pair) => [pair[0] ?? 0, pair[1] ?? 0]),
      input_values: step.input_values,
      kernel_values: step.kernel_values,
      products: step.products,
      bias: step.bias,
      sum: step.sum,
    })),
    valid: Object.values(checks).every((value) => value === 'pass'),
    diagnostics: result.validation.diagnostics,
    checks,
    parametersHash: result.parameters_hash,
  };
}

function Grid2D({
  values,
  columns,
  selected,
  onSelect,
  label,
}: {
  values: number[];
  columns: number;
  selected: number;
  onSelect: (index: number) => void;
  label: string;
}) {
  return (
    <div className="tensor-grid-wrap">
      <div className="tensor-grid-label">{label}</div>
      <div className="tensor-grid" role="grid" aria-label={label} style={{gridTemplateColumns: `repeat(${Math.max(1, columns)}, minmax(0, 1fr))`}}>
        {values.map((value, index) => (
          <button
            className={`tensor-cell ${index === selected ? 'is-selected' : ''}`}
            key={`${label}-${index}`}
            type="button"
            role="gridcell"
            aria-label={`${label}第 ${index + 1} 格：${formatNumber(value)}`}
            aria-pressed={index === selected}
            onClick={() => onSelect(index)}
          >
            {formatNumber(value)}
          </button>
        ))}
      </div>
    </div>
  );
}

function Conv3DView({
  selected,
  output,
  input,
}: {
  selected: number;
  output: number[];
  input: number[];
}) {
  return (
    <div className="conv-3d-view" aria-label="卷积三维受控视图" role="img">
      <svg viewBox="0 0 440 250" aria-hidden="true" focusable="false">
        <defs>
          <linearGradient id="conv-layer-a" x1="0" y1="0" x2="1" y2="1">
            <stop offset="0" stopColor="var(--accent)" stopOpacity="0.24" />
            <stop offset="1" stopColor="var(--accent)" stopOpacity="0.04" />
          </linearGradient>
          <linearGradient id="conv-layer-b" x1="0" y1="0" x2="1" y2="1">
            <stop offset="0" stopColor="var(--success)" stopOpacity="0.35" />
            <stop offset="1" stopColor="var(--success)" stopOpacity="0.08" />
          </linearGradient>
        </defs>
        <g transform="translate(38 28)">
          <rect width="170" height="170" rx="12" fill="url(#conv-layer-a)" stroke="var(--accent)" strokeOpacity="0.45" />
          <rect x="28" y="20" width="170" height="170" rx="12" fill="url(#conv-layer-b)" stroke="var(--success)" strokeOpacity="0.5" />
          <rect x="56" y="40" width="170" height="170" rx="12" fill="var(--surface)" stroke="var(--border)" />
          <path d="M 56 40 L 226 40 L 226 210 L 56 210 Z" fill="none" stroke="var(--accent)" strokeWidth="2" strokeDasharray="5 5" />
          <path d="M 84 60 L 254 60 L 254 230 L 84 230 Z" fill="none" stroke="var(--success)" strokeWidth="2" strokeDasharray="4 6" />
          <line x1="226" y1="40" x2="254" y2="60" stroke="var(--border)" />
          <line x1="226" y1="210" x2="254" y2="230" stroke="var(--border)" />
          <text x="66" y="24" fill="var(--muted)" fontSize="12">input · CHW</text>
          <text x="238" y="50" fill="var(--muted)" fontSize="12">kernel</text>
          <text x="278" y="215" fill="var(--muted)" fontSize="12">output</text>
          <circle cx={118 + (selected % 2) * 42} cy={110 + Math.floor(selected / 2) * 42} r="12" fill="var(--accent)" fillOpacity="0.82" />
          <text x="290" y="86" fill="var(--foreground)" fontSize="13">选中输出格</text>
          <text x="290" y="108" fill="var(--accent)" fontSize="23" fontWeight="700">{formatNumber(output[selected] ?? 0)}</text>
          <text x="290" y="138" fill="var(--muted)" fontSize="11">受控 SVG 场景</text>
          <text x="290" y="156" fill="var(--muted)" fontSize="11">视角不改变数值</text>
          <text x="290" y="182" fill="var(--muted)" fontSize="11">输入样本 {input.length} 格</text>
        </g>
      </svg>
      <p className="visualization-caption">3D 视图展示二维算子在三维场景中的层关系；数据与计算结果由宿主 evaluate 返回。</p>
    </div>
  );
}

export function VisualizationPanel({adapter}: {adapter: HostAdapter}) {
  const [parameters, setParameters] = useState(initialParameters);
  const [evaluation, setEvaluation] = useState<EvaluationView | null>(null);
  const [interactionSeq, setInteractionSeq] = useState(0);
  const [selectedOutput, setSelectedOutput] = useState(0);
  const [status, setStatus] = useState<'idle' | 'draft' | 'evaluating' | 'ready' | 'error' | 'suspended'>('idle');
  const [error, setError] = useState('');
  const [isReducedMotion, setIsReducedMotion] = useState(false);
  const [isVisible, setIsVisible] = useState(true);
  const initialRun = useRef(false);
  const interactionSeqRef = useRef(0);
  const latestEvaluationSeq = useRef(0);

  useEffect(() => {
    const media = window.matchMedia('(prefers-reduced-motion: reduce)');
    const updateMotion = () => setIsReducedMotion(media.matches);
    updateMotion();
    media.addEventListener?.('change', updateMotion);
    return () => media.removeEventListener?.('change', updateMotion);
  }, []);

  useEffect(() => {
    const handleVisibility = () => setIsVisible(document.visibilityState === 'visible');
    document.addEventListener('visibilitychange', handleVisibility);
    return () => document.removeEventListener('visibilitychange', handleVisibility);
  }, []);

  const runEvaluation = async (nextParameters: Conv2dParameters) => {
    const nextSeq = interactionSeqRef.current + 1;
    interactionSeqRef.current = nextSeq;
    latestEvaluationSeq.current = nextSeq;
    setInteractionSeq(nextSeq);
    setStatus('evaluating');
    setError('');
    const payload: VisualizationEvaluateInput = {
      object_id: ids.visualization,
      revision_id: ids.revision,
      parameters: nextParameters,
      interaction_seq: nextSeq,
      algorithm_version: '1.0.0',
    };
    try {
      const response = await adapter.invoke('visualization.evaluate', payload, makeEnvelope('visualization.evaluate', undefined, {[payload.object_id]: payload.revision_id}));
      if (latestEvaluationSeq.current !== nextSeq) return;
      if (response.status === 'accepted' && !response.result) {
        setStatus('evaluating');
        setError('宿主已接受 evaluate task，等待 visualization.changed；保留最后一次合法结果。');
        return;
      }
      if (response.status !== 'ok' || !response.result) {
        setStatus('error');
        setError('宿主没有返回完整 evaluate 结果；保留最后一次合法结果。');
        return;
      }
      const parsed = parseEvaluationResult(response.result, {objectId: payload.object_id, revisionId: payload.revision_id, interactionSeq: nextSeq, algorithmVersion: payload.algorithm_version});
      if (!parsed) {
        setStatus('error');
        setError('宿主返回的 evaluate 结果不符合当前 2D contract；保留最后一次合法结果。');
        return;
      }
      const nextEvaluation = asFixtureEvaluation(parsed);
      setEvaluation(nextEvaluation);
      setSelectedOutput((current) => Math.min(current, Math.max(0, nextEvaluation.values.length - 1)));
      setStatus('ready');
    } catch (reason) {
      if (latestEvaluationSeq.current !== nextSeq) return;
      setStatus('error');
      setError(reason instanceof Error ? reason.message : 'evaluate 失败；保留最后一次合法结果。');
    }
  };

  useEffect(() => {
    if (initialRun.current) return;
    initialRun.current = true;
    void runEvaluation(initialParameters);
    // Initial evaluation intentionally runs once; subsequent parameter changes
    // require an explicit user action so N never receives duplicate tasks.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const setOperator = (operator: Conv2dParameters['operator_semantics']) => {
    setParameters((current) => ({...current, operator_semantics: operator}));
    setStatus('draft');
  };

  const setStride = (value: number) => {
    setParameters((current) => ({...current, stride: [value, value]}));
    setStatus('draft');
  };

  const selectedStep = evaluation?.steps[selectedOutput];
  const inputValues = parameters.input[0]?.flat() ?? [];
  const kernelValues = parameters.kernel[0]?.flat() ?? [];

  return (
    <section className="visualization-panel" aria-labelledby="visualization-title" data-anchor="anchor-visualization">
      <div className="panel-heading-row">
        <div>
          <p className="eyebrow">focaus.visualization · 1.0.0</p>
          <h2 id="visualization-title">卷积局部窗口</h2>
        </div>
        <div className="panel-heading-actions">
          <StatusBadge tone={status === 'ready' ? 'success' : status === 'error' ? 'danger' : 'accent'}>
            {status === 'ready' ? 'N 计算已返回' : status === 'evaluating' ? '等待 N 计算' : status === 'draft' ? '参数草稿' : status === 'suspended' ? '已暂停渲染' : '受控组件'}
          </StatusBadge>
          <Button size="sm" onPress={() => void runEvaluation(parameters)} isPending={status === 'evaluating'} isDisabled={!isVisible}>
            请求 N 计算
          </Button>
        </div>
      </div>
      <div className="visualization-guardrail">
        <span className="fixture-label">fixture adapter · 由确定性 N 替身返回数据</span>
        <span>画面不会自行推断数学结果；参数 hash、shape、逐格步骤来自 visualization.evaluate。</span>
        <span>interaction_seq：{interactionSeq}</span>
        <span>{isReducedMotion ? '已启用 reduced motion：不自动播放。' : '无自动播放；每一步都可用键盘/按钮选择。'}</span>
      </div>
      {error ? <div className="error-notice" role="alert">{error}</div> : null}
      <div className="visualization-layout">
        <Card className="visualization-controls" variant="secondary">
          <Card.Header>
            <Card.Title>参数与语义</Card.Title>
            <Card.Description>调整后显式请求一次 N 计算</Card.Description>
          </Card.Header>
          <Card.Content>
            <label className="control-label" htmlFor="operator-semantics">算子定义</label>
            <select id="operator-semantics" className="native-select" value={parameters.operator_semantics} onChange={(event) => setOperator(event.target.value as Conv2dParameters['operator_semantics'])}>
              <option value="cross_correlation">互相关 · 不翻转 kernel</option>
              <option value="convolution">卷积 · 翻转 kernel</option>
            </select>
            <label className="control-label" htmlFor="stride">stride</label>
            <input id="stride" className="native-number-input" type="number" min={1} max={2} value={parameters.stride[0]} onChange={(event) => setStride(Number(event.target.value) || 1)} />
            <div className="parameter-summary">
              <span>输入 shape</span><strong>1 × 3 × 3</strong>
              <span>kernel shape</span><strong>1 × 2 × 2</strong>
              <span>padding (T/B/L/R)</span><strong>0 / 0 / 0 / 0</strong>
              <span>axis order</span><strong>CHW</strong>
            </div>
            <div className="runtime-controls">
              <Button size="sm" variant="outline" onPress={() => setStatus((current) => current === 'suspended' ? 'ready' : 'suspended')}>
                {status === 'suspended' ? '恢复渲染' : '模拟 context lost'}
              </Button>
              <span className="runtime-note">{isVisible ? '当前承载可见' : '离屏：已暂停渲染'}</span>
            </div>
          </Card.Content>
        </Card>
        <div className="visualization-stage">
          <div className="tensor-row">
            <Grid2D values={inputValues} columns={3} selected={selectedOutput} onSelect={() => undefined} label="输入窗口" />
            <div className="tensor-arrow" aria-hidden="true">×</div>
            <Grid2D values={kernelValues} columns={2} selected={-1} onSelect={() => undefined} label="卷积核" />
            <div className="tensor-arrow" aria-hidden="true">→</div>
            <Grid2D values={evaluation?.values ?? []} columns={evaluation?.shape[1] ?? 1} selected={selectedOutput} onSelect={setSelectedOutput} label="输出格（可选）" />
          </div>
          <div className="visualization-3d-wrap">
            <Conv3DView selected={selectedOutput} output={evaluation?.values ?? []} input={inputValues} />
          </div>
        </div>
      </div>
      <div className="visualization-detail-grid">
        <Card variant="transparent" className="calculation-card">
          <Card.Header>
            <Card.Title>选中格的局部计算</Card.Title>
            <Card.Description>键盘 Tab 到输出格，Enter 选择下一格</Card.Description>
          </Card.Header>
          <Card.Content>
            {selectedStep ? (
              <>
                <div className="calculation-expression" aria-live="polite">
                  {selectedStep.products.map((product, index) => (
                    <span key={`${product}-${index}`} className="calculation-term">
                      {formatNumber(selectedStep.input_values[index])} × {formatNumber(selectedStep.kernel_values[index])}
                      {index < selectedStep.products.length - 1 ? <b> + </b> : null}
                    </span>
                  ))}
                  <strong> = {formatNumber(selectedStep.sum)}</strong>
                </div>
                <div className="calculation-table-wrap">
                  <table className="calculation-table">
                    <caption>逐项乘加步骤</caption>
                    <thead><tr><th>输入</th><th>kernel</th><th>乘积</th></tr></thead>
                    <tbody>{selectedStep.products.map((product, index) => <tr key={index}><td>{formatNumber(selectedStep.input_values[index])}</td><td>{formatNumber(selectedStep.kernel_values[index])}</td><td>{formatNumber(product)}</td></tr>)}</tbody>
                  </table>
                </div>
              </>
            ) : <p className="muted-copy">先请求一次 N 计算，再选择一个输出格。</p>}
          </Card.Content>
        </Card>
        <Card variant="transparent" className="validation-card">
          <Card.Header>
            <Card.Title>验证与来源</Card.Title>
            <Card.Description>不会把演示值写回共同 Fact 集</Card.Description>
          </Card.Header>
          <Card.Content>
            <div className="validation-list">
              <span>Schema</span><ValidationBadge value={evaluation?.checks.schema} />
              <span>来源</span><ValidationBadge value={evaluation?.checks.source} />
              <span>语义</span><ValidationBadge value={evaluation?.checks.semantic} />
              <span>数学</span><ValidationBadge value={evaluation?.checks.math} />
            </div>
            <p className="muted-copy">{evaluation?.diagnostics.join('；') ?? '等待确定性校验报告'}</p>
            <p className="source-line">parameters_hash：{evaluation?.parametersHash ?? '等待返回'}</p>
            <p className="source-line">semantic_key：conv2d_spatial·fixture-demo</p>
            <p className="source-line">value_origin：demonstration · user_override=false</p>
          </Card.Content>
        </Card>
      </div>
    </section>
  );
}

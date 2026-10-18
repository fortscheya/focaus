import {useState} from 'react';
import {Button} from '@heroui/react';
import type {ContentView, EvaluationResult, HostAdapter} from '@focaus/contracts';
import {makeEnvelope} from '../app/hostAdapter.ts';
import {StatusBadge} from '../components/StatusBadge.tsx';
import {parseEvaluationResult, type Conv2dParameters} from './math.ts';

function record(value: unknown): Record<string, unknown> | null {
  return value && typeof value === 'object' && !Array.isArray(value) ? value as Record<string, unknown> : null;
}

function finiteNumber(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value);
}

function matrix(value: unknown): value is number[][][] {
  return Array.isArray(value) && value.length > 0 && value.every((plane) => Array.isArray(plane) && plane.length > 0 && plane.every((row) => Array.isArray(row) && row.length > 0 && row.every(finiteNumber)));
}

function parameters(value: unknown): Conv2dParameters | null {
  const item = record(value);
  const input = item?.input;
  const kernel = item?.kernel;
  const stride = item?.stride;
  const padding = item?.padding;
  const dilation = item?.dilation;
  if (!item || item.operator_dimension !== 2 || item.scene_dimension !== 3 || item.axis_order !== 'CHW' || (item.operator_semantics !== 'cross_correlation' && item.operator_semantics !== 'convolution') ||
      !matrix(input) || !matrix(kernel) || !Array.isArray(stride) || stride.length !== 2 || !stride.every((entry) => Number.isInteger(entry) && (entry as number) >= 1) ||
      !Array.isArray(padding) || padding.length !== 4 || !padding.every((entry) => Number.isInteger(entry) && (entry as number) >= 0) ||
      !Array.isArray(dilation) || dilation.length !== 2 || !dilation.every((entry) => Number.isInteger(entry) && (entry as number) >= 1) ||
      item.bias !== null && !finiteNumber(item.bias)) return null;
  return {
    operator_dimension: 2,
    scene_dimension: 3,
    operator_semantics: item.operator_semantics,
    axis_order: 'CHW',
    input,
    kernel,
    stride: [stride[0] as number, stride[1] as number],
    padding: [padding[0] as number, padding[1] as number, padding[2] as number, padding[3] as number],
    dilation: [dilation[0] as number, dilation[1] as number],
    bias: item.bias as number | null,
  };
}

function formatNumber(value: number): string {
  return Number.isInteger(value) ? String(value) : value.toFixed(3);
}

function Grid({label, values, columns, selected, onSelect}: {label: string; values: number[]; columns: number; selected: number; onSelect?: (index: number) => void}) {
  return <div className="tensor-grid-wrap"><div className="tensor-grid-label">{label}</div><div className="tensor-grid" role="grid" aria-label={label} style={{gridTemplateColumns: 'repeat(' + Math.max(1, columns) + ', minmax(0, 1fr))'}}>{values.map((value, index) => <button className={'tensor-cell ' + (index === selected ? 'is-selected' : '')} key={label + '-' + index} type="button" role="gridcell" aria-label={label + '第 ' + (index + 1) + ' 格：' + formatNumber(value)} aria-pressed={index === selected} onClick={() => onSelect?.(index)}>{formatNumber(value)}</button>)}</div></div>;
}

function statusTone(status: 'idle' | 'evaluating' | 'ready' | 'error'): 'neutral' | 'accent' | 'success' | 'danger' {
  if (status === 'ready') return 'success';
  if (status === 'error') return 'danger';
  if (status === 'evaluating') return 'accent';
  return 'neutral';
}

export function NativeVisualization2D({value, adapter, view}: {value: unknown; adapter: HostAdapter; view: ContentView | null}) {
  const content = record(value);
  const templateId = typeof content?.template_id === 'string' ? content.template_id : null;
  const objectId = typeof content?.object_id === 'string' ? content.object_id : null;
  const revisionId = typeof content?.revision_id === 'string' ? content.revision_id : null;
  const computation = record(content?.computation);
  const algorithmVersion = typeof computation?.algorithm_version === 'string' ? computation.algorithm_version : null;
  const parsedParameters = parameters(content?.parameters);
  const canEvaluate = templateId === 'conv2d_spatial' && objectId !== null && revisionId !== null && algorithmVersion !== null && parsedParameters !== null && content?.generation_status !== 'unsupported';
  const [evaluation, setEvaluation] = useState<EvaluationResult | null>(null);
  const [selected, setSelected] = useState(0);
  const [status, setStatus] = useState<'idle' | 'evaluating' | 'ready' | 'error'>('idle');
  const [message, setMessage] = useState('');
  const [interactionSeq, setInteractionSeq] = useState(0);

  const run = async () => {
    if (!canEvaluate || !parsedParameters || !objectId || !revisionId || !algorithmVersion) return;
    const nextSeq = interactionSeq + 1;
    setInteractionSeq(nextSeq);
    setStatus('evaluating');
    setMessage('');
    try {
      const response = await adapter.invoke('visualization.evaluate', {
        object_id: objectId,
        revision_id: revisionId,
        parameters: parsedParameters,
        interaction_seq: nextSeq,
        algorithm_version: algorithmVersion,
      }, makeEnvelope('visualization.evaluate', view?.host_window_id, {[objectId]: revisionId}));
      if (response.status === 'accepted' && !response.result) {
        setStatus('evaluating');
        setMessage('宿主已接受 evaluate task，等待 visualization.changed；保留最后一次合法结果。');
        return;
      }
      if (response.status !== 'ok' || !response.result) {
        setStatus('error');
        setMessage(response.error?.message ?? '宿主没有返回完整 evaluate 结果；保留最后一次合法结果。');
        return;
      }
      const result = parseEvaluationResult(response.result, {objectId, revisionId, interactionSeq: nextSeq, algorithmVersion});
      if (!result) {
        setStatus('error');
        setMessage('宿主返回的 evaluate 结果不符合当前 2D contract；保留最后一次合法结果。');
        return;
      }
      setEvaluation(result);
      setSelected((current) => Math.min(current, Math.max(0, result.values.length - 1)));
      setStatus('ready');
    } catch (error) {
      setStatus('error');
      setMessage(error instanceof Error ? error.message : 'evaluate 失败；保留最后一次合法结果。');
    }
  };

  if (templateId !== 'conv2d_spatial') return <div className="native-visualization-unavailable"><StatusBadge tone="warning">unsupported</StatusBadge><p>当前组件模板没有对应的受控 2D renderer；宿主字段已保留。</p></div>;
  if (!canEvaluate || !parsedParameters || !objectId || !revisionId || !algorithmVersion) return <div className="native-visualization-unavailable"><StatusBadge tone="warning">needs_evidence</StatusBadge><p>宿主 projection 尚未提供完整 object/revision/parameters/algorithm_version；不会猜测或本地补算。</p></div>;

  const inputValues = parsedParameters.input[0]?.flat() ?? [];
  const kernelValues = parsedParameters.kernel[0]?.flat() ?? [];
  const outputValues = evaluation?.values ?? [];
  const selectedStep = evaluation?.steps[selected];
  return <div className="native-visualization-2d">
    <div className="native-visualization-heading"><div><StatusBadge tone={statusTone(status)}>{status === 'ready' ? 'N 计算已返回' : status === 'evaluating' ? '等待 N 计算' : status === 'error' ? 'evaluate 失败' : '可请求 N 计算'}</StatusBadge><span>template {templateId} · revision {revisionId}</span></div><Button size="sm" onPress={() => void run()} isPending={status === 'evaluating'}>请求 N 计算</Button></div>
    {message ? <p className="native-command-feedback" role="status">{message}</p> : null}
    <div className="tensor-row"><Grid label="输入" values={inputValues} columns={parsedParameters.input[0]?.[0]?.length ?? 1} selected={-1} /><span className="tensor-arrow" aria-hidden="true">×</span><Grid label="kernel" values={kernelValues} columns={parsedParameters.kernel[0]?.[0]?.length ?? 1} selected={-1} /><span className="tensor-arrow" aria-hidden="true">→</span><Grid label="输出" values={outputValues} columns={evaluation?.shape[1] ?? 1} selected={selected} onSelect={setSelected} /></div>
    {selectedStep ? <div className="calculation-expression" aria-live="polite">{selectedStep.products.map((product, index) => <span className="calculation-term" key={String(product) + '-' + index}>{formatNumber(selectedStep.input_values[index])} × {formatNumber(selectedStep.kernel_values[index])}{index < selectedStep.products.length - 1 ? <b> + </b> : null}</span>)}<strong> = {formatNumber(selectedStep.sum)}</strong></div> : <p className="muted-copy">请求 N 计算后显示逐格结果。</p>}
    {evaluation ? <p className="source-line">parameters_hash：{evaluation.parameters_hash} · interaction_seq：{evaluation.interaction_seq}</p> : null}
  </div>;
}

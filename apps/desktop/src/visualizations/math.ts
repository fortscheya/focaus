import type {CheckState, EvaluationResult} from '@focaus/contracts';

export type OperatorSemantics = 'cross_correlation' | 'convolution';

export interface Conv2dParameters {
  operator_dimension: 2;
  scene_dimension: 3;
  operator_semantics: OperatorSemantics;
  axis_order: 'CHW';
  input: number[][][];
  kernel: number[][][];
  stride: [number, number];
  padding: [number, number, number, number];
  dilation: [number, number];
  bias: number | null;
}

export interface EvaluationStep {
  output_index: [number, number];
  input_indices: Array<[number, number]>;
  input_values: number[];
  kernel_values: number[];
  products: number[];
  bias: number;
  sum: number;
}

export interface FixtureEvaluation {
  interaction_seq: number;
  shape: [number, number];
  values: number[];
  steps: EvaluationStep[];
  valid: boolean;
  diagnostics: string[];
}

export interface EvaluationExpectation {
  objectId: string;
  revisionId: string;
  interactionSeq: number;
  algorithmVersion: string;
}

function objectRecord(value: unknown): Record<string, unknown> | null {
  return value && typeof value === 'object' && !Array.isArray(value) ? value as Record<string, unknown> : null;
}

function isFiniteNumber(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value);
}

function numberArray(value: unknown): value is number[] {
  return Array.isArray(value) && value.every(isFiniteNumber);
}

function checkState(value: unknown): value is CheckState {
  return value === 'pending' || value === 'pass' || value === 'fail' || value === 'unknown';
}

function validHash(value: unknown): value is string {
  return typeof value === 'string' && /^[0-9a-f]{64}$/.test(value);
}

function validStep(value: unknown): boolean {
  const step = objectRecord(value);
  const outputIndex = step?.output_index;
  const inputIndices = step?.input_indices;
  const inputValues = step?.input_values;
  const kernelValues = step?.kernel_values;
  const products = step?.products;
  return Boolean(
    step && Array.isArray(outputIndex) && outputIndex.length === 2 && outputIndex.every((item) => Number.isInteger(item) && (item as number) >= 0) &&
    Array.isArray(inputIndices) && numberArray(inputValues) && inputIndices.length === inputValues.length && inputIndices.every((pair) => Array.isArray(pair) && pair.length === 2 && pair.every((item) => Number.isInteger(item))) &&
    numberArray(kernelValues) && numberArray(products) && inputValues.length === kernelValues.length && kernelValues.length === products.length &&
    isFiniteNumber(step.bias) && isFiniteNumber(step.sum)
  );
}

/** Validate a host evaluate response before it can change the controlled 2D view. */
export function parseEvaluationResult(value: unknown, expected: EvaluationExpectation): EvaluationResult | null {
  const result = objectRecord(value);
  const validation = objectRecord(result?.validation);
  const checks = objectRecord(validation?.checks);
  const shape = result?.shape;
  const values = result?.values;
  const steps = result?.steps;
  if (!result || !validation || !checks || !Array.isArray(shape) || shape.length !== 2 || !shape.every((item) => Number.isInteger(item) && (item as number) >= 1) ||
      !numberArray(values) || !Array.isArray(steps) || steps.length !== values.length || values.length !== (shape[0] as number) * (shape[1] as number) || !steps.every(validStep) ||
      typeof result.object_id !== 'string' || typeof result.revision_id !== 'string' || !Number.isInteger(result.interaction_seq) || (result.interaction_seq as number) < 0 ||
      !validHash(result.parameters_hash) || typeof result.algorithm_version !== 'string' || !validHash(validation.parameters_hash) || typeof validation.validator_version !== 'string' || typeof validation.validated_revision !== 'string' ||
      !Array.isArray(validation.diagnostics) || !validation.diagnostics.every((item) => typeof item === 'string') || !checkState(checks.schema) || !checkState(checks.source) || !checkState(checks.semantic) || !checkState(checks.math)) return null;
  if (result.object_id !== expected.objectId || result.revision_id !== expected.revisionId || result.interaction_seq !== expected.interactionSeq || result.algorithm_version !== expected.algorithmVersion || validation.parameters_hash !== result.parameters_hash || validation.validated_revision !== expected.revisionId) return null;
  return result as unknown as EvaluationResult;
}

export function evaluateConv2d(parameters: Conv2dParameters, interactionSeq: number): FixtureEvaluation {
  const input = parameters.input[0] ?? [];
  const kernel = parameters.kernel[0] ?? [];
  const inputHeight = input.length;
  const inputWidth = input[0]?.length ?? 0;
  const kernelHeight = kernel.length;
  const kernelWidth = kernel[0]?.length ?? 0;
  const [strideY, strideX] = parameters.stride;
  // Registry order is [top, bottom, left, right], deliberately not CSS order.
  const [padTop, padBottom, padLeft, padRight] = parameters.padding;
  const [dilationY, dilationX] = parameters.dilation;
  const effectiveHeight = dilationY * (kernelHeight - 1) + 1;
  const effectiveWidth = dilationX * (kernelWidth - 1) + 1;
  const outputHeight = Math.floor((inputHeight + padTop + padBottom - effectiveHeight) / strideY) + 1;
  const outputWidth = Math.floor((inputWidth + padLeft + padRight - effectiveWidth) / strideX) + 1;
  const diagnostics: string[] = [];

  if (
    strideY < 1 || strideX < 1 || dilationY < 1 || dilationX < 1 ||
    padTop < 0 || padRight < 0 || padBottom < 0 || padLeft < 0 ||
    outputHeight < 1 || outputWidth < 1 || kernelHeight < 1 || kernelWidth < 1
  ) {
    return {interaction_seq: interactionSeq, shape: [Math.max(0, outputHeight), Math.max(0, outputWidth)], values: [], steps: [], valid: false, diagnostics: ['stride、dilation、padding 或 kernel 尺寸无效']};
  }

  const values: number[] = [];
  const steps: EvaluationStep[] = [];
  for (let outputY = 0; outputY < outputHeight; outputY += 1) {
    for (let outputX = 0; outputX < outputWidth; outputX += 1) {
      const inputValues: number[] = [];
      const kernelValues: number[] = [];
      const products: number[] = [];
      const inputIndices: Array<[number, number]> = [];
      let sum = parameters.bias ?? 0;
      for (let kernelY = 0; kernelY < kernelHeight; kernelY += 1) {
        for (let kernelX = 0; kernelX < kernelWidth; kernelX += 1) {
          const sourceY = outputY * strideY + kernelY * dilationY - padTop;
          const sourceX = outputX * strideX + kernelX * dilationX - padLeft;
          const inputValue = sourceY >= 0 && sourceY < inputHeight && sourceX >= 0 && sourceX < inputWidth ? input[sourceY][sourceX] : 0;
          const kernelIndexY = parameters.operator_semantics === 'convolution' ? kernelHeight - 1 - kernelY : kernelY;
          const kernelIndexX = parameters.operator_semantics === 'convolution' ? kernelWidth - 1 - kernelX : kernelX;
          const kernelValue = kernel[kernelIndexY][kernelIndexX];
          inputValues.push(inputValue);
          kernelValues.push(kernelValue);
          products.push(inputValue * kernelValue);
          inputIndices.push([sourceY, sourceX]);
          sum += inputValue * kernelValue;
        }
      }
      values.push(sum);
      steps.push({output_index: [outputY, outputX], input_indices: inputIndices, input_values: inputValues, kernel_values: kernelValues, products, bias: parameters.bias ?? 0, sum});
    }
  }
  diagnostics.push(parameters.operator_semantics === 'convolution' ? 'kernel 已按定义翻转' : 'kernel 按互相关顺序使用');
  return {interaction_seq: interactionSeq, shape: [outputHeight, outputWidth], values, steps, valid: true, diagnostics};
}

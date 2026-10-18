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

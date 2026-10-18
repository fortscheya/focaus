import {invoke as tauriInvoke} from '@tauri-apps/api/core';
import {listen} from '@tauri-apps/api/event';
import type {
  Command,
  CommandInputs,
  CommandName,
  CommandResult,
  EventBatch,
  HostAdapter,
  AppSnapshot,
  ContentView,
  StateOpenInput,
  StateOpenResult,
  EvaluationResult,
  VisualizationEvaluateInput,
} from '@focaus/contracts';

export const CONTRACT_VERSION = '2.0.0' as const;
export const MATERIAL_FORMAT = 'focaus.material/2.0.0' as const;
export const VISUALIZATION_PROTOCOL = 'focaus.visualization/1.0.0' as const;

export const FIXTURE_WINDOW_ID = '0b2e8d5a-2bd7-4a48-b6f8-1ef4f27e1a11';
let activeWindowId = FIXTURE_WINDOW_ID;

export type AdapterCommandLog = Pick<Command, 'name' | 'actor' | 'window_id' | 'payload'> & {
  command_id: string;
  at: string;
};

export type FixtureCommandResult = CommandResult & {
  result?: unknown;
};

function id(prefix: string): string {
  void prefix;
  return crypto.randomUUID();
}

export function setActiveWindowId(windowId: string): void {
  activeWindowId = windowId;
}

export function makeEnvelope(name: CommandName, windowId = activeWindowId, expectedRevisions: Record<string, string> = {}): Omit<Command, 'name' | 'payload'> {
  const commandId = id('cmd');
  return {
    contract_version: CONTRACT_VERSION,
    command_id: commandId,
    actor: 'user',
    window_id: windowId,
    idempotency_key: `${name}:${commandId}`,
    expected_revisions: expectedRevisions,
  };
}

export class FixtureHostAdapter implements HostAdapter {
  readonly mode = 'fixture' as const;
  readonly commands: AdapterCommandLog[] = [];

  async invoke<N extends CommandName>(
    name: N,
    payload: CommandInputs[N],
    envelope: Omit<Command, 'name' | 'payload'>,
  ): Promise<CommandResult> {
    this.commands.push({
      name,
      actor: envelope.actor,
      window_id: envelope.window_id,
      payload,
      command_id: envelope.command_id,
      at: new Date().toISOString(),
    });

    if (name === 'state.open') {
      const snapshot: AppSnapshot = {
        contract_version: CONTRACT_VERSION,
        host_run_id: '4c11a85f-f3da-4f4f-9701-000000000021',
        snapshot_seq: this.commands.length,
        scopes: [{kind: 'global'}, {kind: 'lesson', lesson_id: '4c11a85f-f3da-4f4f-9701-000000000002'}],
        global: {recording_state: 'stopped', fixture: true},
        records: {},
        continuations: [],
      };
      const result: StateOpenResult = {
        subscription_id: '4c11a85f-f3da-4f4f-9701-000000000022',
        snapshot,
        snapshot_seq: snapshot.snapshot_seq,
        negotiated: {contract_version: CONTRACT_VERSION, material_version: '2.0.0', visualization_version: '1.0.0', write_allowed: false, unsupported: []},
      };
      return {
        contract_version: CONTRACT_VERSION,
        command_id: envelope.command_id,
        status: 'ok',
        result,
        committed_revisions: {},
        event_seq: this.commands.length,
      };
    }

    if (name === 'state.ack') {
      return {
        contract_version: CONTRACT_VERSION,
        command_id: envelope.command_id,
        status: 'ok',
        result: {acknowledged: true},
        committed_revisions: {},
        event_seq: this.commands.length,
      };
    }

    if (name === 'visualization.evaluate') {
      return {
        contract_version: CONTRACT_VERSION,
        command_id: envelope.command_id,
        status: 'ok',
        result: fixtureEvaluation(payload as VisualizationEvaluateInput),
        committed_revisions: {},
        event_seq: this.commands.length,
      };
    }

    if (name === 'search.open') {
      return {
        contract_version: CONTRACT_VERSION,
        command_id: envelope.command_id,
        status: 'ok',
        result: {
          search_id: id('search'),
          status: 'ready',
          index_generation: 7,
          visibility_epoch: 3,
        },
        committed_revisions: {},
        event_seq: this.commands.length,
      };
    }

    if (
      name === 'recording.start' ||
      name === 'recording.pause' ||
      name === 'recording.resume' ||
      name === 'recording.stop' ||
      name === 'view.move' ||
      name === 'view.ready' ||
      name === 'catchup.request' ||
      name === 'content.patch' ||
      name === 'content.undo' ||
      name === 'correction.draft' ||
      name === 'correction.accept' ||
      name === 'correction.reject' ||
      name === 'correction.accept_batch' ||
      name === 'language.select' ||
      name === 'source.resolve' ||
      name === 'source.diff' ||
      name === 'visualization.patch' ||
      name === 'catchup.display_ack'
    ) {
      return {
        contract_version: CONTRACT_VERSION,
        command_id: envelope.command_id,
        status: 'accepted',
        task_id: id('task'),
        current_state: 'queued',
        committed_revisions: {},
        event_seq: this.commands.length,
      };
    }

    return {
      contract_version: CONTRACT_VERSION,
      command_id: envelope.command_id,
      status: 'error',
      error: {
        code: 'UNSUPPORTED_CAPABILITY',
        retryable: false,
        scope: 'command',
        message: 'Fixture adapter has no canned response; native operation has not run.',
      },
    };
  }
}

export interface NativeBootstrapResult {
  window_id: string;
  contract_version: string;
  view_context?: {
    view_id: string;
    transfer_id: string | null;
    base_view_revision: string | null;
  } | null;
}

type NativeViewTransferContext = NonNullable<NativeBootstrapResult['view_context']>;

/**
 * Native entry point. Tauri owns the authoritative window id and command validation.
 * The fixture never calls this function; it remains available for host wiring.
 */
export async function bootstrapNativeWindow(): Promise<NativeBootstrapResult> {
  return tauriInvoke<NativeBootstrapResult>('focaus_bootstrap');
}

export function createNativeHostAdapter(windowId: string): HostAdapter {
  return {
    mode: 'native',
    async invoke<N extends CommandName>(
      name: N,
      payload: CommandInputs[N],
      envelope: Omit<Command, 'name' | 'payload'>,
    ): Promise<CommandResult> {
      const command: Command = {name, payload, ...envelope} as Command;
      // The authoritative window_id is inside Command. Native verifies it
      // against the actual WebView; the bridge receives only {command}.
      if (command.window_id !== windowId) {
        throw new Error('Native window identity changed before command dispatch.');
      }
      return tauriInvoke<CommandResult>('focaus_command', {command});
    },
  };
}

export interface NativeRuntime {
  mode: 'native';
  windowId: string;
  adapter: HostAdapter;
  state: StateOpenResult;
  eventSeq: number;
  stopEvents: () => void;
  readyView?: ContentView;
}

export interface FixtureRuntime {
  mode: 'fixture';
  windowId: string;
  adapter: FixtureHostAdapter;
  eventSeq: number;
}

export type HostRuntime = NativeRuntime | FixtureRuntime;

export const hostCapabilities: StateOpenInput['capabilities'] = {
  supported_contract_majors: [2],
  supported_material_majors: [2],
  visualization_protocol_majors: [1],
  templates: {function_curve_2d: ['1.0.0'], conv2d_spatial: ['1.0.0']},
  search_projection_versions: [1],
  focaus_result_semantics: ['focaus_context_v2'],
};

export async function bootstrapNativeRuntime(onEventBatch: (batch: EventBatch) => void): Promise<NativeRuntime> {
  const bootstrap = await bootstrapNativeWindow();
  if (!bootstrap.window_id || bootstrap.contract_version !== CONTRACT_VERSION) {
    throw new Error(`focaus_bootstrap 返回的 window_id/contract_version 无效：${JSON.stringify(bootstrap)}`);
  }
  const adapter = createNativeHostAdapter(bootstrap.window_id);
  setActiveWindowId(bootstrap.window_id);
  let transferContext: NativeViewTransferContext | null = bootstrap.view_context ?? null;
  // Subscribe before opening state so events committed during bootstrap are
  // not lost between the snapshot and the first acknowledgement.
  const stopEvents = await listen<EventBatch>('focaus://events', (event) => onEventBatch(event.payload));
  const stopTransferEvents = await listen<NativeViewTransferContext>('focaus://view-transfer', (event) => {
    transferContext = event.payload;
  });
  try {
    const openResponse = await adapter.invoke('state.open', {
      scopes: [{kind: 'global'}],
      last_seq: null,
      capabilities: hostCapabilities,
    }, makeEnvelope('state.open'));
    if (openResponse.status !== 'ok' || !openResponse.result) {
      throw new Error(openResponse.error?.message ?? 'state.open 未返回有效快照。');
    }
    const state = openResponse.result as StateOpenResult;
    if (state.snapshot.contract_version !== CONTRACT_VERSION) {
      throw new Error(`state.open 快照契约版本不匹配：${state.snapshot.contract_version}`);
    }
    const ackResponse = await adapter.invoke('state.ack', {subscription_id: state.subscription_id, upto_seq: state.snapshot_seq}, makeEnvelope('state.ack'));
    if (ackResponse.status === 'error') {
      throw new Error(ackResponse.error?.message ?? 'state.ack 失败。');
    }
    let readyView: ContentView | undefined;
    const viewContext = transferContext;
    if (viewContext?.transfer_id) {
      const viewResponse = await adapter.invoke('content.get', {
        entity_id: viewContext.view_id,
        revision_id: viewContext.base_view_revision,
        selector: null,
      }, makeEnvelope('content.get', undefined, viewContext.base_view_revision ? {[viewContext.view_id]: viewContext.base_view_revision} : {}));
      if (viewResponse.status !== 'ok' || !viewResponse.result) {
        throw new Error(viewResponse.error?.message ?? 'view_context content.get 失败，旧承载保持不变。');
      }
      const viewResult = viewResponse.result && typeof viewResponse.result === 'object' && 'view' in viewResponse.result ? (viewResponse.result as {view: unknown}).view : viewResponse.result;
      if (!viewResult || typeof viewResult !== 'object' || typeof (viewResult as {view_revision?: unknown}).view_revision !== 'string') {
        throw new Error('view_context content.get 未返回合法 ContentView，旧承载保持不变。');
      }
      readyView = viewResult as ContentView;
      const readyResponse = await adapter.invoke('view.ready', {transfer_id: viewContext.transfer_id, view_revision: readyView.view_revision}, makeEnvelope('view.ready', undefined, {[viewContext.view_id]: readyView.view_revision}));
      if (readyResponse.status === 'error') {
        throw new Error(readyResponse.error?.message ?? 'view.ready 失败，旧承载保持不变。');
      }
    }
    return {mode: 'native', windowId: bootstrap.window_id, adapter, state, eventSeq: state.snapshot_seq, stopEvents: () => {stopEvents(); stopTransferEvents();}, readyView};
  } catch (error) {
    stopEvents();
    stopTransferEvents();
    throw error;
  }
}

function fixtureEvaluation(input: VisualizationEvaluateInput): EvaluationResult {
  const parameters = input.parameters as {
    input: number[][][];
    kernel: number[][][];
    stride: [number, number];
    padding: [number, number, number, number];
    dilation: [number, number];
    bias: number | null;
    operator_semantics: 'cross_correlation' | 'convolution';
  };
  const inputPlane = parameters.input[0] ?? [];
  const kernelPlane = parameters.kernel[0] ?? [];
  const [strideY, strideX] = parameters.stride;
  // Registry order is [top, bottom, left, right], deliberately not CSS order.
  const [padTop, padBottom, padLeft, padRight] = parameters.padding;
  const [dilationY, dilationX] = parameters.dilation;
  const kernelHeight = kernelPlane.length;
  const kernelWidth = kernelPlane[0]?.length ?? 0;
  const inputHeight = inputPlane.length;
  const inputWidth = inputPlane[0]?.length ?? 0;
  const effectiveHeight = dilationY * (kernelHeight - 1) + 1;
  const effectiveWidth = dilationX * (kernelWidth - 1) + 1;
  const outputHeight = Math.floor((inputHeight + padTop + padBottom - effectiveHeight) / strideY) + 1;
  const outputWidth = Math.floor((inputWidth + padLeft + padRight - effectiveWidth) / strideX) + 1;

  if (
    strideY < 1 ||
    strideX < 1 ||
    dilationY < 1 ||
    dilationX < 1 ||
    padTop < 0 ||
    padRight < 0 ||
    padBottom < 0 ||
    padLeft < 0 ||
    outputHeight < 1 ||
    outputWidth < 1
  ) {
    throw new Error('Fixture evaluation received invalid convolution parameters.');
  }

  const values: number[] = [];
  const steps = [];
  for (let outputY = 0; outputY < outputHeight; outputY += 1) {
    for (let outputX = 0; outputX < outputWidth; outputX += 1) {
      const inputValues: number[] = [];
      const kernelValues: number[] = [];
      const products: number[] = [];
      const inputIndices: number[][] = [];
      let sum = parameters.bias ?? 0;
      for (let kernelY = 0; kernelY < kernelHeight; kernelY += 1) {
        for (let kernelX = 0; kernelX < kernelWidth; kernelX += 1) {
          const sourceY = outputY * strideY + kernelY * dilationY - padTop;
          const sourceX = outputX * strideX + kernelX * dilationX - padLeft;
          const inputValue = sourceY >= 0 && sourceY < inputHeight && sourceX >= 0 && sourceX < inputWidth ? inputPlane[sourceY][sourceX] : 0;
          const kernelIndexY = parameters.operator_semantics === 'convolution' ? kernelHeight - 1 - kernelY : kernelY;
          const kernelIndexX = parameters.operator_semantics === 'convolution' ? kernelWidth - 1 - kernelX : kernelX;
          const kernelValue = kernelPlane[kernelIndexY][kernelIndexX];
          inputValues.push(inputValue);
          kernelValues.push(kernelValue);
          products.push(inputValue * kernelValue);
          inputIndices.push([sourceY, sourceX]);
          sum += inputValue * kernelValue;
        }
      }
      values.push(sum);
      steps.push({
        output_index: [outputY, outputX],
        input_indices: inputIndices,
        input_values: inputValues,
        kernel_values: kernelValues,
        products,
        bias: parameters.bias ?? 0,
        sum,
      });
    }
  }

  return {
    object_id: input.object_id,
    revision_id: input.revision_id,
    interaction_seq: input.interaction_seq,
    parameters_hash: 'a'.repeat(64),
    algorithm_version: input.algorithm_version,
    shape: [outputHeight, outputWidth],
    values,
    steps,
    validation: {
      checks: {schema: 'pass', source: 'pass', semantic: 'pass', math: 'pass'},
      validator_version: '1.0.0',
      parameters_hash: 'a'.repeat(64),
      validated_revision: input.revision_id,
      diagnostics: ['fixture adapter: deterministic result; native N validator not called'],
    },
  };
}

export * from './generated.ts';
import type { CommandInputs, CommandOutputs, CommandName, Command, CommandResult } from './generated.ts';
/** Dispatches to the host; implementations must validate responses against the shared schema. */
export interface HostAdapter {
  readonly mode: 'fixture' | 'native';
  invoke<N extends CommandName>(name: N, payload: CommandInputs[N], envelope: Omit<Command, 'name' | 'payload'>): Promise<CommandResult>;
}
export type TypedResponse<N extends CommandName> = CommandOutputs[N];
export const CONTRACT_VERSION = '2.0.0';
export const MATERIAL_FORMAT = 'focaus.material/2.0.0';
export const VISUALIZATION_PROTOCOL = 'focaus.visualization/1.0.0';

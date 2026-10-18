import type { Command, CommandInputs, CommandName, CommandResult } from './generated.ts';
import type { HostAdapter } from './index.ts';
/** Tests/UI previews must supply canned responses explicitly; unknown calls fail closed. */
export class FixtureAdapter implements HostAdapter {
  readonly mode = 'fixture' as const;
  constructor(private responses: Partial<Record<CommandName, (input: unknown) => CommandResult>>) {}
  async invoke<N extends CommandName>(name:N,payload:CommandInputs[N],envelope:Omit<Command,'name'|'payload'>):Promise<CommandResult> {
    const response=this.responses[name];
    if (response) return response(payload);
    return {contract_version:'2.0.0',command_id:envelope.command_id,status:'error',error:{code:'UNSUPPORTED_CAPABILITY',retryable:false,scope:'command',message:'Fixture has no response; native operation has not run'}};
  }
}

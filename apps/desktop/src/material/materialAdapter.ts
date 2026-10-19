export type MaterialPurpose = 'textbook' | 'courseware' | 'reference';

export type MaterialTaskStatus = 'idle' | 'accepted' | 'running' | 'partial' | 'completed' | 'failed' | 'error';

export interface MaterialImportRequest {
  lessonId: string;
  purpose: MaterialPurpose;
  pages: number[];
}

export interface MaterialTaskError {
  code: string | null;
  message: string;
  scope: string | null;
  retryable: boolean | null;
}

function record(value: unknown): Record<string, unknown> | null {
  return value && typeof value === 'object' ? value as Record<string, unknown> : null;
}

/** Keep host SafeError fields available to the UI without importing host-specific code here. */
export function parseMaterialTaskError(value: unknown): MaterialTaskError | null {
  const outer = record(value);
  const nested = outer && 'error' in outer ? outer.error : value;
  const item = record(nested);
  const code = typeof item?.code === 'string' ? item.code : typeof item?.error_code === 'string' ? item.error_code : null;
  const message = typeof item?.message === 'string' ? item.message : typeof item?.error_message === 'string' ? item.error_message : typeof nested === 'string' ? nested : value instanceof Error ? value.message : null;
  const scope = typeof item?.scope === 'string' ? item.scope : null;
  const retryable = typeof item?.retryable === 'boolean' ? item.retryable : null;
  if (!code && !message && !scope && retryable === null) return null;
  return {code, message: message ?? '宿主资料任务失败。', scope, retryable};
}

export interface MaterialImportResult {
  status: 'accepted' | 'error';
  taskId?: string;
  materialId?: string;
  sourceVersionId?: string;
  parseVersionId?: string;
  message: string;
  error?: MaterialTaskError | null;
}

export interface MaterialTaskFeedback {
  status: MaterialTaskStatus;
  taskId: string | null;
  materialId: string | null;
  sourceVersionId: string | null;
  parseVersionId: string | null;
  message: string;
  error?: MaterialTaskError | null;
}

/**
 * The UI never receives or submits a filesystem path. The host owns the
 * picker, permission boundary, source handle and subsequent material task.
 */
export interface MaterialImportAdapter {
  readonly mode: 'native' | 'fixture';
  requestMaterialImport(request: MaterialImportRequest): Promise<MaterialImportResult>;
}

export const unavailableMaterialImportAdapter: MaterialImportAdapter = {
  mode: 'native',
  async requestMaterialImport(): Promise<MaterialImportResult> {
    return {status: 'error', message: '宿主文件选择入口尚未接线；UI 不会接收任意路径。'};
  },
};


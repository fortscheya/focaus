export type MaterialPurpose = 'textbook' | 'courseware' | 'reference';

export type MaterialTaskStatus = 'idle' | 'accepted' | 'running' | 'partial' | 'completed' | 'failed' | 'error';

export interface MaterialImportRequest {
  lessonId: string;
  purpose: MaterialPurpose;
  pages: number[];
}

export interface MaterialImportResult {
  status: 'accepted' | 'error';
  taskId?: string;
  materialId?: string;
  sourceVersionId?: string;
  parseVersionId?: string;
  message: string;
}

export interface MaterialTaskFeedback {
  status: MaterialTaskStatus;
  taskId: string | null;
  materialId: string | null;
  sourceVersionId: string | null;
  parseVersionId: string | null;
  message: string;
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


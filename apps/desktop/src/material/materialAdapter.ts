export type MaterialPurpose = 'textbook' | 'courseware' | 'reference';

export interface MaterialImportRequest {
  lessonId: string;
  purpose: MaterialPurpose;
  pages: number[];
}

export interface MaterialImportResult {
  status: 'accepted' | 'error';
  taskId?: string;
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


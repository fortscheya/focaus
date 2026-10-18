import type {CatchupResult, LanguageVariant, RecordingState} from '@focaus/contracts';

export const ids = {
  course: '4c11a85f-f3da-4f4f-9701-000000000001',
  lesson: '4c11a85f-f3da-4f4f-9701-000000000002',
  recording: '4c11a85f-f3da-4f4f-9701-000000000003',
  note: '4c11a85f-f3da-4f4f-9701-000000000004',
  view: '4c11a85f-f3da-4f4f-9701-000000000005',
  block: '4c11a85f-f3da-4f4f-9701-000000000006',
  revision: '4c11a85f-f3da-4f4f-9701-000000000007',
  source: '4c11a85f-f3da-4f4f-9701-000000000008',
  sourceRevision: '4c11a85f-f3da-4f4f-9701-000000000009',
  searchUnit: '4c11a85f-f3da-4f4f-9701-000000000010',
  visualization: '4c11a85f-f3da-4f4f-9701-000000000011',
} as const;

export type WorkspaceViewKind = 'note' | 'raw' | 'secondary' | 'material' | 'focaus' | 'visualization';

export interface WorkspaceViewFixture {
  id: string;
  kind: WorkspaceViewKind;
  title: string;
  shortTitle: string;
  description: string;
  anchorId: string;
}

export const workspaceViews: WorkspaceViewFixture[] = [
  {id: 'view-note', kind: 'note', title: '课堂笔记', shortTitle: '笔记', description: '同一 Fact 集的当前第 3 档呈现', anchorId: 'anchor-note'},
  {id: 'view-raw', kind: 'raw', title: '原始转写', shortTitle: '原始', description: '保留老师原话与原始时间', anchorId: 'anchor-raw'},
  {id: 'view-secondary', kind: 'secondary', title: '二次转写', shortTitle: '二次', description: '原话修剪与订正候选', anchorId: 'anchor-secondary'},
  {id: 'view-material', kind: 'material', title: '课件', shortTitle: '课件', description: '当前课堂范围的材料与来源', anchorId: 'anchor-material'},
  {id: 'view-focaus', kind: 'focaus', title: 'focaus', shortTitle: 'focaus', description: '当前问题、衔接与必要前置', anchorId: 'anchor-focaus'},
  {id: 'view-visualization', kind: 'visualization', title: '交互知识组件', shortTitle: '组件', description: '受控参数与确定性计算结果', anchorId: 'anchor-visualization'},
];

export interface SearchFixtureEntry {
  id: string;
  kind: 'material' | 'note' | 'raw' | 'secondary';
  title: string;
  text: string;
  fieldPath: string;
  sourceRole: 'teacher_said' | 'material_supplement' | 'derived' | 'system_explanation';
  anchorLabel: string;
  revisionId: string;
  equivalenceGroupId?: string;
  navKind: 'text' | 'page_region' | 'object_focus';
  timeLabel?: string;
  coverageLabel?: string;
}

export const searchEntries: SearchFixtureEntry[] = [
  {
    id: 'search-note-conv',
    kind: 'note',
    title: '课堂笔记 · 公式',
    text: '卷积运算把局部窗口与卷积核逐项相乘再求和。',
    fieldPath: 'objects[formula].latex',
    sourceRole: 'derived',
    anchorLabel: '笔记·公式对象',
    revisionId: ids.revision,
    equivalenceGroupId: 'equiv-conv-definition',
    navKind: 'object_focus',
  },
  {
    id: 'search-material-conv',
    kind: 'material',
    title: '课件 · 第 4 页',
    text: '卷积运算把局部窗口与卷积核逐项相乘再求和。',
    fieldPath: 'pages[3].regions[1].text',
    sourceRole: 'material_supplement',
    anchorLabel: '课件·第 4 页·区域 2',
    revisionId: '4c11a85f-f3da-4f4f-9701-000000000012',
    equivalenceGroupId: 'equiv-conv-definition',
    navKind: 'page_region',
    coverageLabel: '同源对象·一次计数',
  },
  {
    id: 'search-raw-window',
    kind: 'raw',
    title: '原始转写 · 00:05:12',
    text: '我们现在把这个窗口向右移动一个步幅。',
    fieldPath: 'utterances[12].text',
    sourceRole: 'teacher_said',
    anchorLabel: '原始句 13',
    revisionId: '4c11a85f-f3da-4f4f-9701-000000000013',
    navKind: 'text',
    timeLabel: '00:05:12.400–00:05:16.020',
  },
  {
    id: 'search-secondary-window',
    kind: 'secondary',
    title: '二次转写 · 00:05:12',
    text: '这个窗口向右移动一个步幅。',
    fieldPath: 'segments[8].text',
    sourceRole: 'derived',
    anchorLabel: '二次段 9',
    revisionId: '4c11a85f-f3da-4f4f-9701-000000000014',
    navKind: 'text',
    timeLabel: '原始时间 00:05:12.400–00:05:16.020',
  },
  {
    id: 'search-material-unicode',
    kind: 'material',
    title: '课件 · 术语表',
    text: 'Straße 学习 Café 😀',
    fieldPath: 'pages[6].regions[0].text',
    sourceRole: 'material_supplement',
    anchorLabel: '课件·第 7 页·术语表',
    revisionId: '4c11a85f-f3da-4f4f-9701-000000000015',
    navKind: 'page_region',
  },
  {
    id: 'search-note-table',
    kind: 'note',
    title: '课堂笔记 · 表格单元格',
    text: '步幅 stride 1',
    fieldPath: 'objects[table].cells[cell-stride].value',
    sourceRole: 'derived',
    anchorLabel: '笔记·表格·stride',
    revisionId: ids.revision,
    navKind: 'object_focus',
  },
  {
    id: 'search-material-unprocessed',
    kind: 'material',
    title: '课件 · 第 9 页（未 OCR）',
    text: '',
    fieldPath: 'pages[8].regions[*]',
    sourceRole: 'material_supplement',
    anchorLabel: '课件·第 9 页',
    revisionId: '4c11a85f-f3da-4f4f-9701-000000000016',
    navKind: 'page_region',
    coverageLabel: '未 OCR：搜索覆盖不完整',
  },
];

export interface CorrectionFixture {
  id: string;
  original: string;
  suggestion: string;
  reason: string;
  risk: 'ordinary' | 'critical';
  state: 'pending' | 'accepted' | 'rejected';
}

export const correctionFixtures: CorrectionFixture[] = [
  {id: 'suggestion-1', original: '这个核在每个位置移动。', suggestion: '这个卷积核在每个位置移动。', reason: '补回课堂术语，不改变数值', risk: 'ordinary', state: 'pending'},
  {id: 'suggestion-2', original: '步幅为 2。', suggestion: '步幅为 1。', reason: '数值变化，需要用户明确采纳', risk: 'critical', state: 'pending'},
];

export const initialRecording: {state: RecordingState; elapsedMs: number; watermarkMs: number; assignmentEpoch: number} = {
  state: 'stopped',
  elapsedMs: 0,
  watermarkMs: 312000,
  assignmentEpoch: 3,
};

export const initialLanguageVariant: LanguageVariant = {
  variant_id: '4c11a85f-f3da-4f4f-9701-000000000017',
  note_id: ids.note,
  tier: 3,
  fact_set_revision: ids.revision,
  fact_ids: ['fact-conv-1', 'fact-conv-2'],
  presentation_revision: ids.revision,
  sync_status: 'stale',
};

export const fixtureFocausResult: CatchupResult = {
  result_semantics: 'focaus_context_v2',
  result_id: '4c11a85f-f3da-4f4f-9701-000000000018',
  request_generation: 1,
  snapshot_id: '4c11a85f-f3da-4f4f-9701-000000000019',
  body_revision_id: ids.revision,
  requested_window: {recording_id: ids.recording, start_ms: 150000, end_ms: 450000, start_source: null},
  effective_window: {recording_id: ids.recording, start_ms: 168000, end_ms: 450000, start_source: null},
  estimate_basis: 'fixture：按用户触发时的可得进度估计，默认至少回看 5 分钟',
  current_focus: {
    topic: '二维卷积的滑动窗口',
    problem: '为什么输出格的数值可以由一个局部窗口得到？',
    activity: 'derivation',
    step: '逐项相乘并求和',
    source_refs: [],
    status: 'supported',
  },
  path_nodes: [
    {node_id: 'path-missed', role: 'missed_key', fact_ids: ['fact-conv-1'], definition_ids: ['definition-conv'], binding_ids: [], body_block_ids: ['body-1'], evidence_role: 'teacher_path', source_refs: []},
    {node_id: 'path-bridge', role: 'bridge', fact_ids: ['fact-conv-2'], definition_ids: [], binding_ids: [], body_block_ids: ['body-2'], evidence_role: 'teacher_path', source_refs: []},
    {node_id: 'path-current', role: 'current', fact_ids: ['fact-conv-3'], definition_ids: [], binding_ids: ['binding-stride'], body_block_ids: ['body-3'], evidence_role: 'system_explanation', source_refs: []},
  ],
  path_edges: [
    {from_node_id: 'path-missed', to_node_id: 'path-bridge', relation: 'dependency', source_refs: [], certainty: 'supported'},
    {from_node_id: 'path-bridge', to_node_id: 'path-current', relation: 'temporal', source_refs: [], certainty: 'supported'},
  ],
  prerequisites: [
    {definition_id: 'definition-dot-product', binding_id: null, fact_id: null, reason: '需要知道局部窗口与核的对应位置', source_refs: [], scope_id: ids.lesson, outside_window: true},
  ],
  prerequisites_reason: null,
  body: [
    {block_id: 'body-1', text: '老师先把输入看成由局部窗口组成；每个窗口与卷积核对齐后，输出格只依赖这块区域。'},
    {block_id: 'body-2', text: '因此，滑动一步会得到下一个局部计算，而不是重新读取整张输入。'},
    {block_id: 'body-3', text: '当前正在把这个过程写成逐项相乘再求和，步幅决定下一个窗口从哪里开始。'},
  ],
  presentation_tier: 3,
  body_evidence_cutoff: [{recording_id: ids.recording, end_ms: 300000, status: 'known'}],
  body_generated_at: '2026-10-18T08:10:12.000Z',
  progress_at_trigger: null,
  progress_unavailable_reason: null,
  progress_binding: {lesson_id: ids.lesson, recording_id: ids.recording, assignment_epoch: 3, no_pointer_reason: null},
  coverage: {audio_gaps: [], asr_gaps: [], material_supplement_used: true, range_status: 'complete'},
  limitations: ['fixture 结果用于 UI/接线审查；尚未执行真实模型、录音或 15 秒验收'],
  completion_status: 'complete',
  triggered_at: '2026-10-18T08:10:10.700Z',
  completed_at: '2026-10-18T08:10:12.000Z',
  displayed_complete_elapsed_ms: 1300,
};

import type {ContentView} from '@focaus/contracts';
import type {NativeProjection} from '../nativeContentProjection.ts';

export type NativePaneSlot = {
  id: string;
  title: string;
  description: string;
  state: ContentView['state'] | 'empty';
  view: ContentView | null;
  projection: NativeProjection | null;
  primary: boolean;
};

const labels: Record<string, string> = {
  note: '笔记',
  raw: '原始转写',
  secondary: '二次转写',
  material: '课件与资料',
  qa: '课堂问答',
  arrangements: '安排与提醒',
};

function contentKind(view: ContentView): string {
  return view.target.kind === 'lesson_collection' ? view.target.content_kind : 'entity';
}

export function projectionForView(view: ContentView, projections: Record<string, NativeProjection>): NativeProjection | null {
  if (view.target.kind === 'entity') return projections[view.target.entity_id] ?? null;
  if (view.target.kind !== 'lesson_collection') return null;
  const kind = view.target.content_kind;
  return projections[view.view_id] ?? Object.values(projections).find((projection) => String(projection.kind) === kind) ?? null;
}

export function buildNativePaneSlots(
  views: ContentView[],
  projections: Record<string, NativeProjection>,
  activeView: ContentView | null,
): NativePaneSlot[] {
  const openViews = views.filter((view) => view.state !== 'closed').slice(0, 6);
  if (!openViews.length) {
    return [
      {id: 'native-empty-primary', title: '当前课堂内容', description: '等待宿主返回真实 ContentView', state: 'empty', view: null, projection: null, primary: true},
      {id: 'native-empty-transcript', title: '转写与笔记', description: '原始、二次与笔记由真实 projection 提供', state: 'empty', view: null, projection: null, primary: false},
      {id: 'native-empty-material', title: '课件与资料', description: '资料导入后由宿主 content head 返回', state: 'empty', view: null, projection: null, primary: false},
    ];
  }
  const activeIndex = activeView ? openViews.findIndex((view) => view.view_id === activeView.view_id) : -1;
  const ordered = activeIndex > 0 ? [openViews[activeIndex], ...openViews.slice(0, activeIndex), ...openViews.slice(activeIndex + 1)] : openViews;
  return ordered.map((view, index) => {
    const kind = contentKind(view);
    const title = labels[kind] ?? (kind === 'entity' ? '内容对象' : kind);
    return {
      id: view.view_id,
      title,
      description: `${view.state} · revision ${view.view_revision}`,
      state: view.state,
      view,
      projection: projectionForView(view, projections),
      primary: index === 0,
    };
  });
}

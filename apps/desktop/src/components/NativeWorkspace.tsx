import {useEffect, useState} from 'react';
import {Button, Card, Description, Input, Label, TextField} from '@heroui/react';
import type {Course, HostAdapter, Lesson, RecordingSession} from '@focaus/contracts';
import {RecordingBar} from './RecordingBar.tsx';
import {StatusBadge} from './StatusBadge.tsx';

export function NativeWorkspace({
  course,
  lesson,
  recording,
  contentHeadCount,
  eventSeq,
  snapshotSeq,
  notice,
  screen,
  onCreateCourseLesson,
  onOpenWorkspace,
  onBack,
  onToggleRecording,
  onStopRecording,
  onStartUnassigned,
  onRetryEvents,
}: {
  course: Course | null;
  lesson: Lesson | null;
  recording: RecordingSession | null;
  contentHeadCount: number;
  eventSeq: number;
  snapshotSeq: number;
  notice: string;
  screen: 'home' | 'workspace';
  onCreateCourseLesson: (courseName: string, lessonTitle: string) => Promise<void>;
  onOpenWorkspace: () => void;
  onBack: () => void;
  onToggleRecording: () => void;
  onStopRecording: () => void;
  onStartUnassigned: () => void;
  onRetryEvents: () => void;
}) {
  const [courseName, setCourseName] = useState(course?.name ?? '');
  const [lessonTitle, setLessonTitle] = useState(lesson?.title ?? '');
  const [formError, setFormError] = useState('');
  const [creating, setCreating] = useState(false);
  const bindingLabel = course && lesson ? `${course.name} · ${lesson.title}` : course ? `${course.name} · 尚未创建课堂` : '未归属即时采集';

  useEffect(() => {
    if (course) setCourseName(course.name);
  }, [course?.course_id, course?.name]);

  useEffect(() => {
    if (lesson) setLessonTitle(lesson.title);
  }, [lesson?.lesson_id, lesson?.title]);

  const submit = async () => {
    if ((!course && !courseName.trim()) || !lessonTitle.trim()) {
      setFormError('课程名称和课堂标题都需要由用户明确填写。');
      return;
    }
    setFormError('');
    setCreating(true);
    try {
      await onCreateCourseLesson(courseName.trim() || course?.name || '', lessonTitle.trim());
    } finally {
      setCreating(false);
    }
  };

  return (
    <div className="native-shell">
      <header className="app-topbar">
        <div className="app-brand"><span className="app-brand-mark" aria-hidden="true">F</span><span><strong>focaus</strong><small>真实 native 宿主</small></span></div>
        <div className="app-topbar-status"><StatusBadge tone="success" dot>native · snapshot {snapshotSeq}</StatusBadge><span className="topbar-notice">事件通道 focaus://events · ACK {eventSeq}</span></div>
        <div className="app-topbar-actions"><Button size="sm" variant="ghost" onPress={onOpenWorkspace}>打开课程窗</Button></div>
      </header>
      {screen === 'home' ? (
        <main className="native-page page-shell">
          <section className="native-hero">
            <div>
              <p className="eyebrow">真实宿主快照</p>
              <h1>{course ? '继续配置当前课程' : '从第一堂课开始'}</h1>
              <p>当前页面只显示 Tauri 宿主返回的课程、课堂、录音与事件状态。空库不会被本地 fixture 内容填充。</p>
            </div>
            <StatusBadge tone="success">state.open → snapshot → state.ack</StatusBadge>
          </section>
          <Card className="native-create-card" variant="tertiary">
            <Card.Header><Card.Title>{course ? '为当前课程创建第一堂课' : '创建课程并创建课堂'}</Card.Title><Card.Description>将依次调用 course.create → lesson.create；ID、revision 和归属都以宿主响应为准。</Card.Description></Card.Header>
            <Card.Content>
              {course ? <div className="native-existing-record"><span>已存在课程</span><strong>{course.name}</strong><small>course_id {course.course_id}</small></div> : null}
              <div className="native-form-grid">
                {!course ? <TextField value={courseName} onChange={setCourseName} isRequired><Label>课程名称</Label><Input placeholder="例如：概率论" /><Description>写入前不会生成本地 ID。</Description></TextField> : null}
                <TextField value={lessonTitle} onChange={setLessonTitle} isRequired><Label>课堂标题</Label><Input placeholder="例如：第 4 讲 · 卷积" /><Description>课堂实际日期与时区由宿主后续返回。</Description></TextField>
              </div>
              {formError ? <div className="error-notice" role="alert">{formError}</div> : null}
            </Card.Content>
            <Card.Footer><Button onPress={() => void submit()} isPending={creating}>{creating ? '提交中…' : '创建并打开课程窗'}</Button><Button variant="secondary" onPress={onStartUnassigned}>先开始未归属录音</Button></Card.Footer>
          </Card>
          {notice ? <div className="inline-notice" role="status">{notice}{notice.includes('事件批次') ? <Button size="sm" variant="outline" onPress={onRetryEvents}>重试未确认事件</Button> : null}</div> : null}
        </main>
      ) : (
        <main className="native-page page-shell">
          <header className="native-workspace-header"><div><Button size="sm" variant="ghost" onPress={onBack}>← 返回</Button><p className="eyebrow">真实课程窗 · 宿主数据</p><h1>{bindingLabel}</h1></div><StatusBadge tone={lesson ? 'success' : 'warning'}>{lesson ? '已绑定课堂' : '未归属录音'}</StatusBadge></header>
          <RecordingBar state={recording?.state ?? 'stopped'} elapsedMs={recording?.durable_watermark_ms ?? 0} watermarkMs={recording?.durable_watermark_ms ?? 0} assignmentEpoch={recording?.assignment_epoch ?? 0} bindingLabel={bindingLabel} onToggle={onToggleRecording} onStop={onStopRecording} />
          {notice ? <div className="inline-notice" role="status">{notice}</div> : null}
          <div className="native-content-grid" aria-label="真实宿主内容状态">
            {['课堂笔记', '原始转写', '二次转写', '课件', 'focaus', '交互知识组件'].map((label) => <Card key={label} className="native-empty-card" variant="secondary"><Card.Header><Card.Title>{label}</Card.Title><Card.Description>宿主 content heads：{contentHeadCount} · 等待对应投影</Card.Description></Card.Header><Card.Content><div className="native-empty-icon" aria-hidden="true">·</div><p>当前没有可安全展示的真实内容。此 native 壳不会使用 fixture 正文、搜索命中或模型结果。</p></Card.Content></Card>)}
          </div>
          {!lesson ? <div className="native-empty-action"><Button onPress={onBack}>返回创建课堂</Button></div> : null}
        </main>
      )}
      <footer className="app-footer"><span>契约 2.0.0 · native adapter</span><span>recording watermark 只来自宿主 durable_watermark_ms</span></footer>
    </div>
  );
}


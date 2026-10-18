import {Button, Card} from '@heroui/react';
import type {RecordingState} from '@focaus/contracts';
import {StatusBadge} from './StatusBadge.tsx';

export function StartHome({
  recordingState,
  onStartRecording,
  onOpenWorkspace,
}: {
  recordingState: RecordingState;
  onStartRecording: () => void;
  onOpenWorkspace: () => void;
}) {
  return (
    <main className="start-page page-shell">
      <section className="start-hero" aria-labelledby="start-title">
        <div className="start-hero-copy">
          <StatusBadge tone="accent">Focaus · 本地工作区</StatusBadge>
          <p className="eyebrow">学习过程的可追溯工作台</p>
          <h1 id="start-title">从一堂课开始，留下可回到原处的理解。</h1>
          <p className="hero-description">
            录音、课件、原始转写、二次转写与笔记共享同一堂课绑定。每次来源跳转都会保留版本、时间与用户修改边界。
          </p>
          <div className="start-actions">
            <Button size="lg" onPress={onStartRecording}>
              <span aria-hidden="true" className="button-glyph button-glyph-record" />
              开始录音
            </Button>
            <Button size="lg" variant="secondary" onPress={onOpenWorkspace}>
              打开课程窗
            </Button>
          </div>
          <p className="start-note">
            当前为 <strong>fixture 演示模式</strong>。开始录音只更新可审查的本地 UI 状态，尚未连接真实麦克风。
          </p>
        </div>
        <div className="start-hero-visual" aria-label="课程上下文预览" role="img">
          <div className="hero-visual-ring hero-visual-ring-one" />
          <div className="hero-visual-ring hero-visual-ring-two" />
          <div className="hero-visual-core">
            <span className="hero-visual-kicker">当前课堂</span>
            <strong>概率论 · 第 4 讲</strong>
            <span>卷积与局部窗口</span>
          </div>
          <div className="hero-visual-line hero-visual-line-top" />
          <div className="hero-visual-line hero-visual-line-bottom" />
        </div>
      </section>

      <section className="start-section" aria-labelledby="recent-title">
        <div className="section-heading-row">
          <div>
            <p className="eyebrow">最近课堂</p>
            <h2 id="recent-title">继续上一次留下的上下文</h2>
          </div>
          <StatusBadge tone={recordingState === 'recording' ? 'danger' : 'neutral'} dot={recordingState === 'recording'}>
            {recordingState === 'recording' ? '正在录音' : '没有进行中的录音'}
          </StatusBadge>
        </div>
        <div className="recent-course-grid">
          <Card className="recent-course-card" variant="tertiary">
            <Card.Header>
              <div className="course-card-icon" aria-hidden="true">P</div>
              <Card.Title>概率论</Card.Title>
              <Card.Description>第 4 讲 · 2026 年 10 月 18 日</Card.Description>
            </Card.Header>
            <Card.Content>
              <div className="course-progress" aria-label="课堂进度 62%">
                <span style={{width: '62%'}} />
              </div>
              <p>已保留笔记、课件和原始转写的当前版本。</p>
            </Card.Content>
            <Card.Footer>
              <Button size="sm" variant="outline" onPress={onOpenWorkspace}>继续课程窗</Button>
              <span className="card-meta">课堂已绑定</span>
            </Card.Footer>
          </Card>
          <Card className="recent-course-card recent-course-card-muted" variant="secondary">
            <Card.Header>
              <div className="course-card-icon course-card-icon-muted" aria-hidden="true">+</div>
              <Card.Title>新建课堂</Card.Title>
              <Card.Description>为下一次课程准备独立上下文</Card.Description>
            </Card.Header>
            <Card.Content>
              <p>可以先进入工作区，稍后再决定是否绑定课程。</p>
            </Card.Content>
            <Card.Footer>
              <Button size="sm" variant="ghost" onPress={onOpenWorkspace}>进入空白工作区</Button>
            </Card.Footer>
          </Card>
        </div>
      </section>
    </main>
  );
}

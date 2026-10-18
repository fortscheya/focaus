import {Button} from '@heroui/react';
import type {RecordingState} from '@focaus/contracts';
import {StatusBadge} from './StatusBadge.tsx';

function formatDuration(milliseconds: number): string {
  const totalSeconds = Math.max(0, Math.floor(milliseconds / 1000));
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;
  return [hours, minutes, seconds].map((value) => value.toString().padStart(2, '0')).join(':');
}

export function RecordingBar({
  state,
  elapsedMs,
  watermarkMs,
  assignmentEpoch,
  onToggle,
  onStop,
}: {
  state: RecordingState;
  elapsedMs: number;
  watermarkMs: number;
  assignmentEpoch: number;
  onToggle: () => void;
  onStop: () => void;
}) {
  const isRecording = state === 'recording';
  const isPaused = state === 'paused';
  const label = isRecording ? '正在录音' : isPaused ? '已暂停' : state === 'interrupted' ? '录音中断' : '录音已停止';

  return (
    <div className="recording-bar" role="status" aria-live="polite">
      <div className="recording-bar-leading">
        <StatusBadge tone={isRecording ? 'danger' : isPaused ? 'warning' : 'neutral'} dot={isRecording}>
          {label}
        </StatusBadge>
        <span className="recording-duration" aria-label={`录音时长 ${formatDuration(elapsedMs)}`}>
          {formatDuration(elapsedMs)}
        </span>
        <span className="recording-binding">
          <span className="recording-binding-label">课堂绑定</span>
          <strong>概率论 · 第 4 讲</strong>
          <span className="recording-epoch">epoch {assignmentEpoch}</span>
        </span>
      </div>
      <div className="recording-bar-trailing">
        <span className="watermark-label">已稳固 {Math.round(watermarkMs / 1000)}s</span>
        <Button size="sm" variant={isRecording ? 'secondary' : 'primary'} onPress={onToggle}>
          <span aria-hidden="true" className={`button-glyph ${isRecording ? 'button-glyph-pause' : 'button-glyph-play'}`} />
          {isRecording ? '暂停' : isPaused ? '继续' : '开始'}
        </Button>
        <Button size="sm" variant="tertiary" isDisabled={state === 'stopped'} onPress={onStop}>停止</Button>
      </div>
    </div>
  );
}

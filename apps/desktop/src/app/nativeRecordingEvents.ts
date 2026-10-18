export type RecordingEventKind = 'coverage' | 'watermark' | 'session' | 'unknown';

export function classifyRecordingEvent(eventType: string, entityId: string | null, recordingId: string | null): RecordingEventKind {
  if (eventType === 'recording.coverage_changed' || eventType.includes('.coverage_')) return 'coverage';
  if (eventType === 'recording.watermark_changed' || eventType.includes('.watermark')) return 'watermark';
  if (
    eventType === 'recording.state_changed' ||
    eventType === 'recording.changed' ||
    eventType === 'recording.started' ||
    eventType === 'recording.paused' ||
    eventType === 'recording.resumed' ||
    eventType === 'recording.stopped' ||
    eventType === 'recording.interrupted' ||
    eventType === 'recording.failed' ||
    (eventType.startsWith('recording.') && entityId !== null && entityId === recordingId)
  ) return 'session';
  return 'unknown';
}


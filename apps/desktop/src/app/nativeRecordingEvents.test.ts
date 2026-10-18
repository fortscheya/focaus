import {describe, expect, it} from 'vitest';
import {classifyRecordingEvent} from './nativeRecordingEvents.ts';

describe('native recording event routing', () => {
  it('keeps coverage entities out of RecordingSession parsing', () => {
    const recordingId = '4c11a85f-f3da-4f4f-9701-000000000003';
    expect(classifyRecordingEvent('recording.coverage_changed', 'coverage-entity', recordingId)).toBe('coverage');
    expect(classifyRecordingEvent('recording.state_changed', recordingId, recordingId)).toBe('session');
    expect(classifyRecordingEvent('recording.watermark_changed', 'watermark-entity', recordingId)).toBe('watermark');
    expect(classifyRecordingEvent('recording.coverage_changed', recordingId, recordingId)).toBe('coverage');
    expect(classifyRecordingEvent('recording.unrecognized', 'other-entity', recordingId)).toBe('unknown');
  });

  it('routes a real capture update sequence without collapsing entity kinds', () => {
    const recordingId = '4c11a85f-f3da-4f4f-9701-000000000003';
    const sequence = [
      ['recording.coverage_changed', 'audio-span-1'],
      ['recording.state_changed', recordingId],
      ['recording.watermark_changed', 'watermark-1'],
    ] as const;
    expect(sequence.map(([eventType, entityId]) => classifyRecordingEvent(eventType, entityId, recordingId))).toEqual(['coverage', 'session', 'watermark']);
  });
});


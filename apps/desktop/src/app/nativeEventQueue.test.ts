import {describe, expect, it} from 'vitest';
import {SerialNativeEventQueue} from './nativeEventQueue.ts';

describe('SerialNativeEventQueue', () => {
  it('preserves batch order and does not acknowledge a failed batch', async () => {
    const queue = new SerialNativeEventQueue();
    const processed: string[] = [];
    const stateAckCalls: string[] = [];

    const first = queue.enqueue({seq: 1}, async (batch) => {
      processed.push(`start-${batch.seq}`);
      await Promise.resolve();
      processed.push(`finish-${batch.seq}`);
      stateAckCalls.push(String(batch.seq));
    });
    const second = queue.enqueue({seq: 2}, async (batch) => {
      processed.push(`start-${batch.seq}`);
      processed.push(`finish-${batch.seq}`);
      stateAckCalls.push(String(batch.seq));
    });
    await Promise.all([first, second]);

    expect(processed).toEqual(['start-1', 'finish-1', 'start-2', 'finish-2']);
    expect(stateAckCalls).toEqual(['1', '2']);

    const failed = queue.enqueue({seq: 3}, async () => {
      throw new Error('content.get failed');
    });
    await expect(failed).rejects.toThrow('content.get failed');
    expect(stateAckCalls).toEqual(['1', '2']);
    expect(queue.blocked).toBe(true);

    const skipped = queue.enqueue({seq: 4}, async (batch) => {
      stateAckCalls.push(String(batch.seq));
    });
    await expect(skipped).rejects.toThrow('blocked');
    expect(stateAckCalls).toEqual(['1', '2']);

    queue.reset();
    await queue.enqueue({seq: 3}, async (batch) => {
      stateAckCalls.push(String(batch.seq));
    });
    expect(stateAckCalls).toEqual(['1', '2', '3']);
  });
});


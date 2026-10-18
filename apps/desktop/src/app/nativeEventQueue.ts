export interface NativeEventBatchQueue {
  readonly blocked: boolean;
  enqueue<T>(batch: T, process: (batch: T) => Promise<void>): Promise<void>;
  reset(): void;
}

/**
 * Keeps event batches ordered and stops the acknowledgement cursor when a
 * projection read fails. The host can retry the failed batch after a resync.
 */
export class SerialNativeEventQueue implements NativeEventBatchQueue {
  private tail: Promise<void> = Promise.resolve();
  private isBlocked = false;

  get blocked(): boolean {
    return this.isBlocked;
  }

  enqueue<T>(batch: T, process: (batch: T) => Promise<void>): Promise<void> {
    const run = async () => {
      if (this.isBlocked) throw new Error('native event queue is blocked until the failed batch is retried');
      try {
        await process(batch);
      } catch (error) {
        this.isBlocked = true;
        throw error;
      }
    };
    const scheduled = this.tail.then(run, run);
    this.tail = scheduled;
    void scheduled.catch(() => undefined);
    return scheduled;
  }

  reset(): void {
    this.isBlocked = false;
    this.tail = Promise.resolve();
  }
}


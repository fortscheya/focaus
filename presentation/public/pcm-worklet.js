/** Streaming box-filter resampler. Fractional windows survive render-quantum boundaries. */
class Pcm16Encoder {


  weighted = 0;
  samples = [];

  constructor(inputRate, outputRate = 16_000) {
    if (!Number.isFinite(inputRate) || inputRate <= 0 || !Number.isFinite(outputRate) || outputRate <= 0) {
      throw new Error('Invalid audio sample rate');
    }
    this.ratio = inputRate / outputRate;
    this.remaining = this.ratio;
  }

  push(input) {
    for (const raw of input) {
      const sample = Number.isFinite(raw) ? Math.max(-1, Math.min(1, raw)) : 0;
      let available = 1;
      while (available > 1e-8) {
        const weight = Math.min(available, this.remaining);
        this.weighted += sample * weight;
        this.remaining -= weight;
        available -= weight;
        if (this.remaining < 1e-8) {
          const value = Math.max(-1, Math.min(1, this.weighted / this.ratio));
          this.samples.push(Math.round(value * (value < 0 ? 32_768 : 32_767)));
          this.weighted = 0;
          this.remaining = this.ratio;
        }
      }
    }
  }

  take(minimumSamples = 1) {
    if (this.samples.length < minimumSamples) return null;
    const buffer = new ArrayBuffer(this.samples.length * 2);
    const view = new DataView(buffer);
    this.samples.forEach((sample, index) => view.setInt16(index * 2, sample, true));
    this.samples = [];
    return buffer;
  }
}

class FocausPresentationPcm extends AudioWorkletProcessor {
  constructor() {
    super();
    this.encoder = new Pcm16Encoder(sampleRate);
    this.active = true;
    this.frames = 0;
    this.port.onmessage = event => {
      if (event.data !== 'flush') return;
      this.active = false;
      const pcm = this.encoder.take();
      if (pcm) this.port.postMessage({type: 'pcm', pcm}, [pcm]);
      this.port.postMessage({type: 'flushed'});
    };
  }
  process(inputs) {
    const channel = inputs[0]?.[0];
    if (!this.active || !channel) return true;
    this.encoder.push(channel);
    const pcm = this.encoder.take(1600);
    if (pcm) this.port.postMessage({type: 'pcm', pcm}, [pcm]);
    if (++this.frames % 12 === 0) {
      const rms = Math.sqrt(channel.reduce((s,v) => s + v*v, 0) / channel.length);
      this.port.postMessage({type: 'level', value: Math.min(1, rms * 5)});
    }
    return true;
  }
}
registerProcessor('focaus-presentation-pcm', FocausPresentationPcm);

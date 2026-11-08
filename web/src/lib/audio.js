// WebAudio: decode a song, play it with a metronome rendered on the grid and
// kit sounds on every target's hit time. Scheduling runs 150 ms ahead on a
// 50 ms timer, the standard look-ahead scheduler, so clicks are sample-timed.

export class Player {
  constructor() {
    this.ctx = null;
    this.buffer = null;
    this.source = null;
    this.startCtxTime = 0;
    this.startSongTime = 0;
    this.playing = false;
    this.grid = { bpm: 128, offset: 0, enabled: true };
    this.hits = []; // [{time (song s), kind}] sorted
    this.hitsEnabled = true;
    this._timer = null;
    this._nextBeat = 0;
    this._nextHit = 0;
    this.gain = 1.0;
    this.duration = 0;
    this.onEnd = null;
  }

  ensure() {
    if (!this.ctx) this.ctx = new (window.AudioContext || window.webkitAudioContext)();
    if (this.ctx.state === 'suspended') this.ctx.resume();
    return this.ctx;
  }

  async decode(file) {
    const ctx = this.ensure();
    const bytes = await file.arrayBuffer();
    this.buffer = await ctx.decodeAudioData(bytes);
    this.duration = this.buffer.duration;
    return this.buffer;
  }

  /** Mono mix of the decoded song for the analyser. */
  mono() {
    if (!this.buffer) return null;
    const n = this.buffer.length;
    const out = new Float32Array(n);
    const ch = this.buffer.numberOfChannels;
    for (let c = 0; c < ch; c++) {
      const d = this.buffer.getChannelData(c);
      for (let i = 0; i < n; i++) out[i] += d[i] / ch;
    }
    return out;
  }

  /** Song time now (seconds into the file), or the paused position. */
  position() {
    if (!this.playing) return this.startSongTime;
    return this.startSongTime + (this.ctx.currentTime - this.startCtxTime);
  }

  play(fromSongTime = null) {
    const ctx = this.ensure();
    this.stop(false);
    const from = fromSongTime ?? this.startSongTime;
    this.startSongTime = Math.max(0, from);
    this.startCtxTime = ctx.currentTime + 0.05;
    if (this.buffer) {
      this.source = ctx.createBufferSource();
      this.source.buffer = this.buffer;
      const g = ctx.createGain();
      g.gain.value = this.gain;
      this.source.connect(g).connect(ctx.destination);
      this.source.start(this.startCtxTime, this.startSongTime);
      this.source.onended = () => {
        if (this.playing && this.position() >= this.duration - 0.05) {
          this.stop();
          this.onEnd?.();
        }
      };
    }
    this.playing = true;
    const spb = 60 / this.grid.bpm;
    this._nextBeat = Math.max(0, Math.ceil((this.startSongTime - this.grid.offset) / spb - 1e-6));
    this._nextHit = this.hits.findIndex((h) => h.time >= this.startSongTime - 1e-6);
    if (this._nextHit < 0) this._nextHit = this.hits.length;
    this._timer = setInterval(() => this._schedule(), 50);
    this._schedule();
  }

  pause() {
    const pos = this.position();
    this.stop(false);
    this.startSongTime = pos;
  }

  stop(rewind = true) {
    if (this._timer) clearInterval(this._timer);
    this._timer = null;
    if (this.source) {
      try {
        this.source.onended = null;
        this.source.stop();
      } catch {
        /* already stopped */
      }
      this.source = null;
    }
    this.playing = false;
    if (rewind) this.startSongTime = 0;
  }

  seek(songTime) {
    const was = this.playing;
    this.stop(false);
    this.startSongTime = Math.max(0, songTime);
    if (was) this.play();
  }

  _schedule() {
    if (!this.playing) return;
    const ctx = this.ctx;
    const horizon = this.position() + 0.15;
    const spb = 60 / this.grid.bpm;
    while (this.grid.enabled) {
      const t = this.grid.offset + this._nextBeat * spb;
      if (t > horizon) break;
      if (t >= this.startSongTime - 1e-6) this.click(this.songToCtx(t), this._nextBeat % 4 === 0);
      this._nextBeat++;
    }
    while (this.hitsEnabled && this._nextHit < this.hits.length) {
      const h = this.hits[this._nextHit];
      if (h.time > horizon) break;
      if (h.time >= this.startSongTime - 1e-6) this.kit(this.songToCtx(h.time), h.kind);
      this._nextHit++;
    }
    if (!this.buffer && this.duration > 0 && this.position() >= this.duration) {
      this.stop();
      this.onEnd?.();
    }
    void ctx;
  }

  songToCtx(songTime) {
    return this.startCtxTime + (songTime - this.startSongTime);
  }

  click(at, downbeat) {
    const ctx = this.ctx;
    const o = ctx.createOscillator();
    const g = ctx.createGain();
    o.frequency.value = downbeat ? 1320 : 880;
    g.gain.setValueAtTime(downbeat ? 0.5 : 0.3, at);
    g.gain.exponentialRampToValueAtTime(0.001, at + (downbeat ? 0.055 : 0.04));
    o.connect(g).connect(ctx.destination);
    o.start(at);
    o.stop(at + 0.08);
  }

  /** The kit: FLICK kick, SWITCH snare, MICRO hat, TRACK cymbal, RUSH tom. */
  kit(at, kind) {
    const ctx = this.ctx;
    const g = ctx.createGain();
    g.connect(ctx.destination);
    if (kind === 'flick' || kind === 'rush') {
      const o = ctx.createOscillator();
      const f0 = kind === 'flick' ? 150 : 220;
      o.frequency.setValueAtTime(f0, at);
      o.frequency.exponentialRampToValueAtTime(kind === 'flick' ? 45 : 90, at + 0.12);
      g.gain.setValueAtTime(0.7, at);
      g.gain.exponentialRampToValueAtTime(0.001, at + 0.18);
      o.connect(g);
      o.start(at);
      o.stop(at + 0.2);
      return;
    }
    const len = kind === 'micro' ? 0.03 : kind === 'switch' ? 0.12 : 0.3;
    const n = Math.floor(ctx.sampleRate * len);
    const buf = ctx.createBuffer(1, n, ctx.sampleRate);
    const d = buf.getChannelData(0);
    for (let i = 0; i < n; i++) d[i] = (Math.random() * 2 - 1) * (1 - i / n) ** 2;
    const s = ctx.createBufferSource();
    s.buffer = buf;
    const f = ctx.createBiquadFilter();
    f.type = kind === 'micro' || kind === 'track' ? 'highpass' : 'bandpass';
    f.frequency.value = kind === 'micro' ? 7000 : kind === 'track' ? 4000 : 1800;
    g.gain.value = kind === 'micro' ? 0.25 : kind === 'track' ? 0.3 : 0.5;
    s.connect(f).connect(g);
    s.start(at);
  }
}

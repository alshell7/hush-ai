// Gapless A/B playback with Web Audio. Every version of a scene plays at once from one start
// time, each through its own gain; switching only ramps the gains, so versions never drift
// apart and the switch is instant. Files are decoded before the first click with an
// OfflineAudioContext, so nothing asks for audio permission early; sound starts only on a click.

export type Version = "noisy" | "deepfilter" | "reuse";

type Loaded = { version: Version; buffer: AudioBuffer };

const FADE = 0.045;

type AudioWindow = Window & { webkitAudioContext?: typeof AudioContext };
type SessionNavigator = Navigator & { audioSession?: { type: string } };

export class AbPlayer {
  private ctx: AudioContext | null = null;
  private cache = new Map<string, Promise<ArrayBuffer>>();
  private decoded = new Map<string, Promise<AudioBuffer>>();
  private nodes: { source: AudioBufferSourceNode; gain: GainNode; version: Version }[] = [];
  private startedAt = 0;
  private offset = 0;
  private current: Version = "noisy";
  private loaded: Loaded[] = [];
  private endTimer = 0;
  duration = 0;
  playing = false;
  onEnded: () => void = () => undefined;

  /** Downloads a file once; safe to call early. */
  fetch(url: string): Promise<ArrayBuffer> {
    let p = this.cache.get(url);
    if (!p) {
      p = fetch(url).then((r) => (r.ok ? r.arrayBuffer() : Promise.reject(new Error(`${r.status} ${url}`))));
      p.catch(() => this.cache.delete(url));
      this.cache.set(url, p);
    }
    return p;
  }

  /** Decodes a file; before any click this uses an offline context, which never plays. */
  decode(url: string): Promise<AudioBuffer> {
    let p = this.decoded.get(url);
    if (!p) {
      p = this.fetch(url).then((data) => {
        const ctx = this.ctx ?? new OfflineAudioContext(1, 1, 48000);
        // decodeAudioData detaches the buffer; keep the download reusable.
        return ctx.decodeAudioData(data.slice(0));
      });
      p.catch(() => this.decoded.delete(url));
      this.decoded.set(url, p);
    }
    return p;
  }

  /** Loads a scene's versions. Resolves when all of them can play. */
  async load(files: Partial<Record<Version, string>>): Promise<void> {
    const entries = Object.entries(files) as [Version, string][];
    const buffers = await Promise.all(entries.map(([, url]) => this.decode(url)));
    this.stopNodes();
    this.playing = false;
    this.offset = 0;
    this.loaded = entries.map(([version], i) => ({ version, buffer: buffers[i] }));
    this.duration = Math.min(...buffers.map((b) => b.duration));
  }

  get position(): number {
    if (!this.playing || !this.ctx) return this.offset;
    return Math.min(this.duration, this.offset + (this.ctx.currentTime - this.startedAt));
  }

  /** Creates the audio context. Call it synchronously inside the click, before any await. */
  unlock(): void {
    if (!this.ctx) {
      const Ctx = window.AudioContext ?? (window as AudioWindow).webkitAudioContext;
      // Safari: play through the ringer switch, like a video would.
      const session = (navigator as SessionNavigator).audioSession;
      if (session) session.type = "playback";
      this.ctx = new Ctx({ latencyHint: "interactive" });
    }
    if (this.ctx.state !== "running") this.ctx.resume().catch(() => undefined);
  }

  /** Starts playback. Call unlock() first, from a click. */
  async play(): Promise<void> {
    if (!this.loaded.length) return;
    this.unlock();
    const ctx = this.ctx!;
    if (ctx.state !== "running") await ctx.resume();
    if (this.offset >= this.duration - 0.05) this.offset = 0;
    this.stopNodes();
    const at = ctx.currentTime + 0.03;
    for (const { version, buffer } of this.loaded) {
      const source = ctx.createBufferSource();
      source.buffer = buffer;
      const gain = ctx.createGain();
      gain.gain.value = version === this.current ? 1 : 0;
      source.connect(gain).connect(ctx.destination);
      source.start(at, this.offset);
      this.nodes.push({ source, gain, version });
    }
    this.startedAt = at;
    this.playing = true;
    this.scheduleEnd();
  }

  pause(): void {
    if (!this.playing) return;
    this.offset = this.position;
    this.playing = false;
    this.stopNodes();
  }

  seek(t: number): void {
    const next = Math.max(0, Math.min(this.duration, t));
    if (this.playing) {
      this.offset = next;
      this.play();
    } else {
      this.offset = next;
    }
  }

  /** Crossfades to another version without touching playback. */
  select(version: Version): void {
    this.current = version;
    if (!this.ctx) return;
    const now = this.ctx.currentTime;
    for (const n of this.nodes) {
      const g = n.gain.gain;
      g.cancelScheduledValues(now);
      g.setValueAtTime(g.value, now);
      g.linearRampToValueAtTime(n.version === version ? 1 : 0, now + FADE);
    }
  }

  dispose(): void {
    this.stopNodes();
    this.ctx?.close().catch(() => undefined);
    this.ctx = null;
  }

  private scheduleEnd() {
    clearTimeout(this.endTimer);
    const left = this.duration - this.position;
    this.endTimer = window.setTimeout(() => {
      if (!this.playing) return;
      if (this.position < this.duration - 0.02) return this.scheduleEnd();
      this.playing = false;
      this.offset = 0;
      this.stopNodes();
      this.onEnded();
    }, Math.max(20, left * 1000 + 40));
  }

  private stopNodes() {
    clearTimeout(this.endTimer);
    for (const n of this.nodes) {
      try {
        n.source.stop();
      } catch {
        /* not started */
      }
      n.source.disconnect();
      n.gain.disconnect();
    }
    this.nodes = [];
  }
}

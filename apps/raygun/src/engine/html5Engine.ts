import type { EngineCommand, EngineEvent, EngineReport } from '@raygun/bindings';

/** The parts of HTMLMediaElement the engine uses, so tests can supply a fake. */
export type MediaElementLike = Pick<
  HTMLMediaElement,
  | 'src'
  | 'currentTime'
  | 'volume'
  | 'playbackRate'
  | 'defaultPlaybackRate'
  | 'duration'
  | 'error'
  | 'play'
  | 'pause'
  | 'load'
  | 'removeAttribute'
  | 'addEventListener'
  | 'removeEventListener'
>;

const PROGRESS_INTERVAL_MS = 250;

const MEDIA_ERROR_MESSAGES: Record<number, string> = {
  1: 'Playback was aborted',
  2: 'A network error interrupted playback',
  3: 'The media could not be decoded',
  4: 'The media format is not supported or the source is unreachable',
};

/**
 * Plays streams in a webview media element on behalf of the Rust playback service
 * (ARCHITECTURE §6.2). It executes engine commands and reports what the element does, tagging
 * every report with the current load id so the service can drop reports about replaced tracks.
 */
export class Html5Engine {
  private loadId: number | null = null;
  private pendingStartMs = 0;
  private lastProgressAt = Number.NEGATIVE_INFINITY;
  private readonly listeners: [string, EventListener][];

  constructor(
    private readonly media: MediaElementLike,
    private readonly report: (report: EngineReport) => void,
    private readonly now: () => number = () => Date.now(),
  ) {
    this.listeners = [
      ['loadedmetadata', () => this.onLoadedMetadata()],
      ['durationchange', () => this.sendProgress(true)],
      ['timeupdate', () => this.sendProgress(false)],
      ['seeked', () => this.sendProgress(true)],
      ['playing', () => this.emit({ type: 'playing' })],
      ['pause', () => this.emit({ type: 'paused' })],
      ['waiting', () => this.emit({ type: 'buffering' })],
      ['ended', () => this.emit({ type: 'ended' })],
      ['error', () => this.onError()],
    ];
    for (const [type, listener] of this.listeners) {
      media.addEventListener(type, listener);
    }
  }

  handle(command: EngineCommand) {
    switch (command.type) {
      case 'load':
        this.loadId = command.loadId;
        this.pendingStartMs = command.startMs ?? 0;
        this.lastProgressAt = Number.NEGATIVE_INFINITY;
        this.media.volume = command.volume ?? 1;
        this.media.src = command.url;
        this.emit({ type: 'buffering' });
        if (command.autoplay) {
          this.play();
        }
        break;
      case 'play':
        this.play();
        break;
      case 'pause':
        this.media.pause();
        break;
      case 'seek':
        this.media.currentTime = (command.positionMs ?? 0) / 1000;
        break;
      case 'setVolume':
        this.media.volume = command.volume ?? 1;
        break;
      case 'setRate':
        // The default survives new sources, which reset the rate to it (M46.8).
        this.media.defaultPlaybackRate = command.rate ?? 1;
        this.media.playbackRate = command.rate ?? 1;
        break;
      case 'stop':
        this.loadId = null;
        this.media.pause();
        this.media.removeAttribute('src');
        this.media.load();
        break;
    }
  }

  dispose() {
    for (const [type, listener] of this.listeners) {
      this.media.removeEventListener(type, listener);
    }
  }

  private play() {
    const loadId = this.loadId;
    this.media.play().catch((error: unknown) => {
      // A newer load or pause interrupting play() is normal, not a failure.
      if (error instanceof DOMException && error.name === 'AbortError') {
        return;
      }
      if (loadId === this.loadId) {
        this.emit({ type: 'error', message: String(error) });
      }
    });
  }

  private onLoadedMetadata() {
    if (this.pendingStartMs > 0) {
      this.media.currentTime = this.pendingStartMs / 1000;
      this.pendingStartMs = 0;
    }
    this.sendProgress(true);
  }

  private onError() {
    const code = this.media.error?.code ?? 0;
    this.emit({ type: 'error', message: MEDIA_ERROR_MESSAGES[code] ?? 'Playback failed' });
  }

  private sendProgress(force: boolean) {
    const now = this.now();
    if (!force && now - this.lastProgressAt < PROGRESS_INTERVAL_MS) {
      return;
    }
    this.lastProgressAt = now;
    const duration = this.media.duration;
    this.emit({
      type: 'progress',
      positionMs: this.media.currentTime * 1000,
      durationMs: Number.isFinite(duration) ? duration * 1000 : null,
    });
  }

  private emit(event: EngineEvent) {
    if (this.loadId !== null) {
      this.report({ loadId: this.loadId, event });
    }
  }
}

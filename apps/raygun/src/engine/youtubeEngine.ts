import type { EngineCommand, EngineEvent, EngineReport } from '@raygun/bindings';

/** The iframe the engine drives; a structural type so tests can supply a fake. */
export type EmbedFrame = {
  src: string;
  readonly contentWindow: Pick<Window, 'postMessage'> | null;
};

export type EmbedMessage = {
  source: unknown;
  origin: string;
  data: unknown;
};

/**
 * `token` names the load a reply is about (the load id the page was given), so replies the page
 * sent about the previous video before it saw the new load are dropped (M46.7).
 */
type EmbedReply =
  | { raygunEmbed: true; type: 'ready' }
  | { raygunEmbed: true; type: 'state'; state: string; token?: number | null }
  | {
      raygunEmbed: true;
      type: 'progress';
      positionMs: number;
      durationMs: number | null;
      token?: number | null;
    }
  | { raygunEmbed: true; type: 'error'; code: number; token?: number | null };

type EmbedCommand =
  | {
      type: 'load';
      videoId: string;
      startMs: number;
      autoplay: boolean;
      volume: number;
      token: number;
    }
  | { type: 'play' }
  | { type: 'pause' }
  | { type: 'seek'; positionMs: number }
  | { type: 'volume'; volume: number }
  | { type: 'rate'; rate: number }
  | { type: 'stop' };

/** IFrame Player API error codes, phrased for people. */
export const YOUTUBE_ERRORS: Record<number, string> = {
  2: 'The YouTube video ID is invalid',
  5: "The video can't be played in this player",
  100: 'The video was removed or is private',
  101: "The video's owner doesn't allow it to be played in other apps",
  150: "The video's owner doesn't allow it to be played in other apps",
  152: 'YouTube refused the embedded player',
  153: 'YouTube refused the embedded player',
};

/** How long the embed may sit unstarted after being asked to play, ms. */
const UNSTARTED_GRACE_MS = 3000;

const STATE_EVENTS: Partial<Record<string, EngineEvent>> = {
  playing: { type: 'playing' },
  paused: { type: 'paused' },
  buffering: { type: 'buffering' },
  ended: { type: 'ended' },
};

const isReply = (data: unknown): data is EmbedReply =>
  typeof data === 'object' &&
  data !== null &&
  (data as { raygunEmbed?: unknown }).raygunEmbed === true;

/**
 * Plays YouTube videos through YouTube's embedded player (M08). The player lives in an iframe
 * pointing at Raygun's loopback embed page; commands and reports travel by `postMessage`, and only
 * messages from that iframe's window and origin are trusted.
 */
export class YouTubeEmbedEngine {
  private loadId: number | null = null;
  private pageReady = false;
  private queued: EmbedCommand[] = [];
  private origin: string | null = null;
  private readonly unlisten: () => void;
  /** The listener's volume, the song's gain (only down: the embed can't go louder) and a fade. */
  private master = 1;
  private gain = 1;
  private fade = 1;
  private fadeTimer: ReturnType<typeof setInterval> | null = null;
  /** The song's end trim, ms (M46.12), and whether it was reached. */
  private endMs: number | null = null;
  private finished = false;
  /** A load asked to play that hasn't started; the embed sitting unstarted means it was held. */
  private awaitingStart = false;
  private startTimer: ReturnType<typeof setTimeout> | null = null;

  constructor(
    private readonly frame: EmbedFrame,
    listen: (handler: (message: EmbedMessage) => void) => () => void,
    private readonly report: (report: EngineReport) => void,
  ) {
    this.unlisten = listen((message) => this.onMessage(message));
  }

  handle(command: EngineCommand) {
    switch (command.type) {
      case 'loadYouTube': {
        this.loadId = command.loadId;
        this.clearStart();
        this.awaitingStart = command.autoplay;
        this.master = command.volume ?? 1;
        this.gain = Math.min(1, 10 ** ((command.gainDb ?? 0) / 20));
        this.endMs = command.endMs ?? null;
        this.finished = false;
        const fading = (command.fadeMs ?? 0) > 0;
        this.stopFade();
        this.fade = fading ? 0 : 1;
        this.ensurePage(command.embedUrl);
        this.emit({ type: 'buffering' });
        this.send({
          type: 'load',
          videoId: command.videoId,
          startMs: command.startMs ?? 0,
          autoplay: command.autoplay,
          volume: this.volume(),
          token: command.loadId,
        });
        if (fading) this.fadeTo(1, command.fadeMs ?? 0);
        break;
      }
      case 'play':
        this.awaitingStart = true;
        this.send({ type: 'play' });
        break;
      case 'pause':
        this.clearStart();
        this.send({ type: 'pause' });
        break;
      case 'seek':
        this.send({ type: 'seek', positionMs: command.positionMs ?? 0 });
        break;
      case 'setVolume':
        this.master = command.volume ?? 1;
        this.send({ type: 'volume', volume: this.volume() });
        break;
      case 'setTrackGain':
        this.gain = Math.min(1, 10 ** ((command.gainDb ?? 0) / 20));
        this.send({ type: 'volume', volume: this.volume() });
        break;
      case 'setEnd':
        this.endMs = command.endMs ?? null;
        break;
      case 'fadeOut':
        // The `<audio>` engine takes over; this video fades away under it.
        this.loadId = null;
        this.clearStart();
        this.fadeTo(0, command.fadeMs ?? 0, () => this.send({ type: 'stop' }));
        break;
      case 'setEffects':
        // The embed plays outside Web Audio: no equalizer or effects reach it.
        break;
      case 'setRate':
        this.send({ type: 'rate', rate: command.rate ?? 1 });
        break;
      case 'stop':
        this.loadId = null;
        this.clearStart();
        this.stopFade();
        this.send({ type: 'stop' });
        break;
      case 'load':
        break;
    }
  }

  dispose() {
    this.stopFade();
    this.clearStart();
    this.unlisten();
  }

  private clearStart() {
    this.awaitingStart = false;
    if (this.startTimer) clearTimeout(this.startTimer);
    this.startTimer = null;
  }

  private volume() {
    return Math.max(0, Math.min(1, this.master * this.gain * this.fade));
  }

  /** Steps the embed's volume to `target` over `ms` (it has no smoother way). */
  private fadeTo(target: number, ms: number, then?: () => void) {
    this.stopFade();
    const steps = Math.max(1, Math.round(ms / 100));
    const from = this.fade;
    let step = 0;
    this.fadeTimer = setInterval(() => {
      step += 1;
      this.fade = from + ((target - from) * step) / steps;
      this.send({ type: 'volume', volume: this.volume() });
      if (step >= steps) {
        this.stopFade();
        then?.();
      }
    }, 100);
  }

  private stopFade() {
    if (this.fadeTimer) clearInterval(this.fadeTimer);
    this.fadeTimer = null;
  }

  private ensurePage(embedUrl: string) {
    if (this.frame.src === embedUrl) {
      return;
    }
    this.origin = new URL(embedUrl).origin;
    this.pageReady = false;
    this.frame.src = embedUrl;
  }

  private send(command: EmbedCommand) {
    if (!this.pageReady) {
      // Only the latest load matters; earlier queued commands belonged to replaced tracks.
      if (command.type === 'load') {
        this.queued = [];
      }
      this.queued.push(command);
      return;
    }
    this.frame.contentWindow?.postMessage({ raygunCommand: true, ...command }, this.origin ?? '*');
  }

  private onMessage(message: EmbedMessage) {
    if (
      message.source !== this.frame.contentWindow ||
      message.origin !== this.origin ||
      !isReply(message.data)
    ) {
      return;
    }
    const reply = message.data;
    // Late news about the video before this load: its position or its end would land on this one.
    if (reply.type !== 'ready' && reply.token !== this.loadId) {
      return;
    }
    switch (reply.type) {
      case 'ready':
        this.pageReady = true;
        for (const command of this.queued.splice(0)) {
          this.send(command);
        }
        break;
      case 'state': {
        if (reply.state === 'playing' || reply.state === 'buffering') {
          if (this.startTimer) clearTimeout(this.startTimer);
          this.startTimer = null;
          if (reply.state === 'playing') this.awaitingStart = false;
        } else if (reply.state === 'unstarted' && this.awaitingStart && !this.startTimer) {
          // Asked to play, and still unstarted a moment later: the webview held it back until
          // there's a tap. It says paused, so Play (that tap) starts it.
          this.startTimer = setTimeout(() => {
            this.startTimer = null;
            if (!this.awaitingStart) return;
            this.awaitingStart = false;
            this.emit({ type: 'paused' });
          }, UNSTARTED_GRACE_MS);
        }
        const event = STATE_EVENTS[reply.state];
        // The pause after an end trim isn't the listener's.
        if (event && !(this.finished && event.type === 'paused')) {
          this.emit(event);
        }
        break;
      }
      case 'progress':
        if (this.endMs !== null && reply.positionMs >= this.endMs) {
          if (!this.finished) {
            this.finished = true;
            this.send({ type: 'pause' });
            this.emit({ type: 'ended' });
          }
          break;
        }
        this.emit({
          type: 'progress',
          positionMs: reply.positionMs,
          durationMs: reply.durationMs,
        });
        break;
      case 'error':
        this.emit({
          type: 'error',
          message: YOUTUBE_ERRORS[reply.code] ?? `YouTube player error ${reply.code}`,
        });
        break;
    }
  }

  private emit(event: EngineEvent) {
    if (this.loadId !== null) {
      this.report({ loadId: this.loadId, event });
    }
  }
}

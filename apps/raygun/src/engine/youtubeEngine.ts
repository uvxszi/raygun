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

  constructor(
    private readonly frame: EmbedFrame,
    listen: (handler: (message: EmbedMessage) => void) => () => void,
    private readonly report: (report: EngineReport) => void,
  ) {
    this.unlisten = listen((message) => this.onMessage(message));
  }

  handle(command: EngineCommand) {
    switch (command.type) {
      case 'loadYouTube':
        this.loadId = command.loadId;
        this.ensurePage(command.embedUrl);
        this.emit({ type: 'buffering' });
        this.send({
          type: 'load',
          videoId: command.videoId,
          startMs: command.startMs ?? 0,
          autoplay: command.autoplay,
          volume: command.volume ?? 1,
          token: command.loadId,
        });
        break;
      case 'play':
        this.send({ type: 'play' });
        break;
      case 'pause':
        this.send({ type: 'pause' });
        break;
      case 'seek':
        this.send({ type: 'seek', positionMs: command.positionMs ?? 0 });
        break;
      case 'setVolume':
        this.send({ type: 'volume', volume: command.volume ?? 1 });
        break;
      case 'setRate':
        this.send({ type: 'rate', rate: command.rate ?? 1 });
        break;
      case 'stop':
        this.loadId = null;
        this.send({ type: 'stop' });
        break;
      case 'load':
        break;
    }
  }

  dispose() {
    this.unlisten();
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
        const event = STATE_EVENTS[reply.state];
        if (event) {
          this.emit(event);
        }
        break;
      }
      case 'progress':
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

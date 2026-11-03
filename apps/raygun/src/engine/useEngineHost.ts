import { commands, events } from '@raygun/bindings';
import type { EngineCommand, EngineReport } from '@raygun/bindings';
import { useEffect } from 'react';

import { Html5Engine } from './html5Engine';
import { registerSink } from './outputSink';
import { WebAudioSound, elementSound } from './soundGraph';
import type { SoundBackend } from './soundGraph';
import { createVideoDock, removeVideoDock } from './videoDock';
import { YouTubeEmbedEngine } from './youtubeEngine';

type WebviewEngine = { handle: (command: EngineCommand) => void; dispose: () => void };

/** Web Audio's graph where there is one (not in tests), else the bare elements. */
const soundBackend = (): SoundBackend => {
  if (typeof AudioContext === 'undefined') return elementSound();
  try {
    const context = new AudioContext({ latencyHint: 'playback' });
    // The output picked in Devices (M46.43).
    registerSink(context);
    return new WebAudioSound(context);
  } catch {
    return elementSound();
  }
};

/**
 * Runs the webview playback engines for the lifetime of the app shell: `<audio>` for streams and
 * files, YouTube's embedded player for YouTube. Rust treats both as "the webview engine", so this
 * host routes each load to the right one and stops the other when it switches.
 */
export const useEngineHost = () => {
  useEffect(() => {
    const report = (engineReport: EngineReport) => {
      void commands.playbackEngineReport(engineReport);
    };

    // Two elements take turns so songs can crossfade (M46.12). They go through Web Audio for the
    // equalizer and effects; the stream proxy answers with CORS, which Web Audio needs to hear
    // them. Without Web Audio they play as they are.
    const decks: [HTMLAudioElement, HTMLAudioElement] = [new Audio(), new Audio()];
    for (const deck of decks) {
      deck.preload = 'auto';
      deck.crossOrigin = 'anonymous';
      registerSink(deck);
    }
    const html5 = new Html5Engine(decks, report, undefined, soundBackend());

    const { frame } = createVideoDock();
    const youtube = new YouTubeEmbedEngine(
      frame,
      (handler) => {
        const listener = (event: MessageEvent) => handler(event);
        window.addEventListener('message', listener);
        return () => window.removeEventListener('message', listener);
      },
      report,
    );

    let active: WebviewEngine = html5;
    // In a crossfade the engine being left fades its song out instead of cutting it (M46.12).
    const switchTo = (next: WebviewEngine, fadeMs: number | null) => {
      if (active !== next) {
        active.handle(fadeMs ? { type: 'fadeOut', fadeMs } : { type: 'stop' });
        active = next;
      }
    };

    const unlisten = events.engineCommandEvent.listen(({ payload }) => {
      if (payload.type === 'load') {
        switchTo(html5, payload.fadeMs);
      } else if (payload.type === 'loadYouTube') {
        switchTo(youtube, payload.fadeMs);
      }
      // The equalizer lives in the `<audio>` engine's graph whichever engine plays.
      if (payload.type === 'setEffects' && active !== html5) html5.handle(payload);
      active.handle(payload);
    });

    return () => {
      void unlisten.then((stop) => stop());
      html5.handle({ type: 'stop' });
      youtube.handle({ type: 'stop' });
      html5.dispose();
      youtube.dispose();
      removeVideoDock();
    };
  }, []);
};

import { commands, events } from '@raygun/bindings';
import type { EngineCommand, EngineReport } from '@raygun/bindings';
import { useEffect } from 'react';

import { Html5Engine } from './html5Engine';
import { createVideoDock, removeVideoDock } from './videoDock';
import { YouTubeEmbedEngine } from './youtubeEngine';

type WebviewEngine = { handle: (command: EngineCommand) => void; dispose: () => void };

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

    const audio = new Audio();
    audio.preload = 'auto';
    const html5 = new Html5Engine(audio, report);

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
    const switchTo = (next: WebviewEngine) => {
      if (active !== next) {
        active.handle({ type: 'stop' });
        active = next;
      }
    };

    const unlisten = events.engineCommandEvent.listen(({ payload }) => {
      if (payload.type === 'load') {
        switchTo(html5);
      } else if (payload.type === 'loadYouTube') {
        switchTo(youtube);
      }
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

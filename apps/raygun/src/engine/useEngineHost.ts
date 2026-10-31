import { commands, events } from '@raygun/bindings';
import { useEffect } from 'react';

import { Html5Engine } from './html5Engine';

/** Runs the webview playback engine for the lifetime of the app shell. */
export const useEngineHost = () => {
  useEffect(() => {
    const audio = new Audio();
    audio.preload = 'auto';
    const engine = new Html5Engine(audio, (report) => {
      void commands.playbackEngineReport(report);
    });
    const unlisten = events.engineCommandEvent.listen((event) => engine.handle(event.payload));

    return () => {
      void unlisten.then((stop) => stop());
      engine.handle({ type: 'stop' });
      engine.dispose();
    };
  }, []);
};

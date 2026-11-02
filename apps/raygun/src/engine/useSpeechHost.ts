import { commands, events } from '@raygun/bindings';
import { useEffect } from 'react';

type SpeechState = 'speaking' | 'ended' | 'error';

/**
 * Read-aloud on desktop (M46.6): the webview's `speechSynthesis` with the system's voices. Rust
 * splits the text and sends `speechCommandEvent`s; this speaks the pieces in order and reports
 * back in the Android engine's JSON: `speaking` as each piece starts, `ended` after the last.
 * Mount once, in the app shell; on Android nothing is ever sent to it.
 */
export const useSpeechHost = () => {
  useEffect(() => {
    const synth = typeof window.speechSynthesis === 'undefined' ? null : window.speechSynthesis;
    void commands.ttsSetAvailable(synth !== null);
    if (!synth) return;
    // A newer command makes callbacks of the cancelled utterances stale.
    let generation = 0;
    const report = (state: SpeechState, paragraph: number) =>
      void commands.ttsReport(JSON.stringify({ state, paragraph }));

    const unlisten = events.speechCommandEvent.listen(({ payload }) => {
      generation += 1;
      const current = generation;
      synth.cancel();
      if (payload.type === 'stop') return;
      for (const item of payload.items) {
        const paragraph = Number.parseInt(item.id, 10);
        const last = item.id.endsWith(':end');
        const utterance = new SpeechSynthesisUtterance(item.text);
        utterance.rate = payload.rate ?? 1;
        utterance.onstart = () => {
          if (current === generation) report('speaking', paragraph);
        };
        utterance.onend = () => {
          if (current === generation && last) report('ended', paragraph);
        };
        utterance.onerror = (event) => {
          // Cancelling (a new command, Stop) isn't a failure.
          if (current !== generation || event.error === 'interrupted' || event.error === 'canceled')
            return;
          report('error', paragraph);
        };
        synth.speak(utterance);
      }
    });
    return () => {
      generation += 1;
      synth.cancel();
      void unlisten.then((stop) => stop());
    };
  }, []);
};

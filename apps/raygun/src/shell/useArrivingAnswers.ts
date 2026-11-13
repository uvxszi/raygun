import { useEffect, useState } from 'react';

/** A Rust event carrying `[request, answers so far]` (`CoverOptionsEvent`, `StremioStreamsEvent`…). */
export type ArrivingEvent<T> = {
  listen: (handler: (event: { payload: [string, T[]] }) => void) => Promise<() => void>;
};

/**
 * What a command asked under `request` has found so far, as Rust sends it while its sources still
 * answer (each event carries everything found until then); followed while `asking`. Undefined
 * until something came for this request.
 */
export const useArrivingAnswers = <T>(
  event: ArrivingEvent<T>,
  request: string,
  asking: boolean,
): T[] | undefined => {
  const [arriving, setArriving] = useState<{ request: string; answers: T[] } | null>(null);
  useEffect(() => {
    if (!asking) return;
    const unlisten = event.listen(({ payload: [asked, answers] }) => {
      if (asked === request) setArriving({ request, answers });
    });
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, [event, request, asking]);
  return arriving?.request === request ? arriving.answers : undefined;
};

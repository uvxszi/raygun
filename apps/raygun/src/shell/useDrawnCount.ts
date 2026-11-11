import { startTransition, useEffect, useState } from 'react';

/**
 * How many of a long list's entries to draw: the first `first` with the list, then `step` more
 * each time the last part is on the screen, as a transition (a tap meanwhile is answered first),
 * until every entry is drawn. A tab or pill showing hundreds of results puts its first ones on
 * the screen at once and the rest follow without holding anything up. A list of up to `first`
 * entries is drawn whole at once. A new `list` (another search) starts again from `first`.
 */
export const useDrawnCount = (total: number, first: number, step = first, list: unknown = null) => {
  const [drawn, setDrawn] = useState({ list, count: first });
  let count = drawn.count;
  if (drawn.list !== list) {
    count = first;
    setDrawn({ list, count });
  }
  const more = count < total;
  useEffect(() => {
    if (!more) return;
    const timer = setTimeout(() =>
      startTransition(() =>
        setDrawn((current) =>
          current.list === list ? { list, count: current.count + step } : current,
        ),
      ),
    );
    return () => clearTimeout(timer);
  }, [more, count, step, list]);
  return Math.min(count, total);
};
